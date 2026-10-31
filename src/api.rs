use crate::{
    auth::Auth,
    crypto, dns,
    error::{AppError, Result},
    model::{Domain, Email, Template},
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
use zeroize::Zeroizing;

pub async fn health(State(state): State<AppState>) -> Result<Json<Value>> {
    sqlx::query("SELECT 1").execute(&state.db).await?;
    Ok(Json(
        json!({"status":"ok","version":env!("CARGO_PKG_VERSION")}),
    ))
}

pub async fn config(auth: Auth, State(state): State<AppState>) -> Result<Json<Value>> {
    auth.require_admin()?;
    let domains = sqlx::query_as::<_, Domain>("SELECT * FROM domains ORDER BY name")
        .fetch_all(&state.db)
        .await?;
    let verified_domains: Vec<&str> = domains
        .iter()
        .filter(|d| d.can_send())
        .map(|d| d.name.as_str())
        .collect();
    let ready_to_test = state.config.delivery_enabled
        && state.config.public_ip.is_some()
        && !verified_domains.is_empty();
    let rdns_warning: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM domains WHERE checked_at IS NOT NULL AND COALESCE(checks->>'ptr','false') <> 'true')").fetch_one(&state.db).await?;
    Ok(Json(
        json!({"version":env!("CARGO_PKG_VERSION"),"mail_hostname":state.config.mail_hostname,"public_ip":state.config.public_ip,"delivery_enabled":state.config.delivery_enabled,"smtp_enabled":state.config.smtp_bind.is_some(),"smtp_tls":state.config.smtp_cert.is_some(),"smtp_port":state.config.smtp_bind.map(|a|a.port()),"verified_domains":verified_domains,"ready_to_test":ready_to_test,"rdns_warning":rdns_warning,"cloudflare_configured":state.config.cloudflare_api_token.is_some()}),
    ))
}

pub async fn stats(auth: Auth, State(state): State<AppState>) -> Result<Json<Value>> {
    auth.require_admin()?;
    let (queued,sending,sent,failed):(i64,i64,i64,i64)=sqlx::query_as("SELECT count(*) FILTER(WHERE status='queued'),count(*) FILTER(WHERE status='sending'),count(*) FILTER(WHERE status='sent'),count(*) FILTER(WHERE status='failed') FROM emails").fetch_one(&state.db).await?;
    Ok(Json(
        json!({"queued":queued,"sending":sending,"sent":sent,"failed":failed}),
    ))
}

