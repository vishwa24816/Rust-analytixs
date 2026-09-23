use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{auth::password, db::DbPool, error::AppError};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SharedLink {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub slug: String,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SharedLinkInput {
    pub name: String,
    #[serde(default)]
    pub password: Option<String>,
}

fn sha256_hex(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

impl SharedLink {
    pub async fn get_or_create(db: &DbPool, site_id: &str, input: &SharedLinkInput) -> Result<Self, AppError> {
        if input.name.trim().is_empty() {
            return Err(AppError::BadRequest("name required".into()));
        }
        if let Some(existing) = sqlx::query_as::<_, Self>(
            "SELECT id, site_id, name, slug, password_hash, created_at FROM shared_links WHERE site_id = ? AND name = ?",
        )
        .bind(site_id)
        .bind(input.name.trim())
        .fetch_optional(db)
        .await?
        {
            return Ok(existing);
        }
        let slug: String = rand::thread_rng().sample_iter(&Alphanumeric).take(16).map(char::from).collect();
        let pw_hash = match &input.password {
            Some(p) if !p.is_empty() => Some(sha256_hex(p)),
            _ => None,
        };
        let lid = ulid::Ulid::new().to_string();
        // ponytail: execute (not RETURNING) so the write commits before we read it back
        sqlx::query("INSERT INTO shared_links (id, site_id, name, slug, password_hash) VALUES (?, ?, ?, ?, ?)")
            .bind(&lid)
            .bind(site_id)
            .bind(input.name.trim())
            .bind(&slug)
            .bind(pw_hash)
            .execute(db)
            .await?;
        sqlx::query_as::<_, Self>(
            "SELECT id, site_id, name, slug, password_hash, created_at FROM shared_links WHERE id = ?",
        )
        .bind(&lid)
        .fetch_one(db)
        .await
        .map_err(Into::into)
    }

    pub async fn list(db: &DbPool, site_id: &str) -> Result<Vec<Self>, AppError> {
        sqlx::query_as::<_, Self>(
            "SELECT id, site_id, name, slug, password_hash, created_at FROM shared_links WHERE site_id = ? ORDER BY name",
        )
        .bind(site_id)
        .fetch_all(db)
        .await
        .map_err(Into::into)
    }

    pub fn check_password(&self, password: Option<&str>) -> bool {
        match &self.password_hash {
            None => true,
            Some(h) => password.map(|p| &sha256_hex(p) == h).unwrap_or(false),
        }
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PluginToken {
    pub id: String,
    pub site_id: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub key_hash: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct PluginTokenCreated {
    #[serde(flatten)]
    pub token: PluginToken,
    pub raw_token: String,
}

pub async fn create_token(db: &DbPool, site_id: &str, name: &str) -> Result<PluginTokenCreated, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    let raw: String = rand::thread_rng().sample_iter(&Alphanumeric).take(48).map(char::from).collect();
    let raw_token = format!("plausible_site_{raw}");
    let tid = ulid::Ulid::new().to_string();
    // ponytail: execute (not RETURNING) so the write commits before we read it back
    sqlx::query("INSERT INTO plugin_tokens (id, site_id, name, key_hash) VALUES (?, ?, ?, ?)")
        .bind(&tid)
        .bind(site_id)
        .bind(name.trim())
        .bind(sha256_hex(&raw_token))
        .execute(db)
        .await?;
    let token = sqlx::query_as::<_, PluginToken>(
        "SELECT id, site_id, name, key_hash, created_at FROM plugin_tokens WHERE id = ?",
    )
    .bind(&tid)
    .fetch_one(db)
    .await?;
    Ok(PluginTokenCreated { token, raw_token })
}

pub async fn verify_token(db: &DbPool, raw: &str) -> Result<Option<PluginToken>, AppError> {
    sqlx::query_as::<_, PluginToken>(
        "SELECT id, site_id, name, key_hash, created_at FROM plugin_tokens WHERE key_hash = ?",
    )
    .bind(sha256_hex(raw))
    .fetch_optional(db)
    .await
    .map_err(Into::into)
}

/// argon2 password check helper shared by login flows (kept here for reuse).
pub fn verify_password(password: &str, hash: &str) -> bool {
    password::verify(password, hash)
}
