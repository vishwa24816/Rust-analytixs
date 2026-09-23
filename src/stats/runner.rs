//! Executors: aggregate, timeseries, breakdown, realtime, compare, CSV.

use sqlx::Row;

use crate::{db::DbPool, error::AppError, stats::query::Query};

fn bind_all<'q>(sql: &'q str, args: &'q [String]) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    let mut q = sqlx::query(sql);
    for a in args {
        q = q.bind(a);
    }
    q
}

fn cell(row: &sqlx::sqlite::SqliteRow, col: &str) -> serde_json::Value {
    if let Ok(v) = row.try_get::<i64, _>(col) {
        return serde_json::json!(v);
    }
    if let Ok(v) = row.try_get::<f64, _>(col) {
        return serde_json::json!(v);
    }
    if let Ok(v) = row.try_get::<String, _>(col) {
        return serde_json::json!(v);
    }
    serde_json::Value::Null
}

async fn fetch_metrics(
    db: &DbPool,
    sql: &str,
    args: &[String],
    metrics: &[String],
) -> Result<serde_json::Map<String, serde_json::Value>, AppError> {
    let rows = bind_all(sql, args).fetch_all(db).await?;
    let mut out = serde_json::Map::new();
    if let Some(row) = rows.first() {
        for m in metrics {
            out.insert(m.clone(), cell(row, m));
        }
    } else {
        for m in metrics {
            out.insert(m.clone(), serde_json::json!(0));
        }
    }
    Ok(out)
}

fn metric_names(q: &Query) -> Vec<String> {
    q.metrics.iter().map(|m| m.name().to_string()).collect()
}

pub async fn aggregate(db: &DbPool, q: &Query) -> Result<serde_json::Value, AppError> {
    let (sql, args) = q.aggregate_sql(&q.range);
    let names = metric_names(q);
    let mut results = fetch_metrics(db, &sql, &args, &names).await?;
    if q.compare {
        let prev = q.range.previous();
        let (psql, pargs) = q.aggregate_sql(&prev);
        let presults = fetch_metrics(db, &psql, &pargs, &names).await?;
        let mut comp = serde_json::Map::new();
        for m in &names {
            comp.insert(m.clone(), change(results.get(m), presults.get(m)));
        }
        results.insert("comparison".to_string(), serde_json::Value::Object(comp));
    }
    Ok(serde_json::Value::Object(results))
}

fn change(cur: Option<&serde_json::Value>, prev: Option<&serde_json::Value>) -> serde_json::Value {
    let num = |v: Option<&serde_json::Value>| v.and_then(|x| x.as_f64());
    match (num(cur), num(prev)) {
        (_, Some(0.0)) | (_, None) => serde_json::Value::Null,
        (Some(c), Some(p)) => serde_json::json!(((c - p) / p * 100.0).round()),
        _ => serde_json::Value::Null,
    }
}

pub async fn timeseries(db: &DbPool, q: &Query) -> Result<serde_json::Value, AppError> {
    let (sql, args) = q.timeseries_sql(&q.range);
    let names = metric_names(q);
    let rows = bind_all(&sql, &args).fetch_all(db).await?;
    let mut out = vec![];
    for row in rows {
        let mut obj = serde_json::Map::new();
        obj.insert("date".to_string(), cell(&row, "date"));
        for m in &names {
            obj.insert(m.clone(), cell(&row, m));
        }
        out.push(serde_json::Value::Object(obj));
    }
    Ok(serde_json::json!({"results": out}))
}

pub async fn breakdown(db: &DbPool, q: &Query) -> Result<serde_json::Value, AppError> {
    let (sql, args) = q.breakdown_sql(&q.range);
    let names = metric_names(q);
    let rows = bind_all(&sql, &args).fetch_all(db).await?;
    let mut out = vec![];
    for row in rows.iter().take(q.limit as usize) {
        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), cell(&row, "dim"));
        for m in &names {
            obj.insert(m.clone(), cell(&row, m));
        }
        out.push(serde_json::Value::Object(obj));
    }
    Ok(serde_json::json!({"results": out}))
}

pub async fn realtime(db: &DbPool, site_id: &str) -> Result<serde_json::Value, AppError> {    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT session_id) FROM events WHERE site_id = ? AND ts >= datetime('now', '-5 minutes')",
    )
    .bind(site_id)
    .fetch_one(db)
    .await?;
    Ok(serde_json::json!(n))
}

