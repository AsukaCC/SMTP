use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    env,
    net::{IpAddr, SocketAddr},
};
use url::Url;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub bind: SocketAddr,
    pub app_url: Url,
    pub mail_hostname: String,
    pub public_ip: Option<IpAddr>,
    pub encryption_key: [u8; 32],
    pub owner_email: String,
    pub bootstrap_password: Option<String>,
    pub delivery_enabled: bool,
    pub smtp_bind: Option<SocketAddr>,
    pub smtp_cert: Option<String>,
    pub smtp_key: Option<String>,
    pub development: bool,
    pub update_repository: Option<String>,
    pub cloudflare_api_token: Option<zeroize::Zeroizing<String>>,
}

fn required(key: &str) -> Result<String> {
    env::var(key).with_context(|| format!("Missing {key}"))
}

fn boolean(key: &str, default: bool) -> Result<bool> {
    match env::var(key).as_deref() {
        Ok("true") => Ok(true),
        Ok("false") => Ok(false),
        Err(_) => Ok(default),
        _ => bail!("{key} must be true or false"),
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let development = env::var("APP_ENV").as_deref() == Ok("development");
        let app_url: Url = required("APP_URL")?.parse()?;
        if !matches!(app_url.scheme(), "http" | "https")
            || app_url.host_str().is_none()
            || app_url.path() != "/"
            || !app_url.username().is_empty()
            || app_url.password().is_some()
            || app_url.query().is_some()
            || app_url.fragment().is_some()
        {
            bail!("APP_URL must be an HTTP(S) origin");
        }
        if !development && app_url.scheme() != "https" {
            bail!("Production requires an HTTPS APP_URL behind a TLS reverse proxy");
        }
        let key = STANDARD
            .decode(required("DKIM_ENCRYPTION_KEY")?)
            .context("Invalid DKIM_ENCRYPTION_KEY")?;
        let encryption_key: [u8; 32] = key
            .try_into()
            .map_err(|_| anyhow::anyhow!("DKIM_ENCRYPTION_KEY must decode to 32 bytes"))?;
        let mail_hostname = required("MAIL_HOSTNAME")?.to_lowercase();
        if !valid_domain(&mail_hostname) {
            bail!("Invalid MAIL_HOSTNAME");
        }
        let public_ip = env::var("PUBLIC_IP")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.parse())
            .transpose()?;
        let delivery_enabled = boolean("DELIVERY_ENABLED", false)?;
        if delivery_enabled && public_ip.is_none() {
            bail!("PUBLIC_IP is required to enable delivery");
        }
        if public_ip.is_some_and(|ip| !crate::dns::public_address(ip)) {
            bail!("PUBLIC_IP must be a globally routable address");
        }
        let smtp_bind = env::var("SMTP_BIND")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<SocketAddr>())
            .transpose()?;
        let smtp_cert = env::var("SMTP_TLS_CERT").ok().filter(|s| !s.is_empty());
        let smtp_key = env::var("SMTP_TLS_KEY").ok().filter(|s| !s.is_empty());
        if smtp_cert.is_some() != smtp_key.is_some() {
            bail!("Configure both SMTP_TLS_CERT and SMTP_TLS_KEY");
        }
        if smtp_bind.is_some() && smtp_cert.is_none() {
            bail!("SMTP submission requires TLS certificates");
        }
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            bind: env::var("HTTP_BIND")
                .unwrap_or_else(|_| "127.0.0.1:3000".into())
                .parse()?,
            app_url,
            mail_hostname,
            public_ip,
            encryption_key,
            owner_email: required("OWNER_EMAIL")?.to_lowercase(),
            bootstrap_password: env::var("BOOTSTRAP_PASSWORD")
                .ok()
                .filter(|s| !s.is_empty()),
            delivery_enabled,
            smtp_bind,
            smtp_cert,
            smtp_key,
            development,
            update_repository: crate::updates::repository(
                &env::var("UPDATE_REPOSITORY").unwrap_or_else(|_| "AsukaCC/SMTP".into()),
            )?,
            cloudflare_api_token: env::var("CLOUDFLARE_API_TOKEN")
                .ok()
                .filter(|s| !s.is_empty())
                .map(zeroize::Zeroizing::new),
        })
    }
}

pub fn valid_domain(value: &str) -> bool {
    value.is_ascii()
        && value.len() <= 253
        && value.contains('.')
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        && value.parse::<IpAddr>().is_err()
}
