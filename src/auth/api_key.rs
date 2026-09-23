use rand::{distributions::Alphanumeric, Rng};
use sha2::{Digest, Sha256};

use crate::db::DbPool;
use crate::error::AppError;

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct ApiKey {
    pub id: String,
    pub user_id: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub key_hash: String,
    pub created_at: String,
}

#[derive(Debug, serde::Serialize)]
pub struct ApiKeyCreated {
    #[serde(flatten)]
    pub key: ApiKey,
    /// shown once — never stored
    pub raw_token: String,
}

fn sha256_hex(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

pub async fn create(db: &DbPool, user_id: &str, name: &str) -> Result<ApiKeyCreated, AppError> {
    let raw: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect();
    let token = format!("plausible_{raw}");
    let id = ulid::Ulid::new().to_string();
    // ponytail: execute (not RETURNING) so the write commits before we read it back
    sqlx::query("INSERT INTO api_keys (id, user_id, name, key_hash) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(user_id)
        .bind(name)
        .bind(sha256_hex(&token))
        .execute(db)
        .await?;
    let row = sqlx::query_as::<_, ApiKey>(
        "SELECT id, user_id, name, key_hash, created_at FROM api_keys WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(db)
    .await?;
    Ok(ApiKeyCreated { key: row, raw_token: token })
}

pub async fn verify(db: &DbPool, token: &str) -> Result<Option<ApiKey>, AppError> {
    sqlx::query_as::<_, ApiKey>("SELECT id, user_id, name, key_hash, created_at FROM api_keys WHERE key_hash = ?")
        .bind(sha256_hex(token))
        .fetch_optional(db)
        .await
        .map_err(Into::into)
}