pub async fn breakdown_csv(db: &DbPool, q: &Query) -> Result<String, AppError> {
    let (sql, args) = q.breakdown_sql(&q.range);
    let names = metric_names(q);
    let rows = bind_all(&sql, &args).fetch_all(db).await?;
    let mut w = csv::WriterBuilder::new().from_writer(vec![]);
    let mut header = vec!["name".to_string()];
    header.extend(names.clone());
    w.write_record(&header).map_err(|e| AppError::BadRequest(e.to_string()))?;
    for row in rows.iter().take(q.limit as usize) {
        let mut rec = vec![cell(&row, "dim").as_str().unwrap_or("").to_string()];
        for m in &names {
            let c = cell(&row, m);
            rec.push(c.as_str().map(str::to_string).unwrap_or_else(|| c.to_string()));
        }
        w.write_record(&rec).map_err(|e| AppError::BadRequest(e.to_string()))?;
    }
    String::from_utf8(w.into_inner().map_err(|e| AppError::BadRequest(e.to_string()))?)
        .map_err(|e| AppError::BadRequest(e.to_string()))
}

/// Ordered funnel: sessions completing step k must have completed k-1 first.
/// Returns [{step, visitors, dropoff_rate}].
pub async fn funnel(
    db: &DbPool,
    site_id: &str,
    range: &crate::stats::period::Range,
    steps: &[crate::sites::goals::Goal],
) -> Result<serde_json::Value, AppError> {
    // (session, earliest ts of previous step) carried forward
    let mut current: Vec<(String, String)> = vec![];
    let mut out = vec![];
    for (i, goal) in steps.iter().enumerate() {
        let pred = goal.predicate();
        let step_n: i64 = if i == 0 {
            sqlx::query_scalar(&format!(
                "SELECT COUNT(DISTINCT e.session_id) FROM events e WHERE e.site_id = ? AND e.ts >= ? AND e.ts <= ? AND {pred}"
            ))
            .bind(site_id)
            .bind(range.start_ts())
            .bind(range.end_ts())
            .fetch_one(db)
            .await?
        } else {
            // sessions from previous step with this step's event after their prev-step ts
            let mut n = 0i64;
            let mut next: Vec<(String, String)> = vec![];
            for (sess, after) in &current {
                let ts: Option<String> = sqlx::query_scalar(&format!(
                    "SELECT MIN(e.ts) FROM events e WHERE e.site_id = ? AND e.session_id = ? AND e.ts > ? AND {pred}"
                ))
                .bind(site_id)
                .bind(sess)
                .bind(after)
                .fetch_optional(db)
                .await?
                .flatten();
                if let Some(t) = ts {
                    n += 1;
                    next.push((sess.clone(), t));
                }
            }
            current = next;
            n
        };
        if i == 0 {
            // capture entry timestamps for step 2 chaining
            let rows: Vec<(String, String)> = sqlx::query_as(&format!(
                "SELECT e.session_id, MIN(e.ts) FROM events e WHERE e.site_id = ? AND e.ts >= ? AND e.ts <= ? AND {pred} GROUP BY e.session_id"
            ))
            .bind(site_id)
            .bind(range.start_ts())
            .bind(range.end_ts())
            .fetch_all(db)
            .await?;
            current = rows;
        }
        let prev = out.last().and_then(|s: &serde_json::Value| s["visitors"].as_i64()).unwrap_or(step_n);
        let dropoff = if prev > 0 { 1.0 - step_n as f64 / prev as f64 } else { 0.0 };
        out.push(serde_json::json!({"step": goal.event_name, "visitors": step_n, "dropoff_rate": dropoff}));
    }
    Ok(serde_json::json!({"results": out}))
}

/// Journey: top next-pages after the given entry page.
pub async fn journey(
    db: &DbPool,
    site_id: &str,
    range: &crate::stats::period::Range,
    entry: &str,
    limit: i64,
) -> Result<serde_json::Value, AppError> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT e2.pathname AS next, COUNT(DISTINCT e2.session_id) AS visitors
         FROM events e JOIN events e2
           ON e2.site_id = e.site_id AND e2.session_id = e.session_id
           AND e2.name = 'pageview' AND e2.ts > e.ts
         WHERE e.site_id = ? AND e.name = 'pageview' AND e.pathname = ?
           AND e.ts >= ? AND e.ts <= ?
         GROUP BY next ORDER BY visitors DESC LIMIT ?",
    )
    .bind(site_id)
    .bind(entry)
    .bind(range.start_ts())
    .bind(range.end_ts())
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(serde_json::json!({"entry": entry, "results": rows.iter().map(|(p, v)| serde_json::json!({"path": p, "visitors": v})).collect::<Vec<_>>()}))
}
