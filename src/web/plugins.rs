use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{delete, get, put},
    Form, Json, Router,
};
use serde::Deserialize;

use crate::{
    error::AppError,
    sites::{
        favicon,
        funnels::Funnel,
        goals::{Goal, GoalInput},
        membership::{self, Role},
        plugins::{self, SharedLinkInput},
    },
    state::AppState,
    web::middleware_auth::{token_from, CurrentUser},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/plugins/v1/capabilities", get(capabilities))
        .route("/api/plugins/v1/shared_links", get(pl_list).put(pl_create))
        .route("/api/plugins/v1/shared_links/:id", get(pl_get).delete(pl_delete))
        .route("/api/plugins/v1/goals", get(pl_goals).put(pl_goals_create))
        .route("/api/plugins/v1/goals/:id", get(pl_goal_get).delete(pl_goal_delete))
        .route("/api/plugins/v1/goals", delete(pl_goals_bulk))
        .route("/api/plugins/v1/funnels", get(pl_funnels))
        .route("/api/plugins/v1/funnels/:id", get(pl_funnel_get))
        .route("/api/plugins/v1/custom_props", put(pl_props_enable).delete(pl_props_disable))
        .route("/api/plugins/v1/tracker_script_configuration", get(pl_tc_get).put(pl_tc_update))
        .route("/api/sites/:id/plugin-tokens", get(list_tokens).put(create_token))
        .route("/api/sites/:id/plugin-tokens/:tid", delete(delete_token))
        .route("/api/sites/:id/icon", get(site_icon))
        .route("/share/:slug", get(shared_dashboard))
        .route("/api/share/:slug/stats/:endpoint", get(shared_stats))
}

async fn capabilities() -> impl IntoResponse {
    Json(serde_json::json!({
        "custom_properties": ["enable", "disable"],
        "funnels": ["list", "get"],
        "goals": ["list", "get", "create", "delete", "delete_bulk"],
        "shared_links": ["list", "get", "create", "delete"],
        "tracker_script_configuration": ["get", "update"],
    }))
}

/// Site-scoped token (plausible_site_*) → site_id. Mirrors AuthorizePluginsAPI.
async fn authorized_site(s: &AppState, headers: &HeaderMap) -> Result<String, AppError> {
    let token = token_from(headers).ok_or(AppError::Auth("missing token".into()))?;
    // user API keys also work when scoped to a site via ?site_id=
    if token.starts_with("plausible_site_") {
        match plugins::verify_token(&s.db, &token).await? {
            Some(t) => Ok(t.site_id),
            None => Err(AppError::Auth("invalid site token".into())),
        }
    } else {
        Err(AppError::Auth("site token required".into()))
    }
}

async fn resolve(s: &AppState, user: &CurrentUser, id: &str, need: Role) -> Result<String, AppError> {
    let sid: Option<String> = sqlx::query_scalar("SELECT id FROM sites WHERE id = ? OR domain = ?")
        .bind(id)
        .bind(id.to_lowercase())
        .fetch_optional(&s.db)
        .await?;
    match sid {
        Some(sid) => {
            membership::require(&s.db, &sid, &user.user_id, need).await?;
            Ok(sid)
        }
        None => Err(AppError::BadRequest("site not found".into())),
    }
}

// ---- shared links (plugins API) ----
async fn pl_list(State(s): State<AppState>, headers: HeaderMap) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    Ok(Json(plugins::SharedLink::list(&s.db, &sid).await?))
}

async fn pl_get(State(s): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    let link: plugins::SharedLink = sqlx::query_as(
        "SELECT id, site_id, name, slug, password_hash, created_at FROM shared_links WHERE id = ? AND site_id = ?",
    )
    .bind(&id)
    .bind(&sid)
    .fetch_one(&s.db)
    .await
    .map_err(|_| AppError::BadRequest("shared link not found".into()))?;
    Ok(Json(link))
}

async fn pl_create(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    // accept {"name":..,"password":..} or {"shared_link":{...}} (upstream shape)
    let v: serde_json::Value = serde_json::from_slice(&body).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let inner = v.get("shared_link").unwrap_or(&v);
    let input: SharedLinkInput = serde_json::from_value(inner.clone()).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let link = plugins::SharedLink::get_or_create(&s.db, &sid, &input).await?;
    Ok((axum::http::StatusCode::CREATED, Json(link)))
}

async fn pl_delete(State(s): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    sqlx::query("DELETE FROM shared_links WHERE id = ? AND site_id = ?").bind(&id).bind(&sid).execute(&s.db).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

// ---- goals (plugins API) ----
async fn pl_goals(State(s): State<AppState>, headers: HeaderMap) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    Ok(Json(Goal::list(&s.db, &sid).await?))
}

async fn pl_goal_get(State(s): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    let g: Goal = sqlx::query_as(
        "SELECT id, site_id, kind, event_name, page_path, currency, created_at FROM goals WHERE id = ? AND site_id = ?",
    )
    .bind(&id)
    .bind(&sid)
    .fetch_one(&s.db)
    .await
    .map_err(|_| AppError::BadRequest("goal not found".into()))?;
    Ok(Json(g))
}

