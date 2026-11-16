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
#[path = "support/public_api.rs"]
mod public_api;

#[test]
fn removed_password_command_is_rejected_before_startup() {
    let directory = tempfile::tempdir().unwrap();
    for args in [
        vec!["--reset-password"],
        vec!["--reset-password", "--healthcheck"],
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_bunmail-rs"))
            .args(args)
            .env_clear()
            .current_dir(directory.path())
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("Unsupported arguments")
        );
    }
}

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

async fn verify_delivery_switch(state: &AppState, cookie: &str, bearer: &str, input: &SendRequest) {
    let mut config = (*state.config).clone();
    config.delivery_enabled = false;
    let mut fixture = state.clone();
    fixture.config = Arc::new(config.clone());
    let app = web::router(fixture.clone());
    let path = "/api/v1/config/delivery";
    assert!(!fixture.delivery_enabled().await.unwrap());
    for (credential, origin, status) in [
        (None, true, StatusCode::UNAUTHORIZED),
        (Some(bearer), true, StatusCode::FORBIDDEN),
        (Some(cookie), false, StatusCode::FORBIDDEN),
    ] {
        assert_eq!(
            request(
                &app,
                "PUT",
                path,
                json!({"enabled":true}),
                credential,
                origin
            )
            .await
            .0,
            status
        );
    }
    config.public_ip = None;
    let mut missing_ip = fixture.clone();
    missing_ip.config = Arc::new(config);
    assert_eq!(
        request(
            &web::router(missing_ip.clone()),
            "PUT",
            path,
            json!({"enabled":true}),
            Some(cookie),
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    sqlx::query("UPDATE domains SET checked_at=now()-interval '25 hours'")
        .execute(&state.db)
        .await
        .unwrap();
    assert_eq!(
        request(
            &app,
            "PUT",
            path,
            json!({"enabled":true}),
            Some(cookie),
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    sqlx::query("UPDATE domains SET checked_at=now()")
        .execute(&state.db)
        .await
        .unwrap();
    assert_eq!(
        request(
            &app,
            "PUT",
            path,
            json!({"enabled":true}),
            Some(cookie),
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(fixture.delivery_enabled().await.unwrap());
    assert!(!missing_ip.delivery_enabled().await.unwrap());
    // Fresh app state reads the persisted switch even when the environment default is false.
    let restarted = web::router(fixture.clone());
    assert_eq!(
        request(
            &restarted,
            "GET",
            "/api/v1/config",
            json!({}),
            Some(cookie),
            false
        )
        .await
        .2["ready_to_test"],
        true
    );
    let (id, _) = send::enqueue(
        &fixture,
        &Auth {
            api_key_id: None,
            account_id: None,
            role: bunmail_rs::auth::Role::Admin,
        },
        input.clone(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        request(
            &app,
            "PUT",
            path,
            json!({"enabled":false}),
            Some(cookie),
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(!state.delivery_enabled().await.unwrap());
    assert!(
        send::enqueue(
            state,
            &Auth {
                api_key_id: None,
                account_id: None,
                role: bunmail_rs::auth::Role::Admin,
            },
            input.clone(),
            None,
        )
        .await
        .is_err()
    );
    assert!(queue::claim(state).await.unwrap().is_none());
    let status: String = sqlx::query_scalar("SELECT status FROM emails WHERE id=$1")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(status, "queued");
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/emails/test",
            json!({"from":"test@example.test","to":"recipient@example.net","language":"en"}),
            Some(cookie),
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
            &format!("/api/v1/emails/{id}/retry"),
            json!({}),
            Some(cookie),
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &app,
            "PUT",
            path,
            json!({"enabled":true}),
            Some(cookie),
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(queue::claim(state).await.unwrap().unwrap().id, id);
    sqlx::query("DELETE FROM emails WHERE id=$1")
        .bind(id)
        .execute(&state.db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM delivery_settings")
        .execute(&state.db)
        .await
        .unwrap();
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
    verify_single_admin_migration(&db).await;
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    let state = AppState {
        cloudflare_lock: Default::default(),
        updates: Default::default(),
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
            update_repository: None,
            update_socket: None,
            cloudflare_api_token: None,
            api_allowed_origins: vec![],
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
    verify_dns_cooldown(&state, &cookie, id).await;
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
    verify_update_access(&state, &cookie, &bearer).await;
    for action in ["preview", "apply"] {
        let path = format!("/api/v1/domains/{id}/cloudflare/{action}");
        let body = json!({"fingerprint":"test"});
        assert_eq!(
            request(&app, "POST", &path, body.clone(), None, true)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            request(&app, "POST", &path, body.clone(), Some(&bearer), true)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(&app, "POST", &path, body.clone(), Some(&cookie), false)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        let (status, _, error) = request(&app, "POST", &path, body, Some(&cookie), true).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error["error"]["message"],
            "Configure CLOUDFLARE_API_TOKEN on the server first"
        );
    }
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
        from_name: "中文发件人, 团队".into(),
        to: "recipient@example.net".into(),
        subject: "中文标题".into(),
        html: "<p>你好</p>".into(),
        text: "你好".into(),
        ..Default::default()
    };
    let key_id = Uuid::parse_str(key["id"].as_str().unwrap()).unwrap();
    let identity = Auth {
        api_key_id: Some(key_id),
        account_id: None,
        role: bunmail_rs::auth::Role::Key,
    };
    assert!(
        send::enqueue(&state, &identity, send_input.clone(), None)
            .await
            .is_err()
    );
    verify_without_ptr(&state, &cookie, &stored).await;
    // Reproduce an upgrade with fresh required checks but the old PTR-dependent flag.
    sqlx::query("UPDATE domains SET verified=false WHERE id=$1")
        .bind(id)
        .execute(&db)
        .await
        .unwrap();
    let (_, _, listing) = request(
        &app,
        "GET",
        "/api/v1/domains",
        json!({}),
        Some(&cookie),
        false,
    )
    .await;
    assert_eq!(listing["data"][0]["domain"]["verified"], true);
    let (_, _, configuration) = request(
        &app,
        "GET",
        "/api/v1/config",
        json!({}),
        Some(&cookie),
        false,
    )
    .await;
    assert_eq!(configuration["ready_to_test"], true);
    assert_eq!(configuration["rdns_warning"], true);
    assert_eq!(configuration["cloudflare_configured"], false);
    verify_delivery_switch(&state, &cookie, &bearer, &send_input).await;
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
    // Search uses literal text and retains the API key's ownership boundary.
    let mut private_input = send_input.clone();
    private_input.subject = "Private 100%_fixture".into();
    let (private_id, _) = send::enqueue(
        &state,
        &Auth {
            api_key_id: None,
            account_id: None,
            role: bunmail_rs::auth::Role::Admin,
        },
        private_input,
        None,
    )
    .await
    .unwrap();
    for (term, credential, expected) in [
        ("100%_", cookie.as_str(), 1),
        ("100%_", bearer.as_str(), 0),
        ("RECIPIENT@EXAMPLE.NET", bearer.as_str(), 1),
        ("中文标题", bearer.as_str(), 1),
        ("does-not-exist", cookie.as_str(), 0),
        ("' OR TRUE --", cookie.as_str(), 0),
    ] {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("search", term)
            .finish();
        let (status, _, data) = request(
            &app,
            "GET",
            &format!("/api/v1/emails?{query}"),
            json!({}),
            Some(credential),
            false,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(data["data"].as_array().unwrap().len(), expected);
    }
    for path in [
        "/api/v1/emails?status=unknown".to_owned(),
        format!("/api/v1/emails?search={}", "a".repeat(201)),
    ] {
        assert_eq!(
            request(&app, "GET", &path, json!({}), Some(&cookie), false)
                .await
                .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/v1/emails?status=failed&search=fixture",
            json!({}),
            Some(&cookie),
            false
        )
        .await
        .2["data"],
        json!([])
    );
    sqlx::query("DELETE FROM emails WHERE id=$1")
        .bind(private_id)
        .execute(&db)
        .await
        .unwrap();
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
    assert!(!domain.verified);
    assert!(domain.can_send());
    let mut invalid_key = domain.clone();
    invalid_key.private_key.clear();
    let error = queue::deliver(&state, &email, &invalid_key)
        .await
        .unwrap_err();
    assert_eq!(error.detail, "Could not build or sign the message");
    let signed = queue::build_message(&email, &domain, &[42; 32]).unwrap();
    let raw = String::from_utf8(signed.formatted()).unwrap();
    let parsed = mail_parser::MessageParser::default()
        .parse(raw.as_bytes())
        .unwrap();
    assert_eq!(
        parsed.from().unwrap().first().unwrap().name.as_deref(),
        Some("中文发件人, 团队")
    );
    assert_eq!(
        signed.envelope().from().unwrap().to_string(),
        "hello@example.test"
    );
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
        .from(lettre::message::Mailbox::new(
            Some("SMTP 中文发件人".into()),
            "hello@example.test".parse().unwrap(),
        ))
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
    let smtp_from: String =
        sqlx::query_scalar("SELECT from_address FROM emails WHERE subject='SMTP fixture'")
            .fetch_one(&db)
            .await
            .unwrap();
    let smtp_from: lettre::message::Mailbox = smtp_from.parse().unwrap();
    assert_eq!(smtp_from.name.as_deref(), Some("SMTP 中文发件人"));
    stop.send(true).unwrap();
    smtp_task.await.unwrap();
    verify_templates_preview_and_test_send(&app, &state, &cookie, &bearer).await;
    if std::env::var("BUNMAIL_BROWSER_TEST").as_deref() == Ok("true") {
        sqlx::query("UPDATE domains SET verified=false")
            .execute(&db)
            .await
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut browser_config = (*state.config).clone();
        browser_config.app_url = format!("http://{address}").parse().unwrap();
        browser_config.delivery_enabled = false;
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
    // The browser password flow deliberately revokes every session, including this fixture's.
    let cookie = if std::env::var("BUNMAIL_BROWSER_TEST").as_deref() == Ok("true") {
        password_login(&app, "correct-horse-battery-staple-123").await
    } else {
        cookie
    };
    public_api::verify(&state, &cookie, &bearer).await;
    if std::env::var("BUNMAIL_THIRD_PARTY_TEST").as_deref() == Ok("true") {
        public_api::browser(&state, &bearer).await;
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
    verify_single_admin(&app, &state, &cookie).await;
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
    verify_password_change(&app, &state).await;
    drop(app);
    drop(state);
    db.close().await;
    sqlx::query(&format!("DROP DATABASE {db_name} WITH (FORCE)"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

async fn verify_without_ptr(state: &AppState, cookie: &str, domain: &Domain) {
    use hickory_resolver::{
        config::{NameServerConfigGroup, ResolverConfig, ResolverOpts},
        proto::{
            op::{Message, MessageType, ResponseCode},
            rr::{
                RData, Record, RecordType,
                rdata::{A, TXT},
            },
        },
    };
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let records = bunmail_rs::dns::records(state, domain);
    let server = tokio::spawn(async move {
        let mut buffer = [0; 4096];
        loop {
            let (size, peer) = socket.recv_from(&mut buffer).await.unwrap();
            let query = Message::from_vec(&buffer[..size]).unwrap();
            let mut response = Message::new();
            response
                .set_id(query.id())
                .set_message_type(MessageType::Response)
                .set_recursion_available(true)
                .add_queries(query.queries().to_vec());
            for question in query.queries() {
                let name = question.name().to_utf8();
                for record in &records {
                    if record.name != name.trim_end_matches('.') {
                        continue;
                    }
                    let data = match (question.query_type(), record.kind) {
                        (RecordType::TXT, "TXT") => Some(RData::TXT(TXT::new(
                            record
                                .value
                                .as_bytes()
                                .chunks(200)
                                .map(|part| String::from_utf8(part.to_vec()).unwrap())
                                .collect(),
                        ))),
                        (RecordType::A, "A") => Some(RData::A(A(record.value.parse().unwrap()))),
                        _ => None,
                    };
                    if let Some(data) = data {
                        response.add_answer(Record::from_rdata(question.name().clone(), 60, data));
                    }
                }
            }
            if response.answers().is_empty() {
                response.set_response_code(ResponseCode::NXDomain);
            }
            socket
                .send_to(&response.to_vec().unwrap(), peer)
                .await
                .unwrap();
        }
    });
    let mut fixture = state.clone();
    fixture.dns = TokioAsyncResolver::tokio(
        ResolverConfig::from_parts(
            None,
            vec![],
            NameServerConfigGroup::from_ips_clear(&["127.0.0.1".parse().unwrap()], port, true),
        ),
        ResolverOpts::default(),
    );
    let (status, _, result) = request(
        &web::router(fixture),
        "POST",
        &format!("/api/v1/domains/{}/verify", domain.id),
        json!({}),
        Some(cookie),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["verified"], true);
    assert_eq!(result["checks"]["ptr"], false);
    assert_eq!(result["checks"]["forward"], true);
    server.abort();
    let _ = server.await;
}

async fn verify_update_access(state: &AppState, cookie: &str, bearer: &str) {
    let app = web::router(state.clone());
    let install_body = json!({"version":"0.1.1","request_id":Uuid::new_v4()});
    for (credential, origin, expected) in [
        (None, true, StatusCode::UNAUTHORIZED),
        (Some(bearer), true, StatusCode::FORBIDDEN),
        (Some(cookie), false, StatusCode::FORBIDDEN),
        (Some(cookie), true, StatusCode::CONFLICT),
    ] {
        assert_eq!(
            request(
                &app,
                "POST",
                "/api/v1/updates/install",
                install_body.clone(),
                credential,
                origin
            )
            .await
            .0,
            expected
        );
    }
    for (method, path) in [
        ("GET", "/api/v1/updates"),
        ("POST", "/api/v1/updates/check"),
    ] {
        assert_eq!(
            request(&app, method, path, json!({}), None, true).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            request(&app, method, path, json!({}), Some(bearer), true)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        let (status, _, data) = request(&app, method, path, json!({}), Some(cookie), true).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(data["status"], "disabled");
    }
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/updates/check",
            json!({}),
            Some(cookie),
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let mut fixture = state.clone();
    let mut config = (*state.config).clone();
    config.update_repository = Some("AsukaCC/SMTP".into());
    fixture.config = Arc::new(config);
    fixture.updates = Default::default();
    let app = web::router(fixture.clone());
    let (_, _, data) = request(
        &app,
        "GET",
        "/api/v1/updates",
        json!({}),
        Some(cookie),
        false,
    )
    .await;
    assert_eq!(data["status"], "not_checked");
    for status in ["update_available", "check_failed"] {
        *fixture.updates.lock().await = Some(bunmail_rs::updates::Check {
            status: status.into(),
            checked_at: chrono::Utc::now(),
            latest_version: None,
            release_url: None,
            packages: vec![],
        });
        let (_, _, first) = request(
            &app,
            "GET",
            "/api/v1/updates",
            json!({}),
            Some(cookie),
            false,
        )
        .await;
        let (_, _, cached) = request(
            &app,
            "POST",
            "/api/v1/updates/check",
            json!({}),
            Some(cookie),
            true,
        )
        .await;
        assert_eq!(cached["status"], status);
        assert_eq!(first["check"], cached["check"]);
        assert!(cached["retry_after_seconds"].as_i64().unwrap() > 0);
    }
    let old_check = chrono::Utc::now() - chrono::Duration::hours(7);
    fixture.updates.lock().await.as_mut().unwrap().checked_at = old_check;
    let (_, _, data) = request(
        &app,
        "GET",
        "/api/v1/updates",
        json!({}),
        Some(cookie),
        false,
    )
    .await;
    assert_eq!(data["status"], "check_failed");
    assert_eq!(data["retry_after_seconds"], 0);
    assert_eq!(
        fixture.updates.lock().await.as_ref().unwrap().checked_at,
        old_check
    );
    verify_update_installer_socket(&fixture, cookie).await;
}

async fn verify_update_installer_socket(state: &AppState, cookie: &str) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    // A real Unix HTTP socket exercises the bridge without invoking Docker.
    let directory = tempfile::tempdir().unwrap();
    let socket = directory.path().join("control.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let request_id = Uuid::new_v4();
    let job = json!({"request_id":request_id,"version":"0.1.1","status":"queued","error":null,"updated_at":chrono::Utc::now()});
    let job_response = job.clone();
    let server = tokio::spawn(async move {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 2048];
            let (header_end, length) = loop {
                let count = stream.read(&mut buffer).await.unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&buffer[..count]);
                if let Some(end) = bytes.windows(4).position(|value| value == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                    let length: usize = header
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap_or("0")
                        .parse()
                        .unwrap();
                    break (end + 4, length);
                }
            };
            while bytes.len() < header_end + length {
                let count = stream.read(&mut buffer).await.unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&buffer[..count]);
            }
            let (status, body) = if bytes.starts_with(b"POST /install ") {
                let body: Value =
                    serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap();
                assert_eq!(body["request_id"], request_id.to_string());
                assert_eq!(body["repository"], "AsukaCC/SMTP");
                assert_eq!(body["version"], "0.1.1");
                ("202 Accepted", job_response.clone())
            } else {
                assert!(bytes.starts_with(b"GET /status "));
                (
                    "200 OK",
                    json!({"status":"ready","repository":"AsukaCC/SMTP","platform":"linux/amd64","current_version":env!("CARGO_PKG_VERSION"),"job":null}),
                )
            };
            let body = body.to_string();
            stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        }
    });
    let mut fixture = state.clone();
    let mut config = (*state.config).clone();
    config.update_socket = Some(socket);
    fixture.config = Arc::new(config);
    *fixture.updates.lock().await = Some(bunmail_rs::updates::Check {
        status: "update_available".into(),
        checked_at: chrono::Utc::now(),
        latest_version: Some("0.1.1".into()),
        release_url: None,
        packages: vec![bunmail_rs::updates::Package {
            name: "bunmail-rs-0.1.1-linux-amd64.tar.gz".into(),
            platform: "linux/amd64".into(),
            size: 100,
        }],
    });
    let app = web::router(fixture);
    let (_, _, status) = request(
        &app,
        "GET",
        "/api/v1/updates",
        json!({}),
        Some(cookie),
        false,
    )
    .await;
    assert_eq!(status["can_install"], true);
    let (code, _, body) = request(
        &app,
        "POST",
        "/api/v1/updates/install",
        json!({"version":"0.1.1","request_id":request_id}),
        Some(cookie),
        true,
    )
    .await;
    assert_eq!(code, StatusCode::ACCEPTED);
    assert_eq!(body, job);
    server.await.unwrap();
}

async fn verify_dns_cooldown(state: &AppState, cookie: &str, id: Uuid) {
    use hickory_resolver::{
        config::{NameServerConfigGroup, ResolverConfig, ResolverOpts},
        proto::op::{Message, MessageType, ResponseCode},
    };
    // A local DNS fixture returns NXDOMAIN, making unsuccessful verification deterministic without public DNS.
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let mut buffer = [0; 4096];
        loop {
            let (size, peer) = socket.recv_from(&mut buffer).await.unwrap();
            let query = Message::from_vec(&buffer[..size]).unwrap();
            let mut response = Message::new();
            response
                .set_id(query.id())
                .set_message_type(MessageType::Response)
                .set_recursion_desired(true)
                .set_recursion_available(true)
                .set_response_code(ResponseCode::NXDomain)
                .add_queries(query.queries().to_vec());
            socket
                .send_to(&response.to_vec().unwrap(), peer)
                .await
                .unwrap();
        }
    });
    let mut fixture = state.clone();
    fixture.dns = TokioAsyncResolver::tokio(
        ResolverConfig::from_parts(
            None,
            vec![],
            NameServerConfigGroup::from_ips_clear(&["127.0.0.1".parse().unwrap()], port, true),
        ),
        ResolverOpts::default(),
    );
    let app = web::router(fixture);
    let key = format!("verify:{id}");
    let path = format!("/api/v1/domains/{id}/verify");
    for index in 1..=3 {
        let (status, _, data) = request(&app, "POST", &path, json!({}), Some(cookie), true).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(data["verified"], false);
        let retry = data["retry_after_seconds"].as_u64().unwrap();
        assert_eq!(retry > 0, index == 3);
    }
    let expires = state.limits.lock().unwrap()[&key].1;
    let (status, headers, data) = request(&app, "POST", &path, json!({}), Some(cookie), true).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(data["error"]["code"], "RATE_LIMITED");
    let retry = headers["retry-after"]
        .to_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    assert!((1..=60).contains(&retry));
    assert_eq!(data["error"]["retry_after_seconds"], retry);
    assert_eq!(state.limits.lock().unwrap()[&key].1, expires);
    let (_, _, list) = request(
        &app,
        "GET",
        "/api/v1/domains",
        json!({}),
        Some(cookie),
        false,
    )
    .await;
    assert!(list["data"][0]["retry_after_seconds"].as_u64().unwrap() > 0);
    state.limits.lock().unwrap().get_mut(&key).unwrap().1 =
        std::time::Instant::now() - std::time::Duration::from_secs(1);
    assert_eq!(
        request(&app, "POST", &path, json!({}), Some(cookie), true)
            .await
            .0,
        StatusCode::OK
    );
    state.limits.lock().unwrap().remove(&key);
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        response.headers()["cache-control"],
        "no-store, no-transform"
    );
    assert!(
        response.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("script-src 'self';")
    );
    server.abort();
    let _ = server.await;
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

async fn verify_single_admin_migration(db: &PgPool) {
    // Roll back the upgrade fixture so the same isolated database also tests a fresh install.
    let mut tx = db.begin().await.unwrap();
    for sql in [
        include_str!("../migrations/0001_initial.sql"),
        include_str!("../migrations/0002_delivery_settings.sql"),
        "INSERT INTO owner (email,password_hash) VALUES ('owner@example.test','admin-hash')",
        include_str!("../migrations/0003_accounts.sql"),
        "INSERT INTO accounts (id,email,password_hash,role) VALUES (gen_random_uuid(),'person@example.test','user-hash','user');
         INSERT INTO sessions (token_hash,account_id,expires_at) SELECT role,id,now()+interval '1 hour' FROM accounts;
         INSERT INTO account_tokens (token_hash,purpose,email,account_id,expires_at) SELECT role,'reset',email,id,now()+interval '1 hour' FROM accounts;
         INSERT INTO account_tokens (token_hash,purpose,email,password_hash,expires_at) VALUES ('pending','verify','pending@example.test','pending-hash',now()+interval '1 day');
         INSERT INTO registration_settings (enabled) VALUES (true);
         INSERT INTO templates (id,name,subject) VALUES (gen_random_uuid(),'kept-template','subject');
         INSERT INTO api_keys (id,name,prefix,token_hash) VALUES (gen_random_uuid(),'kept-key','prefix','hash');
         INSERT INTO domains (id,name,public_key,private_key) VALUES (gen_random_uuid(),'example.test','public','encrypted');
         INSERT INTO emails (id,domain_id,from_address,to_address,subject,message_id,request_hash) SELECT gen_random_uuid(),id,'from@example.test','to@example.test','kept-email','message','hash' FROM domains;",
    ] {
        sqlx::raw_sql(sql).execute(&mut *tx).await.unwrap();
    }
    let admin_id: Uuid = sqlx::query_scalar("SELECT id FROM accounts WHERE role='admin'")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/0004_single_admin.sql"))
        .execute(&mut *tx)
        .await
        .unwrap();
    let accounts: Vec<(Uuid, String, String)> =
        sqlx::query_as("SELECT id,email,password_hash FROM accounts")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    assert_eq!(
        accounts,
        vec![(admin_id, "owner@example.test".into(), "admin-hash".into())]
    );
    for table in ["sessions", "account_tokens"] {
        let ids: Vec<Uuid> = sqlx::query_scalar(&format!("SELECT account_id FROM {table}"))
            .fetch_all(&mut *tx)
            .await
            .unwrap();
        assert_eq!(ids, vec![admin_id]);
    }
    sqlx::raw_sql(include_str!(
        "../migrations/0005_remove_password_recovery.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    assert!(
        sqlx::query_scalar::<_, Option<String>>("SELECT to_regclass('account_tokens')::text")
            .fetch_one(&mut *tx)
            .await
            .unwrap()
            .is_none()
    );
    for table in ["owner", "templates", "api_keys", "domains", "emails"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
                .fetch_one(&mut *tx)
                .await
                .unwrap(),
            1
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM emails")
            .fetch_one(&mut *tx)
            .await
            .unwrap(),
        "queued"
    );
    tx.rollback().await.unwrap();
}

async fn verify_single_admin(app: &axum::Router, state: &AppState, cookie: &str) {
    let email_count: i64 = sqlx::query_scalar("SELECT count(*) FROM emails")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let password_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM accounts WHERE role='admin'")
            .fetch_one(&state.db)
            .await
            .unwrap();
    for (method, path) in [
        ("GET", "/api/v1/registration"),
        ("POST", "/api/v1/register"),
        ("POST", "/api/v1/register/verify"),
        ("PUT", "/api/v1/config/registration"),
        ("POST", "/api/v1/password/forgot"),
        ("POST", "/api/v1/password/reset"),
    ] {
        for credential in [None, Some(cookie)] {
            assert_eq!(
                request(app, method, path, json!({"enabled":true,"email":"owner@example.test","token":"a".repeat(64),"password":"unused-new-password"}), credential, true)
                    .await
                    .0,
                StatusCode::NOT_FOUND
            );
        }
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM emails")
            .fetch_one(&state.db)
            .await
            .unwrap(),
        email_count
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT password_hash FROM accounts WHERE role='admin'")
            .fetch_one(&state.db)
            .await
            .unwrap(),
        password_hash
    );
    assert_eq!(
        request(app, "GET", "/api/v1/me", json!({}), Some(cookie), false)
            .await
            .0,
        StatusCode::OK
    );
    assert!(
        request(app, "GET", "/api/v1/config", json!({}), Some(cookie), false)
            .await
            .2
            .get("registration_enabled")
            .is_none()
    );
    for role in ["admin", "user"] {
        assert!(sqlx::query("INSERT INTO accounts (id,email,password_hash,role) VALUES ($1,'extra@example.test','unused',$2)")
            .bind(Uuid::new_v4()).bind(role).execute(&state.db).await.is_err());
    }
    assert_eq!(
        request(
            app,
            "POST",
            "/api/v1/login",
            json!({"email":"person@example.test","password":"registered-user-password"}),
            None,
            true
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}

async fn password_login(app: &axum::Router, password: &str) -> String {
    let (status, headers, _) = request(
        app,
        "POST",
        "/api/v1/login",
        json!({"email":"owner@example.test","password":password}),
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    headers["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

async fn verify_password_change(app: &axum::Router, state: &AppState) {
    state.limits.lock().unwrap().clear();
    let original = "correct-horse-battery-staple-123";
    let next = "a-new-administrator-password-456";
    let first = password_login(app, original).await;
    let second = password_login(app, original).await;
    let input = json!({"current_password":original,"new_password":next});
    let (_, _, key) = request(
        app,
        "POST",
        "/api/v1/api-keys",
        json!({"name":"password permissions"}),
        Some(&first),
        true,
    )
    .await;
    let bearer = format!("Bearer {}", key["token"].as_str().unwrap());
    let before: String =
        sqlx::query_scalar("SELECT password_hash FROM accounts WHERE role='admin'")
            .fetch_one(&state.db)
            .await
            .unwrap();
    for (auth, origin, body, expected) in [
        (None, true, input.clone(), StatusCode::UNAUTHORIZED),
        (
            Some(bearer.as_str()),
            true,
            input.clone(),
            StatusCode::FORBIDDEN,
        ),
        (
            Some(first.as_str()),
            false,
            input.clone(),
            StatusCode::FORBIDDEN,
        ),
        (
            Some(first.as_str()),
            true,
            json!({"current_password":original,"new_password":"short"}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            Some(first.as_str()),
            true,
            json!({"current_password":original,"new_password":"a".repeat(257)}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            Some(first.as_str()),
            true,
            json!({"current_password":original,"new_password":original}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            Some(first.as_str()),
            true,
            json!({"current_password":"wrong-password","new_password":next}),
            StatusCode::FORBIDDEN,
        ),
    ] {
        assert_eq!(
            request(app, "PUT", "/api/v1/admin/password", body, auth, origin)
                .await
                .0,
            expected
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT password_hash FROM accounts WHERE role='admin'")
            .fetch_one(&state.db)
            .await
            .unwrap(),
        before
    );
    let (status, headers, _) = request(
        app,
        "PUT",
        "/api/v1/admin/password",
        input,
        Some(&first),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    for cookie in [&first, &second] {
        assert_eq!(
            request(app, "GET", "/api/v1/me", json!({}), Some(cookie), false)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        request(
            app,
            "POST",
            "/api/v1/login",
            json!({"email":"owner@example.test","password":original}),
            None,
            true
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let current = password_login(app, next).await;
    let (account, owner): (String, String) =
        sqlx::query_as("SELECT a.password_hash,o.password_hash FROM accounts a CROSS JOIN owner o")
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_ne!(account, before);
    assert_ne!(account, next);
    assert_eq!(account, owner);
    assert_eq!(
        request(
            app,
            "GET",
            "/api/v1/auth/check",
            json!({}),
            Some(&bearer),
            false
        )
        .await
        .0,
        StatusCode::OK
    );
    state.limits.lock().unwrap().clear();
    for attempt in 0..11 {
        assert_eq!(
            request(
                app,
                "PUT",
                "/api/v1/admin/password",
                json!({"current_password":"wrong-password","new_password":original}),
                Some(&current),
                true
            )
            .await
            .0,
            if attempt < 10 {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::TOO_MANY_REQUESTS
            }
        );
    }
    state.limits.lock().unwrap().clear();
    let candidate_a = "concurrent-new-password-one";
    let candidate_b = "concurrent-new-password-two";
    let (a, b) = tokio::join!(
        request(
            app,
            "PUT",
            "/api/v1/admin/password",
            json!({"current_password":next,"new_password":candidate_a}),
            Some(&current),
            true
        ),
        request(
            app,
            "PUT",
            "/api/v1/admin/password",
            json!({"current_password":next,"new_password":candidate_b}),
            Some(&current),
            true
        )
    );
    assert_eq!(
        [a.0, b.0]
            .iter()
            .filter(|status| **status == StatusCode::OK)
            .count(),
        1
    );
    assert!([a.0, b.0].iter().all(|status| {
        [
            StatusCode::OK,
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
        ]
        .contains(status)
    }));
    let winner = if a.0 == StatusCode::OK {
        candidate_a
    } else {
        candidate_b
    };
    let _ = password_login(app, winner).await;
}
