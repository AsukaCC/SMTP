use crate::{api, auth, send, state::AppState};
use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, Request},
    http::{HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}
async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../static/app.css"),
    )
}
async fn js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../static/app.js"),
    )
}
async fn lucide() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../static/vendor/lucide.min.js"),
    )
}
async fn logo() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/svg+xml")],
        include_str!("../static/logo.svg"),
    )
}
async fn security(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-transform"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers
        .entry(header::X_FRAME_OPTIONS)
        .or_insert(HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    headers.entry(header::CONTENT_SECURITY_POLICY).or_insert(HeaderValue::from_static("default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; connect-src 'self'; font-src 'self'; frame-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'"));
    response
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/dashboard", get(index))
        .route("/dashboard/login", get(index))
        .route("/assets/app.css", get(css))
        .route("/assets/app.js", get(js))
        .route("/assets/lucide.js", get(lucide))
        .route("/assets/logo.svg", get(logo))
        .route("/health", get(api::health))
        .route("/api/v1/login", post(auth::login))
        .route("/api/v1/logout", post(auth::logout))
        .route("/api/v1/me", get(auth::me))
        .route("/api/v1/config", get(api::config))
        .route(
            "/api/v1/config/delivery",
            axum::routing::put(api::set_delivery),
        )
        .route("/api/v1/updates", get(crate::updates::status))
        .route("/api/v1/updates/check", post(crate::updates::check))
        .route("/api/v1/stats", get(api::stats))
        .route(
            "/api/v1/domains",
            get(api::list_domains).post(api::add_domain),
        )
        .route(
            "/api/v1/domains/{id}",
            axum::routing::delete(api::delete_domain),
        )
        .route("/api/v1/domains/{id}/verify", post(api::verify_domain))
        .route(
            "/api/v1/domains/{id}/cloudflare/preview",
            post(crate::cloudflare::preview),
        )
        .route(
            "/api/v1/domains/{id}/cloudflare/apply",
            post(crate::cloudflare::apply),
        )
        .route("/api/v1/api-keys", get(api::list_keys).post(api::add_key))
        .route(
            "/api/v1/api-keys/{id}",
            axum::routing::delete(api::revoke_key),
        )
        .route(
            "/api/v1/templates",
            get(api::list_templates).post(api::add_template),
        )
        .route(
            "/api/v1/templates/initialize",
            post(crate::initial_templates::initialize),
        )
        .route("/api/v1/templates/preview", post(crate::preview::preview))
        .route(
            "/api/v1/templates/{id}",
            axum::routing::put(api::update_template).delete(api::delete_template),
        )
        .route("/api/v1/emails", get(api::list_emails))
        .route("/api/v1/emails/send", post(send::send))
        .route("/api/v1/emails/test", post(send::test_send))
        .route("/api/v1/emails/{id}", get(api::email_detail))
        .route("/api/v1/emails/{id}/retry", post(api::retry_email))
        .route("/api/v1/emails/{id}/cancel", post(api::cancel_email))
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                axum::Json(
                    serde_json::json!({"error":{"code":"NOT_FOUND","message":"Record not found"}}),
                ),
            )
        })
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .layer(middleware::from_fn(security))
        .with_state(state)
}
