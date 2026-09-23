use serde::{Deserialize, Serialize};

use crate::{db::DbPool, error::AppError, stats::filters};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Segment {
    pub id: String,
    pub site_id: String,
    pub owner_id: String,
    pub name: String,
    pub filters: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SegmentInput {
    pub name: String,
    #[serde(default)]
    pub filters: String,
}

impl Segment {
    pub async fn create(db: &DbPool, site_id: &str, owner: &str, input: &SegmentInput) -> Result<Self, AppError> {
        if input.name.trim().is_empty() {
            return Err(AppError::BadRequest("name required".into()));
        }
        // validate stored filter syntax now, not at query time
        filters::parse_v1(&input.filters).map_err(AppError::BadRequest)?;
        let nid = ulid::Ulid::new().to_string();
        // ponytail: execute (not RETURNING) so the write commits before we read it back
        sqlx::query("INSERT INTO segments (id, site_id, owner_id, name, filters) VALUES (?, ?, ?, ?, ?)")
            .bind(&nid)
            .bind(site_id)
            .bind(owner)
            .bind(input.name.trim())
            .bind(&input.filters)
            .execute(db)
            .await?;
        sqlx::query_as::<_, Self>(
            "SELECT id, site_id, owner_id, name, filters, created_at FROM segments WHERE id = ?",
        )
        .bind(&nid)
        .fetch_one(db)
        .await
        .map_err(Into::into)
    }

    pub async fn list(db: &DbPool, site_id: &str) -> Result<Vec<Self>, AppError> {
        sqlx::query_as::<_, Self>(
            "SELECT id, site_id, owner_id, name, filters, created_at FROM segments WHERE site_id = ? ORDER BY name",
        )
        .bind(site_id)
        .fetch_all(db)
        .await
        .map_err(Into::into)
    }
}
