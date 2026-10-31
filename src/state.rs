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
    pub db: PgPool,
    pub config: Arc<Config>,
    pub dns: TokioAsyncResolver,
    pub limits: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
}
impl AppState {
    pub fn rate_limit(&self, key: String, max: u32, seconds: u64) -> bool {
        let mut limits = self.limits.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        limits.retain(|_, (_, expires)| *expires > now);
        if limits.len() > 10_000 && !limits.contains_key(&key) {
            return false;
        }
        let entry = limits
            .entry(key)
            .or_insert((0, now + std::time::Duration::from_secs(seconds)));
        entry.0 += 1;
        entry.0 <= max
    }
}
