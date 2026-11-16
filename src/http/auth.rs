use crate::{
    crypto,
    error::{AppError, Result},
    state::AppState,
};
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    Json,
    extract::{ConnectInfo, FromRequestParts, State},
    http::{HeaderMap, StatusCode, header, request::Parts},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::net::SocketAddr;
use uuid::Uuid;

pub async fn bootstrap(state: &AppState) -> anyhow::Result<()> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE role='admin')")
            .fetch_one(&state.db)
            .await?;
    if exists {
        return Ok(());
    }
    let password = state
        .config
        .bootstrap_password
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Set BOOTSTRAP_PASSWORD for the first start"))?;
    if password.len() < 15 || password.len() > 256 {
        anyhow::bail!("BOOTSTRAP_PASSWORD must have 15 to 256 characters");
    }
    let hash = tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .map(|h| h.to_string())
    })
    .await?
    .map_err(|_| anyhow::anyhow!("Password hashing failed"))?;
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "INSERT INTO owner (email, password_hash) VALUES ($1,$2) ON CONFLICT (id) DO NOTHING",
    )
    .bind(&state.config.owner_email)
    .bind(&hash)
    .execute(&mut *tx)
    .await?;
    let inserted = sqlx::query(
        "INSERT INTO accounts (id, email, password_hash, role) VALUES ($1,$2,$3,'admin') ON CONFLICT (email) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(&state.config.owner_email)
    .bind(&hash)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 1 {
        crate::initial_templates::insert_missing(&mut tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

pub fn cookie_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == "bm_session").then_some(value)
        })
}

