use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect},
    routing::{delete, get, post},
    Form, Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use rand::{distributions::Alphanumeric, Rng};

use crate::{
    auth::{
        oauth_google,
        session,
        token::{self, TokenKind},
        totp,
        user::{LoginInput, RegisterInput, User},
    },
    error::AppError,
    state::AppState,
    web::middleware_auth::{self, CurrentUser},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .route("/login", get(login_page))
        .route("/register", get(register_page))
        .route("/verify-email", post(verify_email))
        .route("/password/forgot", post(forgot_password))
        .route("/password/reset", post(reset_password))
        .route("/oauth/google", get(oauth_google_start))
        .route("/oauth/google/callback", get(oauth_google_callback))
        .route("/totp/enroll", post(totp_enroll))
        .route("/totp/confirm", post(totp_confirm))
        .route("/totp/disable", post(totp_disable))
        .route("/api/keys", get(list_keys).post(create_key))
        .route("/api/keys/:id", delete(delete_key))
}

async fn register(
    State(s): State<AppState>,
    headers: HeaderMap,
    Form(input): Form<RegisterInput>,
) -> Result<impl IntoResponse, AppError> {
    if s.config.disable_registration {
        return Err(AppError::BadRequest("registration disabled".into()));
    }
    if !s.auth_limiter.allow(&format!("register:{}", client_ip(&headers))) {
        return Err(AppError::Locked("too many attempts, slow down".into()));
    }
    let user = User::create(&s.db, &input).await?;
    crate::audit::record(&s.db, Some(&user.id), "auth.register", "user", &user.id).await;
    let token = session::create(&s.db, &user.id).await?;
    let raw = token::issue(&s.db, TokenKind::VerifyEmail, &user.id).await?;
    let link = format!("{}/verify-email?token={raw}", s.config.app_url);
    s.mailer.send_verify(&user.email, &link).await;
    Ok(session_response(&headers, &token, user.id, Some(user.email)))
}

/// Browser form posts (Accept: text/html) get a cookie + redirect;
/// API clients get JSON. Same session either way.
fn session_response(
    headers: &HeaderMap,
    token: &str,
    id: String,
    email: Option<String>,
) -> axum::response::Response {
    let cookie = middleware_auth::set_cookie_header(token);
    let wants_html = headers
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|a| a.contains("text/html"))
        .unwrap_or(false);
    if wants_html {
        (
            [(axum::http::header::SET_COOKIE, cookie)],
            Redirect::to("/"),
        )
            .into_response()
    } else {
        (
            axum::http::StatusCode::CREATED,
            [(axum::http::header::SET_COOKIE, cookie)],
            Json(json!({"id": id, "email": email, "session": token})),
        )
            .into_response()
    }
}

async fn login(
    State(s): State<AppState>,
    headers: HeaderMap,
    Form(input): Form<LoginInput>,
) -> Result<impl IntoResponse, AppError> {
    if !s.auth_limiter.allow(&format!("login:{}", client_ip(&headers))) {
        return Err(AppError::Locked("too many attempts, slow down".into()));
    }
    let user = User::by_email(&s.db, &input.email)
        .await?
        .ok_or(AppError::BadRequest("invalid credentials".into()))?;
    // ponytail: argon2 verify is blocking; login path only, keep sync
    if !user.verify_password(&input.password).await {
        return Err(AppError::BadRequest("invalid credentials".into()));
    }
    if let Some(secret) = &user.totp_secret {
        match &input.totp_code {
            Some(code) if totp::verify(secret, code) => {}
            _ => return Err(AppError::BadRequest("totp code required".into())),
        }
    }
    let token = session::create(&s.db, &user.id).await?;
    crate::audit::record(&s.db, Some(&user.id), "auth.login", "user", &user.id).await;
    Ok(session_response(&headers, &token, user.id, None))
}

async fn logout(State(s): State<AppState>, headers: HeaderMap) -> Result<impl IntoResponse, AppError> {
    if let Some(token) = middleware_auth::token_from(&headers) {
        session::destroy(&s.db, &token).await?;
    }
    Ok((
        [(axum::http::header::SET_COOKIE, middleware_auth::clear_cookie_header())],
        Json(json!({"ok": true})),
    ))
}

fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

async fn me(user: CurrentUser) -> impl IntoResponse {
    Json(json!({"user_id": user.user_id}))
}

async fn login_page() -> Html<&'static str> {
    Html(include_str!("../../templates/auth/login.html"))
}

async fn register_page() -> Html<&'static str> {
    Html(include_str!("../../templates/auth/register.html"))
}

#[derive(Deserialize)]
struct TokenForm {
    token: String,
}

async fn verify_email(
    State(s): State<AppState>,
    Form(f): Form<TokenForm>,
) -> Result<impl IntoResponse, AppError> {
    match token::consume(&s.db, TokenKind::VerifyEmail, &f.token).await? {
        Some(uid) => {
            sqlx::query("UPDATE users SET email_verified = 1 WHERE id = ?")
                .bind(&uid)
                .execute(&s.db)
                .await?;
            Ok(Json(json!({"ok": true})))
        }
        None => Err(AppError::BadRequest("invalid or expired token".into())),
    }
}

#[derive(Deserialize)]
struct ForgotForm {
    email: String,
}

