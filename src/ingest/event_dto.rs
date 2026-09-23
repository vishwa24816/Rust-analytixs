//! Tracker payload mirror. Field names match tracker/src/track.js:
//! n=event, v=script version, u=url, d=domain, r=referrer, m=meta(legacy),
//! p=props, i=interactive, $=revenue, h=hash mode, sd/scroll, e/engagement.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct RawEvent {
    /// event name (string or int for 404 tracking)
    pub n: serde_json::Value,
    #[serde(default)]
    pub v: i64,
    #[serde(default)]
    pub u: String,
    #[serde(default)]
    pub d: String,
    #[serde(default)]
    pub r: Option<String>,
    #[serde(default)]
    pub m: Option<String>,
    #[serde(default)]
    pub p: Option<HashMap<String, serde_json::Value>>,
    #[serde(default = "default_interactive")]
    pub i: bool,
    #[serde(rename = "$", default)]
    pub revenue: Option<Revenue>,
    #[serde(default)]
    pub h: Option<i64>,
    /// engagement extras (top-level in some script versions)
    #[serde(default)]
    pub sd: Option<i64>,
    #[serde(default)]
    pub e: Option<i64>,
}

fn default_interactive() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct Revenue {
    #[serde(default)]
    pub amount: serde_json::Value,
    #[serde(default)]
    pub currency: Option<String>,
}

pub const MAX_URL: usize = 2000;
pub const MAX_NAME: usize = 120;
/// KLUDGE (mirrors Elixir): old cached scripts sent huge engagement values
pub const TOO_LARGE_ENGAGEMENT_MS: i64 = 30 * 24 * 3600 * 1000;

#[derive(Debug)]
pub struct Event {
    pub name: String,
    pub url: String,
    pub domain: String,
    pub hostname: String,
    pub pathname: String,
    pub query: HashMap<String, String>,
    pub referrer: String,
    pub props: HashMap<String, String>,
    pub scroll_depth: Option<i64>,
    pub engagement_ms: Option<i64>,
    pub revenue_cents: Option<i64>,
    pub interactive: bool,
    pub script_version: i64,
}

impl RawEvent {
    pub fn normalize(self) -> Result<Event, String> {
        let name = match &self.n {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => return Err("event name must be string or integer".into()),
        };
        if name.is_empty() {
            return Err("event name required".into());
        }
        if name.len() > MAX_NAME {
            return Err("event name too long".into());
        }
        if self.u.len() > MAX_URL {
            return Err("url too long".into());
        }
        let parsed = url::Url::parse(&self.u).map_err(|_| "invalid url".to_string())?;
        let hostname = parsed.host_str().unwrap_or("").to_string();
        if hostname.is_empty() {
            return Err("hostname required".into());
        }
        let pathname = {
            let p = parsed.path();
            if p.is_empty() { "/".to_string() } else { p.to_string() }
        };
        let query: HashMap<String, String> = parsed.query_pairs().into_owned().collect();

        // legacy meta JSON merges into props
        let mut props: HashMap<String, String> = HashMap::new();
        if let Some(m) = self.m {
            if let Ok(obj) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&m) {
                flatten_props(obj, &mut props);
            }
        }
        if let Some(p) = self.p {
            flatten_props(p, &mut props);
        }

        // engagement: at least one of sd/e, ignore absurd values
        let mut engagement_ms = self.e.filter(|&e| e >= 0 && e <= TOO_LARGE_ENGAGEMENT_MS);
        let mut scroll_depth = self.sd;
        if name == "engagement" && engagement_ms.is_none() && scroll_depth.is_none() {
            return Err("engagement event requires 'sd' or 'e'".into());
        }
        if engagement_ms == Some(0) {
            engagement_ms = None;
        }
        if scroll_depth == Some(255) {
            scroll_depth = None; // tracker "missing" sentinel
        }

        let revenue_cents = self.revenue.as_ref().and_then(|r| parse_amount(&r.amount));

        Ok(Event {
            name,
            url: self.u,
            domain: self.d,
            hostname,
            pathname,
            query,
            referrer: self.r.unwrap_or_default(),
            props,
            scroll_depth,
            engagement_ms,
            revenue_cents,
            interactive: self.i,
            script_version: self.v,
        })
    }
}

fn flatten_props(src: HashMap<String, serde_json::Value>, dst: &mut HashMap<String, String>) {
    for (k, v) in src.into_iter().take(30) {
        if k.is_empty() || k.len() > 300 || dst.len() >= 30 {
            continue;
        }
        let s = match v {
            serde_json::Value::String(s) => s,
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            _ => continue,
        };
        dst.insert(k, s.chars().take(2000).collect());
    }
}

fn parse_amount(v: &serde_json::Value) -> Option<i64> {
    let s = match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => return None,
    };
    s.trim().parse::<f64>().ok().map(|f| (f * 100.0).round() as i64)
}
