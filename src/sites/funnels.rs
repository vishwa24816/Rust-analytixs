use serde::{Deserialize, Serialize};

use crate::{db::DbPool, error::AppError, sites::goals::Goal};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Funnel {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct FunnelWithSteps {
    #[serde(flatten)]
    pub funnel: Funnel,
    pub steps: Vec<Goal>,
}

#[derive(Debug, Deserialize)]
pub struct FunnelInput {
    pub name: String,
    /// ordered goal ids
    pub steps: Vec<String>,
}

/// Form-friendly input: steps as comma-separated ids.
#[derive(Debug, Deserialize)]
pub struct FunnelForm {
    pub name: String,
    pub steps: String,
}

impl FunnelForm {
    pub fn into_input(self) -> FunnelInput {
        FunnelInput {
            name: self.name,
            steps: self.steps.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        }
    }
}

impl Funnel {
    pub async fn create(db: &DbPool, site_id: &str, input: &FunnelInput) -> Result<FunnelWithSteps, AppError> {
        if input.name.trim().is_empty() {
            return Err(AppError::BadRequest("name required".into()));
        }
        if input.steps.len() < 2 || input.steps.len() > 8 {
            return Err(AppError::BadRequest("funnel needs 2..=8 steps".into()));
        }
        // all goals must belong to this site
        let goals: Vec<Goal> = sqlx::query_as(
            "SELECT id, site_id, kind, event_name, page_path, currency, created_at FROM goals WHERE site_id = ?",
        )
        .bind(site_id)
        .fetch_all(db)
        .await?;
        let mut ordered = vec![];
        for gid in &input.steps {
            match goals.iter().find(|g| &g.id == gid) {
                Some(g) => ordered.push(g.clone()),
                None => return Err(AppError::BadRequest(format!("unknown goal {gid}"))),
            }
        }
        let id = ulid::Ulid::new().to_string();
        let mut tx = db.begin().await?;
        sqlx::query("INSERT INTO funnels (id, site_id, name) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(site_id)
            .bind(input.name.trim())
            .execute(&mut *tx)
            .await?;
        for (i, g) in ordered.iter().enumerate() {
            sqlx::query("INSERT INTO funnel_steps (funnel_id, position, goal_id) VALUES (?, ?, ?)")
                .bind(&id)
                .bind(i as i64)
                .bind(&g.id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        let funnel: Funnel =
            sqlx::query_as("SELECT id, site_id, name, created_at FROM funnels WHERE id = ?")
                .bind(&id)
                .fetch_one(db)
                .await?;
        Ok(FunnelWithSteps { funnel, steps: ordered })
    }

    pub async fn get(db: &DbPool, site_id: &str, id: &str) -> Result<Option<FunnelWithSteps>, AppError> {
        let funnel: Option<Funnel> =
            sqlx::query_as("SELECT id, site_id, name, created_at FROM funnels WHERE id = ? AND site_id = ?")
                .bind(id)
                .bind(site_id)
                .fetch_optional(db)
                .await?;
        match funnel {
            None => Ok(None),
            Some(f) => {
                let steps: Vec<Goal> = sqlx::query_as(
                    "SELECT g.id, g.site_id, g.kind, g.event_name, g.page_path, g.currency, g.created_at FROM goals g JOIN funnel_steps s ON s.goal_id = g.id WHERE s.funnel_id = ? ORDER BY s.position",
                )
                .bind(id)
                .fetch_all(db)
                .await?;
                Ok(Some(FunnelWithSteps { funnel: f, steps }))
            }
        }
    }

    pub async fn list(db: &DbPool, site_id: &str) -> Result<Vec<Funnel>, AppError> {
        sqlx::query_as("SELECT id, site_id, name, created_at FROM funnels WHERE site_id = ? ORDER BY name")
            .bind(site_id)
            .fetch_all(db)
            .await
            .map_err(Into::into)
    }
}
