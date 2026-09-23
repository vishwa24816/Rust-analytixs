use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};

use crate::{
    error::AppError,
    ingest::{
        event_dto::RawEvent,
        validate,
        writer::{self, Incoming},
    },
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/event", post(ingest))
        .route("/js/:script", get(tracker_script))
}

fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

fn user_agent(headers: &HeaderMap) -> String {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

/// Tracker posts Content-Type: text/plain, so parse the body manually.
async fn ingest(
    State(s): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let ip = client_ip(&headers);
    if !s.limiter.allow(&ip) {
        return Ok((StatusCode::TOO_MANY_REQUESTS, "rate limited").into_response());
    }
    let ua = user_agent(&headers);
    let dnt = headers.get("dnt").and_then(|v| v.to_str().ok());
    if validate::should_drop(&ua, dnt) {
        return Err(AppError::Dropped("bot or dnt".into()));
    }
    let raw: RawEvent =
        serde_json::from_slice(&body).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let event = raw.normalize().map_err(AppError::BadRequest)?;
    if event.domain.is_empty() {
        return Err(AppError::BadRequest("domain required".into()));
    }
    writer::write(&s.db, Incoming { event, user_agent: ua, ip }).await?;
    Ok((StatusCode::ACCEPTED, "ok").into_response())
}

async fn tracker_script(Path(script): Path<String>) -> Result<impl IntoResponse, AppError> {
    // ponytail: exact filenames only — no path traversal, no dir walk
    if !script.ends_with(".js") || script.contains('/') || script.contains('\\') {
        return Err(AppError::BadRequest("unknown script".into()));
    }
    let dir = std::env::var("TRACKER_DIR").unwrap_or_else(|_| "tracker/js".to_string());
    let path = std::path::Path::new(&dir).join(&script);
    match std::fs::read(&path) {
        Ok(bytes) => Ok((
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            bytes,
        )),
        Err(_) => Err(AppError::BadRequest("unknown script".into())),
    }
}
