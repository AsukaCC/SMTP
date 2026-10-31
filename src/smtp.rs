use crate::{
    auth::{self, Auth},
    model::SendRequest,
    send,
    state::AppState,
};
use mail_parser::MessageParser;
use mailin::{Action, AuthMechanism, Handler, Response, SessionBuilder, response};
use std::{
    io,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::Semaphore,
};
use tokio_rustls::{TlsAcceptor, rustls};
use zeroize::Zeroizing;

struct Submission {
    state: AppState,
    runtime: tokio::runtime::Handle,
    peer: IpAddr,
    credential: Option<Zeroizing<String>>,
    from: String,
    recipient: String,
    data: Vec<u8>,
}
impl Submission {
    fn authenticate(&mut self, user: &str, password: &str) -> Response {
        self.credential = None;
        if user != "bunmail"
            || !self
                .state
                .rate_limit(format!("smtp-auth:{}", self.peer), 20, 300)
        {
            return response::INVALID_CREDENTIALS;
        }
        match self.runtime.block_on(auth::api_key(&self.state, password)) {
            Ok(_) => {
                self.credential = Some(Zeroizing::new(password.to_owned()));
                response::AUTH_OK
            }
            Err(_) => response::INVALID_CREDENTIALS,
        }
    }
}
impl Handler for Submission {
    fn auth_plain(&mut self, authorization: &str, user: &str, password: &str) -> Response {
        if !authorization.is_empty() && authorization != user {
            return response::INVALID_CREDENTIALS;
        }
        self.authenticate(user, password)
    }
    fn auth_login(&mut self, user: &str, password: &str) -> Response {
        self.authenticate(user, password)
    }
    fn mail(&mut self, _ip: IpAddr, _domain: &str, from: &str) -> Response {
        self.data.clear();
        self.recipient.clear();
        self.from.clear();
        if self.credential.is_none() {
            return response::AUTHENTICATION_REQUIRED;
        }
        if send::validate_address(from).is_err() {
            return response::BAD_MAILBOX;
        }
        self.from = from.into();
        response::OK
    }
    fn rcpt(&mut self, to: &str) -> Response {
        if self.credential.is_none() {
            return response::AUTHENTICATION_REQUIRED;
        }
        if !self.recipient.is_empty() {
            return Response::custom(452, "One recipient per message is supported".into());
        }
        if send::validate_address(to).is_err() {
            return response::BAD_MAILBOX;
        }
        self.recipient = to.into();
        response::OK
    }
    fn data_start(&mut self, _domain: &str, from: &str, _is8bit: bool, to: &[String]) -> Response {
        self.data.clear();
        if self.credential.is_none() {
            return response::AUTHENTICATION_REQUIRED;
        }
        if to.len() != 1 {
            return response::TRANSACTION_FAILED;
        }
        self.from = from.into();
        self.recipient = to[0].clone();
        response::OK
    }
    fn data(&mut self, buf: &[u8]) -> io::Result<()> {
        if self.data.len() + buf.len() > 768 * 1024 {
            return Err(io::Error::other("Message too large"));
        }
        self.data.extend_from_slice(buf);
        Ok(())
    }
    fn data_end(&mut self) -> Response {
        let Some(token) = self.credential.as_ref() else {
            return response::AUTHENTICATION_REQUIRED;
        };
        let Some(message) = MessageParser::default().parse(&self.data) else {
            return response::TRANSACTION_FAILED;
        };
        let from = message
            .from()
            .and_then(|a| a.first())
            .and_then(|a| a.address.as_deref());
        if from != Some(self.from.as_str()) || message.from().is_none_or(|a| a.iter().count() != 1)
        {
            return Response::custom(553, "Header From must match envelope MAIL FROM".into());
        }
        if message.attachment_count() > 0 {
            return Response::custom(552, "Attachments are not supported by this edition".into());
        }
        let input = SendRequest {
            from: self.from.clone(),
            from_name: message
                .from()
                .and_then(|a| a.first())
                .and_then(|a| a.name.as_deref())
                .unwrap_or_default()
                .to_owned(),
            to: self.recipient.clone(),
            subject: message.subject().unwrap_or_default().into(),
            html: message
                .body_html(0)
                .map(|v| v.into_owned())
                .unwrap_or_default(),
            text: message
                .body_text(0)
                .map(|v| v.into_owned())
                .unwrap_or_default(),
            ..Default::default()
        };
        let result = self.runtime.block_on(async {
            let id = auth::api_key(&self.state, token).await?;
            send::enqueue(
                &self.state,
                &Auth {
                    api_key_id: Some(id),
                },
                input,
                None,
            )
            .await
        });
        self.data.clear();
        match result {
            Ok((id, _)) => Response::custom(250, format!("Queued as {id}")),
            Err(error) if error.0.is_server_error() => response::INTERNAL_ERROR,
            Err(error) => Response::custom(550, error.2.replace(['\r', '\n'], " ")),
        }
    }
}

