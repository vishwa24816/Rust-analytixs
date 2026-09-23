use serde::{Deserialize, Serialize};

use crate::{db::DbPool, error::AppError};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Annotation {
    pub id: String,
    pub site_id: String,
    pub date: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AnnotationInput {
    /// YYYY-MM-DD
    pub date: String,
    pub content: String,
}

impl Annotation {
    pub async fn create(db: &DbPool, site_id: &str, input: &AnnotationInput) -> Result<Self, AppError> {
        if chrono::NaiveDate::parse_from_str(&input.date, "%Y-%m-%d").is_err() {
            return Err(AppError::BadRequest("date must be YYYY-MM-DD".into()));
        }
        if input.content.trim().is_empty() {
            return Err(AppError::BadRequest("content required".into()));
        }
        let nid = ulid::Ulid::new().to_string();
        // ponytail: execute (not RETURNING) so the write commits before we read it back
        sqlx::query("INSERT INTO annotations (id, site_id, date, content) VALUES (?, ?, ?, ?)")
            .bind(&nid)
            .bind(site_id)
            .bind(&input.date)
            .bind(input.content.trim())
            .execute(db)
            .await?;
        sqlx::query_as::<_, Self>(
            "SELECT id, site_id, date, content, created_at FROM annotations WHERE id = ?",
        )
        .bind(&nid)
        .fetch_one(db)
        .await
        .map_err(Into::into)
    }

    pub async fn list(db: &DbPool, site_id: &str, from: &str, to: &str) -> Result<Vec<Self>, AppError> {
        sqlx::query_as::<_, Self>(
            "SELECT id, site_id, date, content, created_at FROM annotations WHERE site_id = ? AND date >= ? AND date <= ? ORDER BY date",
        )
        .bind(site_id)
        .bind(from)
        .bind(to)
        .fetch_all(db)
        .await
        .map_err(Into::into)
    }
}
