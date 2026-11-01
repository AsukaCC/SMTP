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

impl Domain {
    pub fn can_send(&self) -> bool {
        self.checked_at
            .is_some_and(|at| at > Utc::now() - chrono::Duration::hours(24))
            && serde_json::from_value::<crate::dns::Checks>(self.checks.clone())
                .is_ok_and(|checks| checks.verified())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sender_eligibility_uses_fresh_required_checks_instead_of_cached_flag() {
        let mut domain = Domain {
            id: Uuid::nil(),
            name: "example.test".into(),
            selector: "bunmail".into(),
            public_key: String::new(),
            private_key: String::new(),
            verified: false,
            checks: json!({"spf":true,"dkim":true,"dmarc":true,"forward":true,"ptr":false}),
            checked_at: Some(Utc::now()),
            created_at: Utc::now(),
        };
        assert!(domain.can_send());
        let good = domain.checks.clone();
        domain.verified = true;
        for key in ["spf", "dkim", "dmarc", "forward"] {
            domain.checks = good.clone();
            domain.checks[key] = json!(false);
            assert!(!domain.can_send());
            domain.checks.as_object_mut().unwrap().remove(key);
            assert!(!domain.can_send());
        }
        for invalid in [
            json!({}),
            json!(null),
            json!({"spf":"true","dkim":true,"dmarc":true,"forward":true}),
        ] {
            domain.checks = invalid;
            assert!(!domain.can_send());
        }
        domain.checks = good;
        domain.checked_at = Some(Utc::now() - chrono::Duration::hours(25));
        assert!(!domain.can_send());
        domain.checked_at = None;
        assert!(!domain.can_send());
    }
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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub from_name: String,
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
