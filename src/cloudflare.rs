use crate::{
    auth::Auth,
    crypto, dns,
    error::{AppError, Result},
    model::Domain,
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, State},
};
use reqwest::{
    Client, Method,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

const API: &str = "https://api.cloudflare.com/client/v4";

struct Cloudflare {
    client: Client,
    base: String,
}

#[derive(Deserialize)]
struct Zone {
    id: String,
    name: String,
}

#[derive(Clone, Deserialize, Serialize)]
struct Record {
    #[serde(rename = "type")]
    kind: String,
    name: String,
    content: String,
    #[serde(default)]
    proxied: bool,
}

#[derive(Clone, Serialize)]
struct PlannedRecord {
    zone_id: Option<String>,
    kind: String,
    name: String,
    value: String,
    action: String,
    existing: Vec<Record>,
}

#[derive(Serialize)]
pub struct Plan {
    records: Vec<PlannedRecord>,
    fingerprint: String,
    can_apply: bool,
}

fn provider_error() -> AppError {
    AppError::invalid("Cloudflare request failed. Check token permissions and connectivity")
}

impl Cloudflare {
    fn new(token: &str, base: &str) -> Result<Self> {
        let mut authorization =
            HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| provider_error())?;
        authorization.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization);
        let client = Client::builder()
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| provider_error())?;
        Ok(Self {
            client,
            base: base.into(),
        })
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<Value> {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base))
            .query(query);
        if let Some(body) = body {
            request = request
                .header("Content-Type", "application/json")
                .body(body.to_string());
        }
        let mut response = request.send().await.map_err(|_| provider_error())?;
        if !response.status().is_success() {
            return Err(provider_error());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| provider_error())? {
            if bytes.len() + chunk.len() > 1024 * 1024 {
                return Err(provider_error());
            }
            bytes.extend_from_slice(&chunk);
        }
        let data: Value = serde_json::from_slice(&bytes).map_err(|_| provider_error())?;
        if data["success"] != true {
            return Err(provider_error());
        }
        Ok(data)
    }

    async fn list(&self, path: &str, query: &[(&str, String)]) -> Result<Vec<Value>> {
        let mut result = Vec::new();
        for page in 1..=20 {
            let mut query = query.to_vec();
            query.extend([("page", page.to_string()), ("per_page", "50".into())]);
            let data = self.request(Method::GET, path, &query, None).await?;
            let rows = data["result"].as_array().ok_or_else(provider_error)?;
            let pages = data["result_info"]["total_pages"]
                .as_u64()
                .or_else(|| {
                    data["result_info"]["total_count"]
                        .as_u64()
                        .map(|n| n.div_ceil(50))
                })
                .unwrap_or(if rows.len() < 50 { page } else { page + 1 });
            if pages > 20 {
                return Err(AppError::invalid(
                    "Too many Cloudflare records. Narrow the token scope",
                ));
            }
            result.extend(rows.iter().cloned());
            if page >= pages {
                return Ok(result);
            }
        }
        Err(provider_error())
    }

    async fn zones(&self) -> Result<Vec<Zone>> {
        let zones: Vec<Zone> = self
            .list("/zones", &[("status", "active".into())])
            .await?
            .into_iter()
            .map(serde_json::from_value)
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| provider_error())?;
        if zones.iter().any(|z| {
            z.id.len() != 32
                || !z.id.bytes().all(|b| b.is_ascii_hexdigit())
                || !crate::config::valid_domain(&z.name)
        }) {
            return Err(provider_error());
        }
        Ok(zones)
    }

    async fn records(&self, zone: &str, name: &str) -> Result<Vec<Record>> {
        let mut records: Vec<Record> = self
            .list(
                &format!("/zones/{zone}/dns_records"),
                &[("name.exact", name.into())],
            )
            .await?
            .into_iter()
            .map(serde_json::from_value)
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| provider_error())?;
        records.retain(|r| normalize_name(&r.name) == normalize_name(name));
        records.sort_by(|a, b| {
            (&a.kind, &a.content, a.proxied).cmp(&(&b.kind, &b.content, b.proxied))
        });
        Ok(records)
    }

    async fn plan(&self, desired: Vec<dns::DnsRecord>) -> Result<Plan> {
        let zones = self.zones().await?;
        let mut records = Vec::new();
        for record in desired.into_iter().filter(|r| r.kind != "PTR") {
            let zone = zone_for(&zones, &record.name);
            let existing = match zone {
                Some(z) => self.records(&z.id, &record.name).await?,
                None => vec![],
            };
            let action = if zone.is_none() {
                "unavailable"
            } else {
                classify(&record, &existing)
            };
            records.push(PlannedRecord {
                zone_id: zone.map(|z| z.id.clone()),
                kind: record.kind.into(),
                name: record.name,
                value: record.value,
                action: action.into(),
                existing,
            });
        }
        let fingerprint =
            crypto::hash(&serde_json::to_string(&records).map_err(|_| AppError::internal())?);
        let can_apply = records.iter().any(|r| r.action == "create")
            && records
                .iter()
                .all(|r| matches!(r.action.as_str(), "create" | "unchanged"));
        Ok(Plan {
            records,
            fingerprint,
            can_apply,
        })
    }

    async fn create(&self, record: &PlannedRecord) -> Result<()> {
        let zone = record.zone_id.as_deref().ok_or_else(provider_error)?;
        let mut body =
            json!({"type":record.kind,"name":record.name,"content":record.value,"ttl":1});
        if matches!(record.kind.as_str(), "A" | "AAAA") {
            body["proxied"] = json!(false);
        }
        self.request(
            Method::POST,
            &format!("/zones/{zone}/dns_records"),
            &[],
            Some(body),
        )
        .await?;
        Ok(())
    }
}

