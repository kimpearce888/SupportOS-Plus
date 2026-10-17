use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use chrono::{Datelike, Timelike};

use crate::error::Result;

pub const MAX_TREE_DEPTH: u32 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewCompileError {
    pub message: String,
}

impl ViewCompileError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ViewCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ViewCompileError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateRangeKind {
    Calendar,
    Rolling,
    Exact,
}

impl DateRangeKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Calendar => "calendar",
            Self::Rolling => "rolling",
            Self::Exact => "exact",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DateRange {
    pub from: String,
    pub to: String,
    pub label: String,
    pub kind: DateRangeKind,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DateRangeInput<'a> {
    pub mode: &'a str,
    pub timezone: &'a str,
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
    pub from_time: Option<&'a str>,
    pub to_time: Option<&'a str>,
    pub now: Option<i64>,
}

#[must_use]
pub fn is_valid_timezone(timezone: &str) -> bool {
    crate::business_hours::is_valid_timezone(timezone)
}

#[must_use]
pub fn resolve_timezone(explicit: Option<&str>, stored_setting: Option<&str>) -> String {
    for candidate in [explicit, stored_setting].into_iter().flatten() {
        if candidate != "system" && is_valid_timezone(candidate) {
            return candidate.to_string();
        }
    }
    if let Ok(tz) = std::env::var("TZ") {
        if is_valid_timezone(&tz) {
            return tz;
        }
    }
    "UTC".to_string()
}

fn parse_time_of_day(v: Option<&str>) -> Option<i64> {
    let v = v?;
    let bytes = v.as_bytes();
    if bytes.len() != 5 || bytes[2] != b':' {
        return None;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(i, b)| i == 2 || b.is_ascii_digit())
    {
        return None;
    }
    let h: i64 = v.get(0..2)?.parse().ok()?;
    let min: i64 = v.get(3..5)?.parse().ok()?;
    if h > 23 || min > 59 {
        return None;
    }
    Some(h * 60 + min)
}

fn wall_parts(ts_ms: i64, tz: &chrono_tz::Tz) -> Option<(i32, u32, u32, i64, u32)> {
    use chrono::TimeZone;
    let utc = chrono::Utc.timestamp_millis_opt(ts_ms).single()?;
    let local = utc.with_timezone(tz);
    let minute = i64::from(local.hour()) * 60 + i64::from(local.minute());
    Some((
        local.year(),
        local.month(),
        local.day(),
        minute,
        local.weekday().num_days_from_sunday(),
    ))
}

fn local_instant_ms(tz: &chrono_tz::Tz, y: i32, mo: u32, d: u32, minute: i64) -> Option<i64> {
    use chrono::{Offset, TimeZone};
    let naive = chrono::NaiveDate::from_ymd_opt(y, mo, d)?
        .and_hms_opt(0, 0, 0)?
        .and_utc()
        + chrono::Duration::milliseconds(minute * 60_000);
    match tz.from_local_datetime(&naive.naive_utc()) {
        chrono::LocalResult::Single(dt) => Some(dt.timestamp_millis()),
        chrono::LocalResult::Ambiguous(earliest, _latest) => Some(earliest.timestamp_millis()),
        chrono::LocalResult::None => {
            let offset = tz.offset_from_utc_datetime(&naive.naive_utc()).fix();
            let guessed = naive - chrono::Duration::seconds(i64::from(offset.local_minus_utc()));
            let offset2 = tz.offset_from_utc_datetime(&guessed.naive_utc()).fix();
            let corrected = naive - chrono::Duration::seconds(i64::from(offset2.local_minus_utc()));
            Some(corrected.timestamp_millis())
        }
    }
}

fn days_in_month(y: i32, mo: u32) -> u32 {
    let (next_y, next_m) = if mo == 12 { (y + 1, 1) } else { (y, mo + 1) };
    let first_of_next = chrono::NaiveDate::from_ymd_opt(next_y, next_m, 1);
    first_of_next
        .and_then(|d| d.pred_opt())
        .map_or(30, |d| d.day())
}

fn iso_utc(ts_ms: i64) -> String {
    use chrono::TimeZone;
    chrono::Utc
        .timestamp_millis_opt(ts_ms)
        .single()
        .map(|t| t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
        .unwrap_or_default()
}

#[must_use]
pub fn iso_utc_pub(ts_ms: i64) -> String {
    iso_utc(ts_ms)
}

#[must_use]
pub fn resolve_date_range(input: &DateRangeInput<'_>) -> Option<DateRange> {
    let mode = crate::catalog::DateMode::ALL
        .iter()
        .find(|m| m.as_str() == input.mode)
        .copied()?;
    let tz: chrono_tz::Tz = if is_valid_timezone(input.timezone) {
        input.timezone.parse().ok()?
    } else {
        "UTC".parse().ok()?
    };
    let now_ms = input.now.unwrap_or_else(now_ms_epoch);
    let (y, m, d, _, dow) = wall_parts(now_ms, &tz)?;

    let midnight_at = |day_offset: i64| -> Option<i64> {
        let base = chrono::NaiveDate::from_ymd_opt(y, m, d)? + chrono::Duration::days(day_offset);
        local_instant_ms(&tz, base.year(), base.month(), base.day(), 0)
    };
    let month_midnight_at = |month_offset: i32, day_of_month: u32| -> Option<i64> {
        let first = chrono::NaiveDate::from_ymd_opt(y, m, 1)?;
        let shifted = first
            .checked_add_months(chrono::Months::new(month_offset.unsigned_abs()))
            .filter(|_| month_offset >= 0)
            .or_else(|| {
                first.checked_sub_months(chrono::Months::new(month_offset.unsigned_abs()))
            })?;
        let day = day_of_month.min(days_in_month(shifted.year(), shifted.month()));
        let clamped = shifted.with_day(day)?;
        local_instant_ms(&tz, clamped.year(), clamped.month(), clamped.day(), 0)
    };
    let week_midnight_at = |week_offset: i64| midnight_at(week_offset * 7 - i64::from(dow));

    let with_time_bounds = |from: i64, to: i64, kind: DateRangeKind, label: &str| -> DateRange {
        let mut f = from;
        let mut t = to;
        if let Some(from_min) = parse_time_of_day(input.from_time) {
            if let Some((fy, fmo, fd, _, _)) = wall_parts(f, &tz) {
                if let Some(inst) = local_instant_ms(&tz, fy, fmo, fd, from_min) {
                    f = inst;
                }
            }
        }
        if let Some(to_min) = parse_time_of_day(input.to_time) {
            if let Some((ly, lmo, ld, _, _)) = wall_parts(t - 1, &tz) {
                if let Some(inst) = local_instant_ms(&tz, ly, lmo, ld, to_min) {
                    t = inst;
                }
            }
        }
        DateRange {
            from: iso_utc(f),
            to: iso_utc(t),
            label: label.to_string(),
            kind,
        }
    };

    let iso_date = |v: Option<&str>| -> Option<(i32, u32, u32)> {
        let v = v?;
        let bytes = v.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        let yy: i32 = v.get(0..4)?.parse().ok()?;
        let mo: u32 = v.get(5..7)?.parse().ok()?;
        let dd: u32 = v.get(8..10)?.parse().ok()?;
        if !(1..=12).contains(&mo) || !(1..=31).contains(&dd) {
            return None;
        }
        let probe = local_instant_ms(&tz, yy, mo, dd, 12)?;
        let (py, pmo, pd, _, _) = wall_parts(probe, &tz)?;
        if py != yy || pmo != mo || pd != dd {
            return None;
        }
        Some((yy, mo, dd))
    };

    match mode {
        crate::catalog::DateMode::Today => Some(with_time_bounds(
            midnight_at(0)?,
            midnight_at(1)?,
            DateRangeKind::Calendar,
            "Today",
        )),
        crate::catalog::DateMode::Yesterday => Some(with_time_bounds(
            midnight_at(-1)?,
            midnight_at(0)?,
            DateRangeKind::Calendar,
            "Yesterday",
        )),
        crate::catalog::DateMode::Tomorrow => Some(with_time_bounds(
            midnight_at(1)?,
            midnight_at(2)?,
            DateRangeKind::Calendar,
            "Tomorrow",
        )),
        crate::catalog::DateMode::ThisWeek => Some(with_time_bounds(
            week_midnight_at(0)?,
            week_midnight_at(1)?,
            DateRangeKind::Calendar,
            "This week",
        )),
        crate::catalog::DateMode::LastWeek => Some(with_time_bounds(
            week_midnight_at(-1)?,
            week_midnight_at(0)?,
            DateRangeKind::Calendar,
            "Last week",
        )),
        crate::catalog::DateMode::ThisMonth => Some(with_time_bounds(
            month_midnight_at(0, 1)?,
            month_midnight_at(1, 1)?,
            DateRangeKind::Calendar,
            "This month",
        )),
        crate::catalog::DateMode::LastMonth => Some(with_time_bounds(
            month_midnight_at(-1, 1)?,
            month_midnight_at(0, 1)?,
            DateRangeKind::Calendar,
            "Last month",
        )),
        crate::catalog::DateMode::Last24h => Some(rolling(now_ms, 24, "Last 24 hours")),
        crate::catalog::DateMode::Last48h => Some(rolling(now_ms, 48, "Last 48 hours")),
        crate::catalog::DateMode::Last7d => Some(rolling(now_ms, 24 * 7, "Last 7 days")),
        crate::catalog::DateMode::Last14d => Some(rolling(now_ms, 24 * 14, "Last 14 days")),
        crate::catalog::DateMode::Last30d => Some(rolling(now_ms, 24 * 30, "Last 30 days")),
        crate::catalog::DateMode::Last90d => Some(rolling(now_ms, 24 * 90, "Last 90 days")),
        crate::catalog::DateMode::ExactDate => {
            let (dy, dmo, dd) = iso_date(input.from)?;
            let start = local_instant_ms(&tz, dy, dmo, dd, 0)?;
            let end = local_instant_ms(&tz, dy, dmo, dd, 0)? + 86_400_000;
            Some(with_time_bounds(
                start,
                end,
                DateRangeKind::Exact,
                &format!("On {}", input.from.unwrap_or_default()),
            ))
        }
        crate::catalog::DateMode::CustomRange => {
            let a = iso_date(input.from)?;
            let b = iso_date(input.to)?;
            let start_a = local_instant_ms(&tz, a.0, a.1, a.2, 0)?;
            let start_b = local_instant_ms(&tz, b.0, b.1, b.2, 0)?;
            let (lo, hi) = if start_a < start_b {
                (start_a, start_b + 86_400_000)
            } else {
                (start_b, start_a + 86_400_000)
            };
            Some(with_time_bounds(
                lo,
                hi,
                DateRangeKind::Exact,
                &format!(
                    "{} to {}",
                    input.from.unwrap_or_default(),
                    input.to.unwrap_or_default()
                ),
            ))
        }
    }
}

fn rolling(now_ms: i64, hours: i64, label: &str) -> DateRange {
    DateRange {
        from: iso_utc(now_ms - hours * 3_600_000),
        to: iso_utc(now_ms),
        label: label.to_string(),
        kind: DateRangeKind::Rolling,
    }
}

fn now_ms_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub const RESPONSE_STATE_SQL: &str = "CASE
  WHEN c.status = 'closed' THEN 'closed'
  WHEN c.status = 'spam' THEN 'closed'
  WHEN c.status = 'active' AND c.snoozed_until IS NOT NULL AND c.snoozed_until > datetime('now') THEN 'snoozed'
  WHEN c.status = 'active' AND c.first_customer_message_at IS NOT NULL AND c.first_response_at IS NULL THEN 'needs_first_response'
  WHEN c.status = 'active'
       AND c.last_customer_reply_at IS NOT NULL
       AND (c.last_human_agent_response_at IS NULL OR c.last_customer_reply_at > c.last_human_agent_response_at) THEN 'customer_waiting'
  WHEN c.status IN ('active', 'pending')
       AND c.last_human_agent_response_at IS NOT NULL
       AND (c.last_customer_reply_at IS NULL OR c.last_human_agent_response_at > c.last_customer_reply_at)
       AND (julianday('now') - julianday(c.last_human_agent_response_at)) * 1440 <= 1440 THEN 'recently_responded'
  WHEN c.status IN ('active', 'pending')
       AND c.last_human_agent_response_at IS NOT NULL
       AND (c.last_customer_reply_at IS NULL OR c.last_human_agent_response_at > c.last_customer_reply_at) THEN 'agent_waiting'
  WHEN c.activity_history_complete = 0
       AND c.first_customer_message_at IS NULL
       AND c.first_response_at IS NULL THEN 'unknown'
  ELSE 'never_responded'
END";

#[must_use]
pub fn activity_field_column(field: &str) -> Option<&'static str> {
    match field {
        "created_at" => Some("c.created_at"),
        "first_customer_message_at" => Some("c.first_customer_message_at"),
        "first_response_at" => Some("c.first_response_at"),
        "last_customer_reply_at" => Some("c.last_customer_reply_at"),
        "last_human_agent_response_at" => Some("c.last_human_agent_response_at"),
        "last_system_response_at" => Some("c.last_system_response_at"),
        "last_note_at" => Some("c.last_note_at"),
        "last_activity_at" => Some("COALESCE(c.last_activity_at, c.created_at)"),
        "closed_at" => Some("c.closed_at"),
        "customer_waiting_since" => Some("c.customer_waiting_since"),
        "last_status_change_at" => Some("c.last_status_change_at"),
        "last_assignment_change_at" => Some("c.last_assignment_change_at"),
        "last_tag_change_at" => Some("c.last_tag_change_at"),
        "last_custom_field_change_at" => Some("c.last_custom_field_change_at"),
        _ => None,
    }
}

