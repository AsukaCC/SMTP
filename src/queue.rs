use crate::{
    crypto, dns,
    model::{Domain, Email},
    state::AppState,
};
use chrono::{Duration as ChronoDuration, Utc};
use hickory_resolver::{error::ResolveErrorKind, proto::op::ResponseCode};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{
        MultiPart, SinglePart,
        dkim::{DkimConfig, DkimSigningAlgorithm, DkimSigningKey},
        header,
    },
    transport::smtp::{
        client::{Tls, TlsParameters},
        extension::ClientId,
    },
};
use std::{error::Error as _, net::IpAddr, time::Duration};
use uuid::Uuid;
use zeroize::Zeroizing;

pub struct DeliveryError {
    pub permanent: bool,
    pub detail: String,
}
impl DeliveryError {
    fn temporary(detail: impl Into<String>) -> Self {
        Self {
            permanent: false,
            detail: detail.into(),
        }
    }
    fn permanent(detail: impl Into<String>) -> Self {
        Self {
            permanent: true,
            detail: detail.into(),
        }
    }
}

pub fn retry_seconds(attempt: i32) -> i64 {
    match attempt {
        0 | 1 => 60,
        2 => 300,
        3 => 900,
        4 => 3600,
        5 => 10800,
        _ => 21600,
    }
}

pub fn build_message(email: &Email, domain: &Domain, key: &[u8; 32]) -> anyhow::Result<Message> {
    let private = Zeroizing::new(crypto::decrypt(key, &domain.name, &domain.private_key)?);
    let signing_key = DkimSigningKey::new(&private, DkimSigningAlgorithm::Rsa)?;
    let builder = Message::builder()
        .from(email.from_address.parse()?)
        .to(email.to_address.parse()?)
        .subject(&email.subject)
        .message_id(Some(email.message_id.clone()))
        .date(email.created_at.into());
    let mut message = if email.html.is_empty() {
        builder.singlepart(SinglePart::plain(email.text.clone()))?
    } else if email.text.is_empty() {
        builder.singlepart(SinglePart::html(email.html.clone()))?
    } else {
        builder.multipart(
            MultiPart::alternative()
                .singlepart(SinglePart::plain(email.text.clone()))
                .singlepart(SinglePart::html(email.html.clone())),
        )?
    };
    let headers = [
        "From",
        "To",
        "Subject",
        "Date",
        "Message-ID",
        "MIME-Version",
        "Content-Type",
    ]
    .into_iter()
    .map(header::HeaderName::new_from_ascii_str)
    .collect();
    message.sign(&DkimConfig::new(
        domain.selector.clone(),
        domain.name.clone(),
        signing_key,
        headers,
        Default::default(),
    ));
    Ok(message)
}

async fn mx_hosts(
    state: &AppState,
    domain: &str,
) -> std::result::Result<Vec<String>, DeliveryError> {
    match state.dns.mx_lookup(domain).await {
        Ok(records) => {
            let mut records = records.iter().collect::<Vec<_>>();
            if records.iter().any(|r| r.exchange().is_root()) {
                return Err(DeliveryError::permanent(
                    "Recipient domain declares null MX",
                ));
            }
            records.sort_by_key(|r| r.preference());
            Ok(records
                .into_iter()
                .take(5)
                .map(|r| r.exchange().to_utf8().trim_end_matches('.').to_string())
                .collect())
        }
        Err(error) => match error.kind() {
            ResolveErrorKind::NoRecordsFound {
                response_code: ResponseCode::NoError,
                ..
            } => Ok(vec![domain.into()]),
            ResolveErrorKind::NoRecordsFound {
                response_code: ResponseCode::NXDomain,
                ..
            } => Err(DeliveryError::permanent("Recipient domain does not exist")),
            _ => Err(DeliveryError::temporary("Temporary MX lookup failure")),
        },
    }
}

async fn send_to_ip(
    state: &AppState,
    message: &Message,
    host: &str,
    ip: IpAddr,
) -> std::result::Result<(), (DeliveryError, bool)> {
    let tls = TlsParameters::builder(host.to_owned())
        .build()
        .map_err(|_| (DeliveryError::temporary("Could not configure TLS"), false))?;
    let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(ip.to_string())
        .port(25)
        .hello_name(ClientId::Domain(state.config.mail_hostname.clone()))
        .tls(Tls::Opportunistic(tls))
        .timeout(Some(Duration::from_secs(30)))
        .build();
    match transport.send(message.clone()).await {
        Ok(_) => Ok(()),
        Err(error) => {
            // Do not persist the remote SMTP text: it can echo message content or credentials.
            let code = error
                .status()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "connection".into());
            let mut source = error.source();
            let mut before_transaction = error.is_tls();
            while let Some(cause) = source {
                if let Some(io) = cause.downcast_ref::<std::io::Error>() {
                    before_transaction |= matches!(
                        io.kind(),
                        std::io::ErrorKind::ConnectionRefused
                            | std::io::ErrorKind::NetworkUnreachable
                            | std::io::ErrorKind::HostUnreachable
                    );
                }
                source = cause.source();
            }
            Err((
                DeliveryError {
                    permanent: error.is_permanent(),
                    detail: format!("MX {host}: SMTP {code}"),
                },
                before_transaction,
            ))
        }
    }
}

