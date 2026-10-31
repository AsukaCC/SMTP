use crate::config::Config;
use hickory_resolver::TokioAsyncResolver;
use sqlx::PgPool;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Clone)]
pub struct AppState {
    pub cloudflare_lock: Arc<tokio::sync::Mutex<()>>,
    pub updates: Arc<tokio::sync::Mutex<Option<crate::updates::Check>>>,
    pub db: PgPool,
    pub config: Arc<Config>,
    pub dns: TokioAsyncResolver,
    pub limits: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
}

pub struct RateLimitDecision {
    pub allowed: bool,
    pub retry_after_seconds: u64,
}

fn seconds_until(expires: Instant, now: Instant) -> u64 {
    expires
        .saturating_duration_since(now)
        .as_millis()
        .div_ceil(1000) as u64
}

fn check_limit(
    limits: &mut HashMap<String, (u32, Instant)>,
    key: String,
    max: u32,
    seconds: u64,
    now: Instant,
) -> RateLimitDecision {
    limits.retain(|_, (_, expires)| *expires > now);
    if limits.len() > 10_000 && !limits.contains_key(&key) {
        return RateLimitDecision {
            allowed: false,
            retry_after_seconds: seconds.max(1),
        };
    }
    let entry = limits
        .entry(key)
        .or_insert((0, now + std::time::Duration::from_secs(seconds)));
    let allowed = entry.0 < max;
    if allowed {
        entry.0 += 1;
    }
    RateLimitDecision {
        allowed,
        retry_after_seconds: if entry.0 >= max {
            seconds_until(entry.1, now)
        } else {
            0
        },
    }
}

impl AppState {
    pub async fn delivery_enabled(&self) -> Result<bool, sqlx::Error> {
        let saved: Option<bool> =
            sqlx::query_scalar("SELECT enabled FROM delivery_settings WHERE id=TRUE")
                .fetch_optional(&self.db)
                .await?;
        Ok(saved.unwrap_or(self.config.delivery_enabled) && self.config.public_ip.is_some())
    }

    pub fn rate_limit(&self, key: String, max: u32, seconds: u64) -> bool {
        self.check_rate_limit(key, max, seconds).allowed
    }

    pub fn check_rate_limit(&self, key: String, max: u32, seconds: u64) -> RateLimitDecision {
        let mut limits = self.limits.lock().unwrap_or_else(|e| e.into_inner());
        check_limit(&mut limits, key, max, seconds, Instant::now())
    }

    pub fn retry_after_seconds(&self, key: &str, max: u32) -> u64 {
        let limits = self.limits.lock().unwrap_or_else(|e| e.into_inner());
        limits
            .get(key)
            .filter(|(count, _)| *count >= max)
            .map_or(0, |(_, expires)| seconds_until(*expires, Instant::now()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn cooldown_does_not_extend_when_requests_are_rejected() {
        let mut limits = HashMap::new();
        let start = Instant::now();
        for attempt in 1..=3 {
            let result = check_limit(&mut limits, "domain-a".into(), 3, 60, start);
            assert!(result.allowed);
            assert_eq!(
                result.retry_after_seconds,
                if attempt == 3 { 60 } else { 0 }
            );
        }
        let denied = check_limit(
            &mut limits,
            "domain-a".into(),
            3,
            60,
            start + Duration::from_millis(20_001),
        );
        assert!(!denied.allowed);
        assert_eq!(denied.retry_after_seconds, 40);
        assert!(check_limit(&mut limits, "domain-b".into(), 3, 60, start).allowed);
        assert!(
            check_limit(
                &mut limits,
                "domain-a".into(),
                3,
                60,
                start + Duration::from_secs(60)
            )
            .allowed
        );
    }
}