#[must_use]
pub fn age_metric_sql(metric: &str) -> Option<(&'static str, &'static str)> {
    match metric {
        "time_since_customer_reply" => Some((
            "(julianday('now') - COALESCE(julianday(c.last_customer_reply_at), julianday('now'))) * 1440",
            "Time since customer reply",
        )),
        "time_since_agent_response" => Some((
            "(julianday('now') - COALESCE(julianday(c.last_human_agent_response_at), julianday('now'))) * 1440",
            "Time since agent response",
        )),
        "customer_waiting_duration" => Some((
            "(julianday('now') - COALESCE(julianday(c.customer_waiting_since), julianday('now'))) * 1440",
            "Customer waiting",
        )),
        "first_response_delay" => Some((
            "(COALESCE(julianday(c.first_response_at), julianday('now')) - COALESCE(julianday(c.created_at), julianday('now'))) * 1440",
            "First response delay",
        )),
        "resolution_duration" => Some((
            "(COALESCE(julianday(c.closed_at), julianday('now')) - COALESCE(julianday(c.created_at), julianday('now'))) * 1440",
            "Resolution duration",
        )),
        "conversation_age" => Some((
            "(julianday('now') - COALESCE(julianday(c.created_at), julianday('now'))) * 1440",
            "Conversation age",
        )),
        _ => None,
    }
}

