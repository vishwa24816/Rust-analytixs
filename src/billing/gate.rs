//! Entitlement gates: trial/subscription status, site + member limits,
//! pageview volume, feature access, site locking.

use crate::{db::DbPool, error::AppError};

#[derive(Debug, sqlx::FromRow)]
pub struct Subscription {
    pub user_id: String,
    pub paddle_subscription_id: String,
    pub plan_kind: String,
    pub plan_generation: i64,
    pub monthly_pageview_limit: i64,
    pub site_limit: i64,
    pub team_member_limit: i64,
    pub status: String,
    pub trial_expiry: String,
    pub next_bill_date: Option<String>,
    pub updated_at: String,
}

/// Subscription row, creating a 30-day trial on first use.
pub async fn subscription_for(db: &DbPool, user_id: &str) -> Result<Subscription, AppError> {
    // ponytail: execute (not RETURNING) so the write commits before we read it back
    sqlx::query("INSERT INTO subscriptions (user_id) VALUES (?) ON CONFLICT (user_id) DO NOTHING")
        .bind(user_id)
        .execute(db)
        .await?;
    sqlx::query_as(
        "SELECT user_id, paddle_subscription_id, plan_kind, plan_generation, monthly_pageview_limit, site_limit, team_member_limit, status, trial_expiry, next_bill_date, updated_at FROM subscriptions WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(Into::into)
}

pub fn entitled(sub: &Subscription, now: &str) -> bool {
    match sub.status.as_str() {
        "active" => true,
        "trial" => sub.trial_expiry.as_str() > now,
        _ => false,
    }
}

pub fn now_ts() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Last-30d pageview volume across all sites owned by the user.
pub async fn monthly_volume(db: &DbPool, user_id: &str) -> Result<i64, AppError> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM events e JOIN memberships m ON m.site_id = e.site_id
         WHERE m.user_id = ? AND m.role = 'owner' AND e.name = 'pageview' AND e.ts >= datetime('now', '-30 days')",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(Into::into)
}

pub async fn site_count(db: &DbPool, user_id: &str) -> Result<i64, AppError> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM memberships WHERE user_id = ? AND role = 'owner'",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(Into::into)
}

/// A site is locked when none of its owners is entitled, or the owner's
/// volume exceeds their limit. Returns the lock reason, if any.
pub async fn lock_reason(db: &DbPool, site_id: &str) -> Result<Option<String>, AppError> {
    let owners: Vec<String> =
        sqlx::query_scalar("SELECT user_id FROM memberships WHERE site_id = ? AND role = 'owner'")
            .bind(site_id)
            .fetch_all(db)
            .await?;
    if owners.is_empty() {
        return Ok(Some("no owner".into()));
    }
    let now = now_ts();
    for owner in owners {
        let sub = subscription_for(db, &owner).await?;
        if !entitled(&sub, &now) {
            continue;
        }
        let vol = monthly_volume(db, &owner).await?;
        if vol > sub.monthly_pageview_limit {
            continue;
        }
        return Ok(None);
    }
    Ok(Some("subscription required".into()))
}

pub async fn check_site_create(db: &DbPool, user_id: &str) -> Result<(), AppError> {
    let sub = subscription_for(db, user_id).await?;
    if !entitled(&sub, &now_ts()) {
        return Err(AppError::Payment("trial expired: subscribe to add sites".into()));
    }
    if site_count(db, user_id).await? >= sub.site_limit {
        return Err(AppError::Payment("site limit reached for your plan".into()));
    }
    Ok(())
}