fn normalize_name(name: &str) -> String {
    name.trim_end_matches('.').to_ascii_lowercase()
}

fn zone_for<'a>(zones: &'a [Zone], name: &str) -> Option<&'a Zone> {
    let name = normalize_name(name);
    zones
        .iter()
        .filter(|z| {
            name == normalize_name(&z.name)
                || name.ends_with(&format!(".{}", normalize_name(&z.name)))
        })
        .max_by_key(|z| z.name.len())
}

fn txt_content(content: &str) -> std::borrow::Cow<'_, str> {
    use hickory_proto::{
        rr::{RData, RecordType},
        serialize::txt::RDataParser,
    };
    let content = content.trim();
    if content.starts_with('"')
        && let Ok(RData::TXT(txt)) = RData::try_from_str(RecordType::TXT, content)
    {
        return String::from_utf8_lossy(&txt.txt_data().concat())
            .into_owned()
            .into();
    }
    content.into()
}

fn classify(desired: &dns::DnsRecord, existing: &[Record]) -> &'static str {
    if existing.iter().any(|r| {
        matches!(r.kind.as_str(), "CNAME" | "NS")
            || (matches!(desired.kind, "A" | "AAAA")
                && matches!(r.kind.as_str(), "A" | "AAAA")
                && r.proxied)
    }) {
        return "conflict";
    }
    let relevant: Vec<_> = existing
        .iter()
        .filter(|r| {
            r.kind == desired.kind
                && (desired.kind != "TXT"
                    || if desired.value.starts_with("v=spf1 ") {
                        txt_content(&r.content)
                            .to_ascii_lowercase()
                            .starts_with("v=spf1")
                    } else if desired.value.starts_with("v=DMARC1;") {
                        dns::tag(&txt_content(&r.content), "v")
                            .is_some_and(|v| v.eq_ignore_ascii_case("DMARC1"))
                    } else {
                        true
                    })
        })
        .collect();
    if relevant.is_empty() {
        return "create";
    }
    if relevant.len() == 1 && txt_content(&relevant[0].content) == desired.value {
        "unchanged"
    } else {
        "conflict"
    }
}

