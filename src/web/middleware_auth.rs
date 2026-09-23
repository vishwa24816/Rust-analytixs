//! Session auth: `CurrentUser` extractor reads a Bearer token or the
//! `session` cookie. Login/register set the cookie; logout clears it.
//! No extra cookie crate — one tiny parser.

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderMap, StatusCode},
};

use crate::{error::AppError, state::AppState};

pub const COOKIE_NAME: &str = "session";

pub struct CurrentUser {
    pub user_id: String,
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::to_string)
}

fn cookie(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|pair| {
        let (k, v) = pair.trim().split_once('=')?;
        (k.trim() == COOKIE_NAME).then(|| v.trim().to_string())
    })
}

pub fn token_from(headers: &HeaderMap) -> Option<String> {
    bearer(headers).or_else(|| cookie(headers))
}

#[async_trait]
impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = token_from(&parts.headers)
            .ok_or(AppError::Auth("missing session".into()))?;
        match crate::auth::session::user_id(&state.db, &token).await? {
            Some(user_id) => Ok(Self { user_id }),
            None => Err(AppError::Auth("invalid or expired session".into())),
        }
    }
}

/// `Set-Cookie: session=…; HttpOnly; SameSite=Lax; Path=/` (Secure omitted for local http).
pub fn set_cookie_header(token: &str) -> String {
    format!("{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000")
}

pub fn clear_cookie_header() -> &'static str {
    "session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
}

pub fn unauthorized() -> (StatusCode, &'static str) {
    (StatusCode::UNAUTHORIZED, "unauthorized")
}
