use crate::{
    auth::Auth,
    error::{AppError, Result},
    state::AppState,
};
use axum::{Json, extract::State, http::StatusCode};
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
    pub packages: Vec<Package>,
}

#[derive(Clone, Serialize)]
pub struct Package {
    pub name: String,
    pub platform: String,
    pub size: u64,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Job {
    request_id: uuid::Uuid,
    version: String,
    status: String,
    error: Option<String>,
    updated_at: String,
}

#[derive(Deserialize, Serialize)]
struct Installer {
    status: String,
    repository: Option<String>,
    platform: Option<String>,
    current_version: Option<String>,
    job: Option<Job>,
}

impl Installer {
    fn unavailable(status: &str) -> Self {
        Self {
            status: status.into(),
            repository: None,
            platform: None,
            current_version: None,
            job: None,
        }
    }
}

#[derive(Serialize)]
pub struct UpdateStatus {
    current_version: &'static str,
    repository: Option<String>,
    releases_url: Option<String>,
    status: String,
    check: Option<Check>,
    retry_after_seconds: i64,
    installer: Installer,
    can_install: bool,
    package: Option<Package>,
}

fn snapshot(repo: &Option<String>, check: Option<Check>, installer: Installer) -> UpdateStatus {
    let package = check.as_ref().and_then(|check| {
        check
            .packages
            .iter()
            .find(|package| Some(&package.platform) == installer.platform.as_ref())
            .cloned()
    });
    let can_install = installer.status == "ready"
        && installer.repository == *repo
        && installer.current_version.as_deref() == Some(env!("CARGO_PKG_VERSION"))
        && package.is_some()
        && check
            .as_ref()
            .is_some_and(|c| c.status == "update_available")
        && !installer.job.as_ref().is_some_and(|job| {
            matches!(
                job.status.as_str(),
                "queued"
                    | "downloading"
                    | "validating"
                    | "backing_up"
                    | "installing"
                    | "rolling_back"
            )
        });
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
        installer,
        can_install,
        package,
    }
}

pub async fn status(auth: Auth, State(state): State<AppState>) -> Result<Json<UpdateStatus>> {
    auth.require_admin()?;
    let cached = state.updates.lock().await.clone();
    Ok(Json(snapshot(
        &state.config.update_repository,
        cached,
        installer_status(&state).await,
    )))
}

pub async fn check(auth: Auth, State(state): State<AppState>) -> Result<Json<UpdateStatus>> {
    auth.require_admin()?;
    refresh(&state).await;
    status(auth, State(state)).await
}

async fn refresh(state: &AppState) {
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
            packages: vec![],
        });
        *cached = Some(result);
    }
}

fn installer_client(state: &AppState) -> anyhow::Result<reqwest::Client> {
    let socket = state
        .config
        .update_socket
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Updater disabled"))?;
    anyhow::ensure!(socket.is_absolute(), "Updater socket must be absolute");
    Ok(reqwest::Client::builder()
        .unix_socket(socket.clone())
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()?)
}

