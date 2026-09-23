use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{delete, get, post},
    Form, Json, Router,
};
use serde::Deserialize;
use std::collections::BTreeMap;

use crate::{
    billing::{gate, plans, webhook},
    error::AppError,
    state::AppState,
    web::middleware_auth::CurrentUser,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/billing/plans", get(list_plans))
        .route("/api/billing/subscription", get(my_subscription))
        .route("/api/billing/webhook", post(paddle_webhook))
        .route("/api/sites/:id/reports", get(list_reports).post(create_report))
        .route("/api/sites/:id/reports/:rid", delete(delete_report))
        .route("/api/audit", get(list_audit))
        .route("/api/admin/users", get(admin_users))
        .route("/api/admin/sites", get(admin_sites))
        .route("/api/admin/users/:uid/admin", post(admin_set))
}

async fn list_plans() -> impl IntoResponse {
    Json(plans::all())
}

async fn my_subscription(State(s): State<AppState>, user: CurrentUser) -> Result<impl IntoResponse, AppError> {
    let sub = gate::subscription_for(&s.db, &user.user_id).await?;
    let volume = gate::monthly_volume(&s.db, &user.user_id).await?;
    Ok(Json(serde_json::json!({
        "status": sub.status, "plan_kind": sub.plan_kind,
        "monthly_pageview_limit": sub.monthly_pageview_limit,
        "site_limit": sub.site_limit, "team_member_limit": sub.team_member_limit,
        "trial_expiry": sub.trial_expiry, "next_bill_date": sub.next_bill_date,
        "monthly_volume": volume,
    })))
}

/// Paddle Classic posts form-encoded alerts with p_signature.
async fn paddle_webhook(
    State(s): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let sandbox = std::env::var("PADDLE_SANDBOX").map(|v| v == "1").unwrap_or(false);
    let params: BTreeMap<String, String> =
        serde_urlencoded::from_bytes(&body).map_err(|e| AppError::BadRequest(e.to_string()))?;
    webhook::verify(&params, sandbox)?;
    webhook::handle(&s.db, &params).await?;
    let _ = headers;
    Ok("ok")
}

// ---- reports ----
async fn list_reports(State(s): State<AppState>, user: CurrentUser, Path(id): Path<String>) -> Result<impl IntoResponse, AppError> {
    let sid = site_id(&s, &user, &id).await?;
    let rows: Vec<serde_json::Value> = sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT id, frequency, recipients, last_sent_at FROM reports WHERE site_id = ? ORDER BY created_at",
    )
    .bind(&sid)
    .fetch_all(&s.db)
    .await?
    .into_iter()
    .map(|(i, f, r, l)| serde_json::json!({"id": i, "frequency": f, "recipients": serde_json::from_str::<serde_json::Value>(&r).unwrap_or_default(), "last_sent_at": l}))
    .collect();
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct ReportForm {
    frequency: String,
    recipients: String,
}

async fn create_report(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Form(f): Form<ReportForm>,
) -> Result<impl IntoResponse, AppError> {
    let sid = admin_on(&s, &user, &id).await?;
    if !matches!(f.frequency.as_str(), "weekly" | "monthly") {
        return Err(AppError::BadRequest("frequency must be weekly|monthly".into()));
    }
    let recips: Vec<String> =
        serde_json::from_str(&f.recipients).map_err(|_| AppError::BadRequest("recipients must be a JSON array".into()))?;
    if recips.is_empty() || recips.iter().any(|e| !e.contains('@')) {
        return Err(AppError::BadRequest("recipients must be non-empty emails".into()));
    }
    let rid = ulid::Ulid::new().to_string();
    sqlx::query("INSERT INTO reports (id, site_id, owner_id, frequency, recipients) VALUES (?, ?, ?, ?, ?)")
        .bind(&rid)
        .bind(&sid)
        .bind(&user.user_id)
        .bind(&f.frequency)
        .bind(serde_json::to_string(&recips).unwrap())
        .execute(&s.db)
        .await?;
    crate::audit::record(&s.db, Some(&user.user_id), "report.create", "report", &rid).await;
    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({"id": rid}))))
}

