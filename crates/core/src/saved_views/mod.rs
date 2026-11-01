use rusqlite::Connection;
use serde::{Deserialize, Serialize};

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

pub use crate::business_hours::is_valid_timezone;

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
        if let Some(from_min) = parse_time_of_day(input.from_time)
            && let Some((fy, fmo, fd, _, _)) = wall_parts(f, &tz)
            && let Some(inst) = local_instant_ms(&tz, fy, fmo, fd, from_min)
        {
            f = inst;
        }
        if let Some(to_min) = parse_time_of_day(input.to_time)
            && let Some((ly, lmo, ld, _, _)) = wall_parts(t - 1, &tz)
            && let Some(inst) = local_instant_ms(&tz, ly, lmo, ld, to_min)
        {
            t = inst;
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

mod engine;

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

pub use crate::search::escape_like;

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

#[cfg(test)]
mod batch31_tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn seed_tagged(conn: &Connection, remote: i64, tag: &str) -> i64 {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (?1, ?1, 'tag seed', 'p', 'active')",
            [remote],
        )
        .expect("conversation");
        let conv = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tags (remote_id, name) VALUES (?1, ?2)",
            rusqlite::params![remote, tag],
        )
        .expect("tag");
        conn.execute(
            "INSERT INTO conversation_tags (conversation_id, tag_id) VALUES (?1, ?2)",
            rusqlite::params![conv, conn.last_insert_rowid()],
        )
        .expect("tag link");
        conv
    }

    fn tag_where(conn: &Connection, tags: &[&str]) -> Vec<i64> {
        let engine = ViewEngine::new("UTC");
        let def = ViewDefinition {
            combinator: "all".to_string(),
            conditions: vec![ViewNode::Tags {
                tags: tags.iter().map(|t| t.to_string()).collect(),
                mode: "any".to_string(),
            }],
        };
        let compiled = engine.compile(&def).expect("compile");
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id FROM conversations c WHERE c.deleted_at IS NULL AND {}",
                compiled.where_sql
            ))
            .expect("prepare");
        let refs: Vec<&dyn rusqlite::ToSql> = compiled
            .params
            .iter()
            .map(|p| p as &dyn rusqlite::ToSql)
            .collect();
        stmt.query_map(refs.as_slice(), |r| r.get(0))
            .expect("query")
            .filter_map(|x| x.ok())
            .collect()
    }

    #[test]
    fn tag_filter_folds_consistently_with_sql() {
        let conn = db();
        let ascii = seed_tagged(&conn, 1, "electronics");
        let accent = seed_tagged(&conn, 2, "ÉCLAIR");

        assert_eq!(tag_where(&conn, &["ELECTRONICS"]), vec![ascii]);

        assert_eq!(tag_where(&conn, &["ÉCLAIR"]), vec![accent]);
    }

    fn seed_property(conn: &Connection, remote: i64, value: &str) -> i64 {
        conn.execute(
            "INSERT INTO customers (remote_id, first_name) VALUES (?1, 'C')",
            [remote],
        )
        .expect("customer");
        let customer = conn.last_insert_rowid();
        let def: i64 = conn
            .query_row(
                "SELECT id FROM customer_property_definitions WHERE slug = 'seats'",
                [],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| {
                conn.execute(
                    "INSERT INTO customer_property_definitions (remote_id, name, slug, type)
                     VALUES (1, 'Seats', 'seats', 'number')",
                    [],
                )
                .expect("definition");
                conn.last_insert_rowid()
            });
        conn.execute(
            "INSERT INTO customer_properties (customer_id, definition_id, value) VALUES (?1, ?2, ?3)",
            rusqlite::params![customer, def, value],
        )
        .expect("property");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, customer_local_id)
             VALUES (?1, ?1, 'prop conv', 'p', 'active', ?2)",
            rusqlite::params![remote, customer],
        )
        .expect("conversation");
        conn.last_insert_rowid()
    }

    fn numeric_lt(conn: &Connection, n: f64) -> Vec<i64> {
        let def: i64 = conn
            .query_row(
                "SELECT id FROM customer_property_definitions WHERE slug = 'seats'",
                [],
                |r| r.get(0),
            )
            .expect("definition id");
        let engine = ViewEngine::new("UTC");
        let def_view = ViewDefinition {
            combinator: "all".to_string(),
            conditions: vec![ViewNode::CustomerProperty {
                definition_id: def,
                op: "lt".to_string(),
                value: Some(n.to_string()),
            }],
        };
        let compiled = engine.compile(&def_view).expect("compile");
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id FROM conversations c WHERE c.deleted_at IS NULL AND {}",
                compiled.where_sql
            ))
            .expect("prepare");
        let refs: Vec<&dyn rusqlite::ToSql> = compiled
            .params
            .iter()
            .map(|p| p as &dyn rusqlite::ToSql)
            .collect();
        stmt.query_map(refs.as_slice(), |r| r.get(0))
            .expect("query")
            .filter_map(|x| x.ok())
            .collect()
    }

    #[test]
    fn numeric_property_filter_ignores_non_numeric_text() {
        let conn = db();
        let small = seed_property(&conn, 1, "5");
        let spaced = seed_property(&conn, 2, " 7 ");
        seed_property(&conn, 3, "N/A");
        seed_property(&conn, 4, "blue");
        let big = seed_property(&conn, 5, "15");

        let mut hits = numeric_lt(&conn, 10.0);
        hits.sort_unstable();
        assert_eq!(hits, vec![small, spaced], "only real numbers below 10");

        let mut all = numeric_lt(&conn, 100.0);
        all.sort_unstable();
        assert_eq!(all.len(), 3, "15 also matches at a higher threshold");
        assert!(all.contains(&big), "15 participates at a higher threshold");
    }
}