pub const VIEW_CONDITION_KINDS: [&str; 22] = [
    "status",
    "assignee",
    "team",
    "mailbox",
    "channel",
    "tags",
    "custom_field",
    "customer_property",
    "customer_text",
    "date_activity",
    "response_state",
    "response_age",
    "sla",
    "priority",
    "ticket_state",
    "known_issue",
    "ai_analyzed",
    "interaction_signal",
    "ai_attribute",
    "unread",
    "snoozed",
    "customer",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewNode {
    Group {
        combinator: String,
        children: Vec<ViewNode>,
    },
    Status {
        statuses: Vec<String>,
    },
    Assignee {
        #[serde(rename = "assigneeLocalIds")]
        assignee_local_ids: Vec<i64>,
        #[serde(rename = "includeUnassigned")]
        include_unassigned: bool,
    },
    Team {
        #[serde(rename = "teamLocalIds")]
        team_local_ids: Vec<i64>,
    },
    Mailbox {
        #[serde(rename = "mailboxLocalIds")]
        mailbox_local_ids: Vec<i64>,
    },
    Channel {
        channels: Vec<String>,
    },
    Tags {
        tags: Vec<String>,
        mode: String,
    },
    CustomField {
        #[serde(rename = "fieldLocalId")]
        field_local_id: i64,
        op: String,
        value: Option<String>,
    },
    CustomerProperty {
        #[serde(rename = "definitionId")]
        definition_id: i64,
        op: String,
        value: Option<String>,
    },
    CustomerText {
        field: String,
        op: String,
        value: Option<String>,
    },
    DateActivity {
        #[serde(rename = "activityField")]
        activity_field: String,
        mode: String,
        from: Option<String>,
        to: Option<String>,
        #[serde(rename = "fromTime")]
        from_time: Option<String>,
        #[serde(rename = "toTime")]
        to_time: Option<String>,
    },
    ResponseState {
        states: Vec<String>,
    },
    ResponseAge {
        metric: String,
        op: String,
        minutes: f64,
    },
    Sla {
        states: Vec<String>,
        negate: bool,
    },
    Priority {
        priorities: Vec<String>,
    },
    TicketState {
        #[serde(rename = "stateIds")]
        state_ids: Vec<i64>,
        #[serde(rename = "includeNoState")]
        include_no_state: bool,
    },
    KnownIssue {
        any: bool,
        #[serde(rename = "knownIssueIds")]
        known_issue_ids: Option<Vec<i64>>,
    },
    AiAnalyzed {
        analyzed: bool,
    },
    InteractionSignal {
        dimension: String,
        value: String,
        negate: bool,
    },
    AiAttribute {
        attribute: String,
        op: String,
        value: String,
    },
    Unread {
        unread: bool,
    },
    Snoozed {
        snoozed: bool,
    },
    Customer {
        #[serde(rename = "customerLocalIds")]
        customer_local_ids: Vec<i64>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewDefinition {
    pub combinator: String,
    pub conditions: Vec<ViewNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedInboxView {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub definition: ViewDefinition,
    pub sort_order: i64,
    pub folder: Option<String>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewIssue {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct CreateViewRequest {
    pub name: String,
    pub description: Option<String>,
    pub definition: ViewDefinition,
    pub sort_order: Option<i64>,
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateViewRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub definition: Option<ViewDefinition>,
    pub sort_order: Option<i64>,
    pub folder: Option<Option<String>>,
}

const RESPONSE_STATES: [&str; 8] = [
    "needs_first_response",
    "customer_waiting",
    "agent_waiting",
    "recently_responded",
    "never_responded",
    "closed",
    "snoozed",
    "unknown",
];
const AGE_METRICS: [&str; 6] = [
    "time_since_customer_reply",
    "time_since_agent_response",
    "customer_waiting_duration",
    "first_response_delay",
    "resolution_duration",
    "conversation_age",
];
const TICKET_PRIORITIES: [&str; 5] = ["none", "low", "medium", "high", "urgent"];
const CONVERSATION_STATUSES: [&str; 4] = ["active", "pending", "closed", "spam"];
const CHANNELS: [&str; 2] = ["email", "chat"];
const TAG_MODES: [&str; 3] = ["any", "all", "none"];
const FIELD_OPS: [&str; 5] = [
    "equals",
    "not_equals",
    "contains",
    "is_empty",
    "is_not_empty",
];
const PROPERTY_OPS: [&str; 9] = [
    "equals",
    "not_equals",
    "contains",
    "is_empty",
    "is_not_empty",
    "gt",
    "gte",
    "lt",
    "lte",
];
const TEXT_OPS: [&str; 5] = [
    "contains",
    "equals",
    "not_contains",
    "is_empty",
    "is_not_empty",
];
const CUSTOMER_TEXT_FIELDS: [&str; 3] = ["name", "email", "organization"];
const SLA_STATES: [&str; 2] = ["at_risk", "breached"];
const AGE_OPS: [&str; 4] = ["gt", "gte", "lt", "lte"];
const AI_ATTRIBUTE_OPS: [&str; 8] = [
    "equals",
    "not_equals",
    "contains",
    "not_contains",
    "gt",
    "gte",
    "lt",
    "lte",
];

fn json_type_name(v: &Json) -> &'static str {
    match v {
        Json::String(_) => "string",
        Json::Number(_) => "number",
        Json::Bool(_) => "boolean",
        Json::Array(_) => "array",
        Json::Object(_) => "object",
        Json::Null => "null",
    }
}

fn enum_message(variants: &[&str], received: &str) -> String {
    let expected = variants
        .iter()
        .map(|v| format!("'{v}'"))
        .collect::<Vec<_>>()
        .join(" | ");
    format!("Invalid enum value. Expected {expected}, received '{received}'")
}

struct IssueSink {
    issues: Vec<ViewIssue>,
}

impl IssueSink {
    fn push(&mut self, path: &str, message: impl Into<String>) {
        self.issues.push(ViewIssue {
            path: path.to_string(),
            message: message.into(),
        });
    }

    fn is_empty(&self) -> bool {
        self.issues.is_empty()
    }
}

fn parse_bounded_string(
    v: Option<&Json>,
    path: &str,
    lo: usize,
    hi: usize,
    sink: &mut IssueSink,
) -> Option<String> {
    match v {
        None | Some(Json::Null) => {
            sink.push(path, "Required");
            None
        }
        Some(Json::String(s)) => {
            if s.chars().count() < lo {
                sink.push(
                    path,
                    format!("String must contain at least {lo} character(s)"),
                );
                None
            } else if s.chars().count() > hi {
                sink.push(
                    path,
                    format!("String must contain at most {hi} character(s)"),
                );
                None
            } else {
                Some(s.clone())
            }
        }
        Some(other) => {
            sink.push(
                path,
                format!("Expected string, received {}", json_type_name(other)),
            );
            None
        }
    }
}

fn parse_nullable_max_string(
    v: Option<&Json>,
    path: &str,
    hi: usize,
    sink: &mut IssueSink,
) -> Option<Option<String>> {
    match v {
        None | Some(Json::Null) => Some(None),
        Some(Json::String(s)) => {
            if s.chars().count() > hi {
                sink.push(
                    path,
                    format!("String must contain at most {hi} character(s)"),
                );
                Some(None)
            } else {
                Some(Some(s.clone()))
            }
        }
        Some(other) => {
            sink.push(
                path,
                format!("Expected string, received {}", json_type_name(other)),
            );
            Some(None)
        }
    }
}

fn parse_bool(v: Option<&Json>, path: &str, sink: &mut IssueSink) -> Option<bool> {
    match v {
        None | Some(Json::Null) => {
            sink.push(path, "Required");
            None
        }
        Some(Json::Bool(b)) => Some(*b),
        Some(other) => {
            sink.push(
                path,
                format!("Expected boolean, received {}", json_type_name(other)),
            );
            None
        }
    }
}

fn parse_positive_int(v: Option<&Json>, path: &str, sink: &mut IssueSink) -> Option<i64> {
    match v {
        None | Some(Json::Null) => {
            sink.push(path, "Required");
            None
        }
        Some(Json::Number(n)) => {
            if let Some(i) = n.as_i64() {
                if i > 0 {
                    Some(i)
                } else {
                    sink.push(path, "Number must be greater than 0");
                    None
                }
            } else {
                sink.push(path, "Expected int, received float");
                None
            }
        }
        Some(other) => {
            sink.push(
                path,
                format!("Expected number, received {}", json_type_name(other)),
            );
            None
        }
    }
}

fn parse_enum_array(
    v: Option<&Json>,
    path: &str,
    variants: &[&str],
    lo: usize,
    hi: usize,
    sink: &mut IssueSink,
) -> Option<Vec<String>> {
    let items = parse_array(v, path, lo, hi, sink)?;
    let mut out = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        match item.as_str() {
            Some(s) if variants.contains(&s) => out.push(s.to_string()),
            Some(s) => {
                sink.push(&format!("{path}.{i}"), enum_message(variants, s));
                return None;
            }
            None => {
                sink.push(
                    &format!("{path}.{i}"),
                    format!("Expected string, received {}", json_type_name(item)),
                );
                return None;
            }
        }
    }
    Some(out)
}

fn parse_id_array(
    v: Option<&Json>,
    path: &str,
    lo: usize,
    hi: usize,
    sink: &mut IssueSink,
) -> Option<Vec<i64>> {
    let items = parse_array(v, path, lo, hi, sink)?;
    let mut out = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        out.push(parse_positive_int(
            Some(item),
            &format!("{path}.{i}"),
            sink,
        )?);
    }
    Some(out)
}

fn parse_array<'v>(
    v: Option<&'v Json>,
    path: &str,
    lo: usize,
    hi: usize,
    sink: &mut IssueSink,
) -> Option<&'v Vec<Json>> {
    match v {
        None | Some(Json::Null) => {
            sink.push(path, "Required");
            None
        }
        Some(Json::Array(items)) => {
            if items.len() < lo {
                sink.push(
                    path,
                    format!(
                        "Array must contain at least {lo} element(s), but the input had {}",
                        items.len()
                    ),
                );
                None
            } else if items.len() > hi {
                sink.push(
                    path,
                    format!(
                        "Array must contain at most {hi} element(s), but the input had {}",
                        items.len()
                    ),
                );
                None
            } else {
                Some(items)
            }
        }
        Some(other) => {
            sink.push(
                path,
                format!("Expected array, received {}", json_type_name(other)),
            );
            None
        }
    }
}

fn parse_time_of_day_field(
    v: Option<&Json>,
    path: &str,
    sink: &mut IssueSink,
) -> Option<Option<String>> {
    match v {
        None | Some(Json::Null) => Some(None),
        Some(Json::String(s)) => {
            let valid = s.len() == 5
                && s.as_bytes().iter().enumerate().all(|(i, b)| match i {
                    2 => *b == b':',
                    _ => b.is_ascii_digit(),
                })
                && {
                    let h: u32 = s[0..2].parse().unwrap_or(99);
                    let mi: u32 = s[3..5].parse().unwrap_or(99);
                    h < 24 && mi < 60
                };
            if valid {
                Some(Some(s.clone()))
            } else {
                sink.push(path, "time must be a valid HH:mm");
                Some(None)
            }
        }
        Some(other) => {
            sink.push(
                path,
                format!("Expected string, received {}", json_type_name(other)),
            );
            Some(None)
        }
    }
}

fn parse_enum_str(
    v: Option<&Json>,
    path: &str,
    variants: &[&str],
    sink: &mut IssueSink,
) -> Option<String> {
    match v {
        None | Some(Json::Null) => {
            sink.push(path, "Required");
            None
        }
        Some(Json::String(s)) => {
            if variants.contains(&s.as_str()) {
                Some(s.clone())
            } else {
                sink.push(path, enum_message(variants, s));
                None
            }
        }
        Some(other) => {
            sink.push(
                path,
                format!("Expected string, received {}", json_type_name(other)),
            );
            None
        }
    }
}

fn parse_view_node(v: &Json, path: &str, sink: &mut IssueSink) -> Option<ViewNode> {
    let Some(obj) = v.as_object() else {
        sink.push(
            path,
            format!("Expected object, received {}", json_type_name(v)),
        );
        return None;
    };
    let kind = match obj.get("kind") {
        Some(Json::String(s)) => s.clone(),
        _ => {
            sink.push(path, "Required");
            return None;
        }
    };
    let get = |k: &str| obj.get(k);
    match kind.as_str() {
        "group" => {
            let combinator = parse_enum_str(
                get("combinator"),
                &format!("{path}.combinator"),
                &["all", "any"],
                sink,
            )?;
            let children = get("children").and_then(Json::as_array);
            let child_path = format!("{path}.children");
            match children {
                None => {
                    sink.push(&child_path, "Required");
                    None
                }
                Some(items) => {
                    if items.is_empty() {
                        sink.push(
                            &child_path,
                            "Array must contain at least 1 element(s), but the input had 0",
                        );
                        return None;
                    }
                    if items.len() > 25 {
                        sink.push(
                            &child_path,
                            format!(
                                "Array must contain at most 25 element(s), but the input had {}",
                                items.len()
                            ),
                        );
                        return None;
                    }
                    let mut nodes = Vec::with_capacity(items.len());
                    for (i, child) in items.iter().enumerate() {
                        nodes.push(parse_view_node(child, &format!("{child_path}.{i}"), sink)?);
                    }
                    Some(ViewNode::Group {
                        combinator,
                        children: nodes,
                    })
                }
            }
        }
        "status" => Some(ViewNode::Status {
            statuses: parse_enum_array(
                get("statuses"),
                &format!("{path}.statuses"),
                &CONVERSATION_STATUSES,
                1,
                4,
                sink,
            )?,
        }),
        "assignee" => Some(ViewNode::Assignee {
            assignee_local_ids: parse_id_array(
                get("assigneeLocalIds"),
                &format!("{path}.assigneeLocalIds"),
                1,
                200,
                sink,
            )?,
            include_unassigned: parse_bool(
                get("includeUnassigned"),
                &format!("{path}.includeUnassigned"),
                sink,
            )?,
        }),
        "team" => Some(ViewNode::Team {
            team_local_ids: parse_id_array(
                get("teamLocalIds"),
                &format!("{path}.teamLocalIds"),
                1,
                200,
                sink,
            )?,
        }),
        "mailbox" => Some(ViewNode::Mailbox {
            mailbox_local_ids: parse_id_array(
                get("mailboxLocalIds"),
                &format!("{path}.mailboxLocalIds"),
                1,
                200,
                sink,
            )?,
        }),
        "channel" => Some(ViewNode::Channel {
            channels: parse_enum_array(
                get("channels"),
                &format!("{path}.channels"),
                &CHANNELS,
                1,
                2,
                sink,
            )?,
        }),
        "tags" => {
            let tags = parse_array(get("tags"), &format!("{path}.tags"), 1, 50, sink)?;
            let mut out = Vec::with_capacity(tags.len());
            for (i, t) in tags.iter().enumerate() {
                out.push(parse_bounded_string(
                    Some(t),
                    &format!("{path}.tags.{i}"),
                    1,
                    200,
                    sink,
                )?);
            }
            Some(ViewNode::Tags {
                tags: out,
                mode: parse_enum_str(get("mode"), &format!("{path}.mode"), &TAG_MODES, sink)?,
            })
        }
        "custom_field" => Some(ViewNode::CustomField {
            field_local_id: parse_positive_int(
                get("fieldLocalId"),
                &format!("{path}.fieldLocalId"),
                sink,
            )?,
            op: parse_enum_str(get("op"), &format!("{path}.op"), &FIELD_OPS, sink)?,
            value: parse_nullable_max_string(get("value"), &format!("{path}.value"), 500, sink)?,
        }),
        "customer_property" => Some(ViewNode::CustomerProperty {
            definition_id: parse_positive_int(
                get("definitionId"),
                &format!("{path}.definitionId"),
                sink,
            )?,
            op: parse_enum_str(get("op"), &format!("{path}.op"), &PROPERTY_OPS, sink)?,
            value: parse_nullable_max_string(get("value"), &format!("{path}.value"), 500, sink)?,
        }),
        "customer_text" => Some(ViewNode::CustomerText {
            field: parse_enum_str(
                get("field"),
                &format!("{path}.field"),
                &CUSTOMER_TEXT_FIELDS,
                sink,
            )?,
            op: parse_enum_str(get("op"), &format!("{path}.op"), &TEXT_OPS, sink)?,
            value: parse_nullable_max_string(get("value"), &format!("{path}.value"), 300, sink)?,
        }),
        "date_activity" => {
            let activity_field = parse_enum_str(
                get("activityField"),
                &format!("{path}.activityField"),
                &crate::catalog::ActivityField::ALL.map(|f| f.as_str()),
                sink,
            )?;
            Some(ViewNode::DateActivity {
                activity_field,
                mode: parse_enum_str(
                    get("mode"),
                    &format!("{path}.mode"),
                    &crate::catalog::DateMode::ALL.map(|m| m.as_str()),
                    sink,
                )?,
                from: parse_nullable_max_string(get("from"), &format!("{path}.from"), 40, sink)?,
                to: parse_nullable_max_string(get("to"), &format!("{path}.to"), 40, sink)?,
                from_time: parse_time_of_day_field(
                    get("fromTime"),
                    &format!("{path}.fromTime"),
                    sink,
                )?,
                to_time: parse_time_of_day_field(get("toTime"), &format!("{path}.toTime"), sink)?,
            })
        }
        "response_state" => Some(ViewNode::ResponseState {
            states: parse_enum_array(
                get("states"),
                &format!("{path}.states"),
                &RESPONSE_STATES,
                1,
                10_000,
                sink,
            )?,
        }),
        "response_age" => {
            let metric =
                parse_enum_str(get("metric"), &format!("{path}.metric"), &AGE_METRICS, sink)?;
            let op = parse_enum_str(get("op"), &format!("{path}.op"), &AGE_OPS, sink)?;
            let minutes = match get("minutes") {
                None | Some(Json::Null) => {
                    sink.push(&format!("{path}.minutes"), "Required");
                    return None;
                }
                Some(Json::Number(n)) => {
                    let f = n.as_f64().unwrap_or(f64::NAN);
                    if !f.is_finite() {
                        sink.push(&format!("{path}.minutes"), "Expected number, received NaN");
                        return None;
                    }
                    if f < 0.0 {
                        sink.push(
                            &format!("{path}.minutes"),
                            "Number must be greater than or equal to 0",
                        );
                        return None;
                    }
                    if f > 525_600.0 {
                        sink.push(
                            &format!("{path}.minutes"),
                            "Number must be less than or equal to 525600",
                        );
                        return None;
                    }
                    f
                }
                Some(other) => {
                    sink.push(
                        &format!("{path}.minutes"),
                        format!("Expected number, received {}", json_type_name(other)),
                    );
                    return None;
                }
            };
            Some(ViewNode::ResponseAge {
                metric,
                op,
                minutes,
            })
        }
        "sla" => Some(ViewNode::Sla {
            states: parse_enum_array(
                get("states"),
                &format!("{path}.states"),
                &SLA_STATES,
                1,
                10_000,
                sink,
            )?,
            negate: parse_bool(get("negate"), &format!("{path}.negate"), sink)?,
        }),
        "priority" => Some(ViewNode::Priority {
            priorities: parse_enum_array(
                get("priorities"),
                &format!("{path}.priorities"),
                &TICKET_PRIORITIES,
                1,
                10_000,
                sink,
            )?,
        }),
        "ticket_state" => Some(ViewNode::TicketState {
            state_ids: parse_id_array(get("stateIds"), &format!("{path}.stateIds"), 1, 200, sink)?,
            include_no_state: parse_bool(
                get("includeNoState"),
                &format!("{path}.includeNoState"),
                sink,
            )?,
        }),
        "known_issue" => Some(ViewNode::KnownIssue {
            any: parse_bool(get("any"), &format!("{path}.any"), sink)?,
            known_issue_ids: match get("knownIssueIds") {
                None | Some(Json::Null) => None,
                Some(v) => Some(parse_id_array(
                    Some(v),
                    &format!("{path}.knownIssueIds"),
                    1,
                    200,
                    sink,
                )?),
            },
        }),
        "ai_analyzed" => Some(ViewNode::AiAnalyzed {
            analyzed: parse_bool(get("analyzed"), &format!("{path}.analyzed"), sink)?,
        }),
        "interaction_signal" => Some(ViewNode::InteractionSignal {
            dimension: parse_bounded_string(
                get("dimension"),
                &format!("{path}.dimension"),
                1,
                40,
                sink,
            )?,
            value: parse_bounded_string(get("value"), &format!("{path}.value"), 1, 60, sink)?,
            negate: parse_bool(get("negate"), &format!("{path}.negate"), sink)?,
        }),
        "ai_attribute" => Some(ViewNode::AiAttribute {
            attribute: parse_enum_str(
                get("attribute"),
                &format!("{path}.attribute"),
                &crate::catalog::AiAttributeKey::ALL.map(|k| k.as_str()),
                sink,
            )?,
            op: parse_enum_str(get("op"), &format!("{path}.op"), &AI_ATTRIBUTE_OPS, sink)?,
            value: parse_bounded_string(get("value"), &format!("{path}.value"), 1, 120, sink)?,
        }),
        "unread" => Some(ViewNode::Unread {
            unread: parse_bool(get("unread"), &format!("{path}.unread"), sink)?,
        }),
        "snoozed" => Some(ViewNode::Snoozed {
            snoozed: parse_bool(get("snoozed"), &format!("{path}.snoozed"), sink)?,
        }),
        "customer" => Some(ViewNode::Customer {
            customer_local_ids: parse_id_array(
                get("customerLocalIds"),
                &format!("{path}.customerLocalIds"),
                1,
                200,
                sink,
            )?,
        }),
        other => {
            let mut all: Vec<&str> = vec!["group"];
            all.extend(VIEW_CONDITION_KINDS);
            sink.push(
                &format!("{path}.kind"),
                format!(
                    "Invalid discriminator value. Expected {}, received '{other}'",
                    all.iter()
                        .map(|k| format!("'{k}'"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                ),
            );
            None
        }
    }
}

pub fn parse_view_definition(v: &Json) -> std::result::Result<ViewDefinition, Vec<ViewIssue>> {
    let mut sink = IssueSink { issues: Vec::new() };
    let Some(obj) = v.as_object() else {
        return Err(vec![ViewIssue {
            path: String::new(),
            message: format!("Expected object, received {}", json_type_name(v)),
        }]);
    };
    let combinator = parse_enum_str(
        obj.get("combinator"),
        "combinator",
        &["all", "any"],
        &mut sink,
    );
    let conditions = match obj.get("conditions") {
        None | Some(Json::Null) => {
            sink.push("conditions", "Required");
            None
        }
        Some(Json::Array(items)) => {
            if items.len() > 50 {
                sink.push(
                    "conditions",
                    format!(
                        "Array must contain at most 50 element(s), but the input had {}",
                        items.len()
                    ),
                );
                None
            } else {
                Some(items.clone())
            }
        }
        Some(other) => {
            sink.push(
                "conditions",
                format!("Expected array, received {}", json_type_name(other)),
            );
            None
        }
    };
    let mut nodes = Vec::new();
    if let Some(items) = conditions {
        for (i, item) in items.iter().enumerate() {
            if let Some(n) = parse_view_node(item, &format!("conditions.{i}"), &mut sink) {
                nodes.push(n);
            }
        }
    }
    if sink.is_empty() {
        Ok(ViewDefinition {
            combinator: combinator.expect("validated"),
            conditions: nodes,
        })
    } else {
        Err(sink.issues)
    }
}

pub fn parse_create_view_request(
    v: &Json,
) -> std::result::Result<CreateViewRequest, Vec<ViewIssue>> {
    let mut sink = IssueSink { issues: Vec::new() };
    let Some(obj) = v.as_object() else {
        return Err(vec![ViewIssue {
            path: String::new(),
            message: format!("Expected object, received {}", json_type_name(v)),
        }]);
    };
    let name = parse_bounded_string(obj.get("name"), "name", 1, 120, &mut sink);
    let description =
        parse_nullable_max_string(obj.get("description"), "description", 500, &mut sink);
    let definition = match obj.get("definition") {
        None | Some(Json::Null) => {
            sink.push("definition", "Required");
            None
        }
        Some(d) => match parse_view_definition(d) {
            Ok(def) => Some(def),
            Err(issues) => {
                for issue in issues {
                    sink.push(&format!("definition.{}", issue.path), issue.message);
                }
                None
            }
        },
    };
    let sort_order = match obj.get("sort_order") {
        None | Some(Json::Null) => None,
        Some(Json::Number(n)) => match n.as_i64() {
            Some(i) if (0..=9999).contains(&i) => Some(i),
            Some(_) => {
                sink.push("sort_order", "Number must be between 0 and 9999");
                None
            }
            None => {
                sink.push("sort_order", "Expected int, received float");
                None
            }
        },
        Some(other) => {
            sink.push(
                "sort_order",
                format!("Expected number, received {}", json_type_name(other)),
            );
            None
        }
    };
    let folder = parse_nullable_max_string(obj.get("folder"), "folder", 80, &mut sink);
    if sink.is_empty() {
        Ok(CreateViewRequest {
            name: name.expect("validated"),
            description: description.flatten(),
            definition: definition.expect("validated"),
            sort_order,
            folder: folder.flatten(),
        })
    } else {
        Err(sink.issues)
    }
}

pub fn parse_update_view_request(
    v: &Json,
) -> std::result::Result<UpdateViewRequest, Vec<ViewIssue>> {
    let mut sink = IssueSink { issues: Vec::new() };
    let Some(obj) = v.as_object() else {
        return Err(vec![ViewIssue {
            path: String::new(),
            message: format!("Expected object, received {}", json_type_name(v)),
        }]);
    };
    let name = match obj.get("name") {
        None | Some(Json::Null) => None,
        Some(_) => parse_bounded_string(obj.get("name"), "name", 1, 120, &mut sink),
    };
    let description = match obj.get("description") {
        None => None,
        Some(Json::Null) => Some(None),
        Some(_) => parse_nullable_max_string(obj.get("description"), "description", 500, &mut sink)
            .flatten()
            .map(Some),
    };
    let definition = match obj.get("definition") {
        None | Some(Json::Null) => None,
        Some(d) => match parse_view_definition(d) {
            Ok(def) => Some(def),
            Err(issues) => {
                for issue in issues {
                    sink.push(&format!("definition.{}", issue.path), issue.message);
                }
                None
            }
        },
    };
    let sort_order = match obj.get("sort_order") {
        None | Some(Json::Null) => None,
        Some(Json::Number(n)) => match n.as_i64() {
            Some(i) if (0..=9999).contains(&i) => Some(i),
            Some(_) => {
                sink.push("sort_order", "Number must be between 0 and 9999");
                None
            }
            None => {
                sink.push("sort_order", "Expected int, received float");
                None
            }
        },
        Some(other) => {
            sink.push(
                "sort_order",
                format!("Expected number, received {}", json_type_name(other)),
            );
            None
        }
    };
    let folder = match obj.get("folder") {
        None => None,
        Some(Json::Null) => Some(None),
        Some(_) => parse_nullable_max_string(obj.get("folder"), "folder", 80, &mut sink)
            .flatten()
            .map(Some),
    };
    if sink.is_empty() {
        Ok(UpdateViewRequest {
            name,
            description,
            definition,
            sort_order,
            folder,
        })
    } else {
        Err(sink.issues)
    }
}

pub type SqlParams = Vec<rusqlite::types::Value>;

struct Fragment {
    sql: String,
    params: SqlParams,
}

impl Fragment {
    fn empty() -> Self {
        Self {
            sql: String::new(),
            params: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CompiledView {
    pub where_sql: String,
    pub params: SqlParams,
    pub notes: Vec<String>,
}

pub type SlaResolver<'a> = Box<dyn Fn(&[&str]) -> Vec<i64> + 'a>;

pub struct ViewEngine<'a> {
    timezone: String,
    now: Option<i64>,
    resolve_sla: Option<SlaResolver<'a>>,
}

impl<'a> ViewEngine<'a> {
    #[must_use]
    pub fn new(timezone: impl Into<String>) -> Self {
        Self {
            timezone: timezone.into(),
            now: None,
            resolve_sla: None,
        }
    }

    #[must_use]
    pub fn with_now(mut self, now_ms: i64) -> Self {
        self.now = Some(now_ms);
        self
    }

    #[must_use]
    pub fn with_sla_resolver(mut self, resolver: SlaResolver<'a>) -> Self {
        self.resolve_sla = Some(resolver);
        self
    }

    pub fn compile(
        &self,
        def: &ViewDefinition,
    ) -> std::result::Result<CompiledView, ViewCompileError> {
        let mut notes = Vec::new();
        let fragment =
            self.compile_nodes(&def.conditions, def.combinator == "all", &mut notes, 0)?;
        if fragment.sql.is_empty() {
            return Ok(CompiledView {
                where_sql: "1=1".to_string(),
                params: Vec::new(),
                notes,
            });
        }
        Ok(CompiledView {
            where_sql: fragment.sql,
            params: fragment.params,
            notes,
        })
    }

    fn compile_nodes(
        &self,
        nodes: &[ViewNode],
        intersect: bool,
        notes: &mut Vec<String>,
        depth: u32,
    ) -> std::result::Result<Fragment, ViewCompileError> {
        if depth > MAX_TREE_DEPTH {
            return Err(ViewCompileError::new(
                "Condition tree nesting deeper than supported (max 10).",
            ));
        }
        let mut fragments = Vec::with_capacity(nodes.len());
        for node in nodes {
            fragments.push(self.compile_node(node, notes, depth)?);
        }
        let non_empty: Vec<&Fragment> = fragments.iter().filter(|f| !f.sql.is_empty()).collect();
        if non_empty.is_empty() {
            return Ok(Fragment::empty());
        }
        let joiner = if intersect { " AND " } else { " OR " };
        let sql = non_empty
            .iter()
            .map(|f| format!("({})", f.sql))
            .collect::<Vec<_>>()
            .join(joiner);
        let mut params = Vec::new();
        for f in non_empty {
            params.extend(f.params.iter().cloned());
        }
        Ok(Fragment { sql, params })
    }

    fn compile_node(
        &self,
        node: &ViewNode,
        notes: &mut Vec<String>,
        depth: u32,
    ) -> std::result::Result<Fragment, ViewCompileError> {
        if let ViewNode::Group {
            combinator,
            children,
        } = node
        {
            if children.is_empty() {
                return Ok(Fragment::empty());
            }
            return self.compile_nodes(children, combinator == "all", notes, depth + 1);
        }
        self.compile_condition(node, notes)
    }

    fn compile_condition(
        &self,
        cond: &ViewNode,
        notes: &mut Vec<String>,
    ) -> std::result::Result<Fragment, ViewCompileError> {
        match cond {
            ViewNode::Group { .. } => unreachable!("groups handled in compile_node"),
            ViewNode::Status { statuses } => {
                if statuses.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!("c.status IN ({})", placeholders(statuses.len()).join(",")),
                    params: statuses.iter().map(|s| text(s)).collect(),
                })
            }
            ViewNode::Assignee {
                assignee_local_ids,
                include_unassigned,
            } => {
                let mut parts: Vec<String> = Vec::new();
                let mut params = SqlParams::new();
                if !assignee_local_ids.is_empty() {
                    parts.push(format!(
                        "c.assignee_local_id IN ({})",
                        placeholders(assignee_local_ids.len()).join(",")
                    ));
                    params.extend(assignee_local_ids.iter().map(|id| integer(*id)));
                }
                if *include_unassigned {
                    parts.push("c.assignee_local_id IS NULL".to_string());
                }
                if parts.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: parts.join(" OR "),
                    params,
                })
            }
            ViewNode::Team { team_local_ids } => {
                if team_local_ids.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!(
                        "c.assigned_team_local_id IN ({})",
                        placeholders(team_local_ids.len()).join(",")
                    ),
                    params: team_local_ids.iter().map(|id| integer(*id)).collect(),
                })
            }
            ViewNode::Mailbox { mailbox_local_ids } => {
                if mailbox_local_ids.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!(
                        "c.mailbox_local_id IN ({})",
                        placeholders(mailbox_local_ids.len()).join(",")
                    ),
                    params: mailbox_local_ids.iter().map(|id| integer(*id)).collect(),
                })
            }
            ViewNode::Channel { channels } => {
                if channels.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!("c.type IN ({})", placeholders(channels.len()).join(",")),
                    params: channels.iter().map(|c| text(c)).collect(),
                })
            }
            ViewNode::Tags { tags, mode } => {
                let lowered: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();
                if lowered.is_empty() {
                    return Ok(Fragment::empty());
                }
                let tag_exists = |names: &[String]| {
                    Fragment {
                    sql: format!(
                        "EXISTS (SELECT 1 FROM conversation_tags ct JOIN tags t ON t.id = ct.tag_id WHERE ct.conversation_id = c.id AND LOWER(t.name) IN ({}))",
                        placeholders(names.len()).join(",")
                    ),
                    params: names.iter().map(|n| text(n)).collect(),
                }
                };
                if mode == "any" {
                    return Ok(tag_exists(&lowered));
                }
                if mode == "none" {
                    let any = tag_exists(&lowered);
                    return Ok(Fragment {
                        sql: format!("NOT {}", any.sql),
                        params: any.params,
                    });
                }
                let parts = lowered
                    .iter()
                    .map(|_| "EXISTS (SELECT 1 FROM conversation_tags ct JOIN tags t ON t.id = ct.tag_id WHERE ct.conversation_id = c.id AND LOWER(t.name) = ?)")
                    .collect::<Vec<_>>();
                Ok(Fragment {
                    sql: parts.join(" AND "),
                    params: lowered.iter().map(|n| text(n)).collect(),
                })
            }
            ViewNode::CustomField {
                field_local_id,
                op,
                value,
            } => {
                let base = "cf.field_id = ?";
                match op.as_str() {
                    "is_empty" => Ok(Fragment {
                        sql: format!("NOT EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND cf.value IS NOT NULL AND cf.value <> '')"),
                        params: vec![integer(*field_local_id)],
                    }),
                    "is_not_empty" => Ok(Fragment {
                        sql: format!("EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND cf.value IS NOT NULL AND cf.value <> '')"),
                        params: vec![integer(*field_local_id)],
                    }),
                    "equals" => Ok(Fragment {
                        sql: format!("EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND LOWER(cf.value) = LOWER(?))"),
                        params: vec![integer(*field_local_id), text(value.as_deref().unwrap_or(""))],
                    }),
                    "not_equals" => Ok(Fragment {
                        sql: format!("NOT EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND LOWER(cf.value) = LOWER(?))"),
                        params: vec![integer(*field_local_id), text(value.as_deref().unwrap_or(""))],
                    }),
                    "contains" => Ok(Fragment {
                        sql: format!("EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND cf.value LIKE ? ESCAPE '\\')"),
                        params: vec![integer(*field_local_id), text(&format!("%{}%", escape_like(value.as_deref().unwrap_or(""))))],
                    }),
                    other => Err(ViewCompileError::new(format!(
                        "Unsupported custom_field operator: {other}"
                    ))),
                }
            }
            ViewNode::CustomerProperty {
                definition_id,
                op,
                value,
            } => {
                let def = "cp.definition_id = ?";
                let cust_join = "cp.customer_id = c.customer_local_id";
                match op.as_str() {
                    "is_empty" => Ok(Fragment {
                        sql: format!("c.customer_local_id IS NULL OR NOT EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND cp.value IS NOT NULL AND cp.value <> '')"),
                        params: vec![integer(*definition_id)],
                    }),
                    "is_not_empty" => Ok(Fragment {
                        sql: format!("EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND cp.value IS NOT NULL AND cp.value <> '')"),
                        params: vec![integer(*definition_id)],
                    }),
                    "equals" => Ok(Fragment {
                        sql: format!("EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND LOWER(cp.value) = LOWER(?))"),
                        params: vec![integer(*definition_id), text(value.as_deref().unwrap_or(""))],
                    }),
                    "not_equals" => Ok(Fragment {
                        sql: format!("NOT EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND LOWER(cp.value) = LOWER(?))"),
                        params: vec![integer(*definition_id), text(value.as_deref().unwrap_or(""))],
                    }),
                    "contains" => Ok(Fragment {
                        sql: format!("EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND cp.value LIKE ? ESCAPE '\\')"),
                        params: vec![integer(*definition_id), text(&format!("%{}%", escape_like(value.as_deref().unwrap_or(""))))],
                    }),
                    "gt" | "gte" | "lt" | "lte" => {
                        let n = value
                            .as_deref()
                            .and_then(|v| v.parse::<f64>().ok())
                            .filter(|n| n.is_finite());
                        let Some(n) = n else {
                            return Err(ViewCompileError::new(
                                "customer_property numeric operator requires a numeric value.",
                            ));
                        };
                        let op_sql = match op.as_str() {
                            "gt" => ">",
                            "gte" => ">=",
                            "lt" => "<",
                            _ => "<=",
                        };
                        Ok(Fragment {
                            sql: format!("EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND CAST(cp.value AS REAL) {op_sql} ?)"),
                            params: vec![integer(*definition_id), real(n)],
                        })
                    }
                    other => Err(ViewCompileError::new(format!(
                        "Unsupported customer_property operator: {other}"
                    ))),
                }
            }
            ViewNode::CustomerText { field, op, value } => {
                let v = value.as_deref().unwrap_or("").trim();
                match field.as_str() {
                    "name" => {
                        let name_expr =
                            "TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, ''))";
                        match op.as_str() {
                            "is_empty" => Ok(Fragment {
                                sql: format!("c.customer_local_id IS NULL OR NOT EXISTS (SELECT 1 FROM customers cu WHERE cu.id = c.customer_local_id AND {name_expr} <> '')"),
                                params: Vec::new(),
                            }),
                            "is_not_empty" => Ok(Fragment {
                                sql: format!("EXISTS (SELECT 1 FROM customers cu WHERE cu.id = c.customer_local_id AND {name_expr} <> '')"),
                                params: Vec::new(),
                            }),
                            "contains" => Ok(Fragment {
                                sql: format!("EXISTS (SELECT 1 FROM customers cu WHERE cu.id = c.customer_local_id AND {name_expr} LIKE ? ESCAPE '\\')"),
                                params: vec![text(&format!("%{}%", escape_like(v)))],
                            }),
                            "equals" => Ok(Fragment {
                                sql: format!("EXISTS (SELECT 1 FROM customers cu WHERE cu.id = c.customer_local_id AND {name_expr} = ?)"),
                                params: vec![text(v)],
                            }),
                            "not_contains" => Ok(Fragment {
                                sql: format!("c.customer_local_id IS NULL OR NOT EXISTS (SELECT 1 FROM customers cu WHERE cu.id = c.customer_local_id AND {name_expr} LIKE ? ESCAPE '\\')"),
                                params: vec![text(&format!("%{}%", escape_like(v)))],
                            }),
                            other => Err(ViewCompileError::new(format!(
                                "Unsupported customer_text operator: {other}"
                            ))),
                        }
                    }
                    "email" => match op.as_str() {
                        "is_empty" => Ok(Fragment {
                            sql: "NOT EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.customer_local_id)".to_string(),
                            params: Vec::new(),
                        }),
                        "is_not_empty" => Ok(Fragment {
                            sql: "EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.customer_local_id)".to_string(),
                            params: Vec::new(),
                        }),
                        "contains" => Ok(Fragment {
                            sql: "EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.customer_local_id AND ce.value LIKE ? ESCAPE '\\')".to_string(),
                            params: vec![text(&format!("%{}%", escape_like(v)))],
                        }),
                        "equals" => Ok(Fragment {
                            sql: "EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.customer_local_id AND ce.value = ?)".to_string(),
                            params: vec![text(v)],
                        }),
                        "not_contains" => Ok(Fragment {
                            sql: "c.customer_local_id IS NULL OR NOT EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.customer_local_id AND ce.value LIKE ? ESCAPE '\\')".to_string(),
                            params: vec![text(&format!("%{}%", escape_like(v)))],
                        }),
                        other => Err(ViewCompileError::new(format!(
                            "Unsupported customer_text operator: {other}"
                        ))),
                    },
                    "organization" => match op.as_str() {
                        "is_empty" => Ok(Fragment {
                            sql: "c.customer_local_id IS NULL OR NOT EXISTS (SELECT 1 FROM customers cu WHERE cu.id = c.customer_local_id AND cu.organization_id IS NOT NULL AND EXISTS (SELECT 1 FROM organizations o WHERE o.id = cu.organization_id AND o.name <> ''))".to_string(),
                            params: Vec::new(),
                        }),
                        "is_not_empty" => Ok(Fragment {
                            sql: "EXISTS (SELECT 1 FROM customers cu JOIN organizations o ON o.id = cu.organization_id WHERE cu.id = c.customer_local_id AND o.name <> '')".to_string(),
                            params: Vec::new(),
                        }),
                        "contains" => Ok(Fragment {
                            sql: "EXISTS (SELECT 1 FROM customers cu JOIN organizations o ON o.id = cu.organization_id WHERE cu.id = c.customer_local_id AND o.name LIKE ? ESCAPE '\\')".to_string(),
                            params: vec![text(&format!("%{}%", escape_like(v)))],
                        }),
                        "equals" => Ok(Fragment {
                            sql: "EXISTS (SELECT 1 FROM customers cu JOIN organizations o ON o.id = cu.organization_id WHERE cu.id = c.customer_local_id AND o.name = ?)".to_string(),
                            params: vec![text(v)],
                        }),
                        "not_contains" => Ok(Fragment {
                            sql: "c.customer_local_id IS NULL OR NOT EXISTS (SELECT 1 FROM customers cu JOIN organizations o ON o.id = cu.organization_id WHERE cu.id = c.customer_local_id AND o.name LIKE ? ESCAPE '\\')".to_string(),
                            params: vec![text(&format!("%{}%", escape_like(v)))],
                        }),
                        other => Err(ViewCompileError::new(format!(
                            "Unsupported customer_text operator: {other}"
                        ))),
                    },
                    other => Err(ViewCompileError::new(format!(
                        "Unsupported customer_text field: {other}"
                    ))),
                }
            }
            ViewNode::DateActivity {
                activity_field,
                mode,
                from,
                to,
                from_time,
                to_time,
            } => {
                let range = resolve_date_range(&DateRangeInput {
                    mode,
                    timezone: &self.timezone,
                    from: from.as_deref(),
                    to: to.as_deref(),
                    from_time: from_time.as_deref(),
                    to_time: to_time.as_deref(),
                    now: self.now,
                });
                let Some(range) = range else {
                    return Err(ViewCompileError::new(format!(
                        "Date filter '{mode}' requires valid from/to dates (YYYY-MM-DD)."
                    )));
                };
                let Some(column) = activity_field_column(activity_field) else {
                    return Err(ViewCompileError::new(format!(
                        "Unknown activity field: {activity_field}"
                    )));
                };
                notes.push(format!(
                    "Date filter '{}' resolved to {} ({} to {}, {} boundaries, {}).",
                    activity_field,
                    range.label,
                    range.from,
                    range.to,
                    range.kind.as_str(),
                    self.timezone
                ));
                Ok(Fragment {
                    sql: format!("{column} IS NOT NULL AND {column} >= ? AND {column} < ?"),
                    params: vec![text(&range.from), text(&range.to)],
                })
            }
            ViewNode::ResponseState { states } => {
                if states.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!(
                        "({RESPONSE_STATE_SQL}) IN ({})",
                        placeholders(states.len()).join(",")
                    ),
                    params: states.iter().map(|s| text(s)).collect(),
                })
            }
            ViewNode::ResponseAge {
                metric,
                op,
                minutes,
            } => {
                let Some((metric_sql, _label)) = age_metric_sql(metric) else {
                    return Err(ViewCompileError::new(format!(
                        "Unsupported response_age metric: {metric}"
                    )));
                };
                let op_sql = match op.as_str() {
                    "gt" => ">",
                    "gte" => ">=",
                    "lt" => "<",
                    _ => "<=",
                };
                Ok(Fragment {
                    sql: format!("{metric_sql} {op_sql} ?"),
                    params: vec![real(*minutes)],
                })
            }
            ViewNode::Sla { states, negate } => {
                let Some(resolve) = &self.resolve_sla else {
                    return Err(ViewCompileError::new(
                        "SLA conditions require the SLA service.",
                    ));
                };
                let queried: Vec<&str> = states.iter().map(String::as_str).collect();
                let ids = resolve(&queried);
                notes.push(format!(
                    "SLA state evaluated live against current mailbox business-hours ({} conversation(s) currently {}).",
                    ids.len(),
                    states.join("/")
                ));
                if ids.is_empty() {
                    return Ok(Fragment {
                        sql: if *negate { "1=1" } else { "1=0" }.to_string(),
                        params: Vec::new(),
                    });
                }
                let in_list = placeholders(ids.len()).join(",");
                Ok(Fragment {
                    sql: if *negate {
                        format!("c.id NOT IN ({in_list})")
                    } else {
                        format!("c.id IN ({in_list})")
                    },
                    params: ids.iter().map(|id| integer(*id)).collect(),
                })
            }
            ViewNode::Priority { priorities } => {
                if priorities.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!(
                        "c.supportos_priority IN ({})",
                        placeholders(priorities.len()).join(",")
                    ),
                    params: priorities.iter().map(|p| text(p)).collect(),
                })
            }
            ViewNode::TicketState {
                state_ids,
                include_no_state,
            } => {
                let mut parts: Vec<String> = Vec::new();
                let mut params = SqlParams::new();
                if !state_ids.is_empty() {
                    parts.push(format!(
                        "c.supportos_state_id IN ({})",
                        placeholders(state_ids.len()).join(",")
                    ));
                    params.extend(state_ids.iter().map(|id| integer(*id)));
                }
                if *include_no_state {
                    parts.push("c.supportos_state_id IS NULL".to_string());
                }
                if parts.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: parts.join(" OR "),
                    params,
                })
            }
            ViewNode::KnownIssue {
                any,
                known_issue_ids,
            } => {
                if *any {
                    return Ok(Fragment {
                        sql: "EXISTS (SELECT 1 FROM known_issue_links kic WHERE kic.conversation_id = c.id)".to_string(),
                        params: Vec::new(),
                    });
                }
                let Some(ids) = known_issue_ids else {
                    return Ok(Fragment::empty());
                };
                if ids.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!(
                        "EXISTS (SELECT 1 FROM known_issue_links kic WHERE kic.conversation_id = c.id AND kic.known_issue_id IN ({}))",
                        placeholders(ids.len()).join(",")
                    ),
                    params: ids.iter().map(|id| integer(*id)).collect(),
                })
            }
            ViewNode::AiAnalyzed { analyzed } => Ok(Fragment {
                sql: if *analyzed {
                    "EXISTS (SELECT 1 FROM ai_runs ar WHERE ar.conversation_id = c.id AND ar.type = 'ticket_analysis' AND ar.status = 'completed')".to_string()
                } else {
                    "NOT EXISTS (SELECT 1 FROM ai_runs ar WHERE ar.conversation_id = c.id AND ar.type = 'ticket_analysis' AND ar.status = 'completed')".to_string()
                },
                params: Vec::new(),
            }),
            ViewNode::InteractionSignal {
                dimension,
                value,
                negate,
            } => {
                let frag = Fragment {
                    sql: "EXISTS (SELECT 1 FROM client_current_signals s, json_each(s.signals_json) je WHERE s.conversation_id = c.id AND json_extract(je.value, '$.dimension') = ? AND json_extract(je.value, '$.value') = ?)".to_string(),
                    params: vec![text(dimension), text(value)],
                };
                Ok(Fragment {
                    sql: if *negate {
                        format!("NOT {}", frag.sql)
                    } else {
                        frag.sql
                    },
                    params: frag.params,
                })
            }
            ViewNode::AiAttribute {
                attribute,
                op,
                value,
            } => {
                let Some(def) = crate::catalog::AiAttributeKey::parse(attribute) else {
                    return Err(ViewCompileError::new(format!(
                        "Unknown AI attribute: {attribute}"
                    )));
                };
                let attr = attribute.as_str();
                if value.to_lowercase() == "unknown" {
                    if op != "equals" {
                        return Err(ViewCompileError::new(
                            "Only 'equals unknown' is supported for the unknown value.",
                        ));
                    }
                    return Ok(Fragment {
                        sql: "NOT EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL)".to_string(),
                        params: vec![text(attr)],
                    });
                }
                if def.value_type() == crate::catalog::AttributeValueType::Number {
                    let Ok(n) = value.parse::<f64>() else {
                        return Err(ViewCompileError::new(
                            "ai_attribute numeric operator requires a numeric value.",
                        ));
                    };
                    if !n.is_finite() {
                        return Err(ViewCompileError::new(
                            "ai_attribute numeric operator requires a numeric value.",
                        ));
                    }
                    let op_sql = match op.as_str() {
                        "gt" => ">",
                        "gte" => ">=",
                        "lt" => "<",
                        "lte" => "<=",
                        _ => {
                            return Err(ViewCompileError::new(format!(
                                "ai_attribute (number) does not support operator {op}."
                            )));
                        }
                    };
                    return Ok(Fragment {
                        sql: format!("EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL AND CAST(aa.value AS REAL) {op_sql} ?)"),
                        params: vec![text(attr), real(n)],
                    });
                }
                match op.as_str() {
                    "equals" => Ok(Fragment {
                        sql: "EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL AND LOWER(aa.value) = LOWER(?))".to_string(),
                        params: vec![text(attr), text(value)],
                    }),
                    "not_equals" => Ok(Fragment {
                        sql: "NOT EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL AND LOWER(aa.value) = LOWER(?))".to_string(),
                        params: vec![text(attr), text(value)],
                    }),
                    "contains" => Ok(Fragment {
                        sql: "EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL AND aa.value LIKE ? ESCAPE '\\')".to_string(),
                        params: vec![text(attr), text(&format!("%{}%", escape_like(value)))],
                    }),
                    "not_contains" => Ok(Fragment {
                        sql: "NOT EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL AND aa.value LIKE ? ESCAPE '\\')".to_string(),
                        params: vec![text(attr), text(&format!("%{}%", escape_like(value)))],
                    }),
                    "gt" | "gte" | "lt" | "lte" => {
                        let vocab = def.values();
                        let Some(idx) = vocab.iter().position(|v| *v == value) else {
                            return Err(ViewCompileError::new(format!(
                                "ai_attribute '{attr}' comparison requires a value from its vocabulary ({}).",
                                vocab.join(", ")
                            )));
                        };
                        let bounds: Vec<&str> = match op.as_str() {
                            "gt" => vocab[idx + 1..].to_vec(),
                            "gte" => vocab[idx..].to_vec(),
                            "lt" => vocab[..idx].to_vec(),
                            _ => vocab[..idx + 1].to_vec(),
                        };
                        if bounds.is_empty() {
                            return Ok(Fragment {
                                sql: "1=0".to_string(),
                                params: Vec::new(),
                            });
                        }
                        let mut params = vec![text(attr)];
                        params.extend(bounds.iter().map(|b| text(b)));
                        Ok(Fragment {
                            sql: format!(
                                "EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL AND aa.value IN ({}))",
                                placeholders(bounds.len()).join(",")
                            ),
                            params,
                        })
                    }
                    other => Err(ViewCompileError::new(format!(
                        "Unsupported ai_attribute operator: {other}"
                    ))),
                }
            }
            ViewNode::Unread { unread } => Ok(Fragment {
                sql: "c.is_unread = ?".to_string(),
                params: vec![integer(i64::from(*unread))],
            }),
            ViewNode::Snoozed { snoozed } => Ok(Fragment {
                sql: if *snoozed {
                    "c.snoozed_until IS NOT NULL AND c.snoozed_until > datetime('now')".to_string()
                } else {
                    "c.snoozed_until IS NULL OR c.snoozed_until <= datetime('now')".to_string()
                },
                params: Vec::new(),
            }),
            ViewNode::Customer { customer_local_ids } => {
                if customer_local_ids.is_empty() {
                    return Ok(Fragment::empty());
                }
                Ok(Fragment {
                    sql: format!(
                        "c.customer_local_id IN ({})",
                        placeholders(customer_local_ids.len()).join(",")
                    ),
                    params: customer_local_ids.iter().map(|id| integer(*id)).collect(),
                })
            }
        }
    }
}

