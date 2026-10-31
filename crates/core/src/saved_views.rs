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
                // Audit item 119: the Rust side folded with Unicode rules
                // (`str::to_lowercase`) while SQL `LOWER` folds ASCII
                // only — a tag stored as "ELECTRONICS" (or any name with
                // a non-ASCII letter) produced a parameter the SQL side
                // could never match. Both sides now fold ASCII-only, so
                // the filter agrees with the stored collation: ASCII
                // tags match case-insensitively, non-ASCII names compare
                // symmetrically on both sides.
                let lowered: Vec<String> = tags.iter().map(|t| t.to_ascii_lowercase()).collect();
                if lowered.is_empty() {
                    return Ok(Fragment::empty());
                }
                let tag_exists = |names: &[String]| Fragment {
                    sql: format!(
                        "EXISTS (SELECT 1 FROM conversation_tags ct JOIN tags t ON t.id = ct.tag_id WHERE ct.conversation_id = c.id AND LOWER(t.name) IN ({}))",
                        placeholders(names.len()).join(",")
                    ),
                    params: names.iter().map(|n| text(n)).collect(),
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
                        sql: format!(
                            "NOT EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND cf.value IS NOT NULL AND cf.value <> '')"
                        ),
                        params: vec![integer(*field_local_id)],
                    }),
                    "is_not_empty" => Ok(Fragment {
                        sql: format!(
                            "EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND cf.value IS NOT NULL AND cf.value <> '')"
                        ),
                        params: vec![integer(*field_local_id)],
                    }),
                    "equals" => Ok(Fragment {
                        sql: format!(
                            "EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND LOWER(cf.value) = LOWER(?))"
                        ),
                        params: vec![
                            integer(*field_local_id),
                            text(value.as_deref().unwrap_or("")),
                        ],
                    }),
                    "not_equals" => Ok(Fragment {
                        sql: format!(
                            "NOT EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND LOWER(cf.value) = LOWER(?))"
                        ),
                        params: vec![
                            integer(*field_local_id),
                            text(value.as_deref().unwrap_or("")),
                        ],
                    }),
                    "contains" => Ok(Fragment {
                        sql: format!(
                            "EXISTS (SELECT 1 FROM conversation_fields cf WHERE cf.conversation_id = c.id AND {base} AND cf.value LIKE ? ESCAPE '\\')"
                        ),
                        params: vec![
                            integer(*field_local_id),
                            text(&format!(
                                "%{}%",
                                escape_like(value.as_deref().unwrap_or(""))
                            )),
                        ],
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
                        sql: format!(
                            "c.customer_local_id IS NULL OR NOT EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND cp.value IS NOT NULL AND cp.value <> '')"
                        ),
                        params: vec![integer(*definition_id)],
                    }),
                    "is_not_empty" => Ok(Fragment {
                        sql: format!(
                            "EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND cp.value IS NOT NULL AND cp.value <> '')"
                        ),
                        params: vec![integer(*definition_id)],
                    }),
                    "equals" => Ok(Fragment {
                        sql: format!(
                            "EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND LOWER(cp.value) = LOWER(?))"
                        ),
                        params: vec![
                            integer(*definition_id),
                            text(value.as_deref().unwrap_or("")),
                        ],
                    }),
                    "not_equals" => Ok(Fragment {
                        sql: format!(
                            "NOT EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND LOWER(cp.value) = LOWER(?))"
                        ),
                        params: vec![
                            integer(*definition_id),
                            text(value.as_deref().unwrap_or("")),
                        ],
                    }),
                    "contains" => Ok(Fragment {
                        sql: format!(
                            "EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} AND cp.value LIKE ? ESCAPE '\\')"
                        ),
                        params: vec![
                            integer(*definition_id),
                            text(&format!(
                                "%{}%",
                                escape_like(value.as_deref().unwrap_or(""))
                            )),
                        ],
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
                        // Audit item 122: `CAST(cp.value AS REAL)` turns
                        // any non-numeric text into 0.0 — "N/A", "blue",
                        // "yes" all satisfied `lt`/`lte` comparisons
                        // against positive thresholds. Only values that
                        // are actually numeric participate now: at least
                        // one digit, and nothing outside a number's
                        // character set (sign, digits, decimal point,
                        // exponent).
                        Ok(Fragment {
                            sql: format!(
                                "EXISTS (SELECT 1 FROM customer_properties cp WHERE {cust_join} AND {def} \
                                  AND TRIM(cp.value) GLOB '*[0-9]*' \
                                  AND TRIM(cp.value) NOT GLOB '*[^0-9.+-eE]*' \
                                  AND CAST(cp.value AS REAL) {op_sql} ?)"
                            ),
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
                        sql: format!(
                            "EXISTS (SELECT 1 FROM ai_attributes aa WHERE aa.conversation_id = c.id AND aa.attribute = ? AND aa.superseded_at IS NULL AND CAST(aa.value AS REAL) {op_sql} ?)"
                        ),
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

    /// Audit item 119: the Rust side folded the filter value with Unicode
    /// rules while SQL LOWER folds ASCII only — a non-ASCII filter value
    /// produced a parameter the SQL side could never match. Both sides
    /// fold ASCII-only now, so a stored non-ASCII tag matches its own
    /// uppercase spelling, and ASCII tags stay case-insensitive.
    #[test]
    fn tag_filter_folds_consistently_with_sql() {
        let conn = db();
        let ascii = seed_tagged(&conn, 1, "electronics");
        let accent = seed_tagged(&conn, 2, "ÉCLAIR");

        // ASCII: case-insensitive both pre- and post-fix.
        assert_eq!(tag_where(&conn, &["ELECTRONICS"]), vec![ascii]);

        // Non-ASCII: pre-fix the Rust fold rewrote É→é and SQL LOWER left
        // the stored É — the stored tag could never match its own name.
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
        // CustomerProperty fragments run on the conversations surface
        // (cp.customer_id = c.customer_local_id), so each customer gets a
        // conversation to match on.
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

    /// Audit item 122: `CAST(cp.value AS REAL)` turned any non-numeric
    /// text into 0.0, so "N/A" satisfied every `lt` comparison against a
    /// positive threshold. Only numeric values participate now.
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
