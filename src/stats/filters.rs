//! Filter parsing. v1 string syntax (`visit:browser==Chrome;event:page==/blog`),
//! v2 JSON (`["is", "visit:browser", ["Chrome"]]`). Both lower to SQL.

#[derive(Debug, Clone)]
pub enum Op {
    Is,
    IsNot,
    Contains,
    NotContains,
}

#[derive(Debug, Clone)]
pub struct Filter {
    pub dimension: String,
    pub op: Op,
    pub values: Vec<String>,
}

/// Maps a public dimension name to (table alias, SQL fragment).
/// Alias `e` = events, `s` = sessions_mat.
pub fn dimension_sql(dim: &str) -> Result<String, String> {
    if let Some(prop) = dim.strip_prefix("event:props:") {
        if prop.is_empty() || prop.len() > 300 {
            return Err(format!("bad property '{dim}'"));
        }
        let safe = prop.replace('\'', "''");
        return Ok(format!("json_extract(e.props, '$.{safe}')"));
    }
    let col = match dim {
        "event:page" => "e.pathname",
        "event:hostname" => return Err("event:hostname breakdown not supported".into()),
        "event:name" => "e.name",
        "visit:source" => "e.referrer_source",
        "visit:referrer" => "e.referrer",
        "visit:utm_medium" => "e.utm_medium",
        "visit:utm_source" => "e.utm_source",
        "visit:utm_campaign" => "e.utm_campaign",
        "visit:utm_content" => "e.utm_content",
        "visit:utm_term" => "e.utm_term",
        "visit:country" => "e.country",
        "visit:browser" => "e.browser",
        "visit:os" => "e.os",
        "visit:device" => "e.device",
        "visit:entry_page" => "s.entry_page",
        "visit:exit_page" => "s.exit_page",
        _ => return Err(format!("unknown dimension '{dim}'")),
    };
    Ok(col.to_string())
}

pub fn needs_sessions(filters: &[Filter], dimension: Option<&str>) -> bool {
    let uses_s = |d: &str| d == "visit:entry_page" || d == "visit:exit_page";
    filters.iter().any(|f| uses_s(&f.dimension))
        || dimension.map(uses_s).unwrap_or(false)
}

pub fn parse_v1(s: &str) -> Result<Vec<Filter>, String> {
    if s.trim().is_empty() {
        return Ok(vec![]);
    }
    s.split(';').map(parse_clause).collect()
}

fn parse_clause(clause: &str) -> Result<Filter, String> {
    for (sym, op) in [("==", Op::Is), ("!=", Op::IsNot), ("=~", Op::Contains), ("!~", Op::NotContains)] {
        if let Some(i) = clause.find(sym) {
            let dim = clause[..i].trim().to_string();
            let vals = clause[i + sym.len()..].split('|').map(|v| v.trim().to_string()).collect();
            dimension_sql(&dim)?;
            return Ok(Filter { dimension: dim, op, values: vals });
        }
    }
    Err(format!("bad filter '{clause}' (want dim==value)"))
}

/// v2 JSON: ["is"|"is_not"|"contains"|"contains_not", dimension, [values]]
pub fn parse_v2(v: &serde_json::Value) -> Result<Vec<Filter>, String> {
    let arr = v.as_array().ok_or("filters must be an array".to_string())?;
    arr.iter()
        .map(|f| {
            let t = f.as_array().ok_or("each filter must be [op, dim, values]".to_string())?;
            if t.len() != 3 {
                return Err("each filter must be [op, dim, values]".to_string());
            }
            let op = match t[0].as_str().unwrap_or("") {
                "is" => Op::Is,
                "is_not" => Op::IsNot,
                "contains" => Op::Contains,
                "contains_not" => Op::NotContains,
                o => return Err(format!("bad filter op '{o}'")),
            };
            let dim = t[1].as_str().ok_or("dimension must be a string".to_string())?.to_string();
            dimension_sql(&dim)?;
            let vals = t[2]
                .as_array()
                .ok_or("values must be an array".to_string())?
                .iter()
                .map(|x| x.as_str().unwrap_or("").to_string())
                .collect();
            Ok(Filter { dimension: dim, op, values: vals })
        })
        .collect()
}

/// Appends `AND (...)` to sql, pushing bound values. `tbl` prefixes unaliased cols.
pub fn apply(filters: &[Filter], sql: &mut String, args: &mut Vec<String>) {
    for f in filters {
        let col = dimension_sql(&f.dimension).unwrap_or_else(|_| "''".to_string());
        let placeholders = f.values.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        for v in &f.values {
            args.push(v.clone());
        }
        let cond = match f.op {
            Op::Is => format!("({col} IN ({placeholders}))"),
            Op::IsNot => format!("(COALESCE({col}, '') NOT IN ({placeholders}))"),
            Op::Contains => {
                let ors = f.values.iter().map(|_| format!("{col} LIKE '%' || ? || '%'")).collect::<Vec<_>>().join(" OR ");
                format!("({ors})")
            }
            Op::NotContains => {
                let ors = f.values.iter().map(|_| format!("COALESCE({col}, '') NOT LIKE '%' || ? || '%'")).collect::<Vec<_>>().join(" AND ");
                format!("({ors})")
            }
        };
        sql.push_str(&format!(" AND {cond}"));
    }
}