async fn pl_goals_create(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    // upstream: {"goal": {"goal_type": ..., ...}} or flat GoalInput
    let v: serde_json::Value = serde_json::from_slice(&body).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let input = goal_input_from(&v)?;
    let g = Goal::create(&s.db, &sid, &input).await?;
    Ok((axum::http::StatusCode::CREATED, Json(g)))
}

fn goal_input_from(v: &serde_json::Value) -> Result<GoalInput, AppError> {
    if let Ok(flat) = serde_json::from_value::<GoalInput>(v.clone()) {
        if !flat.event_name.is_empty() {
            return Ok(flat);
        }
    }
    let g = v.get("goal").unwrap_or(v);
    let t = g.get("goal_type").and_then(|x| x.as_str()).unwrap_or("custom");
    match t {
        "pageview" => Ok(GoalInput {
            kind: "page".into(),
            event_name: "pageview".into(),
            page_path: g.get("path").and_then(|x| x.as_str()).unwrap_or("").into(),
            currency: String::new(),
        }),
        "revenue" => Ok(GoalInput {
            kind: "revenue".into(),
            event_name: g.get("event_name").and_then(|x| x.as_str()).unwrap_or("").into(),
            page_path: String::new(),
            currency: g.get("currency").and_then(|x| x.as_str()).unwrap_or("USD").into(),
        }),
        _ => Ok(GoalInput {
            kind: "custom".into(),
            event_name: g.get("event_name").and_then(|x| x.as_str()).unwrap_or("").into(),
            page_path: String::new(),
            currency: String::new(),
        }),
    }
}

async fn pl_goal_delete(State(s): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    sqlx::query("DELETE FROM goals WHERE id = ? AND site_id = ?").bind(&id).bind(&sid).execute(&s.db).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Deserialize)]
struct BulkDelete {
    #[serde(default)]
    goal_ids: Vec<String>,
}

async fn pl_goals_bulk(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    // accept {"goal_ids": [...]} or bare array
    let v: serde_json::Value = serde_json::from_slice(&body).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let ids: Vec<String> = v.get("goal_ids").and_then(|x| serde_json::from_value(x.clone()).ok()).unwrap_or_else(|| {
        serde_json::from_value(v.clone()).unwrap_or_default()
    });
    let _ = BulkDelete { goal_ids: ids.clone() };
    for gid in ids {
        sqlx::query("DELETE FROM goals WHERE id = ? AND site_id = ?").bind(&gid).bind(&sid).execute(&s.db).await?;
    }
    Ok(Json(serde_json::json!({"ok": true})))
}

// ---- funnels (plugins API, read) ----
async fn pl_funnels(State(s): State<AppState>, headers: HeaderMap) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    Ok(Json(Funnel::list(&s.db, &sid).await?))
}

async fn pl_funnel_get(State(s): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    match Funnel::get(&s.db, &sid, &id).await? {
        Some(f) => Ok(Json(f).into_response()),
        None => Err(AppError::BadRequest("funnel not found".into())),
    }
}

// ---- custom props allowlist ----
#[derive(Deserialize)]
struct PropForm {
    prop: String,
}

async fn pl_props_enable(State(s): State<AppState>, headers: HeaderMap, Form(f): Form<PropForm>) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    sqlx::query("INSERT INTO site_props (site_id, prop, enabled) VALUES (?, ?, 1) ON CONFLICT (site_id, prop) DO UPDATE SET enabled = 1")
        .bind(&sid).bind(f.prop.trim()).execute(&s.db).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn pl_props_disable(State(s): State<AppState>, headers: HeaderMap, Form(f): Form<PropForm>) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    sqlx::query("INSERT INTO site_props (site_id, prop, enabled) VALUES (?, ?, 0) ON CONFLICT (site_id, prop) DO UPDATE SET enabled = 0")
        .bind(&sid).bind(f.prop.trim()).execute(&s.db).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

// ---- tracker script configuration ----
async fn pl_tc_get(State(s): State<AppState>, headers: HeaderMap) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    let cfg: Option<String> = sqlx::query_scalar("SELECT config FROM tracker_config WHERE site_id = ?")
        .bind(&sid).fetch_optional(&s.db).await?;
    Ok(Json(serde_json::from_str::<serde_json::Value>(&cfg.unwrap_or_else(|| "{}".into())).unwrap_or_default()))
}

