use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Serialize, Clone)]
pub struct Domain {
    pub id: Uuid,
    pub name: String,
    pub selector: String,
    pub public_key: String,
    #[serde(skip)]
    pub private_key: String,
    pub verified: bool,
    pub checks: Value,
    pub checked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Deserialize, Clone)]
pub struct Template {
    pub id: Uuid,
    pub name: String,
    pub subject: String,
    pub html: String,
    pub text: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Clone)]
pub struct Email {
    pub id: Uuid,
    pub api_key_id: Option<Uuid>,
    pub domain_id: Uuid,
    pub from_address: String,
    pub to_address: String,
    pub subject: String,
    pub html: String,
    pub text: String,
    pub status: String,
    pub attempts: i32,
    pub next_attempt_at: DateTime<Utc>,
    pub lease_until: Option<DateTime<Utc>>,
    #[serde(skip)]
    pub lease_token: Option<Uuid>,
    pub last_error: Option<String>,
    pub message_id: String,
    pub created_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct SendRequest {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub html: String,
    #[serde(default)]
    pub text: String,
    pub template_id: Option<Uuid>,
    #[serde(default)]
    pub variables: serde_json::Map<String, Value>,
}
