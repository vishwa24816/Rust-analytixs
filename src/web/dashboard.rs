use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
    routing::get,
    Router,
};

use crate::{error::AppError, state::AppState, web::middleware_auth::token_from};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/go", get(go))
        .route("/favicon.ico", get(favicon_ico))
        .route("/dashboard/:domain", get(dashboard))
        .route("/public/*file", get(public_file))
}

async fn index() -> impl IntoResponse {
    Html(include_str!("../../public/index.html"))
}

#[derive(serde::Deserialize)]
struct GoQuery {
    domain: String,
}

async fn go(Query(q): Query<GoQuery>) -> impl IntoResponse {
    Redirect::to(&format!("/dashboard/{}", q.domain.trim()))
}

async fn favicon_ico() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/svg+xml")],
        include_str!("../../public/favicon-placeholder.svg"),
    )
}

async fn dashboard(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(domain): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    // member, valid API key, or public site
    let row: Option<(String, i64)> =
        sqlx::query_as("SELECT id, public FROM sites WHERE domain = ? OR id = ?")
            .bind(&domain)
            .bind(&domain)
            .fetch_optional(&s.db)
            .await?;
    let (site_id, public) = match row {
        Some(r) => r,
        None => return Err(AppError::BadRequest("site not found".into())),
    };
    if let Some(reason) = crate::billing::gate::lock_reason(&s.db, &site_id).await? {
        return Err(AppError::Payment(format!("site locked: {reason}")));
    }
    let allowed = if public == 1 {
        true
    } else if let Some(token) = token_from(&headers) {
        let uid = if token.starts_with("plausible_") {
            crate::auth::api_key::verify(&s.db, &token).await?.map(|k| k.user_id)
        } else {
            crate::auth::session::user_id(&s.db, &token).await?
        };
        match uid {
            Some(u) => {
                crate::sites::membership::role_of(&s.db, &site_id, &u).await?.is_some()
            }
            None => false,
        }
    } else {
        false
    };
    if !allowed {
        let wants_html = headers
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .map(|a| a.contains("text/html"))
            .unwrap_or(false);
        if wants_html {
            return Ok(Redirect::to("/login").into_response());
        }
        return Err(AppError::Auth("login required".into()));
    }
    let html = include_str!("../../public/dashboard.html").replace("{{DOMAIN}}", &domain);
    Ok(Html(html).into_response())
}

async fn is_public(s: &AppState, domain: &str) -> Result<bool, AppError> {
    let p: Option<i64> = sqlx::query_scalar("SELECT public FROM sites WHERE domain = ? OR id = ?")
        .bind(domain)
        .bind(domain)
        .fetch_optional(&s.db)
        .await?;
    Ok(p.unwrap_or(0) == 1)
}

fn mime(path: &str) -> &'static str {
    if path.ends_with(".js") {
        "text/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}

async fn public_file(Path(file): Path<String>) -> Result<impl IntoResponse, AppError> {
    if file.contains("..") || file.starts_with('/') {
        return Err(AppError::BadRequest("bad path".into()));
    }
    let dir = std::env::var("PUBLIC_DIR").unwrap_or_else(|_| "public".to_string());
    let full = std::path::Path::new(&dir).join(&file);
    match std::fs::read(&full) {
        Ok(bytes) => Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, mime(&file)),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            bytes,
        )
            .into_response()),
        Err(_) => Err(AppError::BadRequest("not found".into())),
    }
}
