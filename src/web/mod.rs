use axum::{routing::get, Router};
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};

use crate::state::AppState;

pub mod auth;
pub mod billing;
pub mod csrf;
pub mod dashboard;
pub mod docs;
pub mod growth;
mod health;
pub mod ingest;
mod metrics;
pub mod middleware_auth;
pub mod plugins;
pub mod sites;
pub mod stats;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/metrics", get(metrics::metrics))
        .merge(auth::routes())
        .merge(sites::routes())
        .merge(stats::routes())
        .merge(dashboard::routes())
        .merge(growth::routes())
        .merge(billing::routes())
        .merge(docs::routes())
        .merge(plugins::routes())
        .merge(ingest::routes())
        .layer(axum::middleware::from_fn_with_state(state.clone(), csrf::csrf))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            axum::http::HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::REFERRER_POLICY,
            axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::CONTENT_SECURITY_POLICY,
            axum::http::HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'",
            ),
        ))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