async fn installer_status(state: &AppState) -> Installer {
    if state.config.update_socket.is_none() {
        return Installer::unavailable("disabled");
    }
    async {
        let response = installer_client(state)?
            .get("http://localhost/status")
            .send()
            .await?;
        anyhow::ensure!(response.status().is_success(), "Updater unavailable");
        let body = bounded_body(response, 16 * 1024).await?;
        Ok::<Installer, anyhow::Error>(serde_json::from_slice(&body)?)
    }
    .await
    .unwrap_or_else(|_| Installer::unavailable("offline"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallRequest {
    version: String,
    request_id: uuid::Uuid,
}

pub async fn install(
    auth: Auth,
    State(state): State<AppState>,
    Json(input): Json<InstallRequest>,
) -> Result<(StatusCode, Json<Job>)> {
    auth.require_admin()?;
    let installer = installer_status(&state).await;
    if let Some(job) = &installer.job
        && job.request_id == input.request_id
        && job.version == input.version
    {
        return Ok((StatusCode::ACCEPTED, Json(job.clone())));
    }
    refresh(&state).await;
    let cached = state.updates.lock().await.clone();
    let snapshot = snapshot(&state.config.update_repository, cached, installer);
    if !snapshot.can_install
        || snapshot
            .check
            .as_ref()
            .and_then(|c| c.latest_version.as_deref())
            != Some(input.version.as_str())
    {
        return Err(AppError::conflict("No installable update is available"));
    }
    let client =
        installer_client(&state).map_err(|_| AppError::conflict("Update service unavailable"))?;
    let response = client
        .post("http://localhost/install")
        .header("Content-Type", "application/json")
        .body(
            serde_json::json!({"version":input.version,"request_id":input.request_id,
            "repository":state.config.update_repository})
            .to_string(),
        )
        .send()
        .await
        .map_err(|_| AppError::conflict("Update service unavailable"))?;
    if response.status() != reqwest::StatusCode::ACCEPTED {
        return Err(AppError::conflict(
            "Update could not start; refresh its status",
        ));
    }
    let body = bounded_body(response, 16 * 1024)
        .await
        .map_err(|_| AppError::internal())?;
    let job = serde_json::from_slice(&body).map_err(|_| AppError::internal())?;
    Ok((StatusCode::ACCEPTED, Json(job)))
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
    size: u64,
    state: String,
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
    let mut packages = Vec::new();
    for platform in ["linux/amd64", "linux/arm64"] {
        let name = format!("bunmail-rs-{latest}-{}.tar.gz", platform.replace('/', "-"));
        let assets: Vec<_> = release
            .assets
            .iter()
            .filter(|asset| asset.name == name)
            .collect();
        if let [asset] = assets.as_slice()
            && asset.state == "uploaded"
            && asset.size > 0
            && asset.size <= 2 * 1024 * 1024 * 1024
            && asset.browser_download_url
                == format!(
                    "https://github.com/{repo}/releases/download/{}/{name}",
                    release.tag_name
                )
            && asset
                .digest
                .as_ref()
                .and_then(|value| value.strip_prefix("sha256:"))
                .is_some_and(|value| {
                    value.len() == 64
                        && value
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
        {
            packages.push(Package {
                name,
                platform: platform.into(),
                size: asset.size,
            });
        }
    }
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
        packages,
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
    let response = client
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
            packages: vec![],
        });
    }
    anyhow::ensure!(
        response.status().is_success(),
        "Release service unavailable"
    );
    let body = bounded_body(response, 512 * 1024).await?;
    release_check(
        repo,
        serde_json::from_slice(&body)?,
        env!("CARGO_PKG_VERSION"),
    )
}

async fn bounded_body(mut response: reqwest::Response, limit: usize) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(
            body.len() + chunk.len() <= limit,
            "Release response too large"
        );
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_verified_matching_build_assets_enable_installation() {
        let current = env!("CARGO_PKG_VERSION");
        let mut next = semver::Version::parse(current).unwrap();
        next.patch += 1;
        let mut release = serde_json::json!({
            "tag_name":format!("v{next}"), "draft":false, "prerelease":false,
            "assets":[{"name":format!("bunmail-rs-{next}-linux-amd64.tar.gz"), "size":100,
                "state":"uploaded", "digest":format!("sha256:{}", "a".repeat(64)),
                "browser_download_url":format!("https://github.com/AsukaCC/SMTP/releases/download/v{next}/bunmail-rs-{next}-linux-amd64.tar.gz")}]
        });
        let check = release_check(
            "AsukaCC/SMTP",
            serde_json::from_value(release.clone()).unwrap(),
            current,
        )
        .unwrap();
        assert_eq!(check.packages.len(), 1);
        let installer = || Installer {
            status: "ready".into(),
            repository: Some("AsukaCC/SMTP".into()),
            platform: Some("linux/amd64".into()),
            current_version: Some(env!("CARGO_PKG_VERSION").into()),
            job: None,
        };
        assert!(
            snapshot(
                &Some("AsukaCC/SMTP".into()),
                Some(check.clone()),
                installer()
            )
            .can_install
        );
        let mut wrong = installer();
        wrong.platform = Some("linux/arm64".into());
        assert!(!snapshot(&Some("AsukaCC/SMTP".into()), Some(check.clone()), wrong).can_install);
        assert!(!snapshot(&Some("other/repo".into()), Some(check), installer()).can_install);
        release["assets"][0]["digest"] = serde_json::Value::Null;
        assert!(
            release_check(
                "AsukaCC/SMTP",
                serde_json::from_value(release).unwrap(),
                current
            )
            .unwrap()
            .packages
            .is_empty()
        );
    }

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
                    assets: vec![],
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
                        prerelease: false,
                        assets: vec![],
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
        let current_release = format!(
            r#"{{"tag_name":"v{}","draft":false,"prerelease":false}}"#,
            env!("CARGO_PKG_VERSION")
        );
        for (status, body, expected) in [
            ("200 OK", current_release.as_str(), Some("up_to_date")),
            ("404 Not Found", "{}", Some("no_release")),
            ("403 Forbidden", "{}", None),
            ("200 OK", "invalid", None),
            ("302 Found", "{}", None),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let body = body.to_owned();
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
