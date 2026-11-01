use crate::{
    auth::Auth,
    config::valid_domain,
    crypto,
    error::{AppError, Result},
    model::{Domain, SendRequest, Template},
    state::AppState,
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use lettre::{Address, message::Mailbox};
use minijinja::{AutoEscape, Environment, UndefinedBehavior};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestRequest {
    from: String,
    #[serde(default)]
    from_name: String,
    to: String,
    language: String,
}

pub async fn test_send(
    auth: Auth,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TestRequest>,
) -> Result<(StatusCode, Json<Value>)> {
    auth.require_admin()?;
    if !state.delivery_enabled().await? {
        return Err(AppError::conflict(
            "Complete delivery configuration before sending a test",
        ));
    }
    let preset = crate::initial_templates::presets()
        .into_iter()
        .find(|p| p.category == "test" && p.language == input.language)
        .ok_or_else(|| AppError::invalid("Unsupported email language"))?;
    let mut variables = preset.variables;
    variables.insert("brand".into(), json!("BunMail RS"));
    variables.insert("hostname".into(), json!(state.config.mail_hostname));
    variables.insert(
        "sent_at".into(),
        json!(if input.language == "zh-CN" {
            "见邮件 Date 头"
        } else {
            "See the Date header"
        }),
    );
    let template = Template {
        id: preset.id,
        name: preset.name,
        subject: preset.subject,
        html: preset.html,
        text: preset.text,
        updated_at: chrono::Utc::now(),
    };
    let mut message = SendRequest {
        from: input.from,
        from_name: input.from_name,
        to: input.to,
        variables,
        ..Default::default()
    };
    render_template(&template, &mut message)?;
    // Use the common sender verification, rate limit, idempotency and persistent queue.
    send(auth, State(state), headers, Json(message)).await
}

pub fn validate_address(address: &str) -> Result<Address> {
    if address.len() > 254 || !address.is_ascii() || address.contains(['\r', '\n']) {
        return Err(AppError::invalid("Invalid email address"));
    }
    let parsed: Address = address
        .parse()
        .map_err(|_| AppError::invalid("Invalid email address"))?;
    if !valid_domain(parsed.domain()) {
        return Err(AppError::invalid("Email address must use a DNS domain"));
    }
    Ok(parsed)
}

pub fn sender(address: &str, name: &str) -> Result<Mailbox> {
    if name.len() > 256 || name.chars().any(char::is_control) {
        return Err(AppError::invalid(
            "Sender name must be at most 256 bytes without control characters",
        ));
    }
    Ok(Mailbox::new(
        (!name.trim().is_empty()).then(|| name.trim().to_owned()),
        validate_address(address)?,
    ))
}

fn render(source: &str, variables: &serde_json::Map<String, Value>, html: bool) -> Result<String> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_auto_escape_callback(move |_| {
        if html {
            AutoEscape::Html
        } else {
            AutoEscape::None
        }
    });
    // Only simple {{ variable }} substitutions are supported; no loops, calls, includes or raw HTML bypasses.
    if source.contains("{%") || source.contains("{#") {
        return Err(AppError::invalid(
            "Only {{ variable }} template substitutions are supported",
        ));
    }
    let mut rest = source;
    while let Some((_, tail)) = rest.split_once("{{") {
        let (key, tail) = tail
            .split_once("}}")
            .ok_or_else(|| AppError::invalid("Unclosed template variable"))?;
        if key.trim().is_empty()
            || !key
                .trim()
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(AppError::invalid(
                "Template variables must be plain identifiers",
            ));
        }
        rest = tail;
    }
    env.render_str(source, variables)
        .map_err(|_| AppError::invalid("A template variable is missing or invalid"))
}

pub fn render_template(template: &Template, input: &mut SendRequest) -> Result<()> {
    input.subject = render(&template.subject, &input.variables, false)?;
    input.html = render(&template.html, &input.variables, true)?;
    input.text = render(&template.text, &input.variables, false)?;
    Ok(())
}

