//! One-time tokens for email verification and password reset.
//! Raw token goes to the user (mail link); only SHA-256 is stored.

use rand::{distributions::Alphanumeric, Rng};
use sha2::{Digest, Sha256};

use crate::{db::DbPool, error::AppError};

pub enum TokenKind {
    VerifyEmail,
    PasswordReset,
}

impl TokenKind {
    fn table(&self) -> &'static str {
        match self {
            TokenKind::VerifyEmail => "email_verify_tokens",
            TokenKind::PasswordReset => "password_reset_tokens",
        }
    }
    fn ttl(&self) -> &'static str {
        match self {
            TokenKind::VerifyEmail => "+24 hours",
            TokenKind::PasswordReset => "+1 hour",
        }
    }
}

pub fn sha256_hex(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

/// Returns the raw token; stores only its hash.
pub async fn issue(db: &DbPool, kind: TokenKind, user_id: &str) -> Result<String, AppError> {
    let raw: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect();
    let sql = format!(
        "INSERT INTO {} (token_hash, user_id, expires_at) VALUES (?, ?, datetime('now', ?))",
        kind.table()
    );
    sqlx::query(&sql)
        .bind(sha256_hex(&raw))
        .bind(user_id)
        .bind(kind.ttl())
        .execute(db)
        .await?;
    Ok(raw)
}

/// Consumes a token, returning the user_id, or None if invalid/expired.
pub async fn consume(db: &DbPool, kind: TokenKind, raw: &str) -> Result<Option<String>, AppError> {
    let table = kind.table();
    let uid: Option<String> = sqlx::query_scalar(&format!(
        "SELECT user_id FROM {table} WHERE token_hash = ? AND expires_at > datetime('now')"
    ))
    .bind(sha256_hex(raw))
    .fetch_optional(db)
    .await?;
    if let Some(ref u) = uid {
        sqlx::query(&format!("DELETE FROM {table} WHERE token_hash = ?"))
            .bind(sha256_hex(raw))
            .execute(db)
            .await?;
        let _ = u;
    }
    Ok(uid)
}
