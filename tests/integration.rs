use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use bunmail_rs::{
    auth::{self, Auth},
    config::Config,
    crypto,
    model::{Domain, Email, SendRequest},
    queue, send,
    state::AppState,
    web,
};
use hickory_resolver::TokioAsyncResolver;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;
use uuid::Uuid;

async fn request(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Value,
    auth: Option<&str>,
    origin: bool,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if origin {
        request = request.header("origin", "http://localhost:3000");
    }
    if let Some(auth) = auth {
        request = if auth.starts_with("Bearer ") {
            request.header("authorization", auth)
        } else {
            request.header("cookie", auth)
        };
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(json!({})),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires TEST_DATABASE_URL; creates and drops its own isolated database"]
async fn authenticated_api_queue_and_dkim() {
    let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL");
    let admin = PgPool::connect(&url).await.unwrap();
    let db_name = format!("bunmail_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE DATABASE {db_name}"))
        .execute(&admin)
        .await
        .unwrap();
    let mut db_url = url::Url::parse(&url).unwrap();
    db_url.set_path(&db_name);
    let db = PgPoolOptions::new()
        .max_connections(8)
        .connect(db_url.as_str())
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    let state = AppState {
        db: db.clone(),
        config: Arc::new(Config {
            database_url: db_url.to_string(),
            bind: "127.0.0.1:0".parse().unwrap(),
            app_url: "http://localhost:3000".parse().unwrap(),
            mail_hostname: "mail.example.test".into(),
            public_ip: Some("8.8.8.8".parse().unwrap()),
            encryption_key: [42; 32],
            owner_email: "owner@example.test".into(),
            bootstrap_password: Some("correct-horse-battery-staple-123".into()),
            delivery_enabled: true,
            smtp_bind: None,
            smtp_cert: None,
            smtp_key: None,
            development: true,
        }),
        dns: TokioAsyncResolver::tokio_from_system_conf().unwrap(),
        limits: Arc::new(Mutex::new(HashMap::new())),
    };
    auth::bootstrap(&state).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM templates")
            .fetch_one(&db)
            .await
            .unwrap(),
        8
    );
    let app = web::router(state.clone()).layer(axum::Extension(ConnectInfo(
        "127.0.0.1:10000".parse::<SocketAddr>().unwrap(),
    )));
    assert_eq!(
        request(&app, "GET", "/api/v1/domains", json!({}), None, false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let input = json!({"email":"owner@example.test","password":"correct-horse-battery-staple-123"});
    assert_eq!(
        request(&app, "POST", "/api/v1/login", input.clone(), None, false)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, headers, _) = request(&app, "POST", "/api/v1/login", input, None, true).await;
    assert_eq!(status, StatusCode::OK);
    let cookie = headers["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    assert!(headers["set-cookie"].to_str().unwrap().contains("HttpOnly"));
    let (_, _, domain) = request(
        &app,
        "POST",
        "/api/v1/domains",
        json!({"name":"example.test"}),
        Some(&cookie),
        true,
    )
    .await;
    let id = Uuid::parse_str(domain["domain"]["id"].as_str().unwrap()).unwrap();
    assert!(domain["domain"].get("private_key").is_none());
    let stored = sqlx::query_as::<_, Domain>("SELECT * FROM domains WHERE id=$1")
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(!stored.private_key.contains("PRIVATE KEY"));
    assert!(
        crypto::decrypt(&[42; 32], "example.test", &stored.private_key)
            .unwrap()
            .contains("RSA PRIVATE KEY")
    );
    let (_, _, key) = request(
        &app,
        "POST",
        "/api/v1/api-keys",
        json!({"name":"test"}),
        Some(&cookie),
        true,
    )
    .await;
    let token = key["token"].as_str().unwrap();
    let bearer = format!("Bearer {token}");
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/v1/domains",
            json!({}),
            Some(&bearer),
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (_, _, keys) = request(
        &app,
        "GET",
        "/api/v1/api-keys",
        json!({}),
        Some(&cookie),
        false,
    )
    .await;
    assert!(!keys.to_string().contains(token));
    let send_input = SendRequest {
        from: "hello@example.test".into(),
        to: "recipient@example.net".into(),
        subject: "中文标题".into(),
        html: "<p>你好</p>".into(),
        text: "你好".into(),
        ..Default::default()
    };
    let key_id = Uuid::parse_str(key["id"].as_str().unwrap()).unwrap();
    let identity = Auth {
        api_key_id: Some(key_id),
    };
    assert!(
        send::enqueue(&state, &identity, send_input.clone(), None)
            .await
            .is_err()
    );
    // Fixture bypasses DNS only inside this isolated database. No delivery worker is started.
    sqlx::query("UPDATE domains SET verified=TRUE,checked_at=now() WHERE id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    let (first, second) = tokio::join!(
        send::enqueue(
            &state,
            &identity,
            send_input.clone(),
            Some("same-key".into())
        ),
        send::enqueue(
            &state,
            &identity,
            send_input.clone(),
            Some("same-key".into())
        )
    );
    let (id1, created1) = first.unwrap_or_else(|e| panic!("{}", e.2));
    let (id2, created2) = second.unwrap_or_else(|e| panic!("{}", e.2));
    assert_eq!(id1, id2);
    assert_ne!(created1, created2);
    let mut different = send_input;
    different.subject = "different".into();
    assert!(
        send::enqueue(&state, &identity, different, Some("same-key".into()))
            .await
            .is_err()
    );
    let (a, b) = tokio::join!(queue::claim(&state), queue::claim(&state));
    let a = a.unwrap();
    let b = b.unwrap();
    assert_ne!(a.is_some(), b.is_some());
    let email = a.or(b).unwrap();
    let domain = sqlx::query_as::<_, Domain>("SELECT * FROM domains WHERE name='example.test'")
        .fetch_one(&db)
        .await
        .unwrap();
    let signed = queue::build_message(&email, &domain, &[42; 32]).unwrap();
    let raw = String::from_utf8(signed.formatted()).unwrap();
    assert!(raw.contains("DKIM-Signature:"));
    assert!(raw.contains("d=example.test"));
    assert!(raw.contains("s=bunmail"));
    assert!(raw.contains("Message-ID:"));
    assert!(!signed.formatted().is_empty());
    sqlx::query("UPDATE emails SET lease_until=now()-interval '1 second' WHERE id=$1")
        .bind(email.id)
        .execute(&db)
        .await
        .unwrap();
    let reclaimed = queue::claim(&state).await.unwrap().unwrap();
    assert_ne!(email.lease_token, reclaimed.lease_token);
    queue::finish(&state, &email, Ok(())).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM emails WHERE id=$1")
            .bind(email.id)
            .fetch_one(&db)
            .await
            .unwrap(),
        "sending"
    );
    queue::finish(
        &state,
        &reclaimed,
        Err(queue::DeliveryError {
            permanent: false,
            detail: "temporary fixture failure".into(),
        }),
    )
    .await
    .unwrap();
    let queued = sqlx::query_as::<_, Email>("SELECT * FROM emails WHERE id=$1")
        .bind(email.id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(queued.status, "queued");
    assert!(queued.next_attempt_at > chrono::Utc::now());
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("/api/v1/domains/{}", domain.id),
            json!({}),
            Some(&cookie),
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/api/v1/emails/{}/cancel", email.id),
            json!({}),
            Some(&cookie),
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    let template =
        json!({"name":"测试模板","subject":"Hello {{ name }}","html":"<p>{{ name }}</p>"});
    let (status, _, template) = request(
        &app,
        "POST",
        "/api/v1/templates",
        template,
        Some(&cookie),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("/api/v1/templates/{}", template["id"].as_str().unwrap()),
            json!({}),
            Some(&cookie),
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    // Exercise actual STARTTLS + AUTH submission against the Rust SMTP service.
    let certificates = tempfile::tempdir().unwrap();
    let cert = certificates.path().join("cert.pem");
    let private = certificates.path().join("key.pem");
    assert!(
        std::process::Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-days",
                "1",
                "-subj",
                "/CN=localhost",
                "-addext",
                "subjectAltName=DNS:localhost",
                "-keyout"
            ])
            .arg(&private)
            .arg("-out")
            .arg(&cert)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    );
    let address = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap()
    };
    let mut smtp_config = (*state.config).clone();
    smtp_config.smtp_bind = Some(address);
    smtp_config.smtp_cert = Some(cert.to_string_lossy().into_owned());
    smtp_config.smtp_key = Some(private.to_string_lossy().into_owned());
    let mut smtp_state = state.clone();
    smtp_state.config = Arc::new(smtp_config);
    let (stop, smtp_shutdown) = tokio::sync::watch::channel(false);
    let smtp_task = bunmail_rs::smtp::start(smtp_state, smtp_shutdown)
        .await
        .unwrap();
    use lettre::{
        AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
        transport::smtp::{
            authentication::Credentials,
            client::{Tls, TlsParameters},
        },
    };
    let message = Message::builder()
        .from("hello@example.test".parse().unwrap())
        .to("recipient@example.net".parse().unwrap())
        .subject("SMTP fixture")
        .body("Local submission fixture".to_string())
        .unwrap();
    let tls = TlsParameters::builder("localhost".into())
        .dangerous_accept_invalid_certs(true)
        .build()
        .unwrap();
    let anonymous = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1")
        .port(address.port())
        .tls(Tls::Required(tls.clone()))
        .build();
    assert!(anonymous.send(message.clone()).await.is_err());
    let authenticated = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1")
        .port(address.port())
        .tls(Tls::Required(tls))
        .credentials(Credentials::new("bunmail".into(), token.into()))
        .build();
    authenticated.send(message).await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM emails WHERE subject='SMTP fixture' AND status='queued'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(count, 1);
    stop.send(true).unwrap();
    smtp_task.await.unwrap();
    verify_templates_preview_and_test_send(&app, &state, &cookie, &bearer).await;
    if std::env::var("BUNMAIL_BROWSER_TEST").as_deref() == Ok("true") {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut browser_config = (*state.config).clone();
        browser_config.app_url = format!("http://{address}").parse().unwrap();
        let mut browser_state = state.clone();
        browser_state.config = Arc::new(browser_config);
        // Isolated database and no delivery worker: browser test can enqueue but never send externally.
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                web::router(browser_state).into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        let status = tokio::process::Command::new("node")
            .arg("scripts/test-configured-ui.cjs")
            .env("TEST_UI_URL", format!("http://{address}"))
            .status()
            .await
            .unwrap();
        server.abort();
        let _ = server.await;
        assert!(status.success());
    }
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("/api/v1/api-keys/{key_id}"),
            json!({}),
            Some(&cookie),
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(auth::api_key(&state, token).await.is_err());
    auth::reset_password(&state, "replacement-password-for-test".into())
        .await
        .unwrap();
    assert_eq!(
        request(&app, "GET", "/api/v1/me", json!({}), Some(&cookie), false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, new_headers, _) = request(
        &app,
        "POST",
        "/api/v1/login",
        json!({"email":"owner@example.test","password":"replacement-password-for-test"}),
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let cookie = new_headers["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/logout",
            json!({}),
            Some(&cookie),
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(&app, "GET", "/api/v1/me", json!({}), Some(&cookie), false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    drop(app);
    drop(state);
    db.close().await;
    sqlx::query(&format!("DROP DATABASE {db_name} WITH (FORCE)"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

async fn verify_templates_preview_and_test_send(
    app: &axum::Router,
    state: &AppState,
    cookie: &str,
    bearer: &str,
) {
    let preset_id = "bee00000-0000-4000-8000-000000000001";
    sqlx::query("UPDATE templates SET subject='User edited subject' WHERE id=$1::uuid")
        .bind(preset_id)
        .execute(&state.db)
        .await
        .unwrap();
    let (status, _, data) = request(
        app,
        "POST",
        "/api/v1/templates/initialize",
        json!({}),
        Some(cookie),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(data["inserted"], 0);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT subject FROM templates WHERE id=$1::uuid")
            .bind(preset_id)
            .fetch_one(&state.db)
            .await
            .unwrap(),
        "User edited subject"
    );
    assert_eq!(
        request(
            app,
            "POST",
            "/api/v1/templates/initialize",
            json!({}),
            Some(bearer),
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );

    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair(
            "html",
            "<p style=\"color:red\">{{ name }}</p><script>window.previewUnsafe=true</script>",
        )
        .append_pair("variables", r#"{"name":"<script>escaped</script>"}"#)
        .finish();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/templates/preview")
                .header("cookie", cookie)
                .header("origin", "http://localhost:3000")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let policy = response.headers()["content-security-policy"]
        .to_str()
        .unwrap();
    assert!(policy.contains("default-src 'none'"));
    assert!(policy.contains("sandbox;"));
    assert!(policy.contains("style-src 'unsafe-inline'"));
    assert_eq!(response.headers()["x-frame-options"], "SAMEORIGIN");
    let html = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("&lt;script&gt;escaped"));

    let test = json!({"from":"test@example.test","to":"recipient@example.net","language":"en"});
    assert_eq!(
        request(
            app,
            "POST",
            "/api/v1/emails/test",
            test.clone(),
            Some(bearer),
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            app,
            "POST",
            "/api/v1/emails/test",
            test.clone(),
            Some(cookie),
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    for (enabled, ip) in [(false, Some("8.8.8.8".parse().unwrap())), (true, None)] {
        let mut config = (*state.config).clone();
        config.delivery_enabled = enabled;
        config.public_ip = ip;
        let mut paused = state.clone();
        paused.config = Arc::new(config);
        let paused_app = web::router(paused);
        assert_eq!(
            request(
                &paused_app,
                "GET",
                "/api/v1/config",
                json!({}),
                Some(cookie),
                false
            )
            .await
            .2["ready_to_test"],
            false
        );
        assert_eq!(
            request(
                &paused_app,
                "POST",
                "/api/v1/emails/test",
                test.clone(),
                Some(cookie),
                true
            )
            .await
            .0,
            StatusCode::CONFLICT
        );
    }
    sqlx::query("UPDATE domains SET checked_at=now()-interval '25 hours'")
        .execute(&state.db)
        .await
        .unwrap();
    assert_eq!(
        request(app, "GET", "/api/v1/config", json!({}), Some(cookie), false)
            .await
            .2["ready_to_test"],
        false
    );
    assert_eq!(
        request(
            app,
            "POST",
            "/api/v1/emails/test",
            test.clone(),
            Some(cookie),
            true
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    sqlx::query("UPDATE domains SET checked_at=now(),verified=true")
        .execute(&state.db)
        .await
        .unwrap();
    assert_eq!(
        request(app, "GET", "/api/v1/config", json!({}), Some(cookie), false)
            .await
            .2["ready_to_test"],
        true
    );
    let mut ids = Vec::new();
    for expected in [StatusCode::ACCEPTED, StatusCode::OK] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/emails/test")
                    .header("cookie", cookie)
                    .header("origin", "http://localhost:3000")
                    .header("content-type", "application/json")
                    .header("idempotency-key", "test-feature-idempotency")
                    .body(Body::from(test.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        ids.push(body["id"].as_str().unwrap().to_owned());
    }
    assert_eq!(ids[0], ids[1]);
    let email: Email = sqlx::query_as("SELECT * FROM emails WHERE id=$1::uuid")
        .bind(&ids[0])
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(email.status, "queued");
    assert_eq!(email.subject, "BunMail RS email delivery test");
    assert!(!email.html.contains("{{"));
    let domain: Domain = sqlx::query_as("SELECT * FROM domains WHERE id=$1")
        .bind(email.domain_id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert!(
        String::from_utf8(
            queue::build_message(&email, &domain, &state.config.encryption_key)
                .unwrap()
                .formatted()
        )
        .unwrap()
        .contains("DKIM-Signature:")
    );
}
