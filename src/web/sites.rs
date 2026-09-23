use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Form, Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    error::AppError,
    sites::{
        invitation::{self, InviteInput},
        membership::{self, Role},
        shield::{ShieldInput, ShieldRule},
        site::{CreateSite, Site},
        team::Team,
    },
    state::AppState,
    web::middleware_auth::CurrentUser,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/sites", get(list_sites).post(create_site))
        .route("/api/sites/:id", get(get_site).put(update_site).delete(delete_site))
        .route("/api/sites/:id/transfer", post(transfer_site))
        .route("/api/sites/:id/members", get(list_members))
        .route("/api/sites/:id/members/:uid", put(set_role).delete(remove_member))
        .route("/api/sites/:id/invite", post(invite_member))
        .route("/api/invitations/accept", post(accept_invite))
        .route("/api/sites/:id/shield", get(list_shield).post(add_shield))
        .route("/api/sites/:id/shield/:rid", delete(delete_shield))
        .route("/api/teams", get(list_teams).post(create_team))
        .route("/api/teams/:id", get(get_team).put(rename_team).delete(delete_team))
        .route("/api/teams/:id/members", get(list_team_members).post(add_team_member))
        .route("/api/teams/:id/members/:uid", put(set_team_role).delete(remove_team_member))
}

async fn list_sites(State(s): State<AppState>, user: CurrentUser) -> Result<impl IntoResponse, AppError> {
    Ok(Json(Site::list_for(&s.db, &user.user_id).await?))
}

async fn create_site(
    State(s): State<AppState>,
    user: CurrentUser,
    Form(input): Form<CreateSite>,
) -> Result<impl IntoResponse, AppError> {
    crate::billing::gate::check_site_create(&s.db, &user.user_id).await?;
    let site = Site::create(&s.db, &user.user_id, &input).await?;
    crate::audit::record(&s.db, Some(&user.user_id), "site.create", "site", &site.id).await;
    Ok((axum::http::StatusCode::CREATED, Json(site)))
}

async fn get_site(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    membership::require(&s.db, &id, &user.user_id, Role::Viewer).await?;
    let site: Site = sqlx::query_as("SELECT id, domain, timezone, public, created_at FROM sites WHERE id = ?")
        .bind(&id)
        .fetch_one(&s.db)
        .await
        .map_err(|_| AppError::BadRequest("site not found".into()))?;
    Ok(Json(site))
}

#[derive(Deserialize)]
struct UpdateSite {
    timezone: Option<String>,
    public: Option<bool>,
}

async fn update_site(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Form(u): Form<UpdateSite>,
) -> Result<impl IntoResponse, AppError> {
    membership::require(&s.db, &id, &user.user_id, Role::Admin).await?;
    if let Some(tz) = u.timezone {
        sqlx::query("UPDATE sites SET timezone = ? WHERE id = ?").bind(tz).bind(&id).execute(&s.db).await?;
    }
    if let Some(p) = u.public {
        sqlx::query("UPDATE sites SET public = ? WHERE id = ?").bind(p).bind(&id).execute(&s.db).await?;
    }
    Ok(Json(json!({"ok": true})))
}