fn placeholders(n: usize) -> Vec<&'static str> {
    vec!["?"; n]
}

fn text(s: &str) -> rusqlite::types::Value {
    rusqlite::types::Value::Text(s.to_string())
}

fn integer(i: i64) -> rusqlite::types::Value {
    rusqlite::types::Value::Integer(i)
}

fn real(f: f64) -> rusqlite::types::Value {
    rusqlite::types::Value::Real(f)
}

#[must_use]
pub fn escape_like(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for ch in v.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

pub fn ensure_inbox_views_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS inbox_views (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT,
            definition TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            folder TEXT,
            version INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;
    Ok(())
}

fn now_iso() -> String {
    iso_utc(now_ms_epoch())
}

fn row_to_view(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedInboxView> {
    let definition_json: String = row.get(3)?;
    let definition: ViewDefinition = serde_json::from_str(&definition_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(SavedInboxView {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        definition,
        sort_order: row.get(4)?,
        folder: row.get(5)?,
        version: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

const VIEW_COLUMNS: &str =
    "id, name, description, definition, sort_order, folder, version, created_at, updated_at";

pub fn list_views(conn: &Connection) -> Result<Vec<SavedInboxView>> {
    ensure_inbox_views_table(conn)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {VIEW_COLUMNS} FROM inbox_views ORDER BY sort_order ASC, name ASC"
    ))?;
    let views = stmt
        .query_map([], row_to_view)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(views)
}

pub fn get_view(conn: &Connection, id: i64) -> Result<Option<SavedInboxView>> {
    ensure_inbox_views_table(conn)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {VIEW_COLUMNS} FROM inbox_views WHERE id = ?1"
    ))?;
    let view = stmt.query_row(rusqlite::params![id], row_to_view).ok();
    Ok(view)
}

pub fn create_view(
    conn: &Connection,
    name: &str,
    description: Option<&str>,
    definition: &ViewDefinition,
    sort_order: i64,
    folder: Option<&str>,
) -> Result<SavedInboxView> {
    ensure_inbox_views_table(conn)?;
    let definition_json = serde_json::to_string(definition)?;
    conn.execute(
        "INSERT INTO inbox_views (name, description, definition, sort_order, folder) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![name, description, definition_json, sort_order, folder],
    )?;
    let id = conn.last_insert_rowid();
    get_view(conn, id)?.ok_or_else(|| {
        crate::error::Error::Config(format!("saved view {id} vanished after insert"))
    })
}

pub fn update_view(
    conn: &Connection,
    id: i64,
    name: Option<&str>,
    description: Option<Option<&str>>,
    definition: Option<&ViewDefinition>,
    sort_order: Option<i64>,
    folder: Option<Option<&str>>,
) -> Result<Option<SavedInboxView>> {
    let Some(existing) = get_view(conn, id)? else {
        return Ok(None);
    };
    let new_definition_json = definition.map(serde_json::to_string).transpose()?;
    let definition_changed = new_definition_json.as_deref().is_some_and(|new| {
        Some(new) != serde_json::to_string(&existing.definition).ok().as_deref()
    });
    let new_name = name.unwrap_or(&existing.name);
    let new_description = description
        .map(|d| d.map(String::from))
        .unwrap_or_else(|| existing.description.clone());
    let new_definition = new_definition_json
        .unwrap_or_else(|| serde_json::to_string(&existing.definition).unwrap_or_default());
    let new_sort = sort_order.unwrap_or(existing.sort_order);
    let new_folder = folder
        .map(|f| f.map(String::from))
        .unwrap_or_else(|| existing.folder.clone());
    conn.execute(
        "UPDATE inbox_views SET
           name = ?1, description = ?2, definition = ?3, sort_order = ?4, folder = ?5,
           version = CASE WHEN ?6 THEN version + 1 ELSE version END,
           updated_at = ?7
         WHERE id = ?8",
        rusqlite::params![
            new_name,
            new_description,
            new_definition,
            new_sort,
            new_folder,
            i64::from(definition_changed),
            now_iso(),
            id
        ],
    )?;
    get_view(conn, id)
}

pub fn delete_view(conn: &Connection, id: i64) -> Result<bool> {
    ensure_inbox_views_table(conn)?;
    let changes = conn.execute(
        "DELETE FROM inbox_views WHERE id = ?1",
        rusqlite::params![id],
    )?;
    Ok(changes > 0)
}
