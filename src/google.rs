//! Google APIs: Search Console keywords + refresh-token storage.
//! Tokens land here from the OAuth callback (offline scope); without them
//! the endpoint tells the caller exactly what's missing.

use crate::{error::AppError, state::AppState, stats::period::Range};

const SC_URL: &str = "https://www.googleapis.com/webmasters/v3/sites";

pub async fn store_refresh_token(s: &AppState, user_id: &str, refresh: &str) {
    let _ = sqlx::query(
        "INSERT INTO google_tokens (user_id, refresh_token) VALUES (?, ?) ON CONFLICT (user_id) DO UPDATE SET refresh_token = excluded.refresh_token",
    )
    .bind(user_id)
    .bind(refresh)
    .execute(&s.db)
    .await;
}

/// Top search queries for sc-domain:DOMAIN over the range.
pub async fn search_console(
    s: &AppState,
    user_id: &str,
    domain: &str,
    range: &Range,
) -> Result<serde_json::Value, AppError> {
    if !crate::auth::oauth_google::configured(&s.config) {
        return Err(AppError::BadRequest("google oauth not configured (GOOGLE_CLIENT_ID/SECRET)".into()));
    }
    let refresh: Option<String> =
        sqlx::query_scalar("SELECT refresh_token FROM google_tokens WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(&s.db)
            .await?;
    let refresh = refresh.ok_or(AppError::BadRequest(
        "google not connected: visit /oauth/google to connect".into(),
    ))?;
    // refresh → access token
    let tok: serde_json::Value = s.http
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", s.config.google_client_id.as_str()),
            ("client_secret", s.config.google_client_secret.as_str()),
            ("refresh_token", refresh.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("google token refresh failed: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("google token refresh failed: {e}")))?;
    let access = tok["access_token"].as_str().ok_or(AppError::BadRequest(
        "google token refresh failed: reconnect at /oauth/google".into(),
    ))?;
    let site_url = format!("sc-domain:{domain}");
    let body = serde_json::json!({
        "startDate": range.start.format("%Y-%m-%d").to_string(),
        "endDate": range.end.format("%Y-%m-%d").to_string(),
        "dimensions": ["query"],
        "rowLimit": 50,
    });
    let res: serde_json::Value = s.http
        .post(format!("{SC_URL}/{}/searchAnalytics/query", urlencoding(&site_url)))
        .bearer_auth(access)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("search console query failed: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("search console query failed: {e}")))?;
    Ok(res)
}

fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