pub async fn list_domains(auth: Auth, State(state): State<AppState>) -> Result<Json<Value>> {
    auth.require_admin()?;
    let mut rows = sqlx::query_as::<_, Domain>("SELECT * FROM domains ORDER BY created_at DESC")
        .fetch_all(&state.db)
        .await?;
    // The stored flag may reflect an older verification policy that required PTR.
    for domain in &mut rows {
        domain.verified = domain.can_send();
    }
    Ok(Json(
        json!({"data":rows.iter().map(|d|json!({"domain":d,"records":dns::records(&state,d),"retry_after_seconds":state.retry_after_seconds(&format!("verify:{}",d.id),3)})).collect::<Vec<_>>()}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Name {
    name: String,
}
pub async fn add_domain(
    auth: Auth,
    State(state): State<AppState>,
    Json(input): Json<Name>,
) -> Result<Json<Value>> {
    auth.require_admin()?;
    let name = input.name.trim().to_lowercase();
    if !crate::config::valid_domain(&name) {
        return Err(AppError::invalid("Invalid domain name"));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM domains")
        .fetch_one(&state.db)
        .await?;
    if count >= 100 {
        return Err(AppError::invalid(
            "At most 100 domains are supported per instance",
        ));
    }
    if !state.rate_limit("generate-dkim".into(), 5, 60) {
        return Err(AppError::conflict(
            "Please wait before generating another key",
        ));
    }
    let (private, public) = tokio::task::spawn_blocking(crypto::generate_dkim)
        .await
        .map_err(|_| AppError::internal())?
        .map_err(|_| AppError::internal())?;
    let private = Zeroizing::new(private);
    let encrypted = crypto::encrypt(&state.config.encryption_key, &name, &private)
        .map_err(|_| AppError::internal())?;
    let domain = sqlx::query_as::<_, Domain>(
        "INSERT INTO domains (id,name,public_key,private_key) VALUES ($1,$2,$3,$4) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(name)
    .bind(public)
    .bind(encrypted)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(
        json!({"domain":domain,"records":dns::records(&state,&domain)}),
    ))
}

pub async fn verify_domain(
    auth: Auth,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response> {
    auth.require_admin()?;
    let domain = sqlx::query_as::<_, Domain>("SELECT * FROM domains WHERE id=$1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(AppError::not_found)?;
    let limit = state.check_rate_limit(format!("verify:{id}"), 3, 60);
    if !limit.allowed {
        return Ok((StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER,limit.retry_after_seconds.to_string())],
            Json(json!({"error":{"code":"RATE_LIMITED","message":"Please wait before checking DNS again","retry_after_seconds":limit.retry_after_seconds}}))).into_response());
    }
    let checks = match dns::verify(&state, &domain).await {
        Ok(checks) => checks,
        Err(_) => {
            let retry_after_seconds = state.retry_after_seconds(&format!("verify:{id}"), 3);
            let mut response = AppError::invalid(
                "DNS lookup failed. Try again later; saved verification was not changed",
            )
            .into_response();
            if retry_after_seconds > 0 {
                response.headers_mut().insert(
                    header::RETRY_AFTER,
                    retry_after_seconds
                        .to_string()
                        .parse()
                        .map_err(|_| AppError::internal())?,
                );
            }
            return Ok(response);
        }
    };
    sqlx::query("UPDATE domains SET checks=$1,verified=$2,checked_at=now() WHERE id=$3")
        .bind(json!(checks))
        .bind(checks.verified())
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({"verified":checks.verified(),"checks":checks,"retry_after_seconds":state.retry_after_seconds(&format!("verify:{id}"),3)})).into_response())
}

pub async fn delete_domain(
    auth: Auth,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    auth.require_admin()?;
    if sqlx::query("DELETE FROM domains WHERE id=$1")
        .bind(id)
        .execute(&state.db)
        .await?
        .rows_affected()
        == 0
    {
        return Err(AppError::not_found());
    }
    Ok(Json(json!({"ok":true})))
}

pub async fn list_keys(auth: Auth, State(state): State<AppState>) -> Result<Json<Value>> {
    auth.require_admin()?;
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'name',name,'prefix',prefix,'created_at',created_at,'revoked_at',revoked_at) FROM api_keys ORDER BY created_at DESC").fetch_all(&state.db).await?;
    Ok(Json(json!({"data":rows})))
}
pub async fn add_key(
    auth: Auth,
    State(state): State<AppState>,
    Json(input): Json<Name>,
) -> Result<Json<Value>> {
    auth.require_admin()?;
    if input.name.trim().is_empty() || input.name.len() > 100 {
        return Err(AppError::invalid("Name must contain 1 to 100 bytes"));
    }
    let token = format!("bm_live_{}", crypto::random_token());
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO api_keys (id,name,prefix,token_hash) VALUES ($1,$2,$3,$4)")
        .bind(id)
        .bind(input.name)
        .bind(&token[..16])
        .bind(crypto::hash(&token))
        .execute(&state.db)
        .await?;
    Ok(Json(json!({"id":id,"token":token})))
}
pub async fn revoke_key(
    auth: Auth,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    auth.require_admin()?;
    if sqlx::query("UPDATE api_keys SET revoked_at=now() WHERE id=$1")
        .bind(id)
        .execute(&state.db)
        .await?
        .rows_affected()
        == 0
    {
        return Err(AppError::not_found());
    }
    Ok(Json(json!({"ok":true})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateInput {
    name: String,
    subject: String,
    #[serde(default)]
    html: String,
    #[serde(default)]
    text: String,
}
fn validate_template(input: &TemplateInput) -> Result<()> {
    if input.name.trim().is_empty()
        || input.name.len() > 100
        || input.subject.is_empty()
        || input.subject.len() > 998
        || input.subject.contains(['\r', '\n'])
        || input.html.len() + input.text.len() > 512 * 1024
        || (input.html.is_empty() && input.text.is_empty())
    {
        return Err(AppError::invalid(
            "Template requires a name, subject and message body within the size limits",
        ));
    }
    Ok(())
}
pub async fn list_templates(auth: Auth, State(state): State<AppState>) -> Result<Json<Value>> {
    auth.require_admin()?;
    let rows = sqlx::query_as::<_, Template>("SELECT * FROM templates ORDER BY updated_at DESC")
        .fetch_all(&state.db)
        .await?;
    Ok(Json(
        json!({"data":rows,"presets":crate::initial_templates::presets()}),
    ))
}
pub async fn add_template(
    auth: Auth,
    State(state): State<AppState>,
    Json(input): Json<TemplateInput>,
) -> Result<Json<Template>> {
    auth.require_admin()?;
    validate_template(&input)?;
    Ok(Json(
        sqlx::query_as(
            "INSERT INTO templates (id,name,subject,html,text) VALUES ($1,$2,$3,$4,$5) RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(input.name)
        .bind(input.subject)
        .bind(input.html)
        .bind(input.text)
        .fetch_one(&state.db)
        .await?,
    ))
}
pub async fn update_template(
    auth: Auth,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<TemplateInput>,
) -> Result<Json<Template>> {
    auth.require_admin()?;
    validate_template(&input)?;
    Ok(Json(sqlx::query_as("UPDATE templates SET name=$1,subject=$2,html=$3,text=$4,updated_at=now() WHERE id=$5 RETURNING *")
        .bind(input.name).bind(input.subject).bind(input.html).bind(input.text).bind(id).fetch_optional(&state.db).await?.ok_or_else(AppError::not_found)?))
}
pub async fn delete_template(
    auth: Auth,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    auth.require_admin()?;
    if sqlx::query("DELETE FROM templates WHERE id=$1")
        .bind(id)
        .execute(&state.db)
        .await?
        .rows_affected()
        == 0
    {
        return Err(AppError::not_found());
    }
    Ok(Json(json!({"ok":true})))
}

#[derive(Deserialize, Default)]
pub struct EmailQuery {
    status: Option<String>,
    #[serde(default)]
    page: u32,
}
pub async fn list_emails(
    auth: Auth,
    State(state): State<AppState>,
    Query(query): Query<EmailQuery>,
) -> Result<Json<Value>> {
    if query.page > 10000 {
        return Err(AppError::invalid("Page is too large"));
    }
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'from_address',from_address,'to_address',to_address,'subject',subject,'status',status,'attempts',attempts,'created_at',created_at,'sent_at',sent_at,'last_error',last_error,'next_attempt_at',next_attempt_at) FROM emails WHERE ($1::uuid IS NULL OR api_key_id=$1) AND ($2::text IS NULL OR status=$2) ORDER BY created_at DESC LIMIT 51 OFFSET $3")
        .bind(auth.api_key_id).bind(query.status).bind(i64::from(query.page)*50).fetch_all(&state.db).await?;
    let has_more = rows.len() > 50;
    Ok(Json(
        json!({"data":rows.into_iter().take(50).collect::<Vec<_>>(),"has_more":has_more,"page":query.page}),
    ))
}
pub async fn email_detail(
    auth: Auth,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let email = sqlx::query_as::<_, Email>(
        "SELECT * FROM emails WHERE id=$1 AND ($2::uuid IS NULL OR api_key_id=$2)",
    )
    .bind(id)
    .bind(auth.api_key_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(AppError::not_found)?;
    let attempts:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('result',result,'detail',detail,'created_at',created_at) FROM delivery_attempts WHERE email_id=$1 ORDER BY created_at DESC").bind(id).fetch_all(&state.db).await?;
    Ok(Json(json!({"email":email,"attempts":attempts})))
}
pub async fn retry_email(
    auth: Auth,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    auth.require_admin()?;
    if !state.config.delivery_enabled {
        return Err(AppError::conflict("Delivery is paused"));
    }
    let row=sqlx::query("UPDATE emails SET status='queued',attempts=0,next_attempt_at=now(),last_error=NULL WHERE id=$1 AND status='failed'").bind(id).execute(&state.db).await?;
    if row.rows_affected() == 0 {
        return Err(AppError::conflict("Only failed messages can be retried"));
    }
    Ok(Json(json!({"ok":true})))
}
pub async fn cancel_email(
    auth: Auth,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    auth.require_admin()?;
    let row = sqlx::query("UPDATE emails SET status='cancelled' WHERE id=$1 AND status='queued'")
        .bind(id)
        .execute(&state.db)
        .await?;
    if row.rows_affected() == 0 {
        return Err(AppError::conflict("Only queued messages can be cancelled"));
    }
    Ok(Json(json!({"ok":true})))
}
