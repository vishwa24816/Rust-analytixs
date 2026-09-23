use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("unauthorized: {0}")]
    Auth(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    /// Silently dropped ingest (bot, unknown domain, shield): HTTP 202.
    #[error("dropped: {0}")]
    Dropped(String),
    #[error("payment required: {0}")]
    Payment(String),
    #[error("too many requests: {0}")]
    Locked(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (code, msg) = match &self {
            AppError::Db(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            AppError::Auth(m) => (StatusCode::UNAUTHORIZED, m.clone()),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, m.clone()),
            AppError::Dropped(m) => (StatusCode::ACCEPTED, m.clone()),
            AppError::Payment(m) => (StatusCode::PAYMENT_REQUIRED, m.clone()),
            AppError::Locked(m) => (StatusCode::TOO_MANY_REQUESTS, m.clone()),
        };
        (code, Json(json!({"error": msg}))).into_response()
    }
}
