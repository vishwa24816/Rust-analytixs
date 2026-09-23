use axum::{response::IntoResponse, routing::get, Router};
use utoipa_swagger_ui::SwaggerUi;

use crate::state::AppState;

static SPEC_JSON: &str = include_str!("../../public/openapi.json");

pub fn routes() -> Router<AppState> {
    let api: utoipa::openapi::OpenApi =
        serde_json::from_str(SPEC_JSON).expect("openapi.json must parse");
    // ponytail: SwaggerUi serves the spec at /api-docs/openapi.json itself
    Router::new().merge(SwaggerUi::new("/api-docs").url("/api-docs/openapi.json", api))
}
