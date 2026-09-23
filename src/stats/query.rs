//! Query: parsed v1 params or v2 JSON body. Builds SQL with bound args.

use super::{
    filters::{self, Filter},
    interval::Interval,
    metrics::Metric,
    period::{self, Range},
};

#[derive(Debug, Clone)]
pub struct Query {
    pub site_id: String,
    pub range: Range,
    pub interval: Interval,
    pub filters: Vec<Filter>,
    pub metrics: Vec<Metric>,
    pub dimension: Option<String>,
    pub limit: i64,
    pub page: i64,
    pub compare: bool,
    /// last-30-minutes mode (period=realtime): overrides the date range
    pub realtime: bool,
    /// substring match on the breakdown dimension (details modal search)
    pub search: String,
}

impl Query {
    /// FROM clause: events always; sessions_mat joined when needed.
    fn from(&self) -> String {
        let need_s = self.metrics.iter().any(|m| m.needs_sessions())
            || filters::needs_sessions(&self.filters, self.dimension.as_deref());
        if need_s {
            "events e LEFT JOIN sessions_mat s ON s.site_id = e.site_id AND s.session_id = e.session_id".to_string()
        } else {
            "events e".to_string()
        }
    }

    fn where_base(&self, site_id: &str, range: &Range, args: &mut Vec<String>) -> String {
        args.push(site_id.to_string());
        if self.realtime {
            return "e.site_id = ? AND e.ts >= datetime('now', '-30 minutes')".to_string();
        }
        args.push(range.start_ts());
        args.push(range.end_ts());
        "e.site_id = ? AND e.ts >= ? AND e.ts <= ?".to_string()
    }

    fn session_mode(&self) -> bool {
        self.metrics.iter().any(|m| m.needs_sessions())
    }