async fn pl_tc_update(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> Result<impl IntoResponse, AppError> {
    let sid = authorized_site(&s, &headers).await?;
    let v: serde_json::Value = serde_json::from_slice(&body).map_err(|e| AppError::BadRequest(e.to_string()))?;
    sqlx::query("INSERT INTO tracker_config (site_id, config) VALUES (?, ?) ON CONFLICT (site_id) DO UPDATE SET config = excluded.config, updated_at = datetime('now')")
        .bind(&sid).bind(v.to_string()).execute(&s.db).await?;
    Ok(Json(v))
}

// ---- site-admin token management ----
async fn list_tokens(State(s): State<AppState>, user: CurrentUser, Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Admin).await?;
    let rows: Vec<plugins::PluginToken> = sqlx::query_as(
        "SELECT id, site_id, name, key_hash, created_at FROM plugin_tokens WHERE site_id = ? ORDER BY created_at",
    )
    .bind(&sid).fetch_all(&s.db).await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct TokenForm {
    name: String,
}

async fn create_token(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Form(f): Form<TokenForm>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Admin).await?;
    let created = plugins::create_token(&s.db, &sid, &f.name).await?;
    crate::audit::record(&s.db, Some(&user.user_id), "plugin.token.create", "plugin_token", &created.token.id).await;
    Ok((axum::http::StatusCode::CREATED, Json(created)))
}

async fn delete_token(
    State(s): State<AppState>,
    user: CurrentUser,
    Path((id, tid)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Admin).await?;
    sqlx::query("DELETE FROM plugin_tokens WHERE id = ? AND site_id = ?").bind(&tid).bind(&sid).execute(&s.db).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

// ---- favicon ----
async fn site_icon(State(s): State<AppState>, user: CurrentUser, Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Viewer).await?;
    let (ct, bytes) = favicon::get(&s.db, &s.http, &sid).await?;
    Ok(([(axum::http::header::CONTENT_TYPE, ct)], bytes).into_response())
}

// ---- public shared dashboard ----
#[derive(Deserialize, Default)]
struct ShareQ {
    #[serde(default)]
    password: Option<String>,
}

async fn shared_dashboard(
    State(s): State<AppState>,
    Path(slug): Path<String>,
    Query(q): Query<ShareQ>,
) -> Result<impl IntoResponse, AppError> {
    let link: plugins::SharedLink = sqlx::query_as(
        "SELECT id, site_id, name, slug, password_hash, created_at FROM shared_links WHERE slug = ?",
    )
    .bind(&slug)
    .fetch_one(&s.db)
    .await
    .map_err(|_| AppError::Auth("unknown share link".into()))?;
    if !link.check_password(q.password.as_deref()) {
        return Err(AppError::Auth("password required".into()));
    }
    let domain: String = sqlx::query_scalar("SELECT domain FROM sites WHERE id = ?")
        .bind(&link.site_id)
        .fetch_one(&s.db)
        .await?;
    // ponytail: reuse the dashboard shell; slug auth is enforced on stats via ?share=slug
    let html = include_str!("../../public/dashboard.html")
        .replace("{{DOMAIN}}", &domain)
        .replace("/api/v1/stats/", &format!("/api/share/{slug}/stats/"));
    Ok(axum::response::Html(html))
}

/// Stats through a shared link (password via ?password=). Same shapes as v1.
async fn shared_stats(
    State(s): State<AppState>,
    Path((slug, endpoint)): Path<(String, String)>,
    axum::extract::Query(mut q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let link: plugins::SharedLink = sqlx::query_as(
        "SELECT id, site_id, name, slug, password_hash, created_at FROM shared_links WHERE slug = ?",
    )
    .bind(&slug)
    .fetch_one(&s.db)
    .await
    .map_err(|_| AppError::Auth("unknown share link".into()))?;
    if !link.check_password(q.remove("password").as_deref()) {
        return Err(AppError::Auth("password required".into()));
    }
    if let Some(reason) = crate::billing::gate::lock_reason(&s.db, &link.site_id).await? {
        return Err(AppError::Payment(format!("site locked: {reason}")));
    }
    let v1q = crate::web::stats::V1Query {
        site_id: String::new(),
        period: q.remove("period").unwrap_or_else(|| "30d".into()),
        date: q.remove("date"),
        from: q.remove("from"),
        to: q.remove("to"),
        filters: q.remove("filters").unwrap_or_default(),
        metrics: q.remove("metrics").unwrap_or_default(),
        interval: q.remove("interval"),
        property: q.remove("property"),
        limit: q.remove("limit").unwrap_or_else(|| "100".into()),
        page: q.remove("page").unwrap_or_else(|| "1".into()),
        compare: q.remove("compare"),
        search: q.remove("search").unwrap_or_default(),
    };
    let (p, _) = crate::web::stats::into_params(v1q, &["visitors"]).map_err(AppError::BadRequest)?;
    let query = crate::web::stats::query_for_site(link.site_id.clone(), p, &["visitors"])?;
    let db = &s.db;
    match endpoint.as_str() {
        "aggregate" => Ok(Json(crate::stats::runner::aggregate(db, &query).await?).into_response()),
        "breakdown" => {
            if query.dimension.is_none() {
                return Err(AppError::BadRequest("property is required".into()));
            }
            Ok(Json(crate::stats::runner::breakdown(db, &query).await?).into_response())
        }
        "timeseries" => Ok(Json(crate::stats::runner::timeseries(db, &query).await?).into_response()),
        "realtime/visitors" => Ok(Json(crate::stats::runner::realtime(db, &link.site_id).await?).into_response()),
        _ => Err(AppError::BadRequest("unknown stats endpoint".into())),
    }
}
