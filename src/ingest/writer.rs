//! Single-txn writer: shield check → site lookup → insert event →
//! upsert sessions_mat. No IP is ever stored (privacy by design).

use crate::{
    db::DbPool,
    error::AppError,
    ingest::{
        event_dto::Event,
        sessionize,
        ua,
    },
};

pub struct Incoming {
    pub event: Event,
    pub user_agent: String,
    pub ip: String,
}

pub async fn write(db: &DbPool, inc: Incoming) -> Result<(), AppError> {
    let site_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM sites WHERE domain = ?")
            .bind(&inc.event.domain)
            .fetch_optional(db)
            .await?;
    let site_id = match site_id {
        Some(id) => id,
        None => return Err(AppError::Dropped("unknown domain".into())),
    };

    if let Some(reason) = crate::billing::gate::lock_reason(db, &site_id).await? {
        return Err(AppError::Payment(format!("site locked: {reason}")));
    }

    if shielded(db, &site_id, &inc).await? {
        return Err(AppError::Dropped("shielded".into()));
    }

    let client = ua::parse(&inc.user_agent);
    let day = sessionize::today_utc();
    let session_id = sessionize::session_id(&inc.ip, &inc.user_agent, &inc.event.domain, &day);
    let referrer_source = ref_source(&inc.event.referrer);
    let props_json = serde_json::to_string(&inc.event.props).unwrap_or_else(|_| "{}".to_string());
    let is_pageview = inc.event.name == "pageview";

    let mut tx = db.begin().await?;
    sqlx::query(
        "INSERT INTO events (id, site_id, session_id, name, pathname, hostname, referrer, referrer_source, device, browser, os, utm_source, utm_medium, utm_campaign, utm_content, utm_term, props, revenue_cents, scroll_depth, engagement_ms, interactive)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(&site_id)
    .bind(&session_id)
    .bind(&inc.event.name)
    .bind(&inc.event.pathname)
    .bind(&inc.event.hostname)
    .bind(&inc.event.referrer)
    .bind(&referrer_source)
    .bind(client.device)
    .bind(client.browser)
    .bind(client.os)
    .bind(inc.event.query.get("utm_source").map(String::as_str).unwrap_or(""))
    .bind(inc.event.query.get("utm_medium").map(String::as_str).unwrap_or(""))
    .bind(inc.event.query.get("utm_campaign").map(String::as_str).unwrap_or(""))
    .bind(inc.event.query.get("utm_content").map(String::as_str).unwrap_or(""))
    .bind(inc.event.query.get("utm_term").map(String::as_str).unwrap_or(""))
    .bind(&props_json)
    .bind(inc.event.revenue_cents)
    .bind(inc.event.scroll_depth)
    .bind(inc.event.engagement_ms)
    .bind(inc.event.interactive)
    .execute(&mut *tx)
    .await?;

    // ponytail: one UPSERT keeps the session row fresh; bounce flips on 2nd pageview
    sqlx::query(
        "INSERT INTO sessions_mat (session_id, site_id, started_at, last_at, pageviews, events_count, is_bounce, entry_page, exit_page, referrer, device, browser, os)
         VALUES (?, ?, datetime('now'), datetime('now'), ?, 1, 1, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (site_id, session_id) DO UPDATE SET
           last_at = datetime('now'),
           pageviews = pageviews + ?,
           events_count = events_count + 1,
           is_bounce = CASE WHEN pageviews + ? >= 2 THEN 0 ELSE is_bounce END,
           exit_page = ?",
    )
    .bind(&session_id)
    .bind(&site_id)
    .bind(if is_pageview { 1 } else { 0 })
    .bind(&inc.event.pathname)
    .bind(&inc.event.pathname)
    .bind(&inc.event.referrer)
    .bind(client.device)
    .bind(client.browser)
    .bind(client.os)
    .bind(if is_pageview { 1 } else { 0 })
    .bind(if is_pageview { 1 } else { 0 })
    .bind(&inc.event.pathname)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn shielded(db: &DbPool, site_id: &str, inc: &Incoming) -> Result<bool, AppError> {
    let rules: Vec<(String, String)> =
        sqlx::query_as("SELECT kind, value FROM shield_rules WHERE site_id = ?")
            .bind(site_id)
            .fetch_all(db)
            .await?;
    if rules.is_empty() {
        return Ok(false);
    }
    for (kind, value) in rules {
        let hit = match kind.as_str() {
            "page" => inc.event.pathname == value,
            "hostname" => inc.event.hostname == value,
            "country" => false, // no geo DB in P3; rule stored, never matches
            "ip" => inc.ip == value,
            _ => false,
        };
        if hit {
            return Ok(true);
        }
    }
    Ok(false)
}

fn ref_source(referrer: &str) -> String {
    if referrer.is_empty() {
        return "Direct".to_string();
    }
    url::Url::parse(referrer)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default()
}
