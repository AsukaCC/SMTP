use crate::{auth::Auth, error::AppError, state::AppState};
use axum::{
    Json,
    extract::{MatchedPath, Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tower_http::cors::{AllowOrigin, CorsLayer};
use uuid::Uuid;

pub fn cors(state: &AppState) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(
            state
                .config
                .api_allowed_origins
                .iter()
                .map(|s| HeaderValue::from_str(s).expect("validated origin")),
        ))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::HeaderName::from_static("idempotency-key"),
        ])
        .expose_headers([
            header::HeaderName::from_static("x-request-id"),
            header::RETRY_AFTER,
        ])
        .max_age(std::time::Duration::from_secs(600))
}

pub async fn browser_auth(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if let Some(origin) = request.headers().get(header::ORIGIN) {
        let origin = origin.to_str().unwrap_or("");
        if origin != state.config.app_url.origin().ascii_serialization() {
            if !state
                .config
                .api_allowed_origins
                .iter()
                .any(|allowed| allowed == origin)
            {
                return AppError(
                    StatusCode::FORBIDDEN,
                    "ORIGIN_NOT_ALLOWED",
                    "This website origin is not allowed".into(),
                )
                .into_response();
            }
            // Cross-origin public API calls must never fall back to administrator cookies.
            if !request.headers().contains_key(header::AUTHORIZATION) {
                return AppError::unauthorized().into_response();
            }
        }
    }
    next.run(request).await
}

pub async fn check_auth(auth: Auth) -> Json<serde_json::Value> {
    Json(json!({"authenticated":true,"scope":if auth.api_key_id.is_some(){"send"}else{"admin"}}))
}

pub async fn openapi() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        include_str!("../../static/openapi.json"),
    )
}

pub async fn contract(request: Request, next: Next) -> Response {
    let is_api = request.uri().path().starts_with("/api/v1/");
    let preview = request.uri().path() == "/api/v1/templates/preview";
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| "unmatched".into());
    let method = request.method().clone();
    let request_id = Uuid::new_v4().to_string();
    let mut response = next.run(request).await;
    if !is_api {
        return response;
    }
    let status = response.status();
    let is_json = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    // Preserve sandboxed HTML preview errors; normalize Axum extractor and method rejections.
    if (status.is_client_error() || status.is_server_error()) && !is_json && !preview {
        let (code, message) = match status {
            StatusCode::BAD_REQUEST => ("BAD_REQUEST", "Invalid JSON, path or query parameters"),
            StatusCode::NOT_FOUND => ("NOT_FOUND", "Record not found"),
            StatusCode::METHOD_NOT_ALLOWED => ("METHOD_NOT_ALLOWED", "HTTP method is not allowed"),
            StatusCode::PAYLOAD_TOO_LARGE => ("PAYLOAD_TOO_LARGE", "Request body exceeds 1 MiB"),
            StatusCode::UNSUPPORTED_MEDIA_TYPE => (
                "UNSUPPORTED_MEDIA_TYPE",
                "Content-Type must be application/json",
            ),
            StatusCode::UNPROCESSABLE_ENTITY => (
                "INVALID_INPUT",
                "Missing, unknown or invalid request fields",
            ),
            _ => ("REQUEST_FAILED", "The request could not be completed"),
        };
        *response.body_mut() =
            axum::body::Body::from(json!({"error":{"code":code,"message":message}}).to_string());
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response.headers_mut().remove(header::CONTENT_LENGTH);
    }
    response
        .headers_mut()
        .insert("x-request-id", request_id.parse().expect("UUID header"));
    if status == StatusCode::UNAUTHORIZED {
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer realm=\"BunMail API\""),
        );
    }
    if status == StatusCode::TOO_MANY_REQUESTS
        && !response.headers().contains_key(header::RETRY_AFTER)
    {
        response.headers_mut().insert(
            header::RETRY_AFTER,
            HeaderValue::from_static(if route == "/api/v1/login" {
                "300"
            } else {
                "60"
            }),
        );
    }
    tracing::info!(%request_id, %method, %route, status=status.as_u16(), "API request completed");
    if method == Method::HEAD {
        *response.body_mut() = axum::body::Body::empty();
    }
    response
}
