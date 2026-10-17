use chrono::{Datelike, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BusinessHoursConfig {
    pub timezone: String,
    pub days: Vec<i64>,
    pub start_minute: i64,
    pub end_minute: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SlaTargets {
    pub first_response_target_min: Option<i64>,
    pub resolution_target_min: Option<i64>,
}

#[must_use]
pub fn default_business_hours() -> BusinessHoursConfig {
    BusinessHoursConfig {
        timezone: "UTC".to_string(),
        days: vec![1, 2, 3, 4, 5],
        start_minute: 540,
        end_minute: 1020,
    }
}

const MAX_SPAN_DAYS: i64 = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlaVerdict {
    Met,
    Missed,
    NoTarget,
    Unmeasured,
}

impl SlaVerdict {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Met => "met",
            Self::Missed => "missed",
            Self::NoTarget => "no_target",
            Self::Unmeasured => "unmeasured",
        }
    }
}

#[must_use]
pub fn is_valid_timezone(timezone: &str) -> bool {
    timezone.parse::<Tz>().is_ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WallParts {
    y: i32,
    mo: u32,
    d: u32,
    mi: i64,
    dow: u8,
}

fn parts_in_zone(ts_ms: i64, tz: &Tz) -> Option<WallParts> {
    let utc = Utc.timestamp_millis_opt(ts_ms).single()?;
    let local = utc.with_timezone(tz);
    Some(WallParts {
        y: local.year(),
        mo: local.month(),
        d: local.day(),
        mi: i64::from(local.hour()) * 60 + i64::from(local.minute()),
        dow: local.weekday().num_days_from_sunday() as u8,
    })
}

fn naive_utc_ms(y: i32, mo: u32, d: u32, minute: i64) -> Option<i64> {
    let midnight = NaiveDate::from_ymd_opt(y, mo, d)?.and_hms_opt(0, 0, 0)?;
    let dt = midnight + chrono::Duration::minutes(minute);
    Some(dt.and_utc().timestamp_millis())
}

fn offset_minutes_at(ts_ms: i64, tz: &Tz) -> Option<f64> {
    let p = parts_in_zone(ts_ms, tz)?;
    let as_utc = naive_utc_ms(p.y, p.mo, p.d, p.mi)?;
    Some((as_utc - ts_ms) as f64 / 60_000.0)
}

fn instant_for_wall(y: i32, mo: u32, d: u32, minute: i64, tz: &Tz) -> Option<i64> {
    let naive = naive_utc_ms(y, mo, d, minute)?;
    let mut ts = naive;
    for _ in 0..2 {
        let off = offset_minutes_at(ts, tz)?;
        ts = naive - (off * 60_000.0) as i64;
    }
    Some(ts)
}

#[must_use]
pub fn parse_instant_ms(raw: &str) -> Option<i64> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(day) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return day
            .and_hms_opt(0, 0, 0)
            .map(|ndt| ndt.and_utc().timestamp_millis());
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(ndt.and_utc().timestamp_millis());
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(ndt.and_utc().timestamp_millis());
    }
    None
}

#[must_use]
pub fn business_minutes_between(
    start_iso: &str,
    end_iso: &str,
    cfg: &BusinessHoursConfig,
) -> Option<f64> {
    let start = parse_instant_ms(start_iso)?;
    let end = parse_instant_ms(end_iso)?;
    if end < start {
        return None;
    }
    let tz: Tz = cfg.timezone.parse().ok()?;
    if end - start > MAX_SPAN_DAYS * 86_400_000 {
        return None;
    }

    let active: HashSet<i64> = cfg
        .days
        .iter()
        .copied()
        .filter(|d| (0..=6).contains(d))
        .collect();
    if active.is_empty() {
        return None;
    }
    let day_start = cfg.start_minute.clamp(0, 1440);
    let day_end = std::cmp::max(day_start, cfg.end_minute.clamp(0, 1440));
    if day_end <= day_start {
        return None;
    }

    let start_parts = parts_in_zone(start, &tz)?;
    let end_parts = parts_in_zone(end, &tz)?;
    let (mut cy, mut cmo, mut cd) = (start_parts.y, start_parts.mo, start_parts.d);
    let mut total_ms: i64 = 0;
    for _guard in 0..=(MAX_SPAN_DAYS + 2) {
        let day_ts = instant_for_wall(cy, cmo, cd, 0, &tz)?;
        if active.contains(&i64::from(parts_in_zone(day_ts, &tz)?.dow)) {
            let win_start = instant_for_wall(cy, cmo, cd, day_start, &tz)?;
            let win_end = instant_for_wall(cy, cmo, cd, day_end, &tz)?;
            let overlap_start = std::cmp::max(start, win_start);
            let overlap_end = std::cmp::min(end, win_end);
            if overlap_end > overlap_start {
                total_ms += overlap_end - overlap_start;
            }
        }
        if (cy, cmo, cd) == (end_parts.y, end_parts.mo, end_parts.d) {
            break;
        }
        let next_midnight = instant_for_wall(cy, cmo, cd, 1439, &tz)? + 60_000;
        let np = parts_in_zone(next_midnight, &tz)?;
        cy = np.y;
        cmo = np.mo;
        cd = np.d;
    }
    Some(total_ms as f64 / 60_000.0)
}

