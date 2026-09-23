use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_addr")]
    pub listen_addr: SocketAddr,
    #[serde(default = "default_db")]
    pub database_url: String,
    #[serde(default)]
    pub rust_log: String,
    #[serde(default = "default_app_url")]
    pub app_url: String,
    #[serde(default)]
    pub smtp_url: String,
    #[serde(default = "default_from")]
    pub mail_from: String,
    #[serde(default)]
    pub google_client_id: String,
    #[serde(default)]
    pub google_client_secret: String,
    /// Feature flags (env). When true, /register returns 403.
    #[serde(default)]
    pub disable_registration: bool,
}

fn default_app_url() -> String {
    "http://127.0.0.1:8000".to_string()
}
fn default_from() -> String {
    "Rust Analytix <hello@rust-analytix.local>".to_string()
}

fn default_addr() -> SocketAddr {
    "127.0.0.1:8000".parse().unwrap()
}
fn default_db() -> String {
    "sqlite:./data/app.db?mode=rwc".to_string()
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        config::load()
    }
}

// ponytail: tiny env loader, no figment dependency
mod config {
    use super::Config;
    pub fn load() -> Config {
        Config {
            listen_addr: std::env::var("LISTEN_ADDR")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(super::default_addr),
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| super::default_db()),
            rust_log: std::env::var("RUST_LOG").unwrap_or_default(),
            app_url: std::env::var("APP_URL").unwrap_or_else(|_| super::default_app_url()),
            smtp_url: std::env::var("SMTP_URL").unwrap_or_default(),
            mail_from: std::env::var("MAIL_FROM").unwrap_or_else(|_| super::default_from()),
            google_client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
            google_client_secret: std::env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
            disable_registration: std::env::var("DISABLE_REGISTRATION").map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false),
        }
    }
}