async fn forgot_password(
    State(s): State<AppState>,
    Form(f): Form<ForgotForm>,
) -> Result<impl IntoResponse, AppError> {
    // ponytail: always 200 to avoid email enumeration
    if let Some(user) = User::by_email(&s.db, &f.email).await? {
        let raw = token::issue(&s.db, TokenKind::PasswordReset, &user.id).await?;
        let link = format!("{}/password/reset?token={raw}", s.config.app_url);
        s.mailer.send_reset(&user.email, &link).await;
    }
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct ResetForm {
    token: String,
    password: String,
}

async fn reset_password(
    State(s): State<AppState>,
    Form(f): Form<ResetForm>,
) -> Result<impl IntoResponse, AppError> {
    if f.password.len() < 12 {
        return Err(AppError::BadRequest("password must be at least 12 chars".into()));
    }
    match token::consume(&s.db, TokenKind::PasswordReset, &f.token).await? {
        Some(uid) => {
            let hash = crate::auth::password::hash(&f.password);
            sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
                .bind(&hash)
                .bind(&uid)
                .execute(&s.db)
                .await?;
            Ok(Json(json!({"ok": true})))
        }
        None => Err(AppError::BadRequest("invalid or expired token".into())),
    }
}

async fn oauth_google_start(State(s): State<AppState>) -> Result<impl IntoResponse, AppError> {
    // ponytail: random state prevents CSRF on the callback; not persisted (stateless)
    let state: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(24)
        .map(char::from)
        .collect();
    Ok(Redirect::to(&oauth_google::authorize_url(&s.config, &state)?))
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: String,
}

async fn oauth_google_callback(
    State(s): State<AppState>,
    Query(q): Query<CallbackQuery>,
) -> Result<impl IntoResponse, AppError> {
    let (info, refresh) = oauth_google::exchange(&s.config, &s.http, &q.code).await?;
    if !info.email_verified {
        return Err(AppError::BadRequest("google email not verified".into()));
    }
    let user = match User::by_email(&s.db, &info.email).await? {
        Some(u) => u,
        None => {
            // find-or-create: random unusable password, pre-verified
            let input = RegisterInput {
                email: info.email.clone(),
                password: format!("oauth-{}-{}", ulid::Ulid::new(), ulid::Ulid::new()),
            };
            let u = User::create(&s.db, &input).await?;
            sqlx::query("UPDATE users SET email_verified = 1 WHERE id = ?")
                .bind(&u.id)
                .execute(&s.db)
                .await?;
            u
        }
    };
    let token = session::create(&s.db, &user.id).await?;
    if let Some(r) = refresh {
        crate::google::store_refresh_token(&s, &user.id, &r).await;
    }
    Ok(Json(json!({"id": user.id, "session": token})))
}

/// Step 1: generate a secret, store it, return the otpauth URL for QR.
/// The secret is live immediately; login requires a code from now on.
async fn totp_enroll(State(s): State<AppState>, user: CurrentUser) -> Result<impl IntoResponse, AppError> {
    let secret = totp::new_secret();
    sqlx::query("UPDATE users SET totp_secret = ? WHERE id = ?")
        .bind(&secret)
        .bind(&user.user_id)
        .execute(&s.db)
        .await?;
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = ?")
        .bind(&user.user_id)
        .fetch_one(&s.db)
        .await?;
    Ok(Json(json!({"otpauth_url": totp::otpauth_url(&secret, &email)})))
}

#[derive(Deserialize)]
struct TotpForm {
    code: String,
}

async fn totp_confirm(
    State(s): State<AppState>,
    user: CurrentUser,
    Form(f): Form<TotpForm>,
) -> Result<impl IntoResponse, AppError> {
    let secret: Option<String> = sqlx::query_scalar("SELECT totp_secret FROM users WHERE id = ?")
        .bind(&user.user_id)
        .fetch_one(&s.db)
        .await?;
    match secret {
        Some(sec) if totp::verify(&sec, &f.code) => Ok(Json(json!({"ok": true}))),
        _ => Err(AppError::BadRequest("invalid code".into())),
    }
}

async fn totp_disable(
    State(s): State<AppState>,
    user: CurrentUser,
    Form(f): Form<TotpForm>,
) -> Result<impl IntoResponse, AppError> {
    let secret: Option<String> = sqlx::query_scalar("SELECT totp_secret FROM users WHERE id = ?")
        .bind(&user.user_id)
        .fetch_one(&s.db)
        .await?;
    match secret {
        Some(sec) if totp::verify(&sec, &f.code) => {
            sqlx::query("UPDATE users SET totp_secret = NULL WHERE id = ?")
                .bind(&user.user_id)
                .execute(&s.db)
                .await?;
            Ok(Json(json!({"ok": true})))
        }
        _ => Err(AppError::BadRequest("invalid code".into())),
    }
}

async fn list_keys(State(s): State<AppState>, user: CurrentUser) -> Result<impl IntoResponse, AppError> {
    let keys: Vec<crate::auth::ApiKey> =
        sqlx::query_as("SELECT id, user_id, name, key_hash, created_at FROM api_keys WHERE user_id = ? ORDER BY created_at")
            .bind(&user.user_id)
            .fetch_all(&s.db)
            .await?;
    Ok(Json(keys))
}

#[derive(Deserialize)]
struct KeyForm {
    name: String,
}

async fn create_key(
    State(s): State<AppState>,
    user: CurrentUser,
    Form(f): Form<KeyForm>,
) -> Result<impl IntoResponse, AppError> {
    if f.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    let created = crate::auth::api_key::create(&s.db, &user.user_id, f.name.trim()).await?;
    Ok((axum::http::StatusCode::CREATED, Json(created)))
}

async fn delete_key(
    State(s): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    sqlx::query("DELETE FROM api_keys WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user.user_id)
        .execute(&s.db)
        .await?;
    Ok(Json(json!({"ok": true})))
}
