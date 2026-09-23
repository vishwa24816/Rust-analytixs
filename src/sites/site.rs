use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{db::DbPool, error::AppError, sites::membership::Role};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Site {
    pub id: String,
    pub domain: String,
    pub timezone: String,
    pub public: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateSite {
    #[validate(length(min = 1, max = 253))]
    pub domain: String,
    #[validate(length(min = 1))]
    #[serde(default = "utc")]
    pub timezone: String,
}

fn utc() -> String {
    "UTC".to_string()
}

fn clean_domain(d: &str) -> String {
    let d = d.trim().to_lowercase();
    d.strip_prefix("https://")
        .or_else(|| d.strip_prefix("http://"))
        .unwrap_or(&d)
        .split('/')
        .next()
        .unwrap_or("")
        .to_string()
}

impl Site {
    pub async fn create(db: &DbPool, user_id: &str, input: &CreateSite) -> Result<Self, AppError> {
        input
            .validate()
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let domain = clean_domain(&input.domain);
        if domain.is_empty() || !domain.contains('.') {
            return Err(AppError::BadRequest("invalid domain".into()));
        }
        let id = ulid::Ulid::new().to_string();
        let mut tx = db.begin().await?;
        let site = sqlx::query_as::<_, Self>(
            "INSERT INTO sites (id, domain, timezone) VALUES (?, ?, ?) RETURNING id, domain, timezone, public, created_at",
        )
        .bind(&id)
        .bind(&domain)
        .bind(&input.timezone)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                AppError::BadRequest("domain already registered".into())
            } else {
                e.into()
            }
        })?;
        sqlx::query("INSERT INTO memberships (site_id, user_id, role) VALUES (?, ?, 'owner')")
            .bind(&id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(site)
    }

    pub async fn list_for(db: &DbPool, user_id: &str) -> Result<Vec<Self>, AppError> {
        sqlx::query_as::<_, Self>(
            "SELECT s.id, s.domain, s.timezone, s.public, s.created_at FROM sites s JOIN memberships m ON m.site_id = s.id WHERE m.user_id = ? ORDER BY s.domain",
        )
        .bind(user_id)
        .fetch_all(db)
        .await
        .map_err(Into::into)
    }

    pub async fn transfer(db: &DbPool, site_id: &str, new_owner_id: &str) -> Result<(), AppError> {
        let mut tx = db.begin().await?;
        sqlx::query("UPDATE memberships SET role = 'admin' WHERE site_id = ? AND role = 'owner'")
            .bind(site_id)
            .execute(&mut *tx)
            .await?;
        let n = sqlx::query(
            "INSERT INTO memberships (site_id, user_id, role) VALUES (?, ?, 'owner') ON CONFLICT (site_id, user_id) DO UPDATE SET role = 'owner'",
        )
        .bind(site_id)
        .bind(new_owner_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if n == 0 {
            return Err(AppError::BadRequest("transfer failed".into()));
        }
        tx.commit().await?;
        Ok(())
    }
}
