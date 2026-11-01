use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug)]
pub struct AppError(pub StatusCode, pub &'static str, pub String);
pub type Result<T> = std::result::Result<T, AppError>;

impl AppError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_INPUT",
            message.into(),
        )
    }
    pub fn unauthorized() -> Self {
        Self(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "Please sign in or provide a valid API key".into(),
        )
    }
    pub fn forbidden() -> Self {
        Self(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Administrator access required".into(),
        )
    }
    pub fn not_found() -> Self {
        Self(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Record not found".into(),
        )
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self(StatusCode::CONFLICT, "CONFLICT", message.into())
    }
    pub fn internal() -> Self {
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "The operation could not be completed".into(),
        )
    }
}
impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref e) = error {
            if e.is_unique_violation() {
                return Self::conflict("The record already exists");
            }
            if e.is_foreign_key_violation() {
                return Self::conflict("The record is still in use");
            }
        }
        tracing::error!("Database operation failed");
        Self::internal()
    }
}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            self.0,
            Json(json!({"error": {"code": self.1, "message": self.2}})),
        )
            .into_response()
    }
}
