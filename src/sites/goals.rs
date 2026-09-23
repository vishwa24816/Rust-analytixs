use serde::{Deserialize, Serialize};

use crate::{db::DbPool, error::AppError};

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct Goal {
    pub id: String,
    pub site_id: String,
    pub kind: String,
    pub event_name: String,
    pub page_path: String,
    pub currency: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct GoalInput {
    pub kind: String,
    pub event_name: String,
    #[serde(default)]
    pub page_path: String,
    #[serde(default)]
    pub currency: String,
}

impl Goal {
    pub async fn create(db: &DbPool, site_id: &str, input: &GoalInput) -> Result<Self, AppError> {
        if !matches!(input.kind.as_str(), "page" | "custom" | "revenue") {
            return Err(AppError::BadRequest("kind must be page|custom|revenue".into()));
        }
        if input.event_name.trim().is_empty() || input.event_name.len() > 120 {
            return Err(AppError::BadRequest("event_name required (<=120)".into()));
        }
        if input.kind == "page" && input.page_path.trim().is_empty() {
            return Err(AppError::BadRequest("page_path required for page goals".into()));
        }
        let new_id = ulid::Ulid::new().to_string();
        // ponytail: execute (not RETURNING) so the write commits before we read it back
        sqlx::query(
            "INSERT INTO goals (id, site_id, kind, event_name, page_path, currency) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&new_id)
        .bind(site_id)
        .bind(&input.kind)
        .bind(input.event_name.trim())
        .bind(input.page_path.trim())
        .bind(input.currency.trim())
        .execute(db)
        .await
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                AppError::BadRequest("goal already exists".into())
            } else {
                e.into()
            }
        })?;
        sqlx::query_as::<_, Self>(
            "SELECT id, site_id, kind, event_name, page_path, currency, created_at FROM goals WHERE id = ?",
        )
        .bind(&new_id)
        .fetch_one(db)
        .await
        .map_err(Into::into)
    }

    pub async fn list(db: &DbPool, site_id: &str) -> Result<Vec<Self>, AppError> {
        sqlx::query_as::<_, Self>(
            "SELECT id, site_id, kind, event_name, page_path, currency, created_at FROM goals WHERE site_id = ? ORDER BY event_name",
        )
        .bind(site_id)
        .fetch_all(db)
        .await
        .map_err(Into::into)
    }

    /// SQL predicate matching this goal's conversions (alias `e`).
    pub fn predicate(&self) -> String {
        match self.kind.as_str() {
            "page" => format!(
                "(e.name = 'pageview' AND e.pathname = '{}')",
                self.page_path.replace('\'', "''")
            ),
            "revenue" => format!(
                "(e.name = '{}' AND e.revenue_cents IS NOT NULL)",
                self.event_name.replace('\'', "''")
            ),
            _ => format!("(e.name = '{}')", self.event_name.replace('\'', "''")),
        }
    }
}
