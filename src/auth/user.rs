use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub totp_secret: Option<String>,
    pub email_verified: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterInput {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 12, message = "password must be at least 12 chars"))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginInput {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1))]
    pub password: String,
    #[serde(default)]
    pub totp_code: Option<String>,
}

impl User {
    pub async fn create(
        db: &crate::db::DbPool,
        input: &RegisterInput,
    ) -> Result<Self, crate::error::AppError> {
        input
            .validate()
            .map_err(|e| crate::error::AppError::BadRequest(e.to_string()))?;
        let hash = super::password::hash(&input.password);
        let id = ulid::Ulid::new().to_string();
        let email = input.email.to_lowercase();
        // ponytail: execute (not RETURNING) so the write commits before we read it back
        sqlx::query("INSERT INTO users (id, email, password_hash) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(&email)
            .bind(&hash)
            .execute(db)
            .await
            .map_err(|e| {
                if e.to_string().contains("UNIQUE") {
                    crate::error::AppError::BadRequest("email already registered".into())
                } else {
                    e.into()
                }
            })?;
        sqlx::query_as::<_, Self>(
            "SELECT id, email, password_hash, totp_secret, email_verified, created_at FROM users WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(db)
        .await
        .map_err(Into::into)
    }

    pub async fn by_email(
        db: &crate::db::DbPool,
        email: &str,
    ) -> Result<Option<Self>, crate::error::AppError> {
        sqlx::query_as("SELECT id, email, password_hash, totp_secret, email_verified, created_at FROM users WHERE email = ?")
            .bind(email.to_lowercase())
            .fetch_optional(db)
            .await
            .map_err(Into::into)
    }

    pub async fn verify_password(&self, password: &str) -> bool {
        super::password::verify(password, &self.password_hash)
    }
}
