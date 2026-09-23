//! Period → UTC date range. Mirrors QueryPeriod: day, 7d, 30d, month,
//! 6mo, 12mo, year, custom (from/to), all.

use chrono::{Datelike, NaiveDate, Utc};

#[derive(Debug, Clone)]
pub struct Range {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

pub fn resolve(
    period: &str,
    date: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Range, String> {
    let today = Utc::now().date_naive();
    let day = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| "bad date".to_string());
    match period {
        "day" => {
            let d = date.map(day).transpose()?.unwrap_or(today);
            Ok(Range { start: d, end: d })
        }
        "7d" => Ok(Range { start: today - chrono::Duration::days(6), end: today }),
        "30d" => Ok(Range { start: today - chrono::Duration::days(29), end: today }),
        "month" => {
            let d = date.map(day).transpose()?.unwrap_or(today);
            let first = NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap();
            let next = if d.month() == 12 {
                NaiveDate::from_ymd_opt(d.year() + 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1).unwrap()
            };
            Ok(Range { start: first, end: next - chrono::Duration::days(1) })
        }
        "6mo" => {
            let d = date.map(day).transpose()?.unwrap_or(today);
            Ok(Range { start: shift_months(d, 5), end: d })
        }
        "12mo" => {
            let d = date.map(day).transpose()?.unwrap_or(today);
            Ok(Range { start: shift_months(d, 11), end: d })
        }
        "year" => {
            let d = date.map(day).transpose()?.unwrap_or(today);
            Ok(Range {
                start: NaiveDate::from_ymd_opt(d.year(), 1, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(d.year(), 12, 31).unwrap(),
            })
        }
        "custom" => {
            let f = from.map(day).transpose()?.ok_or("from required".to_string())?;
            let t = to.map(day).transpose()?.ok_or("to required".to_string())?;
            if f > t {
                return Err("from must be <= to".to_string());
            }
            Ok(Range { start: f, end: t })
        }
        "all" => Ok(Range {
            start: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            end: today,
        }),
        _ => Err("invalid period (day|7d|30d|month|6mo|12mo|year|custom|all)".to_string()),
    }
}

fn shift_months(d: NaiveDate, back: u32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap();
    let mut y = first.year();
    let mut m = first.month() as i32 - back as i32;
    while m <= 0 {
        m += 12;
        y -= 1;
    }
    NaiveDate::from_ymd_opt(y, m as u32, 1).unwrap()
}

impl Range {
    /// Previous period of equal length, for compare=previous_period.
    pub fn previous(&self) -> Range {
        let len = (self.end - self.start).num_days() + 1;
        Range {
            start: self.start - chrono::Duration::days(len),
            end: self.start - chrono::Duration::days(1),
        }
    }
    pub fn start_ts(&self) -> String {
        format!("{} 00:00:00", self.start.format("%Y-%m-%d"))
    }
    pub fn end_ts(&self) -> String {
        format!("{} 23:59:59", self.end.format("%Y-%m-%d"))
    }
}
