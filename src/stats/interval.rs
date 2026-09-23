//! Interval bucketing. Default matches upstream: day→hour, 7d/30d/month→date,
//! 6mo+→month. SQLite strftime buckets, UTC.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Interval {
    Minute,
    Hour,
    Day,
    Week,
    Month,
}

impl Interval {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "minute" => Ok(Self::Minute),
            "hour" => Ok(Self::Hour),
            "date" | "day" => Ok(Self::Day),
            "week" => Ok(Self::Week),
            "month" => Ok(Self::Month),
            _ => Err("invalid interval (minute|hour|date|week|month)".to_string()),
        }
    }

    pub fn default_for(period: &str) -> Self {
        match period {
            "realtime" => Self::Minute,
            "day" => Self::Hour,
            "7d" | "30d" | "month" | "custom" => Self::Day,
            _ => Self::Month,
        }
    }

    /// strftime format producing a sortable bucket label.
    pub fn fmt(&self) -> &'static str {
        match self {
            Self::Minute => "%Y-%m-%d %H:%M",
            Self::Hour => "%Y-%m-%d %H:00",
            Self::Day => "%Y-%m-%d",
            Self::Week => "%Y-%W",
            Self::Month => "%Y-%m",
        }
    }

    pub fn bucket_sql(&self, col: &str) -> String {
        format!("strftime('{}', {col})", self.fmt())
    }
}
