use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{delete, get, post},
    Form, Json, Router,
};
use serde::Deserialize;

use crate::{
    error::AppError,
    sites::{
        annotations::{Annotation, AnnotationInput},
        funnels::{Funnel, FunnelForm},
        goals::{Goal, GoalInput},
        membership::{self, Role},
        segments::{Segment, SegmentInput},
    },
    state::AppState,
    stats::{period, runner},
    web::middleware_auth::CurrentUser,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/sites/:id/goals", get(list_goals).post(create_goal))
        .route("/api/sites/:id/goals/:gid", delete(delete_goal))
        .route("/api/sites/:id/funnels", get(list_funnels).post(create_funnel))
        .route("/api/sites/:id/funnels/:fid", get(get_funnel).delete(delete_funnel))
        .route("/api/sites/:id/funnels/:fid/stats", get(funnel_stats))
        .route("/api/sites/:id/journey", get(journey))
        .route("/api/sites/:id/segments", get(list_segments).post(create_segment))
        .route("/api/sites/:id/segments/:sid", delete(delete_segment))
        .route("/api/sites/:id/annotations", get(list_annotations).post(create_annotation))
        .route("/api/sites/:id/annotations/:aid", delete(delete_annotation))
        .route("/api/sites/:id/imports", get(list_imports).post(create_import))
        .route("/api/sites/:id/search-console/keywords", get(sc_keywords))
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

// ---- goals ----
async fn list_goals(State(s): State<AppState>, user: CurrentUser, Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Viewer).await?;
    Ok(Json(Goal::list(&s.db, &sid).await?))
}

async fn create_goal(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Form(input): Form<GoalInput>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Admin).await?;
    let g = Goal::create(&s.db, &sid, &input).await?;
    Ok((axum::http::StatusCode::CREATED, Json(g)))
}

async fn delete_goal(
    State(s): State<AppState>,
    user: CurrentUser,
    Path((id, gid)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Admin).await?;
    sqlx::query("DELETE FROM goals WHERE id = ? AND site_id = ?").bind(&gid).bind(&sid).execute(&s.db).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

// ---- funnels ----
async fn list_funnels(State(s): State<AppState>, user: CurrentUser, Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Viewer).await?;
    Ok(Json(Funnel::list(&s.db, &sid).await?))
}

async fn create_funnel(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Form(input): Form<FunnelForm>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Admin).await?;
    let f = Funnel::create(&s.db, &sid, &input.into_input()).await?;
    Ok((axum::http::StatusCode::CREATED, Json(f)))
}

async fn get_funnel(
    State(s): State<AppState>,
    user: CurrentUser,
    Path((id, fid)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Viewer).await?;
    match Funnel::get(&s.db, &sid, &fid).await? {
        Some(f) => Ok(Json(f).into_response()),
        None => Err(AppError::BadRequest("funnel not found".into())),
    }
}

async fn delete_funnel(
    State(s): State<AppState>,
    user: CurrentUser,
    Path((id, fid)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Admin).await?;
    sqlx::query("DELETE FROM funnels WHERE id = ? AND site_id = ?").bind(&fid).bind(&sid).execute(&s.db).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Deserialize, Default)]
struct RangeQ {
    #[serde(default = "d30")]
    period: String,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
}
fn d30() -> String {
    "30d".to_string()
}

async fn funnel_stats(
    State(s): State<AppState>,
    user: CurrentUser,
    Path((id, fid)): Path<(String, String)>,
    Query(q): Query<RangeQ>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Viewer).await?;
    let f = Funnel::get(&s.db, &sid, &fid).await?.ok_or(AppError::BadRequest("funnel not found".into()))?;
    let range = period::resolve(&q.period, q.date.as_deref(), q.from.as_deref(), q.to.as_deref()).map_err(AppError::BadRequest)?;
    Ok(Json(runner::funnel(&s.db, &sid, &range, &f.steps).await?))
}

// ---- journey ----
#[derive(Deserialize)]
struct JourneyQ {
    #[serde(default = "d30")]
    period: String,
    entry: String,
    #[serde(default = "lim20")]
    limit: String,
}
fn lim20() -> String {
    "20".to_string()
}

async fn journey(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Query(q): Query<JourneyQ>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Viewer).await?;
    let range = period::resolve(&q.period, None, None, None).map_err(AppError::BadRequest)?;
    let limit = q.limit.parse::<i64>().map_err(|_| AppError::BadRequest("bad limit".into()))?.clamp(1, 100);
    Ok(Json(runner::journey(&s.db, &sid, &range, &q.entry, limit).await?))
}