async fn delete_site(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    membership::require(&s.db, &id, &user.user_id, Role::Owner).await?;
    sqlx::query("DELETE FROM sites WHERE id = ?").bind(&id).execute(&s.db).await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct Transfer {
    email: String,
}

async fn transfer_site(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Form(t): Form<Transfer>,
) -> Result<impl IntoResponse, AppError> {
    membership::require(&s.db, &id, &user.user_id, Role::Owner).await?;
    let new_owner: Option<String> =
        sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
            .bind(t.email.to_lowercase())
            .fetch_optional(&s.db)
            .await?;
    match new_owner {
        Some(uid) => {
            Site::transfer(&s.db, &id, &uid).await?;
            Ok(Json(json!({"ok": true})))
        }
        None => Err(AppError::BadRequest("user not found".into())),
    }
}

async fn list_members(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    membership::require(&s.db, &id, &user.user_id, Role::Viewer).await?;
    let rows: Vec<serde_json::Value> = sqlx::query_as::<_, (String, String, String)>(
        "SELECT u.id, u.email, m.role FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.site_id = ?",
    )
    .bind(&id)
    .fetch_all(&s.db)
    .await?
    .into_iter()
    .map(|(uid, email, role)| json!({"user_id": uid, "email": email, "role": role}))
    .collect();
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct SetRole {
    role: Role,
}

async fn set_role(
    State(s): State<AppState>,
    user: CurrentUser,
    Path((id, uid)): Path<(String, String)>,
    Form(r): Form<SetRole>,
) -> Result<impl IntoResponse, AppError> {
    membership::require(&s.db, &id, &user.user_id, Role::Admin).await?;
    sqlx::query("UPDATE memberships SET role = ? WHERE site_id = ? AND user_id = ?")
        .bind(r.role.as_str())
        .bind(&id)
        .bind(&uid)
        .execute(&s.db)
        .await?;
    Ok(Json(json!({"ok": true})))
}

async fn remove_member(
    State(s): State<AppState>,
    user: CurrentUser,
    Path((id, uid)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    membership::require(&s.db, &id, &user.user_id, Role::Admin).await?;
    if uid == user.user_id {
        return Err(AppError::BadRequest("cannot remove yourself".into()));
    }
    sqlx::query("DELETE FROM memberships WHERE site_id = ? AND user_id = ?")
        .bind(&id)
        .bind(&uid)
        .execute(&s.db)
        .await?;
    Ok(Json(json!({"ok": true})))
}

async fn invite_member(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Form(input): Form<InviteInput>,
) -> Result<impl IntoResponse, AppError> {
    let sid: String = sqlx::query_scalar("SELECT id FROM sites WHERE id = ? OR domain = ?")
        .bind(&id)
        .bind(id.to_lowercase())
        .fetch_one(&s.db)
        .await
        .map_err(|_| AppError::BadRequest("site not found".into()))?;
    membership::require(&s.db, &sid, &user.user_id, Role::Admin).await?;
    // team member limit of the inviting owner's plan
    let sub = crate::billing::gate::subscription_for(&s.db, &user.user_id).await?;
    let members: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memberships WHERE site_id = ?")
        .bind(&sid)
        .fetch_one(&s.db)
        .await?;
    if members >= sub.team_member_limit + 1 {
        return Err(AppError::Payment("team member limit reached for your plan".into()));
    }
    let (inv, raw) = invitation::invite(&s.db, &sid, &input).await?;
    let link = format!("{}/accept-invitation?token={raw}", s.config.app_url);
    let domain: String = sqlx::query_scalar("SELECT domain FROM sites WHERE id = ?")
        .bind(&sid)
        .fetch_one(&s.db)
        .await
        .unwrap_or_default();
    s.mailer.send_invite(&inv.email, &domain, &inv.role, &link).await;
    crate::audit::record(&s.db, Some(&user.user_id), "site.invite", "invitation", &inv.id).await;
    // ponytail: dev mode has no mailer; return raw token so tests can accept
    let mut resp = serde_json::json!({"id": inv.id, "email": inv.email});
    if s.config.smtp_url.is_empty() {
        resp["token"] = serde_json::json!(raw);
    }
    Ok((axum::http::StatusCode::CREATED, Json(resp)))
}

#[derive(Deserialize)]
struct Accept {
    token: String,
}

async fn accept_invite(
    State(s): State<AppState>,
    user: CurrentUser,
    Form(a): Form<Accept>,
) -> Result<impl IntoResponse, AppError> {
    let site_id = invitation::accept(&s.db, &a.token, &user.user_id).await?;
    Ok(Json(json!({"site_id": site_id})))
}

async fn list_shield(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    membership::require(&s.db, &id, &user.user_id, Role::Viewer).await?;
    Ok(Json(ShieldRule::list(&s.db, &id).await?))
}

async fn add_shield(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Form(input): Form<ShieldInput>,
) -> Result<impl IntoResponse, AppError> {
    membership::require(&s.db, &id, &user.user_id, Role::Admin).await?;
    let rule = ShieldRule::add(&s.db, &id, &input).await?;
    Ok((axum::http::StatusCode::CREATED, Json(rule)))
}

async fn delete_shield(
    State(s): State<AppState>,
    user: CurrentUser,
    Path((id, rid)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    membership::require(&s.db, &id, &user.user_id, Role::Admin).await?;
    sqlx::query("DELETE FROM shield_rules WHERE id = ? AND site_id = ?")
        .bind(&rid)
        .bind(&id)
        .execute(&s.db)
        .await?;
    Ok(Json(json!({"ok": true})))
}

async fn list_teams(State(s): State<AppState>, user: CurrentUser) -> Result<impl IntoResponse, AppError> {
    Ok(Json(Team::list_for(&s.db, &user.user_id).await?))
}

#[derive(Deserialize)]
struct CreateTeam {
    name: String,
}

async fn create_team(
    State(s): State<AppState>,
    user: CurrentUser,
    Form(t): Form<CreateTeam>,
) -> Result<impl IntoResponse, AppError> {
    let team = Team::create(&s.db, &user.user_id, &t.name).await?;
    Ok((axum::http::StatusCode::CREATED, Json(team)))
}

async fn get_team(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    Team::require_member(&s.db, &id, &user.user_id).await?;
    let team: Team = sqlx::query_as("SELECT id, name, created_at FROM teams WHERE id = ?")
        .bind(&id)
        .fetch_one(&s.db)
        .await
        .map_err(|_| AppError::BadRequest("team not found".into()))?;
    Ok(Json(team))
}

#[derive(Deserialize)]
struct RenameTeam {
    name: String,
}

async fn rename_team(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Form(r): Form<RenameTeam>,
) -> Result<impl IntoResponse, AppError> {
    Team::require_owner(&s.db, &id, &user.user_id).await?;
    if r.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    sqlx::query("UPDATE teams SET name = ? WHERE id = ?")
        .bind(r.name.trim())
        .bind(&id)
        .execute(&s.db)
        .await?;
    Ok(Json(json!({"ok": true})))
}

async fn delete_team(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    Team::require_owner(&s.db, &id, &user.user_id).await?;
    sqlx::query("DELETE FROM teams WHERE id = ?").bind(&id).execute(&s.db).await?;
    Ok(Json(json!({"ok": true})))
}

async fn list_team_members(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    Team::require_member(&s.db, &id, &user.user_id).await?;
    let rows: Vec<serde_json::Value> = sqlx::query_as::<_, (String, String, String)>(
        "SELECT u.id, u.email, m.role FROM team_memberships m JOIN users u ON u.id = m.user_id WHERE m.team_id = ?",
    )
    .bind(&id)
    .fetch_all(&s.db)
    .await?
    .into_iter()
    .map(|(uid, email, role)| json!({"user_id": uid, "email": email, "role": role}))
    .collect();
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct AddTeamMember {
    email: String,
    #[serde(default = "member_role")]
    role: String,
}

fn member_role() -> String {
    "member".to_string()
}

async fn add_team_member(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
    Form(m): Form<AddTeamMember>,
) -> Result<impl IntoResponse, AppError> {
    Team::require_owner(&s.db, &id, &user.user_id).await?;
    Team::add_member(&s.db, &id, &m.email, &m.role).await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct SetTeamRole {
    role: String,
}

async fn set_team_role(
    State(s): State<AppState>,
    user: CurrentUser,
    Path((id, uid)): Path<(String, String)>,
    Form(r): Form<SetTeamRole>,
) -> Result<impl IntoResponse, AppError> {
    Team::require_owner(&s.db, &id, &user.user_id).await?;
    if !matches!(r.role.as_str(), "owner" | "member") {
        return Err(AppError::BadRequest("role must be owner|member".into()));
    }
    sqlx::query("UPDATE team_memberships SET role = ? WHERE team_id = ? AND user_id = ?")
        .bind(&r.role)
        .bind(&id)
        .bind(&uid)
        .execute(&s.db)
        .await?;
    Ok(Json(json!({"ok": true})))
}

async fn remove_team_member(
    State(s): State<AppState>,
    user: CurrentUser,
    Path((id, uid)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    Team::require_owner(&s.db, &id, &user.user_id).await?;
    Team::remove_member(&s.db, &id, &uid).await?;
    Ok(Json(json!({"ok": true})))
}