pub async fn enqueue(
    state: &AppState,
    auth: &Auth,
    mut input: SendRequest,
    idempotency: Option<String>,
) -> Result<(Uuid, bool)> {
    let from = validate_address(&input.from)?;
    let mailbox = sender(&input.from, &input.from_name)?;
    validate_address(&input.to)?;
    if let Some(ref key) = idempotency
        && (key.is_empty()
            || key.len() > 128
            || !key.is_ascii()
            || key.bytes().any(|b| b.is_ascii_control()))
    {
        return Err(AppError::invalid("Invalid Idempotency-Key"));
    }
    let request_hash = crypto::hash(
        &serde_json::to_string(&input).map_err(|_| AppError::invalid("Invalid request"))?,
    );
    let scope = auth
        .api_key_id
        .map_or_else(|| "owner".into(), |id| id.to_string());
    if let Some(ref key) = idempotency
        && let Some((id, existing_hash)) = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id,request_hash FROM emails WHERE idempotency_scope=$1 AND idempotency_key=$2",
        )
        .bind(&scope)
        .bind(key)
        .fetch_optional(&state.db)
        .await?
    {
        if existing_hash != request_hash {
            return Err(AppError(
                StatusCode::CONFLICT,
                "IDEMPOTENCY_CONFLICT",
                "Idempotency-Key was used with a different request".into(),
            ));
        }
        return Ok((id, false));
    }
    if !state.delivery_enabled().await? {
        return Err(AppError(
            StatusCode::CONFLICT,
            "DELIVERY_PAUSED",
            "Delivery is paused. Enable delivery in Settings".into(),
        ));
    }
    if !state.rate_limit(format!("send:{scope}"), 60, 60) {
        return Err(AppError(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "Sending rate exceeded".into(),
        ));
    }
    let domain = sqlx::query_as::<_, Domain>("SELECT * FROM domains WHERE name=$1")
        .bind(from.domain().to_lowercase())
        .fetch_optional(&state.db)
        .await?
        .filter(Domain::can_send)
        .ok_or_else(|| {
            AppError(
                StatusCode::UNPROCESSABLE_ENTITY,
                "DOMAIN_NOT_VERIFIED",
                "Verify the sender domain DNS within the last 24 hours before sending".into(),
            )
        })?;
    if let Some(id) = input.template_id {
        let template = sqlx::query_as::<_, Template>("SELECT * FROM templates WHERE id=$1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(AppError::not_found)?;
        render_template(&template, &mut input)?;
    }
    if input.subject.is_empty() || input.subject.len() > 998 || input.subject.contains(['\r', '\n'])
    {
        return Err(AppError::invalid(
            "Subject is required, at most 998 bytes, without newlines",
        ));
    }
    if input.html.len() + input.text.len() > 512 * 1024
        || (input.html.is_empty() && input.text.is_empty())
    {
        return Err(AppError::invalid(
            "Message body is required and must be at most 512 KiB",
        ));
    }
    let id = Uuid::new_v4();
    let message_id = format!("{id}@{}", state.config.mail_hostname);
    let inserted = sqlx::query("INSERT INTO emails (id,api_key_id,domain_id,from_address,to_address,subject,html,text,message_id,idempotency_scope,idempotency_key,request_hash) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT (idempotency_scope,idempotency_key) DO NOTHING")
        .bind(id).bind(auth.api_key_id).bind(domain.id).bind(mailbox.to_string()).bind(&input.to).bind(&input.subject).bind(&input.html).bind(&input.text).bind(message_id).bind(&scope).bind(&idempotency).bind(&request_hash).execute(&state.db).await?;
    if inserted.rows_affected() == 0 {
        let (id, hash): (Uuid, String) = sqlx::query_as(
            "SELECT id,request_hash FROM emails WHERE idempotency_scope=$1 AND idempotency_key=$2",
        )
        .bind(scope)
        .bind(idempotency)
        .fetch_one(&state.db)
        .await?;
        if hash != request_hash {
            return Err(AppError(
                StatusCode::CONFLICT,
                "IDEMPOTENCY_CONFLICT",
                "Idempotency-Key was used with a different request".into(),
            ));
        }
        return Ok((id, false));
    }
    Ok((id, true))
}

pub async fn send(
    auth: Auth,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SendRequest>,
) -> Result<(StatusCode, Json<Value>)> {
    let key = headers
        .get("idempotency-key")
        .map(|v| v.to_str().map(str::to_owned))
        .transpose()
        .map_err(|_| AppError::invalid("Invalid Idempotency-Key"))?;
    let (id, created) = enqueue(&state, &auth, input, key).await?;
    Ok((
        if created {
            StatusCode::ACCEPTED
        } else {
            StatusCode::OK
        },
        Json(json!({"id":id,"created":created})),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_header_injection() {
        assert!(sender("a@example.com", "name\r\nBcc: x@test.com").is_err());
        assert!(sender("a@example.com", "name\0").is_err());
        let mailbox = sender("a@example.com", "中文发件人, \"团队\"").unwrap();
        let parsed: Mailbox = mailbox.to_string().parse().unwrap();
        assert_eq!(parsed.name, mailbox.name);
        assert_eq!(parsed.email.to_string(), "a@example.com");
        assert!(validate_address("a@test.com\r\nBcc: x@test.com").is_err());
        assert!(validate_address("a@localhost").is_err());
        assert!(validate_address("a@example.com").is_ok());
    }
    #[test]
    fn variables_are_escaped_and_missing_values_fail() {
        let vars = serde_json::from_value(json!({"name":"<script>alert(1)</script>"})).unwrap();
        let rendered = render("Hello {{ name }}", &vars, true).unwrap();
        assert!(!rendered.contains("<script>"));
        assert_eq!(
            html_escape::decode_html_entities(&rendered),
            "Hello <script>alert(1)</script>"
        );
        assert!(render("{{ missing }}", &vars, true).is_err());
        assert!(render("{{ name | safe }}", &vars, true).is_err());
        assert!(render("{% for x in y %}", &vars, true).is_err());
    }
}