trait IoStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> IoStream for T {}

async fn connection(
    state: AppState,
    stream: TcpStream,
    peer: SocketAddr,
    tls: TlsAcceptor,
) -> anyhow::Result<()> {
    let mut builder = SessionBuilder::new(&state.config.mail_hostname);
    builder
        .enable_start_tls()
        .enable_auth(AuthMechanism::Plain)
        .enable_auth(AuthMechanism::Login);
    let handler = Submission {
        runtime: tokio::runtime::Handle::current(),
        state: state.clone(),
        peer: peer.ip(),
        credential: None,
        from: String::new(),
        recipient: String::new(),
        data: Vec::new(),
    };
    let mut session = builder.build(peer.ip(), handler);
    let stream: Box<dyn IoStream> = Box::new(stream);
    let mut reader = BufReader::new(stream);
    reader
        .write_all(format!("220 {} ESMTP\r\n", state.config.mail_hostname).as_bytes())
        .await?;
    let mut encrypted = false;
    loop {
        let mut line = Vec::new();
        let count = tokio::time::timeout(
            Duration::from_secs(30),
            (&mut reader).take(65537).read_until(b'\n', &mut line),
        )
        .await??;
        if count == 0 {
            return Ok(());
        }
        if count > 65536 || !line.ends_with(b"\n") {
            anyhow::bail!("SMTP line exceeds limit");
        }
        let response = tokio::task::block_in_place(|| session.process(&line));
        let mut bytes = Vec::new();
        response.write_to(&mut bytes)?;
        reader.write_all(&bytes).await?;
        reader.flush().await?;
        match response.action {
            Action::Close => return Ok(()),
            Action::UpgradeTls => {
                if encrypted {
                    return Ok(());
                }
                let secured =
                    tokio::time::timeout(Duration::from_secs(15), tls.accept(reader.into_inner()))
                        .await??;
                reader = BufReader::new(Box::new(secured) as Box<dyn IoStream>);
                session.tls_active();
                encrypted = true;
            }
            _ => {}
        }
    }
}

pub async fn start(
    state: AppState,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<tokio::task::JoinHandle<()>> {
    let bind = state
        .config
        .smtp_bind
        .ok_or_else(|| anyhow::anyhow!("SMTP_BIND missing"))?;
    let cert = state
        .config
        .smtp_cert
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("SMTP requires TLS certificate"))?;
    let key = state
        .config
        .smtp_key
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("SMTP requires TLS private key"))?;
    let certs = rustls_pemfile::certs(&mut io::BufReader::new(std::fs::File::open(cert)?))
        .collect::<io::Result<Vec<_>>>()?;
    let key = rustls_pemfile::private_key(&mut io::BufReader::new(std::fs::File::open(key)?))?
        .ok_or_else(|| anyhow::anyhow!("Invalid TLS key"))?;
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_no_client_auth()
    .with_single_cert(certs, key)?;
    let tls = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind(bind).await?;
    let permits = Arc::new(Semaphore::new(32));
    Ok(tokio::spawn(async move {
        loop {
            tokio::select! {
                _=shutdown.changed()=>break,
                accepted=listener.accept()=>{
                    if let Ok((stream,peer))=accepted
                        && let Ok(permit)=permits.clone().try_acquire_owned() {
                            let state=state.clone();let tls=tls.clone();
                            tokio::spawn(async move { let _permit=permit; let _=tokio::time::timeout(Duration::from_secs(180),connection(state,stream,peer,tls)).await; });
                        }
                }
            }
        }
    }))
}
