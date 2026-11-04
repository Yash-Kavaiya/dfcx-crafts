//! Minimal UTC civil-time helpers (no external crates).

use std::time::{SystemTime, UNIX_EPOCH};

/// A calendar date.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i64,
    pub month: u32,
    pub day: u32,
}

impl Date {
    pub fn from_days(days: i64) -> Date {
        let (y, m, d) = civil_from_days(days);
        Date { year: y, month: m, day: d }
    }
    pub fn to_days(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }
    pub fn add_days(&self, n: i64) -> Date {
        Date::from_days(self.to_days() + n)
    }
    /// 0 = Monday … 6 = Sunday.
    pub fn weekday(&self) -> u32 {
        ((self.to_days() + 3).rem_euclid(7)) as u32
    }
    pub fn today() -> Date {
        Date::from_days(now_secs().div_euclid(86_400))
    }
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
    pub fn parse_iso(s: &str) -> Option<Date> {
        let parts: Vec<&str> = s.trim().get(..10)?.split('-').collect();
        if parts.len() != 3 {
            return None;
        }
        let d = Date { year: parts[0].parse().ok()?, month: parts[1].parse().ok()?, day: parts[2].parse().ok()? };
        if d.is_valid() {
            Some(d)
        } else {
            None
        }
    }
    pub fn is_valid(&self) -> bool {
        self.month >= 1 && self.month <= 12 && self.day >= 1 && self.day <= days_in_month(self.year, self.month)
    }
}

pub fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

/// Howard Hinnant's algorithm.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// RFC 3339 timestamp (UTC) with millisecond precision.
pub fn rfc3339_millis(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let milli = ms.rem_euclid(1000);
    let date = Date::from_days(secs.div_euclid(86_400));
    let sod = secs.rem_euclid(86_400);
    format!(
        "{}T{:02}:{:02}:{:02}.{:03}Z",
        date.iso(),
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60,
        milli
    )
}

pub fn now_rfc3339() -> String {
    rfc3339_millis(now_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn civil() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        let d = Date { year: 2024, month: 2, day: 29 };
        assert_eq!(Date::from_days(d.to_days()), d);
        assert_eq!(d.add_days(1).iso(), "2024-03-01");
        assert_eq!(Date { year: 2024, month: 1, day: 1 }.weekday(), 0); // Monday
        assert_eq!(rfc3339_millis(1_000), "1970-01-01T00:00:01.000Z");
        assert_eq!(Date::parse_iso("2023-12-31").unwrap().day, 31);
        assert!(Date::parse_iso("2023-02-30").is_none());
    }
}
