//! CSV event import. Header:
//! timestamp,name,pathname,hostname[,referrer,browser,os,device,country,props]
//! timestamp = "YYYY-MM-DD HH:MM:SS" (UTC). Props = flat JSON object string.
//! Returns inserted count. One txn per 500 rows.

use crate::{db::DbPool, error::AppError};

pub async fn run(db: &DbPool, site_id: &str, text: &str) -> Result<i64, AppError> {
    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(text.as_bytes());
    let headers = rdr.headers().map_err(|e| AppError::BadRequest(format!("bad csv: {e}")))?.clone();
    let col = |name: &str| headers.iter().position(|h| h == name);
    let (c_ts, c_name, c_path, c_host) = match (col("timestamp"), col("name"), col("pathname"), col("hostname")) {
        (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
        _ => return Err(AppError::BadRequest("csv needs header: timestamp,name,pathname,hostname".into())),
    };
    let o_ref = col("referrer");
    let o_br = col("browser");
    let o_os = col("os");
    let o_dev = col("device");
    let o_cty = col("country");
    let o_props = col("props");

    let mut total = 0i64;
    let mut batch: Vec<Vec<String>> = vec![];
    for rec in rdr.records() {
        let rec = rec.map_err(|e| AppError::BadRequest(format!("bad csv row: {e}")))?;
        let get = |i: usize| rec.get(i).unwrap_or("").to_string();
        let opt = |o: Option<usize>| o.map(&get).unwrap_or_default();
        // validate timestamp + name early
        if chrono::NaiveDateTime::parse_from_str(&get(c_ts), "%Y-%m-%d %H:%M:%S").is_err() {
            return Err(AppError::BadRequest(format!("bad timestamp '{}'", get(c_ts))));
        }
        if get(c_name).is_empty() || get(c_name).len() > 120 {
            return Err(AppError::BadRequest("bad event name".into()));
        }
        let props = match o_props {
            Some(i) if !get(i).is_empty() => {
                serde_json::from_str::<serde_json::Value>(&get(i))
                    .map(|v| v.to_string())
                    .unwrap_or_else(|_| "{}".to_string())
            }
            _ => "{}".to_string(),
        };
        batch.push(vec![
            get(c_ts), get(c_name), get(c_path), get(c_host),
            opt(o_ref), opt(o_br), opt(o_os), opt(o_dev), opt(o_cty), props,
        ]);
        if batch.len() >= 500 {
            total += insert_batch(db, site_id, &batch).await?;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        total += insert_batch(db, site_id, &batch).await?;
    }
    Ok(total)
}

async fn insert_batch(db: &DbPool, site_id: &str, batch: &[Vec<String>]) -> Result<i64, AppError> {
    let mut tx = db.begin().await?;
    for r in batch {
        // imported rows share a synthetic session per (day, pathname) — keeps
        // sessions_mat coherent without inventing visitors
        let session_id = format!("import:{}:{}", &r[0][..10], r[2]);
        sqlx::query(
            "INSERT INTO events (id, site_id, ts, session_id, name, pathname, hostname, referrer, browser, os, device, country, props)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(ulid::Ulid::new().to_string())
        .bind(site_id)
        .bind(&r[0])
        .bind(&session_id)
        .bind(&r[1])
        .bind(&r[2])
        .bind(&r[3])
        .bind(&r[4])
        .bind(if r[5].is_empty() { "imported" } else { &r[5] })
        .bind(&r[6])
        .bind(if r[7].is_empty() { "Desktop" } else { &r[7] })
        .bind(if r[8].is_empty() { None } else { Some(r[8].clone()) })
        .bind(&r[9])
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO sessions_mat (session_id, site_id, started_at, last_at, pageviews, events_count, is_bounce, entry_page, exit_page)
             VALUES (?, ?, ?, ?, 1, 1, 1, ?, ?)
             ON CONFLICT (site_id, session_id) DO UPDATE SET last_at = excluded.last_at, pageviews = pageviews + 1, events_count = events_count + 1, exit_page = excluded.exit_page",
        )
        .bind(&session_id)
        .bind(site_id)
        .bind(&r[0])
        .bind(&r[0])
        .bind(&r[2])
        .bind(&r[2])
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(batch.len() as i64)
}
