use serde::{Deserialize, Serialize};

use crate::{auth::token::sha256_hex, db::DbPool, error::AppError, sites::membership::Role};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Invitation {
    pub id: String,
    pub site_id: String,
    pub email: String,
    pub role: String,
    #[serde(skip_serializing)]
    pub token_hash: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct InviteInput {
    pub email: String,
    pub role: Role,
}

/// Creates an invitation, returning (invitation, raw_token for the mail link).
pub async fn invite(
    db: &DbPool,
    site_id: &str,
    input: &InviteInput,
) -> Result<(Invitation, String), AppError> {
    let raw: String = {
        use rand::Rng;
        rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(48)
            .map(char::from)
            .collect()
    };
    let iid = ulid::Ulid::new().to_string();
    // ponytail: execute (not RETURNING) so the write commits before we read it back
    sqlx::query("INSERT INTO invitations (id, site_id, email, role, token_hash) VALUES (?, ?, ?, ?, ?)")
        .bind(&iid)
        .bind(site_id)
        .bind(input.email.to_lowercase())
        .bind(input.role.as_str())
        .bind(sha256_hex(&raw))
        .execute(db)
        .await?;
    let inv = sqlx::query_as::<_, Invitation>(
        "SELECT id, site_id, email, role, token_hash, created_at FROM invitations WHERE id = ?",
    )
    .bind(&iid)
    .fetch_one(db)
    .await?;
    Ok((inv, raw))
}

pub async fn accept(db: &DbPool, raw_token: &str, user_id: &str) -> Result<String, AppError> {
    let inv: Option<Invitation> =
        sqlx::query_as("SELECT id, site_id, email, role, token_hash, created_at FROM invitations WHERE token_hash = ?")
            .bind(sha256_hex(raw_token))
            .fetch_optional(db)
            .await?;
    let inv = inv.ok_or(AppError::BadRequest("invalid invitation".into()))?;
    let mut tx = db.begin().await?;
    sqlx::query(
        "INSERT INTO memberships (site_id, user_id, role) VALUES (?, ?, ?) ON CONFLICT (site_id, user_id) DO UPDATE SET role = excluded.role",
    )
    .bind(&inv.site_id)
    .bind(user_id)
    .bind(&inv.role)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM invitations WHERE id = ?").bind(&inv.id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(inv.site_id)
}
