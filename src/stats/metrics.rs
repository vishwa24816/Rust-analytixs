//! Metrics → SQL over events (alias `e`) and sessions_mat (alias `s`).

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Metric {
    Visitors,
    Visits,
    Pageviews,
    Events,
    BounceRate,
    VisitDuration,
    ViewsPerVisit,
    TimeOnPage,
}

impl Metric {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "visitors" => Ok(Self::Visitors),
            "visits" => Ok(Self::Visits),
            "pageviews" => Ok(Self::Pageviews),
            "events" => Ok(Self::Events),
            "bounce_rate" => Ok(Self::BounceRate),
            "visit_duration" => Ok(Self::VisitDuration),
            "views_per_visit" => Ok(Self::ViewsPerVisit),
            "time_on_page" => Ok(Self::TimeOnPage),
            m => Err(format!("unknown metric '{m}'")),
        }
    }

    pub fn needs_sessions(&self) -> bool {
        matches!(self, Self::Visits | Self::BounceRate | Self::VisitDuration)
    }

    /// Aggregate SQL fragment. `pageview_only` scopes event-level metrics.
    pub fn sql(&self) -> &'static str {
        match self {
            // unique visitors ≈ unique sessions
            Self::Visitors => "COUNT(DISTINCT e.session_id)",
            // sessions in range (sessions_mat is authoritative)
            Self::Visits => "COUNT(DISTINCT s.session_id)",
            Self::Pageviews => "SUM(CASE WHEN e.name = 'pageview' THEN 1 ELSE 0 END)",
            Self::Events => "COUNT(*)",
            // NOTE: session metrics are only valid through the session-grain
            // subquery in query.rs (outer_select); these are single-grain fallbacks.
            Self::BounceRate => {
                "CAST(SUM(s.is_bounce) AS REAL) / NULLIF(COUNT(*), 0)"
            }
            Self::VisitDuration => {
                "AVG(CAST(strftime('%s', s.last_at) - strftime('%s', s.started_at) AS REAL))"
            }
            Self::ViewsPerVisit => {
                "CAST(SUM(CASE WHEN e.name = 'pageview' THEN 1 ELSE 0 END) AS REAL) / NULLIF(COUNT(DISTINCT e.session_id), 0)"
            }
            // avg engagement ms per pageview, seconds
            Self::TimeOnPage => {
                "AVG(CASE WHEN e.name = 'pageview' THEN CAST(e.engagement_ms AS REAL) / 1000 END)"
            }
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Visitors => "visitors",
            Self::Visits => "visits",
            Self::Pageviews => "pageviews",
            Self::Events => "events",
            Self::BounceRate => "bounce_rate",
            Self::VisitDuration => "visit_duration",
            Self::ViewsPerVisit => "views_per_visit",
            Self::TimeOnPage => "time_on_page",
        }
    }
}
