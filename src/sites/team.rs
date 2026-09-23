use serde::Serialize;

use crate::{db::DbPool, error::AppError};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

impl Team {
    pub async fn create(db: &DbPool, user_id: &str, name: &str) -> Result<Self, AppError> {
        if name.trim().is_empty() {
            return Err(AppError::BadRequest("name required".into()));
        }
        let id = ulid::Ulid::new().to_string();
        let mut tx = db.begin().await?;
        let team = sqlx::query_as::<_, Self>(
            "INSERT INTO teams (id, name) VALUES (?, ?) RETURNING id, name, created_at",
        )
        .bind(&id)
        .bind(name.trim())
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES (?, ?, 'owner')")
            .bind(&id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(team)
    }

    pub async fn list_for(db: &DbPool, user_id: &str) -> Result<Vec<Self>, AppError> {
        sqlx::query_as::<_, Self>(
            "SELECT t.id, t.name, t.created_at FROM teams t JOIN team_memberships m ON m.team_id = t.id WHERE m.user_id = ? ORDER BY t.name",
        )
        .bind(user_id)
        .fetch_all(db)
        .await
        .map_err(Into::into)
    }

    async fn role_of(db: &DbPool, team_id: &str, user_id: &str) -> Result<Option<String>, AppError> {
        sqlx::query_scalar("SELECT role FROM team_memberships WHERE team_id = ? AND user_id = ?")
            .bind(team_id)
            .bind(user_id)
            .fetch_optional(db)
            .await
            .map_err(Into::into)
    }

    /// Only team owners manage members.
    pub async fn require_owner(db: &DbPool, team_id: &str, user_id: &str) -> Result<(), AppError> {
        match Self::role_of(db, team_id, user_id).await?.as_deref() {
            Some("owner") => Ok(()),
            _ => Err(AppError::Forbidden("team owner required".into())),
        }
    }

    /// Any member can view the team.
    pub async fn require_member(db: &DbPool, team_id: &str, user_id: &str) -> Result<(), AppError> {
        match Self::role_of(db, team_id, user_id).await? {
            Some(_) => Ok(()),
            None => Err(AppError::Forbidden("not a team member".into())),
        }
    }

    pub async fn add_member(db: &DbPool, team_id: &str, email: &str, role: &str) -> Result<(), AppError> {
        if !matches!(role, "owner" | "member") {
            return Err(AppError::BadRequest("role must be owner|member".into()));
        }
        let uid: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
            .bind(email.to_lowercase())
            .fetch_optional(db)
            .await?;
        match uid {
            Some(u) => {
                sqlx::query(
                    "INSERT INTO team_memberships (team_id, user_id, role) VALUES (?, ?, ?) ON CONFLICT (team_id, user_id) DO UPDATE SET role = excluded.role",
                )
                .bind(team_id)
                .bind(&u)
                .bind(role)
                .execute(db)
                .await?;
                Ok(())
            }
            None => Err(AppError::BadRequest("user not found".into())),
        }
    }

    pub async fn remove_member(db: &DbPool, team_id: &str, target: &str) -> Result<(), AppError> {
        let owners: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM team_memberships WHERE team_id = ? AND role = 'owner'",
        )
        .bind(team_id)
        .fetch_one(db)
        .await?;
        let target_role = Self::role_of(db, team_id, target).await?;
        if target_role.as_deref() == Some("owner") && owners <= 1 {
            return Err(AppError::BadRequest("cannot remove the last owner".into()));
        }
        sqlx::query("DELETE FROM team_memberships WHERE team_id = ? AND user_id = ?")
            .bind(team_id)
            .bind(target)
            .execute(db)
            .await?;
        Ok(())
    }
}
