use serde::{Deserialize, Serialize};

use crate::{db::DbPool, error::AppError};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ShieldRule {
    pub id: String,
    pub site_id: String,
    pub kind: String,
    pub value: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ShieldInput {
    pub kind: String,
    pub value: String,
}

const KINDS: &[&str] = &["ip", "country", "page", "hostname"];

impl ShieldRule {
    pub async fn add(db: &DbPool, site_id: &str, input: &ShieldInput) -> Result<Self, AppError> {
        if !KINDS.contains(&input.kind.as_str()) {
            return Err(AppError::BadRequest("kind must be ip|country|page|hostname".into()));
        }
        if input.value.trim().is_empty() {
            return Err(AppError::BadRequest("value required".into()));
        }
        let rid = ulid::Ulid::new().to_string();
        // ponytail: execute (not RETURNING) so the write commits before we read it back
        sqlx::query("INSERT INTO shield_rules (id, site_id, kind, value) VALUES (?, ?, ?, ?)")
            .bind(&rid)
            .bind(site_id)
            .bind(&input.kind)
            .bind(input.value.trim())
            .execute(db)
            .await
            .map_err(|e| {
                if e.to_string().contains("UNIQUE") {
                    AppError::BadRequest("rule already exists".into())
                } else {
                    e.into()
                }
            })?;
        sqlx::query_as::<_, Self>(
            "SELECT id, site_id, kind, value, created_at FROM shield_rules WHERE id = ?",
        )
        .bind(&rid)
        .fetch_one(db)
        .await
        .map_err(Into::into)
    }

    pub async fn list(db: &DbPool, site_id: &str) -> Result<Vec<Self>, AppError> {
        sqlx::query_as::<_, Self>(
            "SELECT id, site_id, kind, value, created_at FROM shield_rules WHERE site_id = ? ORDER BY kind, value",
        )
        .bind(site_id)
        .fetch_all(db)
        .await
        .map_err(Into::into)
    }
}