async fn context(state: &AppState, id: Uuid) -> Result<(Cloudflare, Vec<dns::DnsRecord>)> {
    let token =
        state.config.cloudflare_api_token.as_ref().ok_or_else(|| {
            AppError::invalid("Configure CLOUDFLARE_API_TOKEN on the server first")
        })?;
    if state.config.public_ip.is_none() {
        return Err(AppError::invalid(
            "Configure PUBLIC_IP before creating DNS records",
        ));
    }
    let domain = sqlx::query_as::<_, Domain>("SELECT * FROM domains WHERE id=$1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(AppError::not_found)?;
    Ok((Cloudflare::new(token, API)?, dns::records(state, &domain)))
}

pub async fn preview(
    auth: Auth,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Plan>> {
    auth.require_admin()?;
    let (client, records) = context(&state, id).await?;
    Ok(Json(client.plan(records).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyRequest {
    fingerprint: String,
}

pub async fn apply(
    auth: Auth,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<ApplyRequest>,
) -> Result<Json<Value>> {
    auth.require_admin()?;
    let _guard = state
        .cloudflare_lock
        .try_lock()
        .map_err(|_| AppError::conflict("A Cloudflare change is already in progress"))?;
    let (client, records) = context(&state, id).await?;
    let result = apply_checked(&client, records, &input.fingerprint).await?;
    // Writing provider records does not prove that public DNS has propagated.
    sqlx::query("UPDATE domains SET verified=false,checked_at=NULL,checks='{}' WHERE id=$1")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Json(result))
}

async fn apply_checked(
    client: &Cloudflare,
    records: Vec<dns::DnsRecord>,
    fingerprint: &str,
) -> Result<Value> {
    let plan = client.plan(records).await?;
    if fingerprint != plan.fingerprint || !plan.can_apply {
        return Err(AppError::conflict(
            "DNS records changed or contain conflicts. Preview again",
        ));
    }
    Ok(apply_plan(client, plan).await)
}

async fn apply_plan(client: &Cloudflare, plan: Plan) -> Value {
    let mut failed = false;
    let mut outcomes = Vec::new();
    for record in plan.records {
        let mut outcome = "unchanged";
        if record.action == "create" {
            if failed {
                outcome = "skipped";
            } else {
                let result = async {
                    let zone = record.zone_id.as_deref().ok_or_else(provider_error)?;
                    let existing = client.records(zone, &record.name).await?;
                    let desired = dns::DnsRecord {
                        kind: match record.kind.as_str() {
                            "A" => "A",
                            "AAAA" => "AAAA",
                            _ => "TXT",
                        },
                        name: record.name.clone(),
                        value: record.value.clone(),
                    };
                    match classify(&desired, &existing) {
                        "unchanged" => Ok("unchanged"),
                        "create" => {
                            client.create(&record).await?;
                            Ok("created")
                        }
                        _ => Err(AppError::conflict(
                            "DNS records changed or contain conflicts. Preview again",
                        )),
                    }
                }
                .await;
                match result {
                    Ok(status) => outcome = status,
                    Err(_) => {
                        outcome = "failed";
                        failed = true;
                    }
                }
            }
        }
        outcomes.push(json!({"kind":record.kind,"name":record.name,"status":outcome}));
    }
    json!({"complete":!failed,"records":outcomes})
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, extract::Query, http::StatusCode, routing::get};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Fixture {
        records: Vec<Record>,
        writes: usize,
        fail_on: Option<usize>,
    }

    async fn fixture() -> (Cloudflare, Arc<Mutex<Fixture>>, tokio::task::JoinHandle<()>) {
        let state = Arc::new(Mutex::new(Fixture::default()));
        let router = Router::new().route("/zones", get(|| async { Json(json!({"success":true,"result":[{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","name":"example.com"}],"result_info":{"total_pages":1}})) }))
            .route("/zones/{id}/dns_records", get(|State(state): State<Arc<Mutex<Fixture>>>, Query(query): Query<std::collections::HashMap<String,String>>| async move {
                let state = state.lock().unwrap();
                let rows: Vec<_> = state.records.iter().filter(|r| Some(&r.name) == query.get("name.exact")).cloned().collect();
                Json(json!({"success":true,"result_info":{"total_count":rows.len()},"result":rows}))
            }).post(|State(state): State<Arc<Mutex<Fixture>>>, headers: HeaderMap, Json(body): Json<Value>| async move {
                assert_eq!(headers[AUTHORIZATION], "Bearer test-token");
                assert_ne!(body["type"], "PTR");
                if body["type"] == "A" || body["type"] == "AAAA" { assert_eq!(body["proxied"], false); }
                let mut state = state.lock().unwrap();
                state.writes += 1;
                if state.fail_on == Some(state.writes) { return (StatusCode::FORBIDDEN, Json(json!({"success":false,"errors":[{"message":"do not leak test-token"}]}))); }
                state.records.push(serde_json::from_value(body).unwrap());
                (StatusCode::OK, Json(json!({"success":true,"result":{}})))
            })).with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (
            Cloudflare::new("test-token", &format!("http://{address}")).unwrap(),
            state,
            server,
        )
    }

    fn desired() -> Vec<dns::DnsRecord> {
        vec![
            dns::DnsRecord {
                kind: "TXT",
                name: "example.com".into(),
                value: "v=spf1 ip4:8.8.8.8 -all".into(),
            },
            dns::DnsRecord {
                kind: "TXT",
                name: "bunmail._domainkey.example.com".into(),
                value: "v=DKIM1; k=rsa; p=public-key".into(),
            },
            dns::DnsRecord {
                kind: "TXT",
                name: "_dmarc.example.com".into(),
                value: "v=DMARC1; p=none".into(),
            },
            dns::DnsRecord {
                kind: "A",
                name: "mail.example.com".into(),
                value: "8.8.8.8".into(),
            },
            dns::DnsRecord {
                kind: "PTR",
                name: "8.8.8.8".into(),
                value: "mail.example.com".into(),
            },
        ]
    }

    #[tokio::test]
    async fn preview_is_read_only_apply_is_repeatable_and_partial_errors_are_reported() {
        let (client, state, server) = fixture().await;
        let plan = client.plan(desired()).await.unwrap();
        assert!(plan.can_apply);
        assert_eq!(plan.records.len(), 4);
        assert_eq!(state.lock().unwrap().writes, 0);
        state.lock().unwrap().fail_on = Some(2);
        let result = apply_checked(&client, desired(), &plan.fingerprint)
            .await
            .unwrap();
        assert_eq!(result["complete"], false);
        assert_eq!(result["records"][0]["status"], "created");
        assert_eq!(result["records"][1]["status"], "failed");
        assert_eq!(result["records"][2]["status"], "skipped");
        assert!(!result.to_string().contains("test-token"));
        let retry = client.plan(desired()).await.unwrap();
        assert_eq!(retry.records[0].action, "unchanged");
        state.lock().unwrap().fail_on = None;
        assert_eq!(
            apply_checked(&client, desired(), &retry.fingerprint)
                .await
                .unwrap()["complete"],
            true
        );
        assert_eq!(state.lock().unwrap().records.len(), 4);
        let done = client.plan(desired()).await.unwrap();
        assert!(!done.can_apply);
        assert!(done.records.iter().all(|r| r.action == "unchanged"));
        assert_eq!(
            apply_checked(&client, desired(), &retry.fingerprint)
                .await
                .unwrap_err()
                .0,
            StatusCode::CONFLICT
        );
        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn changed_preview_conflicts_and_inaccessible_zones_never_write() {
        let (client, state, server) = fixture().await;
        let plan = client.plan(desired()).await.unwrap();
        state
            .lock()
            .unwrap()
            .records
            .push(record("TXT", "v=spf1 include:other.example -all"));
        assert!(
            apply_checked(&client, desired(), &plan.fingerprint)
                .await
                .is_err()
        );
        let conflict = client.plan(desired()).await.unwrap();
        assert!(!conflict.can_apply);
        assert!(
            apply_checked(&client, desired(), &conflict.fingerprint)
                .await
                .is_err()
        );
        let missing = client
            .plan(vec![dns::DnsRecord {
                kind: "A",
                name: "mail.other.test".into(),
                value: "8.8.8.8".into(),
            }])
            .await
            .unwrap();
        assert_eq!(missing.records[0].action, "unavailable");
        assert!(!missing.can_apply);
        assert_eq!(state.lock().unwrap().writes, 0);
        server.abort();
        let _ = server.await;
    }

    fn record(kind: &str, content: &str) -> Record {
        Record {
            kind: kind.into(),
            name: "example.com".into(),
            content: content.into(),
            proxied: false,
        }
    }

    #[test]
    fn preserves_unrelated_records_and_blocks_conflicts() {
        assert_eq!(
            txt_content("\"v=DKIM1; p=first\" \"second\""),
            "v=DKIM1; p=firstsecond"
        );
        let spf = dns::DnsRecord {
            kind: "TXT",
            name: "example.com".into(),
            value: "v=spf1 ip4:8.8.8.8 -all".into(),
        };
        assert_eq!(
            classify(&spf, &[record("TXT", "verification=other-service")]),
            "create"
        );
        assert_eq!(
            classify(&spf, &[record("TXT", "\"v=spf1 ip4:8.8.8.8 -all\"")]),
            "unchanged"
        );
        assert_eq!(
            classify(&spf, &[record("TXT", "v=spf1 include:other.example -all")]),
            "conflict"
        );
        assert_eq!(
            classify(&spf, &[record("CNAME", "other.example")]),
            "conflict"
        );
        assert_eq!(
            classify(
                &spf,
                &[record("TXT", &spf.value), record("TXT", &spf.value)]
            ),
            "conflict"
        );
        let ip = dns::DnsRecord {
            kind: "A",
            name: "example.com".into(),
            value: "8.8.8.8".into(),
        };
        assert_eq!(
            classify(
                &ip,
                &[Record {
                    proxied: true,
                    ..record("A", "8.8.8.8")
                }]
            ),
            "conflict"
        );
        assert_eq!(classify(&ip, &[record("A", "8.8.8.8")]), "unchanged");
    }

    #[test]
    fn selects_longest_zone_on_label_boundaries() {
        let zones = vec![
            Zone {
                id: "one".into(),
                name: "example.com".into(),
            },
            Zone {
                id: "two".into(),
                name: "mail.example.com".into(),
            },
        ];
        assert_eq!(
            zone_for(&zones, "HOST.MAIL.EXAMPLE.COM.").unwrap().id,
            "two"
        );
        assert!(zone_for(&zones, "notexample.com").is_none());
    }
}
