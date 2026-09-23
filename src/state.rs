use crate::{config::Config, db::DbPool, ingest::rate_limit::Limiter, mail::Mailer};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: DbPool,
    pub mailer: Mailer,
    pub http: reqwest::Client,
    pub limiter: std::sync::Arc<Limiter>,
    /// strict limiter for auth endpoints (login/register/password)
    pub auth_limiter: std::sync::Arc<Limiter>,
}

impl AppState {
    pub fn new(config: Config, db: DbPool) -> Self {
        Self {
            mailer: Mailer::new(&config),
            http: reqwest::Client::new(),
            limiter: std::sync::Arc::new(Limiter::new(300)),
            auth_limiter: std::sync::Arc::new(Limiter::new(20)),
            config,
            db,
        }
    }
}
