//! Scheduled email reports: weekly/monthly site digests.
//! Run via `rust_analytix reports-send` (podman timer) or on boot interval.
//! Each due report aggregates the last period and mails recipients.

use crate::{db::DbPool, error::AppError, state::AppState, stats::runner};

pub async fn due_reports(db: &DbPool) -> Result<Vec<Report>, AppError> {
    sqlx::query_as::<_, Report>(
        "SELECT id, site_id, owner_id, frequency, recipients, last_sent_at, created_at FROM reports
         WHERE (frequency = 'weekly' AND (last_sent_at IS NULL OR last_sent_at <= datetime('now', '-7 days')))
            OR (frequency = 'monthly' AND (last_sent_at IS NULL OR last_sent_at <= datetime('now', '-30 days')))",
    )
    .fetch_all(db)
    .await
    .map_err(Into::into)
}

#[derive(Debug, sqlx::FromRow)]
pub struct Report {
    pub id: String,
    pub site_id: String,
    pub owner_id: String,
    pub frequency: String,
    pub recipients: String,
    pub last_sent_at: Option<String>,
    pub created_at: String,
}

pub async fn send_due(s: &AppState) -> Result<usize, AppError> {
    let reports = due_reports(&s.db).await?;
    let n = reports.len();
    for r in reports {
        if let Err(e) = send_one(s, &r).await {
            tracing::error!(report = %r.id, error = %e, "report failed");
        }
    }
    Ok(n)
}

async fn send_one(s: &AppState, r: &Report) -> Result<(), AppError> {
    let period = if r.frequency == "weekly" { "7d" } else { "30d" };
    let site_id = r.site_id.clone();
    let q = crate::stats::query::V1Params {
        site: site_id.clone(),
        period: period.to_string(),
        date: None,
        from: None,
        to: None,
        filters: String::new(),
        metrics: "visitors,visits,pageviews,bounce_rate".to_string(),
        interval: None,
        property: None,
        limit: 100,
        page: 1,
        compare: true,
        search: String::new(),
    }
    .into_query(site_id, &[])
    .map_err(AppError::BadRequest)?;
    let stats = runner::aggregate(&s.db, &q).await?;
    let domain: String = sqlx::query_scalar("SELECT domain FROM sites WHERE id = ?")
        .bind(&r.site_id)
        .fetch_one(&s.db)
        .await?;
    let recipients: Vec<String> = serde_json::from_str(&r.recipients).unwrap_or_default();
    for to in recipients {
        s.mailer.send_report(&to, &domain, &r.frequency, &stats).await;
    }
    sqlx::query("UPDATE reports SET last_sent_at = datetime('now') WHERE id = ?")
        .bind(&r.id)
        .execute(&s.db)
        .await?;
    Ok(())
}
