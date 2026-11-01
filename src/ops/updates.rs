use crate::{auth::Auth, error::Result, state::AppState};
use axum::{Json, extract::State};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub fn repository(value: &str) -> anyhow::Result<Option<String>> {
    if value.is_empty() {
        return Ok(None);
    }
    let parts: Vec<_> = value.split('/').collect();
    anyhow::ensure!(
        parts.len() == 2
            && parts.iter().all(|part| {
                !part.is_empty()
                    && part.len() <= 100
                    && *part != "."
                    && *part != ".."
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            }),
        "UPDATE_REPOSITORY must be owner/repository or empty"
    );
    Ok(Some(value.into()))
}

#[derive(Clone, Serialize)]
pub struct Check {
    pub status: String,
    pub checked_at: DateTime<Utc>,
    pub latest_version: Option<String>,
    pub release_url: Option<String>,
}

#[derive(Serialize)]
pub struct UpdateStatus {
    current_version: &'static str,
    repository: Option<String>,
    releases_url: Option<String>,
    status: String,
    check: Option<Check>,
    retry_after_seconds: i64,
}

fn snapshot(repo: &Option<String>, check: Option<Check>) -> UpdateStatus {
    UpdateStatus {
        current_version: env!("CARGO_PKG_VERSION"),
        repository: repo.clone(),
        releases_url: repo
            .as_ref()
            .map(|r| format!("https://github.com/{r}/releases")),
        status: if repo.is_none() {
            "disabled"
        } else {
            check.as_ref().map_or("not_checked", |c| c.status.as_str())
        }
        .into(),
        retry_after_seconds: check.as_ref().map_or(0, |c| {
            (60 - (Utc::now() - c.checked_at).num_seconds()).clamp(0, 60)
        }),
        check,
    }
}

pub async fn status(auth: Auth, State(state): State<AppState>) -> Result<Json<UpdateStatus>> {
    auth.require_admin()?;
    Ok(Json(snapshot(
        &state.config.update_repository,
        state.updates.lock().await.clone(),
    )))
}

pub async fn check(auth: Auth, State(state): State<AppState>) -> Result<Json<UpdateStatus>> {
    auth.require_admin()?;
    // Serialize checks and reuse the result for one minute, including upstream failures.
    let mut cached = state.updates.lock().await;
    if let Some(repo) = &state.config.update_repository
        && cached
            .as_ref()
            .is_none_or(|c| (Utc::now() - c.checked_at).num_seconds() >= 60)
    {
        let url = format!("https://api.github.com/repos/{repo}/releases/latest");
        let result = fetch_release(&url, repo).await.unwrap_or_else(|_| Check {
            status: "check_failed".into(),
            checked_at: Utc::now(),
            latest_version: None,
            release_url: None,
        });
        *cached = Some(result);
    }
    Ok(Json(snapshot(
        &state.config.update_repository,
        cached.clone(),
    )))
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

fn release_check(repo: &str, release: Release, current: &str) -> anyhow::Result<Check> {
    let number = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);
    let latest = semver::Version::parse(number)?;
    anyhow::ensure!(
        !release.draft && !release.prerelease && latest.pre.is_empty() && latest.build.is_empty(),
        "Not a stable release"
    );
    let current = semver::Version::parse(current)?;
    let status = match latest.cmp(&current) {
        std::cmp::Ordering::Greater => "update_available",
        std::cmp::Ordering::Equal => "up_to_date",
        std::cmp::Ordering::Less => "ahead",
    };
    let mut url = url::Url::parse(&format!("https://github.com/{repo}/releases/tag/"))?;
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("Invalid release URL"))?
        .pop_if_empty()
        .push(&release.tag_name);
    Ok(Check {
        status: status.into(),
        checked_at: Utc::now(),
        latest_version: Some(latest.to_string()),
        release_url: Some(url.into()),
    })
}

async fn fetch_release(url: &str, repo: &str) -> anyhow::Result<Check> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .user_agent(concat!("BunMail-RS/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let mut response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(Check {
            status: "no_release".into(),
            checked_at: Utc::now(),
            latest_version: None,
            release_url: None,
        });
    }
    anyhow::ensure!(
        response.status().is_success(),
        "Release service unavailable"
    );
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(
            body.len() + chunk.len() <= 512 * 1024,
            "Release response too large"
        );
        body.extend_from_slice(&chunk);
    }
    release_check(
        repo,
        serde_json::from_slice(&body)?,
        env!("CARGO_PKG_VERSION"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_repository_and_compares_versions_numerically() {
        for invalid in ["https://example.com/a", "a/../b", "a/b?x", "a/..", "a/b#x"] {
            assert!(repository(invalid).is_err());
        }
        assert!(repository("").unwrap().is_none());
        assert_eq!(
            repository("AsukaCC/SMTP").unwrap().as_deref(),
            Some("AsukaCC/SMTP")
        );
        for (tag, expected) in [
            ("v0.1.10", "update_available"),
            ("0.1.9", "up_to_date"),
            ("v0.1.8", "ahead"),
        ] {
            let result = release_check(
                "AsukaCC/SMTP",
                Release {
                    tag_name: tag.into(),
                    draft: false,
                    prerelease: false,
                },
                "0.1.9",
            )
            .unwrap();
            assert_eq!(result.status, expected);
            assert!(
                result
                    .release_url
                    .unwrap()
                    .starts_with("https://github.com/AsukaCC/SMTP/releases/tag/")
            );
        }
        for tag in ["latest", "v0.2.0-rc.1", "v0.2.0+build"] {
            assert!(
                release_check(
                    "a/b",
                    Release {
                        tag_name: tag.into(),
                        draft: false,
                        prerelease: false
                    },
                    "0.1.0"
                )
                .is_err()
            );
        }
    }

    #[tokio::test]
    async fn handles_upstream_errors_without_claiming_up_to_date() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for (status, body, expected) in [
            (
                "200 OK",
                r#"{"tag_name":"v0.1.0","draft":false,"prerelease":false}"#,
                Some("up_to_date"),
            ),
            ("404 Not Found", "{}", Some("no_release")),
            ("403 Forbidden", "{}", None),
            ("200 OK", "invalid", None),
            ("302 Found", "{}", None),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096];
                assert!(stream.read(&mut request).await.unwrap() > 0);
                stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            });
            let result = fetch_release(&format!("http://{address}"), "AsukaCC/SMTP").await;
            assert_eq!(result.ok().map(|c| c.status), expected.map(str::to_owned));
            server.await.unwrap();
        }
    }
}