async fn delete_report(
    State(s): State<AppState>,
    user: CurrentUser,
    Path((id, rid)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let sid = admin_on(&s, &user, &id).await?;
    sqlx::query("DELETE FROM reports WHERE id = ? AND site_id = ?").bind(&rid).bind(&sid).execute(&s.db).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn site_id(s: &AppState, user: &CurrentUser, id: &str) -> Result<String, AppError> {
    let sid: Option<String> = sqlx::query_scalar("SELECT id FROM sites WHERE id = ? OR domain = ?")
        .bind(id)
        .bind(id.to_lowercase())
        .fetch_optional(&s.db)
        .await?;
    match sid {
        Some(sid) => {
            crate::sites::membership::require(&s.db, &sid, &user.user_id, crate::sites::membership::Role::Viewer).await?;
            Ok(sid)
        }
        None => Err(AppError::BadRequest("site not found".into())),
    }
}

async fn admin_on(s: &AppState, user: &CurrentUser, id: &str) -> Result<String, AppError> {
    let sid: Option<String> = sqlx::query_scalar("SELECT id FROM sites WHERE id = ? OR domain = ?")
        .bind(id)
        .bind(id.to_lowercase())
        .fetch_optional(&s.db)
        .await?;
    match sid {
        Some(sid) => {
            crate::sites::membership::require(&s.db, &sid, &user.user_id, crate::sites::membership::Role::Admin).await?;
            Ok(sid)
        }
        None => Err(AppError::BadRequest("site not found".into())),
    }
}

// ---- audit ----
#[derive(Deserialize, Default)]
struct AuditQ {
    #[serde(default = "lim50")]
    limit: String,
}
fn lim50() -> String {
    "50".to_string()
}

async fn list_audit(
    State(s): State<AppState>,
    user: CurrentUser,
    Query(q): Query<AuditQ>,
) -> Result<impl IntoResponse, AppError> {
    let limit = q.limit.parse::<i64>().unwrap_or(50).clamp(1, 200);
    let rows: Vec<serde_json::Value> = sqlx::query_as::<_, (String, Option<String>, String, String, String, String)>(
        "SELECT id, actor_id, action, entity, entity_id, created_at FROM audit_log WHERE actor_id = ? OR actor_id IS NULL ORDER BY created_at DESC LIMIT ?",
    )
    .bind(&user.user_id)
    .bind(limit)
    .fetch_all(&s.db)
    .await?
    .into_iter()
    .map(|(i, a, ac, e, eid, c)| serde_json::json!({"id": i, "actor": a, "action": ac, "entity": e, "entity_id": eid, "at": c}))
    .collect();
    Ok(Json(rows))
}

// ---- admin ----
async fn require_admin(s: &AppState, user: &CurrentUser) -> Result<(), AppError> {
    let flag: Option<i64> = sqlx::query_scalar("SELECT is_admin FROM users WHERE id = ?")
        .bind(&user.user_id)
        .fetch_optional(&s.db)
        .await?;
    match flag {
        Some(1) => Ok(()),
        _ => Err(AppError::Forbidden("admin required".into())),
    }
}

async fn admin_users(State(s): State<AppState>, user: CurrentUser) -> Result<impl IntoResponse, AppError> {
    require_admin(&s, &user).await?;
    let rows: Vec<serde_json::Value> = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT id, email, is_admin FROM users ORDER BY email LIMIT 200",
    )
    .fetch_all(&s.db)
    .await?
    .into_iter()
    .map(|(i, e, a)| serde_json::json!({"id": i, "email": e, "is_admin": a == 1}))
    .collect();
    Ok(Json(rows))
}

async fn admin_sites(State(s): State<AppState>, user: CurrentUser) -> Result<impl IntoResponse, AppError> {
    require_admin(&s, &user).await?;
    let rows: Vec<serde_json::Value> = sqlx::query_as::<_, (String, String)>(
        "SELECT id, domain FROM sites ORDER BY domain LIMIT 200",
    )
    .fetch_all(&s.db)
    .await?
    .into_iter()
    .map(|(i, d)| serde_json::json!({"id": i, "domain": d}))
    .collect();
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct AdminForm {
    admin: bool,
}

async fn admin_set(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(uid): Path<String>,
    Form(f): Form<AdminForm>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&s, &user).await?;
    sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
        .bind(if f.admin { 1 } else { 0 })
        .bind(&uid)
        .execute(&s.db)
        .await?;
    crate::audit::record(&s.db, Some(&user.user_id), "admin.set", "user", &uid).await;
    Ok(Json(serde_json::json!({"ok": true})))
}
