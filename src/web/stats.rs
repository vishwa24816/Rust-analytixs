use axum::{
    body::Bytes,
    extract::{Query as AxQuery, State},
    http::{header, HeaderMap},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::{
    auth::{api_key, session},
    error::AppError,
    sites::membership,
    state::AppState,
    stats::{
        filters,
        query::{Query, V1Params},
        runner,
    },
    web::middleware_auth::token_from,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/stats/realtime/visitors", get(v1_realtime))
        .route("/api/v1/stats/aggregate", get(v1_aggregate))
        .route("/api/v1/stats/breakdown", get(v1_breakdown))
        .route("/api/v1/stats/timeseries", get(v1_timeseries))
        .route("/api/v1/stats/csv", get(v1_csv))
        .route("/api/v2/query", post(v2_query))
        .route("/api/stats/:domain/query", post(v2_query_domain))
        .route("/api/sites/:sid/suggestions", get(suggestions))
}

/// Anonymous access allowed only for public sites (embed/share).
async fn site_id_for_opt(
    s: &AppState,
    user_id: Option<&str>,
    site: &str,
) -> Result<String, AppError> {
    let row: Option<(String, bool)> = sqlx::query_as(
        "SELECT id, public FROM sites WHERE id = ? OR domain = ?",
    )
    .bind(site)
    .bind(site.to_lowercase())
    .fetch_optional(&s.db)
    .await?;
    match row {
        Some((sid, public)) => {
            if public {
                return Ok(sid);
            }
            match user_id {
                Some(uid) => {
                    membership::require(&s.db, &sid, uid, membership::Role::Viewer).await?;
                    Ok(sid)
                }
                None => Err(AppError::Auth("login required".into())),
            }
        }
        None => Err(AppError::BadRequest("site not found".into())),
    }
}

#[derive(Deserialize, Default)]
pub(crate) struct V1Query {
    #[serde(default)]
    pub(crate) site_id: String,
    #[serde(default = "day")]
    pub(crate) period: String,
    #[serde(default)]
    pub(crate) date: Option<String>,
    #[serde(default)]
    pub(crate) from: Option<String>,
    #[serde(default)]
    pub(crate) to: Option<String>,
    #[serde(default)]
    pub(crate) filters: String,
    #[serde(default)]
    pub(crate) metrics: String,
    #[serde(default)]
    pub(crate) interval: Option<String>,
    #[serde(default)]
    pub(crate) property: Option<String>,
    #[serde(default = "limit100")]
    pub(crate) limit: String,
    #[serde(default = "page1")]
    pub(crate) page: String,
    #[serde(default)]
    pub(crate) compare: Option<String>,
    #[serde(default)]
    pub(crate) search: String,
}

fn day() -> String {
    "day".to_string()
}
fn limit100() -> String {
    "100".to_string()
}
fn page1() -> String {
    "1".to_string()
}

pub(crate) fn into_params(q: V1Query, defaults: &[&str]) -> Result<(V1Params, bool), String> {
    let limit = q.limit.parse::<i64>().map_err(|_| "limit must be a number".to_string())?;
    let page = q.page.parse::<i64>().map_err(|_| "page must be a number".to_string())?;
    Ok((
        V1Params {
            site: q.site_id,
            period: q.period,
            date: q.date,
            from: q.from,
            to: q.to,
            filters: q.filters,
            metrics: q.metrics,
            interval: q.interval,
            property: q.property,
            limit,
            page,
            compare: q.compare.as_deref() == Some("previous_period"),
            search: q.search,
        },
        defaults.is_empty(),
    ))
}

/// Build a Query for an already-authorized site (shared links, plugins).
pub(crate) fn query_for_site(sid: String, p: V1Params, defaults: &[&str]) -> Result<Query, AppError> {
    p.into_query(sid, defaults).map_err(AppError::BadRequest)
}

async fn build_query(s: &AppState, headers: &HeaderMap, p: V1Params, defaults: &[&str]) -> Result<Query, AppError> {
    let uid = requester_opt(s, headers).await?;
    let sid = site_id_for_opt(s, uid.as_deref(), &p.site).await?;
    p.into_query(sid, defaults).map_err(AppError::BadRequest)
}

/// None when no/invalid credentials — public sites still resolve.
async fn requester_opt(s: &AppState, headers: &HeaderMap) -> Result<Option<String>, AppError> {
    let Some(token) = token_from(headers) else {
        return Ok(None);
    };
    if token.starts_with("plausible_") {
        Ok(api_key::verify(&s.db, &token).await?.map(|k| k.user_id))
    } else {
        Ok(session::user_id(&s.db, &token).await?)
    }
}

async fn v1_realtime(
    State(s): State<AppState>,
    headers: HeaderMap,
    AxQuery(q): AxQuery<V1Query>,
) -> Result<impl IntoResponse, AppError> {
    let uid = requester_opt(&s, &headers).await?;
    let sid = site_id_for_opt(&s, uid.as_deref(), &q.site_id).await?;
    Ok(Json(runner::realtime(&s.db, &sid).await?))
}

async fn v1_aggregate(
    State(s): State<AppState>,
    headers: HeaderMap,
    AxQuery(q): AxQuery<V1Query>,
) -> Result<impl IntoResponse, AppError> {
    let (p, _) = into_params(q, &["visitors"]).map_err(AppError::BadRequest)?;
    let query = build_query(&s, &headers, p, &["visitors"]).await?;
    Ok(Json(runner::aggregate(&s.db, &query).await?))
}

async fn v1_breakdown(
    State(s): State<AppState>,
    headers: HeaderMap,
    AxQuery(q): AxQuery<V1Query>,
) -> Result<impl IntoResponse, AppError> {
    if q.property.as_deref().unwrap_or("").is_empty() {
        return Err(AppError::BadRequest("property is required".into()));
    }
    let (p, _) = into_params(q, &["visitors"]).map_err(AppError::BadRequest)?;
    let query = build_query(&s, &headers, p, &["visitors"]).await?;
    Ok(Json(runner::breakdown(&s.db, &query).await?))
}

async fn v1_timeseries(
    State(s): State<AppState>,
    headers: HeaderMap,
    AxQuery(q): AxQuery<V1Query>,
) -> Result<impl IntoResponse, AppError> {
    let (p, _) = into_params(q, &["visitors"]).map_err(AppError::BadRequest)?;
    let query = build_query(&s, &headers, p, &["visitors"]).await?;
    Ok(Json(runner::timeseries(&s.db, &query).await?))
}

async fn v1_csv(
    State(s): State<AppState>,
    headers: HeaderMap,
    AxQuery(q): AxQuery<V1Query>,
) -> Result<impl IntoResponse, AppError> {
    if q.property.as_deref().unwrap_or("").is_empty() {
        return Err(AppError::BadRequest("property is required".into()));
    }
    let (p, _) = into_params(q, &["visitors"]).map_err(AppError::BadRequest)?;
    let query = build_query(&s, &headers, p, &["visitors"]).await?;
    let csv = runner::breakdown_csv(&s.db, &query).await?;
    Ok(([(header::CONTENT_TYPE, "text/csv")], csv))
}

#[derive(Deserialize)]
struct V2Body {
    site_id: String,
    #[serde(default = "day")]
    period: String,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    metrics: Vec<String>,
    #[serde(default)]
    dimensions: Vec<String>,
    #[serde(default)]
    filters: serde_json::Value,
    #[serde(default)]
    order_by: Vec<Vec<String>>,
    #[serde(default = "limit10k")]
    limit: i64,
}

fn limit10k() -> i64 {
    10_000
}

async fn v2_query(
    State(s): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    v2_query_inner(&s, &headers, None, body).await
}

/// Upstream dashboard path: site comes from the URL, not the body.
async fn v2_query_domain(
    State(s): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(domain): axum::extract::Path<String>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    v2_query_inner(&s, &headers, Some(domain), body).await
}

async fn v2_query_inner(
    s: &AppState,
    headers: &HeaderMap,
    domain: Option<String>,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    // ponytail: manual parse so text/plain dashboards keep working
    let mut b: V2Body = serde_json::from_slice(&body).map_err(|e| AppError::BadRequest(e.to_string()))?;
    if let Some(d) = domain {
        b.site_id = d;
    }
    let uid = requester_opt(&s, &headers).await?;
    let sid = site_id_for_opt(&s, uid.as_deref(), &b.site_id).await?;
    let metrics = if b.metrics.is_empty() {
        vec![crate::stats::metrics::Metric::Visitors]
    } else {
        b.metrics.iter().map(|m| crate::stats::metrics::Metric::parse(m)).collect::<Result<Vec<_>, _>>().map_err(AppError::BadRequest)?
    };
    let filters = if b.filters.is_null() {
        vec![]
    } else {
        filters::parse_v2(&b.filters).map_err(AppError::BadRequest)?
    };
    let realtime = b.period == "realtime";
    let eff_period = if realtime { "day" } else { b.period.as_str() };
    let range = crate::stats::period::resolve(eff_period, b.date.as_deref(), b.from.as_deref(), b.to.as_deref()).map_err(AppError::BadRequest)?;

    if b.dimensions.len() > 1 {
        return Err(AppError::BadRequest("one dimension per query".into()));
    }
    let q = Query {
        site_id: sid,
        range,
        interval: crate::stats::interval::Interval::default_for(&b.period),
        filters,
        metrics,
        dimension: b.dimensions.first().cloned(),
        limit: b.limit.min(10_000).max(1),
        page: 1,
        compare: false,
        realtime,
        search: String::new(),
    };
    if q.dimension.is_some() {
        Ok(Json(runner::breakdown(&s.db, &q).await?).into_response())
    } else {
        Ok(Json(runner::aggregate(&s.db, &q).await?).into_response())
    }
}

#[derive(Deserialize, Default)]
struct SuggestQ {
    #[serde(default)]
    dimension: String,
    #[serde(default, rename = "q")]
    query: String,
}

/// Filter value suggestions for the filter modal (top values, optional match).
async fn suggestions(
    State(s): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(sid_or_domain): axum::extract::Path<String>,
    axum::extract::Query(q): axum::extract::Query<SuggestQ>,
) -> Result<impl IntoResponse, AppError> {
    let uid = requester_opt(&s, &headers).await?;
    let sid = site_id_for_opt(&s, uid.as_deref(), &sid_or_domain).await?;
    let col = filters::dimension_sql(&q.dimension).map_err(AppError::BadRequest)?;
    let rows: Vec<(String, i64)> = if q.query.is_empty() {
        sqlx::query_as(&format!(
            "SELECT {col} AS v, COUNT(*) AS n FROM events e WHERE e.site_id = ? AND {col} IS NOT NULL AND {col} != '' GROUP BY v ORDER BY n DESC LIMIT 20"
        ))
        .bind(&sid)
        .fetch_all(&s.db)
        .await?
    } else {
        sqlx::query_as(&format!(
            "SELECT {col} AS v, COUNT(*) AS n FROM events e WHERE e.site_id = ? AND {col} LIKE '%' || ? || '%' GROUP BY v ORDER BY n DESC LIMIT 20"
        ))
        .bind(&sid)
        .bind(&q.query)
        .fetch_all(&s.db)
        .await?
    };
    Ok(Json(rows.into_iter().map(|(v, _)| v).collect::<Vec<_>>()))
}
