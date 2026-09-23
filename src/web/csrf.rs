use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::state::AppState;

/// CSRF: cookie-authed state-changing requests must carry a matching
/// Origin or Referer (same host as APP_URL). Bearer/API-key callers and
/// safe methods pass through untouched.
pub async fn csrf(State(s): State<AppState>, req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    if method == axum::http::Method::GET
        || method == axum::http::Method::HEAD
        || method == axum::http::Method::OPTIONS
    {
        return next.run(req).await;
    }
    let headers = req.headers();
    if headers.contains_key(header::AUTHORIZATION) {
        return next.run(req).await;
    }
    let has_cookie = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|c| c.contains("session="))
        .unwrap_or(false);
    if !has_cookie {
        return next.run(req).await;
    }
    let app_host = host_of(&s.config.app_url);
    let origin_ok = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(|o| host_of(o) == app_host)
        .unwrap_or(false);
    let referer_ok = headers
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        .map(|r| r.starts_with(s.config.app_url.as_str()))
        .unwrap_or(false);
    if origin_ok || referer_ok {
        next.run(req).await
    } else {
        (StatusCode::FORBIDDEN, "csrf check failed").into_response()
    }
}

fn host_of(url: &str) -> String {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default()
}