#[must_use]
pub fn wall_minutes_between(start_iso: &str, end_iso: &str) -> Option<f64> {
    let start = parse_instant_ms(start_iso)?;
    let end = parse_instant_ms(end_iso)?;
    if end < start {
        return None;
    }
    Some((end - start) as f64 / 60_000.0)
}

#[must_use]
pub fn sla_status(minutes: Option<f64>, target_min: Option<i64>) -> SlaVerdict {
    match (target_min, minutes) {
        (None, Some(_)) => SlaVerdict::NoTarget,
        (None, None) => SlaVerdict::Unmeasured,
        (Some(_), None) => SlaVerdict::Unmeasured,
        (Some(target), Some(minutes)) => {
            if minutes <= target as f64 {
                SlaVerdict::Met
            } else {
                SlaVerdict::Missed
            }
        }
    }
}

#[must_use]
pub fn business_minutes_since(iso: &str, cfg: &BusinessHoursConfig) -> Option<f64> {
    let now = now_iso_millis();
    business_minutes_between(iso, &now, cfg)
}

#[must_use]
pub fn now_iso_millis() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

#[must_use]
pub fn iso_days_ago(days: i64) -> String {
    let ts = Utc::now().timestamp_millis() - days * 86_400_000;
    Utc.timestamp_millis_opt(ts)
        .single()
        .map(|t| t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
        .unwrap_or_else(|| Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nine_to_five_utc() -> BusinessHoursConfig {
        BusinessHoursConfig {
            timezone: "UTC".to_string(),
            days: vec![1, 2, 3, 4, 5],
            start_minute: 540,
            end_minute: 1020,
        }
    }

    #[test]
    fn timezone_validation_accepts_known_zones() {
        assert!(is_valid_timezone("UTC"));
        assert!(is_valid_timezone("America/New_York"));
        assert!(is_valid_timezone("Asia/Dhaka"));
        assert!(!is_valid_timezone("Mars/Olympus_Mons"));
        assert!(!is_valid_timezone(""));
    }

    #[test]
    fn business_minutes_inside_one_workday() {
        let cfg = nine_to_five_utc();
        // Wednesday 2024-06-05, 10:00 -> 12:00 UTC = 120 business minutes
        let minutes =
            business_minutes_between("2024-06-05T10:00:00Z", "2024-06-05T12:00:00Z", &cfg).unwrap();
        assert!((minutes - 120.0).abs() < 1e-9);
    }

    #[test]
    fn business_minutes_exclude_nights_and_weekends() {
        let cfg = nine_to_five_utc();
        // Friday 2024-06-07 17:00 UTC -> Monday 2024-06-10 09:00 UTC = 0 minutes
        let minutes =
            business_minutes_between("2024-06-07T17:00:00Z", "2024-06-10T09:00:00Z", &cfg).unwrap();
        assert!((minutes - 0.0).abs() < 1e-9);
    }

    #[test]
    fn business_minutes_spanning_whole_workday() {
        let cfg = nine_to_five_utc();
        // Monday 09:00 -> Tuesday 09:00 UTC = 480 minutes (one full day 09-17)
        let minutes =
            business_minutes_between("2024-06-03T09:00:00Z", "2024-06-04T09:00:00Z", &cfg).unwrap();
        assert!((minutes - 480.0).abs() < 1e-9);
    }

    #[test]
    fn reversed_range_is_rejected() {
        let cfg = nine_to_five_utc();
        assert!(
            business_minutes_between("2024-06-05T12:00:00Z", "2024-06-05T10:00:00Z", &cfg)
                .is_none()
        );
    }

    #[test]
    fn sla_verdict_matrix() {
        assert_eq!(sla_status(Some(30.0), Some(60)), SlaVerdict::Met);
        assert_eq!(sla_status(Some(61.0), Some(60)), SlaVerdict::Missed);
        assert_eq!(sla_status(Some(61.0), None), SlaVerdict::NoTarget);
        assert_eq!(sla_status(None, Some(60)), SlaVerdict::Unmeasured);
        assert_eq!(sla_status(None, None), SlaVerdict::Unmeasured);
    }

    #[test]
    fn wall_minutes_are_calendar_diff() {
        let minutes = wall_minutes_between("2024-06-05T10:00:00Z", "2024-06-05T11:30:00Z");
        assert!((minutes.unwrap() - 90.0).abs() < 1e-9);
    }
}