// ---- segments ----
async fn list_segments(State(s): State<AppState>, user: CurrentUser, Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Viewer).await?;
    Ok(Json(Segment::list(&s.db, &sid).await?))
}

async fn create_segment(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Form(input): Form<SegmentInput>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Admin).await?;
    let g = Segment::create(&s.db, &sid, &user.user_id, &input).await?;
    Ok((axum::http::StatusCode::CREATED, Json(g)))
}

async fn delete_segment(
    State(s): State<AppState>,
    user: CurrentUser,
    Path((id, gid)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Admin).await?;
    sqlx::query("DELETE FROM segments WHERE id = ? AND site_id = ?").bind(&gid).bind(&sid).execute(&s.db).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

// ---- annotations ----
async fn list_annotations(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Query(q): Query<RangeQ>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Viewer).await?;
    let range = period::resolve(&q.period, q.date.as_deref(), q.from.as_deref(), q.to.as_deref()).map_err(AppError::BadRequest)?;
    let list = Annotation::list(
        &s.db,
        &sid,
        &range.start.format("%Y-%m-%d").to_string(),
        &range.end.format("%Y-%m-%d").to_string(),
    )
    .await?;
    Ok(Json(list))
}

async fn create_annotation(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Form(input): Form<AnnotationInput>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Admin).await?;
    let a = Annotation::create(&s.db, &sid, &input).await?;
    Ok((axum::http::StatusCode::CREATED, Json(a)))
}

async fn delete_annotation(
    State(s): State<AppState>,
    user: CurrentUser,
    Path((id, aid)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Admin).await?;
    sqlx::query("DELETE FROM annotations WHERE id = ? AND site_id = ?").bind(&aid).bind(&sid).execute(&s.db).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

// ---- imports ----
async fn list_imports(State(s): State<AppState>, user: CurrentUser, Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Viewer).await?;
    let rows: Vec<serde_json::Value> = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, source, status, label, error FROM imports WHERE site_id = ? ORDER BY created_at DESC",
    )
    .bind(&sid)
    .fetch_all(&s.db)
    .await?
    .into_iter()
    .map(|(i, so, st, l, e)| serde_json::json!({"id": i, "source": so, "status": st, "label": l, "error": e}))
    .collect();
    Ok(Json(rows))
}

/// CSV import. Body = raw CSV text with header:
/// timestamp,name,pathname,hostname[,referrer,browser,os,device,country,props]
/// GA4/UA/Search Console go through the same record pipeline once Google
/// tokens exist; without them the record explains what's missing.
async fn create_import(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Admin).await?;
    let ct = headers.get(axum::http::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("");
    let source = if ct.contains("csv") || ct.contains("text/") { "csv" } else { "csv" };
    let iid = ulid::Ulid::new().to_string();
    sqlx::query("INSERT INTO imports (id, site_id, source, status) VALUES (?, ?, ?, 'running')")
        .bind(&iid)
        .bind(&sid)
        .bind(source)
        .execute(&s.db)
        .await?;
    let db = s.db.clone();
    let sidc = sid.clone();
    let iidc = iid.clone();
    let text = String::from_utf8(body.to_vec()).map_err(|_| AppError::BadRequest("csv must be utf-8".into()))?;
    tokio::spawn(async move {
        match crate::imports::csv::run(&db, &sidc, &text).await {
            Ok(n) => {
                let _ = sqlx::query("UPDATE imports SET status = 'done', label = ? WHERE id = ?")
                    .bind(format!("{n} events"))
                    .bind(&iidc)
                    .execute(&db)
                    .await;
            }
            Err(e) => {
                let _ = sqlx::query("UPDATE imports SET status = 'failed', error = ? WHERE id = ?")
                    .bind(e.to_string())
                    .bind(&iidc)
                    .execute(&db)
                    .await;
            }
        }
    });
    Ok((axum::http::StatusCode::ACCEPTED, Json(serde_json::json!({"id": iid, "source": source, "status": "running"}))))
}

// ---- search console ----
async fn sc_keywords(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Query(q): Query<RangeQ>,
) -> Result<impl IntoResponse, AppError> {
    let sid = resolve(&s, &user, &id, Role::Viewer).await?;
    let domain: String = sqlx::query_scalar("SELECT domain FROM sites WHERE id = ?")
        .bind(&sid)
        .fetch_one(&s.db)
        .await?;
    let range = period::resolve(&q.period, q.date.as_deref(), q.from.as_deref(), q.to.as_deref()).map_err(AppError::BadRequest)?;
    match crate::google::search_console(&s, &user.user_id, &domain, &range).await {
        Ok(v) => Ok(Json(v).into_response()),
        Err(e) => Err(e),
    }
}
