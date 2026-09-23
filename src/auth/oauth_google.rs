//! Google OAuth login. Gated on GOOGLE_CLIENT_ID/SECRET; without them
//! the routes return 400 instead of crashing.

use serde::Deserialize;

use crate::{config::Config, error::AppError};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";

#[derive(Debug, Deserialize)]
pub struct UserInfo {
    pub email: String,
    pub email_verified: bool,
}

pub fn configured(config: &Config) -> bool {
    !config.google_client_id.is_empty() && !config.google_client_secret.is_empty()
}

pub fn authorize_url(config: &Config, state: &str) -> Result<String, AppError> {
    if !configured(config) {
        return Err(AppError::BadRequest("google oauth not configured".into()));
    }
    let redirect = format!("{}/oauth/google/callback", config.app_url);
    let url = url::Url::parse_with_params(
        AUTH_URL,
        [
            ("client_id", config.google_client_id.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("response_type", "code"),
            ("scope", "openid email https://www.googleapis.com/auth/webmasters.readonly"),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("state", state),
        ],
    )
    .map_err(|e| AppError::BadRequest(e.to_string()))?;
    Ok(url.to_string())
}

pub async fn exchange(
    config: &Config,
    http: &reqwest::Client,
    code: &str,
) -> Result<(UserInfo, Option<String>), AppError> {
    if !configured(config) {
        return Err(AppError::BadRequest("google oauth not configured".into()));
    }
    let redirect = format!("{}/oauth/google/callback", config.app_url);
    let token: serde_json::Value = http
        .post(TOKEN_URL)
        .form(&[
            ("code", code),
            ("client_id", config.google_client_id.as_str()),
            ("client_secret", config.google_client_secret.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    let access = token["access_token"].as_str().ok_or(AppError::BadRequest(
        "google token exchange failed".into(),
    ))?;
    let refresh = token["refresh_token"].as_str().map(str::to_string);
    let info: UserInfo = http.get(USERINFO_URL)
        .bearer_auth(access)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    Ok((info, refresh))
}
