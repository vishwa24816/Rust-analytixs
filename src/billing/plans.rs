//! Plan catalog loaded from billing/plans_v5.json (upstream file).
//! Lookup by Paddle product id (monthly or yearly).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub kind: String,
    pub generation: i64,
    pub monthly_pageview_limit: i64,
    #[serde(default)]
    pub monthly_product_id: Option<String>,
    #[serde(default)]
    pub yearly_product_id: Option<String>,
    pub site_limit: i64,
    #[serde(default)]
    pub team_member_limit: Option<i64>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub data_retention_in_years: Option<i64>,
}

static PLANS_JSON: &str = include_str!("../../billing/plans_v5.json");

pub fn all() -> Vec<Plan> {
    serde_json::from_str(PLANS_JSON).unwrap_or_default()
}

pub fn by_product_id(product_id: &str) -> Option<Plan> {
    all().into_iter().find(|p| {
        p.monthly_product_id.as_deref() == Some(product_id)
            || p.yearly_product_id.as_deref() == Some(product_id)
    })
}
