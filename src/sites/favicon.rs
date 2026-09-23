//! Favicon fetch + 7-day DB cache. Falls back to the bundled placeholder
//! (upstream priv/site_favicon_placeholder.svg equivalent).

use crate::{db::DbPool, error::AppError};

static PLACEHOLDER_SVG: &str = include_str!("../../public/favicon-placeholder.svg");

pub async fn get(db: &DbPool, http: &reqwest::Client, site_id: &str) -> Result<(String, Vec<u8>), AppError> {
    if let Some((ct, bytes, fetched)) = sqlx::query_as::<_, (String, Vec<u8>, String)>(
        "SELECT content_type, bytes, fetched_at FROM favicons WHERE site_id = ?",
    )
    .bind(site_id)
    .fetch_optional(db)
    .await?
    {
        if fetched > seven_days_ago() {
            return Ok((ct, bytes));
        }
    }
    let domain: String = sqlx::query_scalar("SELECT domain FROM sites WHERE id = ?")
        .bind(site_id)
        .fetch_one(db)
        .await
        .map_err(|_| AppError::BadRequest("site not found".into()))?;
    let (ct, bytes) = fetch_for(http, &domain).await.unwrap_or_else(|| ("image/svg+xml".to_string(), PLACEHOLDER_SVG.as_bytes().to_vec()));
    sqlx::query(
        "INSERT INTO favicons (site_id, bytes, content_type) VALUES (?, ?, ?) ON CONFLICT (site_id) DO UPDATE SET bytes = excluded.bytes, content_type = excluded.content_type, fetched_at = datetime('now')",
    )
    .bind(site_id)
    .bind(&bytes)
    .bind(&ct)
    .execute(db)
    .await?;
    Ok((ct, bytes))
}

fn seven_days_ago() -> String {
    (chrono::Utc::now() - chrono::Duration::days(7)).format("%Y-%m-%d %H:%M:%S").to_string()
}

async fn fetch_for(http: &reqwest::Client, domain: &str) -> Option<(String, Vec<u8>)> {
    let html = http
        .get(format!("https://{domain}"))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    let href = icon_href(&html)?;
    let url = if href.starts_with("http") {
        href
    } else if href.starts_with("//") {
        format!("https:{href}")
    } else if href.starts_with('/') {
        format!("https://{domain}{href}")
    } else {
        return None;
    };
    let resp = http.get(url).timeout(std::time::Duration::from_secs(5)).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let ct = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("image/x-icon").to_string();
    let bytes = resp.bytes().await.ok()?.to_vec();
    if bytes.is_empty() || bytes.len() > 256 * 1024 {
        return None;
    }
    Some((ct, bytes))
}

/// First <link rel="...icon..." href="..."> in the page.
fn icon_href(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let mut pos = 0;
    while let Some(i) = lower[pos..].find("<link") {
        let tag_start = pos + i;
        let tag_end = lower[tag_start..].find('>').map(|e| tag_start + e)?;
        let tag = &lower[tag_start..tag_end];
        if tag.contains("icon") {
            if let Some(h) = attr(&html[tag_start..tag_end], "href") {
                return Some(h);
            }
        }
        pos = tag_end + 1;
    }
    None
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let key = format!("{name}=");
    let i = lower.find(&key)?;
    let rest = tag[i + key.len()..].trim_start();
    let q = rest.chars().next()?;
    if q == '"' || q == '\'' {
        rest[1..].find(q).map(|e| rest[1..1 + e].to_string())
    } else {
        Some(rest.split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or("").to_string())
    }
}
