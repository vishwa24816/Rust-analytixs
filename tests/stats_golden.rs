//! Golden tests for the P4 stats engine: seed via the real ingest writer,
//! assert aggregate/breakdown/timeseries/realtime/compare.

use rust_analytix::{
    db::DbPool,
    ingest::{
        event_dto::RawEvent,
        writer::{self, Incoming},
    },
    stats::{
        interval::Interval,
        metrics::Metric,
        period,
        query::Query,
        runner,
    },
};

async fn test_db() -> DbPool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    sqlx::query("INSERT INTO sites (id, domain) VALUES ('s1', 'example.com')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id, email, password_hash) VALUES ('u1', 't@t.co', 'x')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO memberships (site_id, user_id, role) VALUES ('s1', 'u1', 'owner')")
        .execute(&pool)
        .await
        .unwrap();
    pool
}

async fn seed(db: &DbPool, ua: &str, ip: &str, body: &str) {
    let raw: RawEvent = serde_json::from_str(body).unwrap();
    let event = raw.normalize().unwrap();
    writer::write(
        db,
        Incoming { event, user_agent: ua.into(), ip: ip.into() },
    )
    .await
    .unwrap();
}

fn pv(url: &str) -> String {
    format!(r#"{{"n":"pageview","u":"{url}","d":"example.com"}}"#)
}

const CHROME: &str = "Mozilla/5.0 (Windows NT 10.0) Chrome/120";
const FF: &str = "Mozilla/5.0 (X11; Linux x86_64) Firefox/121";

fn day_query(metrics: &[&str]) -> Query {
    let today = chrono::Utc::now().date_naive();
    Query {
        site_id: "s1".into(),
        range: period::Range { start: today, end: today },
        interval: Interval::Day,
        filters: vec![],
        metrics: metrics.iter().map(|m| Metric::parse(m).unwrap()).collect(),
        dimension: None,
        limit: 100,
        page: 1,
        compare: false,
        realtime: false,
        search: String::new(),
    }
}

#[tokio::test]
async fn golden_aggregate() {
    let db = test_db().await;
    seed(&db, CHROME, "10.0.0.1", &pv("https://example.com/")).await;
    seed(&db, CHROME, "10.0.0.1", &pv("https://example.com/blog")).await;
    seed(&db, FF, "10.0.0.2", &pv("https://example.com/")).await;

    let q = day_query(&["visitors", "visits", "pageviews", "bounce_rate", "views_per_visit"]);
    let v = runner::aggregate(&db, &q).await.unwrap();
    assert_eq!(v["visitors"], 2);
    assert_eq!(v["visits"], 2);
    assert_eq!(v["pageviews"], 3);
    assert_eq!(v["bounce_rate"], 0.5);
    assert_eq!(v["views_per_visit"], 1.5);
}

#[tokio::test]
async fn golden_breakdown_and_realtime() {
    let db = test_db().await;
    seed(&db, CHROME, "10.0.0.1", &pv("https://example.com/")).await;
    seed(&db, FF, "10.0.0.2", &pv("https://example.com/blog")).await;

    let mut q = day_query(&["visitors"]);
    q.dimension = Some("visit:browser".into());
    let v = runner::breakdown(&db, &q).await.unwrap();
    assert_eq!(v["results"].as_array().unwrap().len(), 2);

    assert_eq!(runner::realtime(&db, "s1").await.unwrap(), 2);
}

#[tokio::test]
async fn golden_timeseries_and_compare() {
    let db = test_db().await;
    seed(&db, CHROME, "10.0.0.1", &pv("https://example.com/")).await;

    let q = day_query(&["visitors"]);
    let v = runner::timeseries(&db, &q).await.unwrap();
    assert_eq!(v["results"].as_array().unwrap().len(), 1);

    let mut q2 = day_query(&["visitors"]);
    q2.compare = true;
    let v2 = runner::aggregate(&db, &q2).await.unwrap();
    assert!(v2.get("comparison").is_some());
}