pub fn check_origin(headers: &HeaderMap, state: &AppState) -> Result<()> {
    if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok())
        != Some(state.config.app_url.origin().ascii_serialization().as_str())
    {
        return Err(AppError(
            StatusCode::FORBIDDEN,
            "INVALID_ORIGIN",
            "Request origin does not match APP_URL".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Admin,
    Key,
}

#[derive(Clone)]
pub struct Auth {
    pub api_key_id: Option<Uuid>,
    pub account_id: Option<Uuid>,
    pub role: Role,
}
impl Auth {
    pub fn require_admin(&self) -> Result<()> {
        if self.role == Role::Admin {
            Ok(())
        } else {
            Err(AppError::forbidden())
        }
    }

    pub fn require_session(&self) -> Result<()> {
        if self.account_id.is_some() {
            Ok(())
        } else {
            Err(AppError::forbidden())
        }
    }
}

async fn password_matches(hash: String, password: String) -> Result<bool> {
    tokio::task::spawn_blocking(move || {
        let password = zeroize::Zeroizing::new(password);
        PasswordHash::new(&hash).is_ok_and(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
    })
    .await
    .map_err(|_| AppError::internal())
}

pub async fn api_key(state: &AppState, token: &str) -> Result<Uuid> {
    if !token.starts_with("bm_live_") || token.len() != 72 {
        return Err(AppError::unauthorized());
    }
    sqlx::query_scalar("SELECT id FROM api_keys WHERE token_hash=$1 AND revoked_at IS NULL")
        .bind(crypto::hash(token))
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(AppError::unauthorized)
}

impl FromRequestParts<AppState> for Auth {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self> {
        if let Some(value) = parts.headers.get(header::AUTHORIZATION) {
            let token = value
                .to_str()
                .ok()
                .and_then(|v| v.strip_prefix("Bearer "))
                .ok_or_else(AppError::unauthorized)?;
            return Ok(Self {
                api_key_id: Some(api_key(state, token).await?),
                account_id: None,
                role: Role::Key,
            });
        }
        let token = cookie_token(&parts.headers).ok_or_else(AppError::unauthorized)?;
        if token.len() != 64 {
            return Err(AppError::unauthorized());
        }
        let account: Option<Uuid> = sqlx::query_scalar(
            "SELECT a.id FROM sessions s JOIN accounts a ON a.id=s.account_id WHERE s.token_hash=$1 AND s.expires_at>now() AND a.role='admin'",
        )
        .bind(crypto::hash(token))
        .fetch_optional(&state.db)
        .await?;
        let Some(account_id) = account else {
            return Err(AppError::unauthorized());
        };
        if parts.method != axum::http::Method::GET && parts.method != axum::http::Method::HEAD {
            check_origin(&parts.headers, state)?;
        }
        Ok(Self {
            api_key_id: None,
            account_id: Some(account_id),
            role: Role::Admin,
        })
    }
}

#[derive(Deserialize)]
pub struct Login {
    email: String,
    password: String,
}

pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<Login>,
) -> Result<(HeaderMap, Json<Value>)> {
    check_origin(&headers, &state)?;
    if !state.rate_limit(format!("login:{}", peer.ip()), 10, 300) {
        return Err(AppError(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "Too many attempts. Try again in five minutes".into(),
        ));
    }
    if input.password.len() > 256 || input.email.len() > 254 {
        return Err(AppError::unauthorized());
    }
    // Serialize login with password changes so a verified old password cannot create a new session.
    let mut tx = state.db.begin().await?;
    let account: Option<(Uuid, String, String, String)> = sqlx::query_as(
        "SELECT id, email, password_hash, role FROM accounts WHERE email=$1 AND role='admin' FOR UPDATE",
    )
    .bind(input.email.to_lowercase())
    .fetch_optional(&mut *tx)
    .await?;
    let Some((id, email, hash, role)) = account else {
        return Err(invalid_login());
    };
    if !password_matches(hash, input.password).await? {
        return Err(invalid_login());
    }
    let response = start_session(&state, &mut tx, id).await?;
    tx.commit().await?;
    Ok((response, Json(json!({"email":email,"role":role}))))
}

fn invalid_login() -> AppError {
    AppError(
        StatusCode::UNAUTHORIZED,
        "INVALID_LOGIN",
        "Invalid email or password".into(),
    )
}

async fn start_session(
    state: &AppState,
    connection: &mut sqlx::PgConnection,
    account_id: Uuid,
) -> Result<HeaderMap> {
    let token = crypto::random_token();
    sqlx::query(
        "INSERT INTO sessions (token_hash,expires_at,account_id) VALUES ($1,now()+interval '12 hours',$2)",
    )
    .bind(crypto::hash(&token))
    .bind(account_id)
    .execute(connection)
    .await?;
    let mut response = HeaderMap::new();
    response.insert(
        header::SET_COOKIE,
        session_cookie(state, &token, 43200)
            .parse()
            .map_err(|_| AppError::internal())?,
    );
    Ok(response)
}

fn session_cookie(state: &AppState, token: &str, age: u32) -> String {
    format!(
        "bm_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={age}{}",
        if state.config.app_url.scheme() == "https" {
            "; Secure"
        } else {
            ""
        }
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangePassword {
    current_password: String,
    new_password: String,
}

pub async fn change_password(
    auth: Auth,
    State(state): State<AppState>,
    Json(input): Json<ChangePassword>,
) -> Result<(HeaderMap, Json<Value>)> {
    auth.require_admin()?;
    let account_id = auth.account_id.ok_or_else(AppError::forbidden)?;
    if !state.rate_limit(format!("password:{account_id}"), 10, 300) {
        return Err(AppError(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "Too many attempts. Try again in five minutes".into(),
        ));
    }
    if input.current_password.len() > 256 || !(15..=256).contains(&input.new_password.len()) {
        return Err(AppError::invalid("Password must have 15 to 256 bytes"));
    }
    if input.current_password == input.new_password {
        return Err(AppError::invalid(
            "New password must differ from current password",
        ));
    }
    let mut tx = state.db.begin().await?;
    let hash: String = sqlx::query_scalar(
        "SELECT password_hash FROM accounts WHERE id=$1 AND role='admin' FOR UPDATE",
    )
    .bind(account_id)
    .fetch_one(&mut *tx)
    .await?;
    if !password_matches(hash, input.current_password).await? {
        return Err(AppError(
            StatusCode::FORBIDDEN,
            "INVALID_CURRENT_PASSWORD",
            "Current password is incorrect".into(),
        ));
    }
    let password = zeroize::Zeroizing::new(input.new_password);
    let hash = tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .map(|hash| hash.to_string())
    })
    .await
    .map_err(|_| AppError::internal())?
    .map_err(|_| AppError::internal())?;
    sqlx::query("UPDATE accounts SET password_hash=$1 WHERE id=$2")
        .bind(&hash)
        .bind(account_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE owner SET password_hash=$1")
        .bind(&hash)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE account_id=$1")
        .bind(account_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        session_cookie(&state, "", 0)
            .parse()
            .map_err(|_| AppError::internal())?,
    );
    Ok((headers, Json(json!({"ok":true}))))
}

pub async fn logout(
    auth: Auth,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<Value>)> {
    auth.require_session()?;
    if let Some(token) = cookie_token(&headers) {
        sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
            .bind(crypto::hash(token))
            .execute(&state.db)
            .await?;
    }
    let mut response = HeaderMap::new();
    response.insert(
        header::SET_COOKIE,
        session_cookie(&state, "", 0)
            .parse()
            .map_err(|_| AppError::internal())?,
    );
    Ok((response, Json(json!({"ok":true}))))
}

pub async fn me(auth: Auth, State(state): State<AppState>) -> Result<Json<Value>> {
    auth.require_session()?;
    let account_id = auth.account_id.ok_or_else(AppError::unauthorized)?;
    let (email, role): (String, String) =
        sqlx::query_as("SELECT email, role FROM accounts WHERE id=$1")
            .bind(account_id)
            .fetch_one(&state.db)
            .await?;
    Ok(Json(json!({"email":email,"role":role})))
}