    /// Session-grain inner select: one row per session (or per group+session).
    /// Columns: sid, pv, ev, b, st, la, eng.
    fn inner_cols() -> &'static str {
        "e.session_id AS sid, \
         SUM(CASE WHEN e.name = 'pageview' THEN 1 ELSE 0 END) AS pv, \
         COUNT(*) AS ev, \
         MAX(COALESCE(s.is_bounce, 1)) AS b, \
         MAX(strftime('%s', COALESCE(s.started_at, e.ts))) AS st, \
         MAX(strftime('%s', COALESCE(s.last_at, e.ts))) AS la, \
         SUM(CASE WHEN e.name = 'pageview' THEN COALESCE(e.engagement_ms, 0) ELSE 0 END) AS eng"
    }

    fn outer_select(&self) -> String {
        self.metrics
            .iter()
            .map(|m| {
                let e = match m {
                    // session-grain outer expressions
                    _ if self.session_mode() => match m.name() {
                        "visitors" => "COUNT(*)",
                        "visits" => "COUNT(*)",
                        "pageviews" => "SUM(pv)",
                        "events" => "SUM(ev)",
                        "bounce_rate" => "AVG(CAST(b AS REAL))",
                        "visit_duration" => "AVG(CAST(la - st AS REAL))",
                        "views_per_visit" => "CAST(SUM(pv) AS REAL) / NULLIF(COUNT(*), 0)",
                        "time_on_page" => "SUM(CAST(eng AS REAL)) / NULLIF(SUM(pv), 0) / 1000",
                        _ => "COUNT(*)",
                    },
                    _ => m.sql(),
                };
                format!("{e} AS \"{}\"", m.name())
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn aggregate_sql(&self, range: &Range) -> (String, Vec<String>) {
        let mut args = vec![];
        let base = self.where_base(&self.site_id, range, &mut args);
        let mut inner_where = format!("WHERE {base}");
        filters::apply(&self.filters, &mut inner_where, &mut args);
        let select = self.outer_select();
        let sql = if self.session_mode() {
            format!(
                "SELECT {select} FROM (SELECT {} FROM {} {inner_where} GROUP BY e.session_id)",
                Self::inner_cols(),
                self.from()
            )
        } else {
            format!("SELECT {select} FROM {} {inner_where}", self.from())
        };
        (sql, args)
    }

    pub fn timeseries_sql(&self, range: &Range) -> (String, Vec<String>) {
        let mut args = vec![];
        let base = self.where_base(&self.site_id, range, &mut args);
        let bucket = self.interval.bucket_sql("e.ts");
        let mut inner_where = format!("WHERE {base}");
        filters::apply(&self.filters, &mut inner_where, &mut args);
        let select = self.outer_select();
        let sql = if self.session_mode() {
            format!(
                "SELECT date, {select} FROM (SELECT {bucket} AS date, {} FROM {} {inner_where} GROUP BY date, e.session_id) GROUP BY date ORDER BY date",
                Self::inner_cols(),
                self.from()
            )
        } else {
            format!("SELECT {bucket} AS date, {select} FROM {} {inner_where} GROUP BY date ORDER BY date", self.from())
        };
        (sql, args)
    }

    pub fn breakdown_sql(&self, range: &Range) -> (String, Vec<String>) {
        let dim = self.dimension.clone().unwrap_or_else(|| "event:page".to_string());
        let col = filters::dimension_sql(&dim).unwrap_or_else(|_| "e.pathname".to_string());
        let mut args = vec![];
        let base = self.where_base(&self.site_id, range, &mut args);
        let mut inner_where =
            format!("WHERE {base} AND {col} IS NOT NULL AND {col} != ''");
        filters::apply(&self.filters, &mut inner_where, &mut args);
        if !self.search.is_empty() {
            inner_where.push_str(&format!(" AND {col} LIKE '%' || ? || '%'"));
            args.push(self.search.clone());
        }
        let select = self.outer_select();
        let order = self.metrics.first().map(|m| m.name()).unwrap_or("visitors");
        let sql = if self.session_mode() {
            format!(
                "SELECT dim, {select} FROM (SELECT {col} AS dim, {} FROM {} {inner_where} GROUP BY dim, e.session_id) GROUP BY dim ORDER BY \"{order}\" DESC LIMIT ? OFFSET ?",
                Self::inner_cols(),
                self.from()
            )
        } else {
            format!(
                "SELECT {col} AS dim, {select} FROM {} {inner_where} GROUP BY dim ORDER BY \"{order}\" DESC LIMIT ? OFFSET ?",
                self.from()
            )
        };
        args.push((self.limit + 1).to_string());
        args.push(((self.page - 1) * self.limit).to_string());
        (sql, args)
    }
}

/// v1 query-string params shared by aggregate/timeseries/breakdown.
pub struct V1Params {
    pub site: String,
    pub period: String,
    pub date: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub filters: String,
    pub metrics: String,
    pub interval: Option<String>,
    pub property: Option<String>,
    pub limit: i64,
    pub page: i64,
    pub compare: bool,
    pub search: String,
}

impl V1Params {
    pub fn into_query(self, site_id: String, default_metrics: &[&str]) -> Result<Query, String> {
        let realtime = self.period == "realtime";
        let period = if realtime { "day" } else { self.period.as_str() };
        let range = period::resolve(period, self.date.as_deref(), self.from.as_deref(), self.to.as_deref())?;
        let interval = match &self.interval {
            Some(i) => Interval::parse(i)?,
            None => Interval::default_for(&self.period),
        };
        let filters = filters::parse_v1(&self.filters)?;
        let metrics = if self.metrics.is_empty() {
            default_metrics.iter().map(|m| Metric::parse(m).unwrap()).collect()
        } else {
            self.metrics.split(',').map(Metric::parse).collect::<Result<Vec<_>, _>>()?
        };
        if self.limit < 1 || self.limit > 1000 {
            return Err("limit must be 1..=1000".to_string());
        }
        Ok(Query {
            site_id,
            range,
            interval,
            filters,
            metrics,
            dimension: self.property,
            limit: self.limit,
            page: self.page.max(1),
            compare: self.compare,
            realtime,
            search: self.search,
        })
    }
}
