use rand::{distributions::Alphanumeric, Rng};

use crate::{db::DbPool, error::AppError};

#[derive(Debug, sqlx::FromRow)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub created_at: String,
    pub expires_at: String,
}

const TTL_DAYS: i64 = 30;

pub async fn create(db: &DbPool, user_id: &str) -> Result<String, AppError> {
    let token: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, datetime('now', ?))")
        .bind(&token)
        .bind(user_id)
        .bind(format!("+{TTL_DAYS} days"))
        .execute(db)
        .await?;
    Ok(token)
}

pub async fn destroy(db: &DbPool, token: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM sessions WHERE id = ?").bind(token).execute(db).await?;
    Ok(())
}

pub async fn user_id(db: &DbPool, token: &str) -> Result<Option<String>, AppError> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT user_id FROM sessions WHERE id = ? AND expires_at > datetime('now')",
    )
    .bind(token)
    .fetch_one(db)
    .await
    .map_err(Into::into)
}
