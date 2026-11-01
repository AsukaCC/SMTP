use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use bunmail_rs::{state::AppState, web};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (StatusCode, HeaderMap, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    for (key, value) in headers {
        builder = builder.header(*key, *value);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        headers,
        serde_json::from_slice(&body).unwrap_or(Value::Null),
    )
}

pub async fn verify(state: &AppState, cookie: &str, bearer: &str) {
    let mut config = (*state.config).clone();
    config.api_allowed_origins = vec!["https://client.example".into()];
    let mut fixture = state.clone();
    fixture.config = Arc::new(config);
    fixture.limits = Default::default();
    let app = web::router(fixture.clone());
    let authorized = [
        ("authorization", bearer),
        ("content-type", "application/json"),
    ];
    let (status, headers, data) = call(&app, "GET", "/api/v1/auth/check", &authorized, "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(data["scope"], "send");
    uuid::Uuid::parse_str(headers["x-request-id"].to_str().unwrap()).unwrap();
    let (status, headers, data) = call(&app, "GET", "/api/v1/auth/check", &[], "").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(data["error"]["code"], "UNAUTHORIZED");
    assert!(headers.contains_key("www-authenticate"));
    let cross = [
        ("authorization", bearer),
        ("origin", "https://client.example"),
        ("content-type", "application/json"),
    ];
    let (_, headers, _) = call(&app, "GET", "/api/v1/auth/check", &cross, "").await;
    assert_eq!(
        headers["access-control-allow-origin"],
        "https://client.example"
    );
    assert!(!headers.contains_key("access-control-allow-credentials"));
    assert!(
        headers["access-control-expose-headers"]
            .to_str()
            .unwrap()
            .contains("x-request-id")
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/v1/auth/check",
            &[("cookie", cookie), ("origin", "https://client.example")],
            ""
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, headers, data) = call(
        &app,
        "GET",
        "/api/v1/auth/check",
        &[
            ("authorization", bearer),
            ("origin", "https://evil.example"),
        ],
        "",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(data["error"]["code"], "ORIGIN_NOT_ALLOWED");
    assert!(!headers.contains_key("access-control-allow-origin"));
    let (status, headers, _) = call(
        &app,
        "OPTIONS",
        "/api/v1/emails/send",
        &[
            ("origin", "https://client.example"),
            ("access-control-request-method", "POST"),
            (
                "access-control-request-headers",
                "authorization,content-type,idempotency-key",
            ),
        ],
        "",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers["access-control-allow-origin"],
        "https://client.example"
    );
    assert!(!headers.contains_key("access-control-allow-credentials"));
    let (_, headers, _) = call(&app, "GET", "/api/v1/config", &cross, "").await;
    assert!(!headers.contains_key("access-control-allow-origin"));
    for (method, path, body, expected, code) in [
        ("POST", "/api/v1/emails/send", "{", 400, "BAD_REQUEST"),
        (
            "POST",
            "/api/v1/emails/send",
            r#"{"from":"hello@example.test","to":"recipient@example.net","unexpected":true}"#,
            422,
            "INVALID_INPUT",
        ),
        ("GET", "/api/v1/emails/not-a-uuid", "", 400, "BAD_REQUEST"),
        ("GET", "/api/v1/emails?page=invalid", "", 400, "BAD_REQUEST"),
        (
            "DELETE",
            "/api/v1/emails/send",
            "",
            405,
            "METHOD_NOT_ALLOWED",
        ),
        ("GET", "/api/v1/missing", "", 404, "NOT_FOUND"),
    ] {
        let (status, headers, data) = call(&app, method, path, &authorized, body).await;
        assert_eq!(status.as_u16(), expected);
        assert_eq!(data["error"]["code"], code);
        assert!(headers.contains_key("x-request-id"));
        assert!(
            headers["content-type"]
                .to_str()
                .unwrap()
                .starts_with("application/json")
        );
    }
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/emails/send",
            &[("authorization", bearer)],
            "{}"
        )
        .await
        .0,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/emails/send",
            &authorized,
            &" ".repeat(1024 * 1024 + 1)
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let (_, _, spec) = call(&app, "GET", "/api/v1/openapi.json", &[], "").await;
    assert_eq!(spec["openapi"], "3.1.0");
    let example=spec["paths"]["/emails/send"]["post"]["requestBody"]["content"]["application/json"]["examples"]["template"]["value"].clone();
    let mut message = json!({"from":"hello@example.test","to":"recipient@example.net","subject":"Public API contract","text":"fixture"});
    let headers = [
        ("authorization", bearer),
        ("content-type", "application/json"),
        ("idempotency-key", "contract-test"),
    ];
    let first = call(
        &app,
        "POST",
        "/api/v1/emails/send",
        &headers,
        &message.to_string(),
    )
    .await;
    assert_eq!(first.0, StatusCode::ACCEPTED);
    assert_eq!(first.2["created"], true);
    let second = call(
        &app,
        "POST",
        "/api/v1/emails/send",
        &headers,
        &message.to_string(),
    )
    .await;
    assert_eq!(second.0, StatusCode::OK);
    assert_eq!(second.2["id"], first.2["id"]);
    message["subject"] = json!("changed");
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/emails/send",
            &headers,
            &message.to_string()
        )
        .await
        .2["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
    let mut template = example;
    template["from"] = json!("hello@example.test");
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/emails/send",
            &authorized,
            &template.to_string()
        )
        .await
        .0,
        StatusCode::ACCEPTED
    );
    let key_id = bunmail_rs::auth::api_key(&fixture, bearer.strip_prefix("Bearer ").unwrap())
        .await
        .unwrap();
    for _ in 0..60 {
        fixture.rate_limit(format!("send:{key_id}"), 60, 60);
    }
    let (status, headers, data) = call(
        &app,
        "POST",
        "/api/v1/emails/send",
        &authorized,
        &message.to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(data["error"]["code"], "RATE_LIMITED");
    assert_eq!(headers["retry-after"], "60");
}

pub async fn browser(state: &AppState, bearer: &str) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let site = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let site_url = format!("http://{}", site.local_addr().unwrap());
    let mut config = (*state.config).clone();
    config.app_url = url.parse().unwrap();
    config.api_allowed_origins = vec![site_url.clone()];
    let mut fixture = state.clone();
    fixture.config = Arc::new(config);
    fixture.limits = Default::default();
    let server = tokio::spawn(async move {
        axum::serve(listener, web::router(fixture)).await.unwrap();
    });
    let site_server = tokio::spawn(async move {
        axum::serve(site,Router::new().route("/",axum::routing::get(|| async {axum::response::Html("<!doctype html><title>Integration origin</title><h1>Integration origin</h1>")}))).await.unwrap();
    });
    let status = tokio::process::Command::new("node")
        .arg("scripts/test-third-party.cjs")
        .env("TEST_API_URL", format!("{url}/api/v1"))
        .env("TEST_ORIGIN_URL", site_url)
        .env("TEST_API_KEY", bearer.strip_prefix("Bearer ").unwrap())
        .status()
        .await
        .unwrap();
    server.abort();
    site_server.abort();
    let _ = server.await;
    let _ = site_server.await;
    assert!(status.success());
}