pub async fn deliver(
    state: &AppState,
    email: &Email,
    domain: &Domain,
) -> std::result::Result<(), DeliveryError> {
    if !domain.can_send() {
        return Err(DeliveryError::temporary(
            "Sender DNS verification is missing or expired",
        ));
    }
    let message = build_message(email, domain, &state.config.encryption_key)
        .map_err(|_| DeliveryError::temporary("Could not build or sign the message"))?;
    let recipient_domain = email
        .to_address
        .rsplit_once('@')
        .ok_or_else(|| DeliveryError::permanent("Invalid recipient"))?
        .1;
    let hosts = mx_hosts(state, recipient_domain).await?;
    let mut last = DeliveryError::temporary("No reachable public MX address");
    for host in hosts {
        let addresses = match state.dns.lookup_ip(&host).await {
            Ok(a) => a,
            Err(_) => continue,
        };
        for ip in addresses
            .iter()
            .filter(|ip| dns::public_address(*ip))
            .filter(|ip| {
                state
                    .config
                    .public_ip
                    .is_some_and(|public| public.is_ipv4() == ip.is_ipv4())
            })
            .take(8)
        {
            match send_to_ip(state, &message, &host, ip).await {
                Ok(()) => return Ok(()),
                Err((error, _)) if error.permanent => return Err(error),
                Err((error, before_transaction)) => {
                    // After an SMTP transaction started, queue the retry instead of immediately sending through another MX.
                    if !before_transaction {
                        return Err(error);
                    }
                    last = error;
                }
            }
        }
    }
    Err(last)
}

pub async fn claim(state: &AppState) -> Result<Option<Email>, sqlx::Error> {
    let token = Uuid::new_v4();
    sqlx::query_as("UPDATE emails SET status='sending',lease_until=now()+interval '5 minutes',lease_token=$1,attempts=attempts+1 WHERE id=(SELECT id FROM emails WHERE (status='queued' AND next_attempt_at<=now()) OR (status='sending' AND lease_until<now()) ORDER BY next_attempt_at FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING *")
        .bind(token).fetch_optional(&state.db).await
}

pub async fn finish(
    state: &AppState,
    email: &Email,
    result: std::result::Result<(), DeliveryError>,
) -> Result<(), sqlx::Error> {
    let (status, error, next) = match result {
        Ok(()) => ("sent", None, Utc::now()),
        Err(error) => {
            let failed = error.permanent
                || email.attempts >= 12
                || email.created_at < Utc::now() - ChronoDuration::hours(48);
            (
                if failed { "failed" } else { "queued" },
                Some(error.detail),
                Utc::now() + ChronoDuration::seconds(retry_seconds(email.attempts)),
            )
        }
    };
    let mut tx = state.db.begin().await?;
    let updated=sqlx::query("UPDATE emails SET status=$1,last_error=$2,next_attempt_at=$3,lease_until=NULL,lease_token=NULL,sent_at=CASE WHEN $1='sent' THEN now() ELSE sent_at END WHERE id=$4 AND lease_token=$5 AND status='sending'")
        .bind(status).bind(&error).bind(next).bind(email.id).bind(email.lease_token).execute(&mut *tx).await?;
    if updated.rows_affected() == 1 {
        sqlx::query("INSERT INTO delivery_attempts (email_id,result,detail) VALUES ($1,$2,$3)")
            .bind(email.id)
            .bind(status)
            .bind(error.unwrap_or_else(|| "Recipient SMTP server accepted the message".into()))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await
}

pub async fn run(state: AppState, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    let mut refresh = tokio::time::Instant::now() - Duration::from_secs(3600);
    loop {
        if *shutdown.borrow() {
            break;
        }
        if refresh.elapsed() > Duration::from_secs(3600) {
            if let Ok(domains) = sqlx::query_as::<_, Domain>("SELECT * FROM domains")
                .fetch_all(&state.db)
                .await
            {
                for domain in domains {
                    if let Ok(checks) = dns::verify(&state, &domain).await {
                        let _ = sqlx::query(
                            "UPDATE domains SET verified=$1,checks=$2,checked_at=now() WHERE id=$3",
                        )
                        .bind(checks.verified())
                        .bind(serde_json::json!(checks))
                        .bind(domain.id)
                        .execute(&state.db)
                        .await;
                    }
                }
            }
            let _ = sqlx::query("DELETE FROM sessions WHERE expires_at<now()")
                .execute(&state.db)
                .await;
            refresh = tokio::time::Instant::now();
        }
        if state.config.delivery_enabled {
            match claim(&state).await {
                Ok(Some(email)) => {
                    let result =
                        match sqlx::query_as::<_, Domain>("SELECT * FROM domains WHERE id=$1")
                            .bind(email.domain_id)
                            .fetch_one(&state.db)
                            .await
                        {
                            Ok(domain) => tokio::time::timeout(
                                Duration::from_secs(120),
                                deliver(&state, &email, &domain),
                            )
                            .await
                            .unwrap_or_else(|_| {
                                Err(DeliveryError::temporary("Delivery attempt timed out"))
                            }),
                            Err(_) => Err(DeliveryError::temporary("Sender domain is unavailable")),
                        };
                    if finish(&state, &email, result).await.is_err() {
                        tracing::error!(email_id=%email.id,"Could not save delivery outcome");
                    }
                }
                Err(_) => tracing::error!("Queue claim failed"),
                _ => {}
            }
        }
        tokio::select! { _=tokio::time::sleep(Duration::from_secs(1))=>{},_=shutdown.changed()=>{} }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retry_backoff_is_bounded() {
        assert_eq!(
            [1, 2, 3, 4, 5, 6, 12].map(retry_seconds),
            [60, 300, 900, 3600, 10800, 21600, 21600]
        );
    }
}
