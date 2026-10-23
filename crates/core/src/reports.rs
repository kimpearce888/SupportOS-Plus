use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::catalog::{GraphNodeKind, ReportDimensionKey, ReportMetricKey};
use crate::error::Result;

pub const M020_TO_M022_SQL: &str = r#"
    -- M020: friction_scores
    CREATE TABLE IF NOT EXISTS friction_scores (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        conversation_id INTEGER NOT NULL,
        effort_score    REAL NOT NULL,
        factors_json    TEXT,
        created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_friction_scores_conv
        ON friction_scores (conversation_id);

    -- M021: customer_timeline
    CREATE TABLE IF NOT EXISTS customer_timeline (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        customer_id     INTEGER NOT NULL,
        event_type      TEXT NOT NULL,
        event_data_json TEXT,
        occurred_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_customer_timeline_customer
        ON customer_timeline (customer_id, occurred_at);

    -- M022: graph_nodes (the human-edge store `graph_edges` — the
    -- reference's support_graph_edges under the documented rename — is
    -- created by support_graph::ensure_graph_edges_schema in the reference
    -- migration-016 shape; GR-02)
    CREATE TABLE IF NOT EXISTS graph_nodes (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        kind            TEXT NOT NULL,
        entity_id       INTEGER,
        label           TEXT,
        properties_json TEXT,
        created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_graph_nodes_kind
        ON graph_nodes (kind, entity_id);

    UPDATE app_state SET schema_version = 22 WHERE id = 1;
"#;

pub fn apply_m020_to_m022(conn: &Connection) -> Result<()> {
    conn.execute_batch(M020_TO_M022_SQL)?;
    crate::support_graph::ensure_graph_edges_schema(conn)?;
    Ok(())
}

pub const M034_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS report_definitions (
        id         INTEGER PRIMARY KEY AUTOINCREMENT,
        name       TEXT NOT NULL,
        config     TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at TEXT NOT NULL DEFAULT (datetime('now')),
        provenance TEXT NOT NULL DEFAULT 'human_local'
    );
    UPDATE app_state SET schema_version = 34 WHERE id = 1;
"#;

pub fn apply_m034(conn: &Connection) -> Result<()> {
    conn.execute_batch(M034_SQL)?;
    ensure_mailbox_business_hours(conn)?;
    Ok(())
}

pub fn ensure_mailbox_business_hours(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS mailbox_business_hours (
            mailbox_local_id INTEGER PRIMARY KEY,
            timezone TEXT NOT NULL DEFAULT 'UTC',
            days TEXT NOT NULL DEFAULT '[1,2,3,4,5]',
            start_minute INTEGER NOT NULL DEFAULT 540,
            end_minute INTEGER NOT NULL DEFAULT 1020,
            first_response_target_min INTEGER,
            resolution_target_min INTEGER,
            updated_at TEXT NOT NULL
        );",
    )?;
    Ok(())
}

pub fn ensure_report_definitions_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS report_definitions (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            name       TEXT NOT NULL,
            config     TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            provenance TEXT NOT NULL DEFAULT 'human_local'
        );",
    )?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DashboardMetrics {
    pub total_conversations: u32,
    pub new_conversations: u32,
    pub closed_conversations: u32,
    pub active_conversations: u32,
    pub customer_waiting: u32,
    pub avg_first_response_minutes: Option<f64>,
    pub avg_resolution_minutes: Option<f64>,
    pub sla_breach_count: u32,
}

pub fn get_dashboard_metrics(
    conn: &Connection,
    mailbox_id: Option<i64>,
    days_back: u32,
) -> Result<DashboardMetrics> {
    let days_back = days_back.max(1) as i64;
    let mailbox_clause = mailbox_id
        .map(|mid| format!("AND mailbox_local_id = {mid}"))
        .unwrap_or_default();

    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM conversations WHERE 1=1 {mailbox_clause}"),
        [],
        |r| r.get(0),
    )?;

    let new_conv: i64 = conn.query_row(
        &format!(
            // Audit item 109: the "new conversations" window keyed on
            // `local_created_at` — the LOCAL INSERT stamp (schema default
            // datetime('now')), never the platform payload — so every
            // backfilled/historical conversation counted as new on the
            // day it was synced. It also counted soft-deleted rows. The
            // window now keys on the real creation stamp and excludes
            // deleted conversations.
            "SELECT COUNT(*) FROM conversations
             WHERE julianday(COALESCE(remote_created_at, created_at)) >= julianday('now', '-{days_back} days')
               AND deleted_at IS NULL
             {mailbox_clause}"
        ),
        [],
        |r| r.get(0),
    )?;

    let closed: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM conversations WHERE status = 'closed' {mailbox_clause}"),
        [],
        |r| r.get(0),
    )?;

    let active: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM conversations WHERE status = 'active' {mailbox_clause}"),
        [],
        |r| r.get(0),
    )?;

    let waiting: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM conversations
             WHERE response_state = 'customer_waiting' {mailbox_clause}"
        ),
        [],
        |r| r.get(0),
    )?;

    let avg_first_response: Option<f64> = conn
        .query_row(
            &format!(
                "SELECT AVG((julianday(first_response_at) - julianday(created_at)) * 24 * 60)
                 FROM conversations
                 WHERE first_response_at IS NOT NULL AND created_at IS NOT NULL
                 {mailbox_clause}"
            ),
            [],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    let avg_resolution: Option<f64> = conn
        .query_row(
            &format!(
                "SELECT AVG((julianday(closed_at) - julianday(created_at)) * 24 * 60)
                 FROM conversations
                 WHERE closed_at IS NOT NULL AND created_at IS NOT NULL
                 AND status = 'closed' {mailbox_clause}"
            ),
            [],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    let sla_breaches: i64 = conn
        .query_row("SELECT COUNT(*) FROM sla_breaches", [], |r| r.get(0))
        .unwrap_or(0);

    Ok(DashboardMetrics {
        total_conversations: u32::try_from(total).unwrap_or(0),
        new_conversations: u32::try_from(new_conv).unwrap_or(0),
        closed_conversations: u32::try_from(closed).unwrap_or(0),
        active_conversations: u32::try_from(active).unwrap_or(0),
        customer_waiting: u32::try_from(waiting).unwrap_or(0),
        avg_first_response_minutes: avg_first_response,
        avg_resolution_minutes: avg_resolution,
        sla_breach_count: u32::try_from(sla_breaches).unwrap_or(0),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportRow {
    pub dimension_value: String,
    pub metric_value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportResult {
    pub metric: String,
    pub dimension: String,
    pub rows: Vec<ReportRow>,
    pub previous_period_rows: Vec<ReportRow>,
    pub metric_definition: String,
    pub metric_limitations: String,
}

pub fn build_report(
    conn: &Connection,
    metric: ReportMetricKey,
    dimension: ReportDimensionKey,
    days_back: u32,
) -> Result<ReportResult> {
    let days_back = days_back.max(1) as i64;

    // The conversation's real creation stamp — the platform payload's
    // createdAt, with the remote alias first — the same key the rest of
    // the reporting surface (digest, segments, AI pipeline) uses.
    // Audit items 109 + 110: the windows and the time dimensions keyed on
    // `local_created_at` (the local insert stamp) and the week dimension
    // paired `%Y` with `%W` — `2025-W53`/`2026-W00` split one natural
    // week at the year boundary. Buckets and windows now share the
    // creation stamp, and weeks are ISO (`%G`-`%V`, SQLite >= 3.44), so
    // Dec 29 and Jan 1 land in the same week bucket.
    let created_stamp = "COALESCE(remote_created_at, created_at)";

    let (dim_expr, dim_label) = match dimension {
        ReportDimensionKey::None => ("'all'".to_string(), "all".to_string()),
        ReportDimensionKey::Day => (
            format!("strftime('%Y-%m-%d', {created_stamp})"),
            "day".to_string(),
        ),
        ReportDimensionKey::Week => (
            format!("strftime('%G-W%V', {created_stamp})"),
            "week".to_string(),
        ),
        ReportDimensionKey::Month => (
            format!("strftime('%Y-%m', {created_stamp})"),
            "month".to_string(),
        ),
        ReportDimensionKey::Mailbox => ("mailbox_local_id".to_string(), "mailbox".to_string()),
        ReportDimensionKey::Status => ("status".to_string(), "status".to_string()),
        ReportDimensionKey::Assignee => ("assignee_local_id".to_string(), "assignee".to_string()),
        _ => ("'all'".to_string(), "all".to_string()),
    };

    let (metric_expr, definition, limitations) = metric_info(metric);

    let sql = format!(
        // Audit item 109: the metric limitation for Conversations claims
        // "Does not include deleted conversations" — no filter did. Every
        // report row now excludes soft-deleted conversations and keys the
        // period window on the creation stamp instead of the local insert stamp.
        "SELECT {dim_expr} as dim, {metric_expr} as val
         FROM conversations
         WHERE julianday({created_stamp}) >= julianday('now', '-{days_back} days')
           AND deleted_at IS NULL
         GROUP BY {dim_expr}
         ORDER BY {dim_expr}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let current_rows: Vec<ReportRow> = stmt
        .query_map([], |r| {
            Ok(ReportRow {
                dimension_value: r.get::<_, String>(0)?,
                metric_value: r.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
            })
        })?
        .filter_map(std::result::Result::ok)
        .collect();

    let prev_days_start = days_back * 2;
    let prev_sql = format!(
        "SELECT {dim_expr} as dim, {metric_expr} as val
         FROM conversations
         WHERE julianday({created_stamp}) >= julianday('now', '-{prev_days_start} days')
           AND julianday({created_stamp}) < julianday('now', '-{days_back} days')
           AND deleted_at IS NULL
         GROUP BY {dim_expr}
         ORDER BY {dim_expr}"
    );
    let mut prev_stmt = conn.prepare(&prev_sql)?;
    let prev_rows: Vec<ReportRow> = prev_stmt
        .query_map([], |r| {
            Ok(ReportRow {
                dimension_value: r.get::<_, String>(0)?,
                metric_value: r.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
            })
        })?
        .filter_map(std::result::Result::ok)
        .collect();

    Ok(ReportResult {
        metric: metric.as_str().to_string(),
        dimension: dim_label.to_string(),
        rows: current_rows,
        previous_period_rows: prev_rows,
        metric_definition: definition.to_string(),
        metric_limitations: limitations.to_string(),
    })
}

#[must_use]
fn metric_info(metric: ReportMetricKey) -> (&'static str, &'static str, &'static str) {
    match metric {
        ReportMetricKey::Conversations => (
            "COUNT(*)",
            "Total number of conversations.",
            "Does not include deleted conversations.",
        ),
        ReportMetricKey::UniqueCustomers => (
            "COUNT(DISTINCT customer_local_id)",
            "Number of unique customers who contacted support.",
            "A customer with multiple conversations is counted once.",
        ),
        ReportMetricKey::Organizations => (
            "COUNT(DISTINCT customer_local_id)",
            "Approximate organization count (proxy via distinct customers).",
            "True organization count requires the customers table's organization field; this is a proxy.",
        ),
        ReportMetricKey::FirstResponses => (
            "COUNT(CASE WHEN first_response_at IS NOT NULL THEN 1 END)",
            "Number of conversations that received a first response.",
            "Only counts conversations with a recorded first_response_at.",
        ),
        ReportMetricKey::AgentReplies => (
            "COUNT(*)",
            "Total conversations (proxy for agent reply count).",
            "True agent reply count requires message-level data; this is a proxy at conversation level.",
        ),
        ReportMetricKey::CustomerReplies => (
            "COUNT(*)",
            "Total conversations (proxy for customer reply count).",
            "True customer reply count requires message-level data.",
        ),
        ReportMetricKey::Closures => (
            "COUNT(CASE WHEN status = 'closed' THEN 1 END)",
            "Number of closed conversations.",
            "Only counts conversations with status='closed'.",
        ),
        ReportMetricKey::AvgFirstResponseMinutes => (
            "AVG((julianday(first_response_at) - julianday(created_at)) * 24 * 60)",
            "Average time to first response in minutes.",
            "Only includes conversations with a first_response_at; NULL values are excluded.",
        ),
        ReportMetricKey::AvgResolutionMinutes => (
            "AVG((julianday(closed_at) - julianday(created_at)) * 24 * 60)",
            "Average time from creation to closure in minutes.",
            "Only includes closed conversations.",
        ),
        ReportMetricKey::AvgWaitHours => (
            "AVG((julianday('now') - julianday(customer_waiting_since)) * 24)",
            "Average customer waiting time in hours.",
            "Only includes conversations currently in 'customer_waiting' state.",
        ),
        ReportMetricKey::SlaBreached => (
            "COUNT(CASE WHEN status = 'closed' AND closed_at IS NOT NULL THEN 1 END)",
            "Proxy for SLA breaches (closed conversations).",
            "True SLA breach count comes from the sla_breaches table; this is a conversation-level proxy.",
        ),
        _ => (
            "COUNT(*)",
            "Generic count metric.",
            "This metric's specific definition is not yet implemented; falls back to conversation count.",
        ),
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReportFilters {
    pub mailbox_local_ids: Option<Vec<i64>>,
    pub channel: Option<String>,
    pub tags_any: Option<Vec<String>>,
    pub tags_none: Option<Vec<String>>,
    pub statuses: Option<Vec<String>>,
    pub assignee_local_ids: Option<Vec<i64>>,
    pub min_priority: Option<String>,
    pub attribute_key: Option<String>,
    pub attribute_value: Option<String>,
    pub state_key: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReportComparison {
    #[default]
    None,
    PreviousPeriod,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReportSort {
    #[default]
    MetricDesc,
    MetricAsc,
    DimensionAsc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReportBuilderConfig {
    pub metric: ReportMetricKey,
    pub dimension: ReportDimensionKey,
    pub date_from: String,
    pub date_to: String,
    pub comparison: ReportComparison,
    pub filters: ReportFilters,
    pub sort: ReportSort,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportBuilderRow {
    pub dimension_value: String,
    pub dimension_label: String,
    pub value: f64,
    pub sample_conversation_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportDateRange {
    #[serde(rename = "dateFrom")]
    pub date_from: String,
    #[serde(rename = "dateTo")]
    pub date_to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportRunOutcome {
    pub metric: crate::catalog::reporting::MetricCatalogEntry,
    pub dimension: crate::catalog::reporting::DimensionCatalogEntry,
    pub rows: Vec<ReportBuilderRow>,
    pub comparison_rows: Option<Vec<ReportBuilderRow>>,
    pub comparison_range: Option<ReportDateRange>,
    pub date_range: ReportDateRange,
    pub notes: Vec<String>,
    pub origin: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedReport {
    pub id: i64,
    pub name: String,
    pub config: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricAnchor {
    Conversations,
    Threads,
    Transitions,
    Outcomes,
    Recipients,
}

struct MetricSpec {
    anchor: MetricAnchor,
    from: &'static str,
    date_expr: &'static str,
    value_expr: &'static str,
    extra_where: &'static [&'static str],
    requires_attribute: bool,
    requires_state: bool,
    total_only: bool,
}

impl MetricSpec {
    fn has_conversation_alias(&self) -> bool {
        self.from.contains("conversations c")
    }

    fn applies_conversation_filters(&self) -> bool {
        matches!(
            self.anchor,
            MetricAnchor::Conversations | MetricAnchor::Threads
        )
    }

    fn is_conversation_anchored(&self) -> bool {
        matches!(
            self.anchor,
            MetricAnchor::Conversations | MetricAnchor::Threads | MetricAnchor::Outcomes
        )
    }
}

struct DimensionSpec {
    expr: &'static str,
    group_by: &'static str,
    joins: &'static [&'static str],
}

fn priority_rank(priority: &str) -> i64 {
    match priority {
        "low" => 1,
        "normal" => 2,
        "high" => 3,
        "urgent" => 4,
        _ => 0,
    }
}

fn attribute_share_expr() -> &'static str {
    "CAST(SUM(CASE WHEN EXISTS (\
      SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id AND a.attribute = ? AND a.superseded_at IS NULL AND LOWER(a.value) = LOWER(?)\
    ) THEN 1 ELSE 0 END) AS REAL) / COUNT(*)"
}

fn attribute_unknown_expr() -> &'static str {
    "CAST(SUM(CASE WHEN NOT EXISTS (\
      SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id AND a.attribute = ? AND a.superseded_at IS NULL\
    ) THEN 1 ELSE 0 END) AS REAL) / COUNT(*)"
}

fn metric_spec(metric: ReportMetricKey) -> MetricSpec {
    const NO_WHERE: &[&str] = &[];
    match metric {
        ReportMetricKey::Conversations => MetricSpec {
            anchor: MetricAnchor::Conversations,
            from: "conversations c",
            date_expr: "c.created_at",
            value_expr: "COUNT(*)",
            extra_where: NO_WHERE,
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::UniqueCustomers => MetricSpec {
            anchor: MetricAnchor::Conversations,
            from: "conversations c",
            date_expr: "c.created_at",
            value_expr: "COUNT(DISTINCT c.customer_local_id)",
            extra_where: NO_WHERE,
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::Organizations => MetricSpec {
            anchor: MetricAnchor::Conversations,
            from: "conversations c",
            date_expr: "c.created_at",
            value_expr: "COUNT(DISTINCT (SELECT cu.organization FROM customers cu WHERE cu.id = c.customer_local_id))",
            extra_where: NO_WHERE,
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::FirstResponses => MetricSpec {
            anchor: MetricAnchor::Conversations,
            from: "conversations c",
            date_expr: "c.first_response_at",
            value_expr: "COUNT(*)",
            extra_where: &["c.first_response_at IS NOT NULL"],
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::AgentReplies => MetricSpec {
            anchor: MetricAnchor::Threads,
            from: "conversation_threads t JOIN conversations c ON c.id = t.conversation_id",
            date_expr: "t.created_at",
            value_expr: "COUNT(*)",
            extra_where: &[
                "t.type = 'reply'",
                "t.deleted_at IS NULL",
                "t.state = 'published'",
            ],
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::CustomerReplies => MetricSpec {
            anchor: MetricAnchor::Threads,
            from: "conversation_threads t JOIN conversations c ON c.id = t.conversation_id",
            date_expr: "t.created_at",
            value_expr: "COUNT(*)",
            extra_where: &[
                "t.type = 'customer'",
                "t.deleted_at IS NULL",
                "t.state = 'published'",
            ],
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::Closures => MetricSpec {
            anchor: MetricAnchor::Conversations,
            from: "conversations c",
            date_expr: "c.closed_at",
            value_expr: "COUNT(*)",
            extra_where: &["c.closed_at IS NOT NULL"],
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::AvgFirstResponseMinutes => MetricSpec {
            anchor: MetricAnchor::Conversations,
            from: "conversations c",
            date_expr: "c.first_response_at",
            value_expr: "AVG((julianday(c.first_response_at) - julianday(c.created_at)) * 1440)",
            extra_where: &[
                "c.first_response_at IS NOT NULL",
                "c.created_at IS NOT NULL",
            ],
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::AvgResolutionMinutes => MetricSpec {
            anchor: MetricAnchor::Conversations,
            from: "conversations c",
            date_expr: "c.closed_at",
            value_expr: "AVG((julianday(c.closed_at) - julianday(c.created_at)) * 1440)",
            extra_where: &["c.closed_at IS NOT NULL", "c.created_at IS NOT NULL"],
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::AvgWaitHours => MetricSpec {
            anchor: MetricAnchor::Conversations,
            from: "conversations c",
            date_expr: "c.closed_at",
            value_expr: "AVG((julianday(c.closed_at) - julianday(c.last_customer_reply_at)) * 24)",
            extra_where: &[
                "c.closed_at IS NOT NULL",
                "c.last_customer_reply_at IS NOT NULL",
                "c.status = 'closed'",
            ],
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::SlaBreached => MetricSpec {
            anchor: MetricAnchor::Conversations,
            from: "conversations c",
            date_expr: "c.created_at",
            value_expr: "COUNT(*)",
            extra_where: &[
                "c.first_response_at IS NOT NULL AND c.created_at IS NOT NULL",
                "(julianday(c.first_response_at) - julianday(c.created_at)) * 1440 > (SELECT mbh.first_response_target_min FROM mailbox_business_hours mbh WHERE mbh.mailbox_local_id = c.mailbox_local_id AND mbh.first_response_target_min IS NOT NULL)",
            ],
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::StateChanges => MetricSpec {
            anchor: MetricAnchor::Transitions,
            from: "ticket_state_transitions tr",
            date_expr: "tr.transitioned_at",
            value_expr: "COUNT(*)",
            extra_where: NO_WHERE,
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::AvgStateHours => MetricSpec {
            anchor: MetricAnchor::Transitions,
            from: "ticket_state_transitions tr JOIN conversations c ON c.remote_id = tr.conversation_remote_id",
            date_expr: "tr.transitioned_at",
            value_expr: "AVG((julianday((SELECT MIN(nx.transitioned_at) FROM ticket_state_transitions nx WHERE nx.conversation_remote_id = tr.conversation_remote_id AND nx.transitioned_at > tr.transitioned_at)) - julianday(tr.transitioned_at)) * 24)",
            extra_where: &["tr.to_state = ?"],
            requires_attribute: false,
            requires_state: true,
            total_only: false,
        },
        ReportMetricKey::HighPriorityRate => MetricSpec {
            anchor: MetricAnchor::Conversations,
            from: "conversations c",
            date_expr: "c.created_at",
            value_expr: "CAST(SUM(CASE WHEN c.supportos_priority IN ('high','urgent') THEN 1 ELSE 0 END) AS REAL) / COUNT(*)",
            extra_where: NO_WHERE,
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::IssueLinkedShare => MetricSpec {
            anchor: MetricAnchor::Conversations,
            from: "conversations c",
            date_expr: "c.created_at",
            value_expr: "CAST(SUM(CASE WHEN (EXISTS (SELECT 1 FROM issue_cluster_conversations icc WHERE icc.conversation_id = c.id) OR EXISTS (SELECT 1 FROM known_issue_links kic WHERE kic.conversation_id = c.id)) THEN 1 ELSE 0 END) AS REAL) / COUNT(*)",
            extra_where: NO_WHERE,
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::AiAttributeShare => MetricSpec {
            anchor: MetricAnchor::Conversations,
            from: "conversations c",
            date_expr: "c.created_at",
            value_expr: "",
            extra_where: NO_WHERE,
            requires_attribute: true,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::AvgCustomerEffort => MetricSpec {
            anchor: MetricAnchor::Outcomes,
            from: "friction_scores o JOIN conversations c ON c.id = o.conversation_id",
            date_expr: "c.created_at",
            value_expr: "AVG(o.effort_score)",
            extra_where: &["o.effort_score IS NOT NULL"],
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::HighFrictionRate => MetricSpec {
            anchor: MetricAnchor::Outcomes,
            from: "friction_scores o JOIN conversations c ON c.id = o.conversation_id",
            date_expr: "c.created_at",
            value_expr: "CAST(SUM(CASE WHEN o.effort_score >= 0.6 THEN 1 ELSE 0 END) AS REAL) / COUNT(*)",
            extra_where: NO_WHERE,
            requires_attribute: false,
            requires_state: false,
            total_only: false,
        },
        ReportMetricKey::CampaignSent => MetricSpec {
            anchor: MetricAnchor::Recipients,
            from: "outreach_recipients r",
            date_expr: "r.sent_at",
            value_expr: "COUNT(*)",
            extra_where: &["r.sent_at IS NOT NULL"],
            requires_attribute: false,
            requires_state: false,
            total_only: true,
        },
        ReportMetricKey::CampaignReplies => MetricSpec {
            anchor: MetricAnchor::Recipients,
            from: "outreach_recipients r",
            date_expr: "r.replied_at",
            value_expr: "COUNT(*)",
            extra_where: &["r.replied_at IS NOT NULL"],
            requires_attribute: false,
            requires_state: false,
            total_only: true,
        },
        ReportMetricKey::CampaignReplyRate => MetricSpec {
            anchor: MetricAnchor::Recipients,
            from: "outreach_recipients r",
            date_expr: "r.sent_at",
            value_expr: "CAST(SUM(CASE WHEN r.replied_at IS NOT NULL THEN 1 ELSE 0 END) AS REAL) / COUNT(*)",
            extra_where: &["r.sent_at IS NOT NULL"],
            requires_attribute: false,
            requires_state: false,
            total_only: true,
        },
    }
}

fn dimension_spec(dimension: ReportDimensionKey) -> DimensionSpec {
    const NO_JOINS: &[&str] = &[];
    match dimension {
        ReportDimensionKey::None => DimensionSpec {
            expr: "'(total)'",
            group_by: "'(total)'",
            joins: NO_JOINS,
        },
        ReportDimensionKey::Day => DimensionSpec {
            expr: "strftime('%Y-%m-%d', {DATE})",
            group_by: "strftime('%Y-%m-%d', {DATE})",
            joins: NO_JOINS,
        },
        ReportDimensionKey::Week => DimensionSpec {
            expr: "strftime('%Y-%W', {DATE})",
            group_by: "strftime('%Y-%W', {DATE})",
            joins: NO_JOINS,
        },
        ReportDimensionKey::Month => DimensionSpec {
            expr: "strftime('%Y-%m', {DATE})",
            group_by: "strftime('%Y-%m', {DATE})",
            joins: NO_JOINS,
        },
        ReportDimensionKey::Mailbox => DimensionSpec {
            expr: "COALESCE((SELECT m.name FROM mailboxes m WHERE m.id = c.mailbox_local_id), '(no mailbox)')",
            group_by: "c.mailbox_local_id",
            joins: NO_JOINS,
        },
        ReportDimensionKey::Channel => DimensionSpec {
            expr: "COALESCE(NULLIF(c.source_type, ''), '(unknown channel)')",
            group_by: "COALESCE(NULLIF(c.source_type, ''), '(unknown channel)')",
            joins: NO_JOINS,
        },
        ReportDimensionKey::Tag => DimensionSpec {
            expr: "tg.name",
            group_by: "tg.name",
            joins: &[
                "JOIN conversation_tags ct ON ct.conversation_id = c.id",
                "JOIN tags tg ON tg.id = ct.tag_id",
            ],
        },
        ReportDimensionKey::Assignee => DimensionSpec {
            expr: "COALESCE((SELECT (u.first_name || ' ' || u.last_name) FROM users u WHERE u.id = c.assignee_local_id), '(unassigned)')",
            group_by: "c.assignee_local_id",
            joins: NO_JOINS,
        },
        ReportDimensionKey::Team => DimensionSpec {
            expr: "COALESCE((SELECT tm2.name FROM team_members tm JOIN teams tm2 ON tm2.id = tm.team_id WHERE tm.user_id = c.assignee_local_id LIMIT 1), '(no team)')",
            group_by: "COALESCE((SELECT tm.team_id FROM team_members tm WHERE tm.user_id = c.assignee_local_id LIMIT 1), -1)",
            joins: NO_JOINS,
        },
        ReportDimensionKey::Status => DimensionSpec {
            expr: "c.status",
            group_by: "c.status",
            joins: NO_JOINS,
        },
        ReportDimensionKey::Priority => DimensionSpec {
            expr: "COALESCE(NULLIF(c.supportos_priority, ''), 'none')",
            group_by: "COALESCE(NULLIF(c.supportos_priority, ''), 'none')",
            joins: NO_JOINS,
        },
        ReportDimensionKey::CustomState => DimensionSpec {
            expr: "COALESCE((SELECT ts.name FROM ticket_states ts WHERE ts.id = c.supportos_state_id), '(no state)')",
            group_by: "c.supportos_state_id",
            joins: NO_JOINS,
        },
        ReportDimensionKey::ResponseState => DimensionSpec {
            expr: crate::response_state_sql::RESPONSE_STATE_SQL,
            group_by: crate::response_state_sql::RESPONSE_STATE_SQL,
            joins: NO_JOINS,
        },
        ReportDimensionKey::Issue => DimensionSpec {
            expr: "COALESCE(\
                (SELECT ('KI: ' || ki.name) FROM known_issue_links kil JOIN known_issues ki ON ki.id = kil.known_issue_id WHERE kil.conversation_id = c.id LIMIT 1), \
                (SELECT ('Cluster: ' || ic2.name) FROM issue_cluster_conversations icm JOIN issue_clusters ic2 ON ic2.id = icm.cluster_id WHERE icm.conversation_id = c.id LIMIT 1), \
                '(not linked to an issue)')",
            group_by: "COALESCE(\
                (SELECT kil.known_issue_id FROM known_issue_links kil WHERE kil.conversation_id = c.id LIMIT 1), \
                (SELECT icm.cluster_id FROM issue_cluster_conversations icm WHERE icm.conversation_id = c.id LIMIT 1), \
                -1)",
            joins: NO_JOINS,
        },
    }
}

fn safe_sql(sql: &str) -> std::result::Result<String, String> {
    if sql.contains(';')
        || sql.contains("--")
        || sql.contains("/*")
        || sql.to_ascii_lowercase().contains(" drop ")
        || sql.to_ascii_lowercase().contains(" delete ")
        || sql.to_ascii_lowercase().contains(" insert ")
        || sql.to_ascii_lowercase().contains(" update ")
    {
        return Err("Compiled report SQL failed the safety guard.".to_string());
    }
    Ok(sql.to_string())
}

type SqlParam = rusqlite::types::Value;

fn apply_conversation_filters(
    f: &ReportFilters,
    where_sql: &mut Vec<String>,
    params: &mut Vec<SqlParam>,
) {
    if let Some(ids) = &f.mailbox_local_ids {
        let ids: Vec<i64> = ids.iter().copied().filter(|n| *n > 0).collect();
        if !ids.is_empty() {
            let marks: Vec<&str> = ids.iter().map(|_| "?").collect();
            where_sql.push(format!("c.mailbox_local_id IN ({})", marks.join(",")));
            params.extend(ids.iter().copied().map(SqlParam::Integer));
        }
    }
    if let Some(channel) = &f.channel {
        let trimmed = channel.trim();
        if !trimmed.is_empty() {
            where_sql.push("LOWER(COALESCE(c.source_type, '')) = LOWER(?)".to_string());
            params.push(SqlParam::Text(trimmed.to_string()));
        }
    }
    if let Some(tags) = &f.tags_any {
        let tags: Vec<&String> = tags.iter().filter(|t| !t.is_empty()).collect();
        if !tags.is_empty() {
            let marks: Vec<&str> = tags.iter().map(|_| "?").collect();
            where_sql.push(format!(
                "EXISTS (SELECT 1 FROM conversation_tags ct2 JOIN tags tg2 ON tg2.id = ct2.tag_id WHERE ct2.conversation_id = c.id AND LOWER(tg2.name) IN ({}))",
                marks.join(",")
            ));
            params.extend(tags.iter().map(|t| SqlParam::Text(t.to_lowercase())));
        }
    }
    if let Some(tags) = &f.tags_none {
        let tags: Vec<&String> = tags.iter().filter(|t| !t.is_empty()).collect();
        if !tags.is_empty() {
            let marks: Vec<&str> = tags.iter().map(|_| "?").collect();
            where_sql.push(format!(
                "NOT EXISTS (SELECT 1 FROM conversation_tags ct2 JOIN tags tg2 ON tg2.id = ct2.tag_id WHERE ct2.conversation_id = c.id AND LOWER(tg2.name) IN ({}))",
                marks.join(",")
            ));
            params.extend(tags.iter().map(|t| SqlParam::Text(t.to_lowercase())));
        }
    }
    if let Some(statuses) = &f.statuses {
        let statuses: Vec<&String> = statuses.iter().filter(|s| !s.is_empty()).collect();
        if !statuses.is_empty() {
            let marks: Vec<&str> = statuses.iter().map(|_| "?").collect();
            where_sql.push(format!("c.status IN ({})", marks.join(",")));
            params.extend(statuses.iter().map(|s| SqlParam::Text((*s).clone())));
        }
    }
    if let Some(ids) = &f.assignee_local_ids {
        let ids: Vec<i64> = ids.to_vec();
        if !ids.is_empty() {
            let marks: Vec<&str> = ids.iter().map(|_| "?").collect();
            where_sql.push(format!("c.assignee_local_id IN ({})", marks.join(",")));
            params.extend(ids.iter().copied().map(SqlParam::Integer));
        }
    }
    if let Some(min_priority) = &f.min_priority {
        let rank = priority_rank(min_priority);
        if rank > 0 {
            where_sql.push(
                "CASE c.supportos_priority WHEN 'low' THEN 1 WHEN 'normal' THEN 2 WHEN 'high' THEN 3 WHEN 'urgent' THEN 4 ELSE 0 END >= ?"
                    .to_string(),
            );
            params.push(SqlParam::Integer(rank));
        }
    }
}

fn sample_conversations(
    conn: &Connection,
    config: &ReportBuilderConfig,
    spec: &MetricSpec,
    dim: &DimensionSpec,
    group_key: Option<&rusqlite::types::Value>,
) -> Vec<i64> {
    let mut params: Vec<SqlParam> = vec![
        SqlParam::Text(config.date_from.clone()),
        SqlParam::Text(config.date_to.clone()),
    ];
    let mut where_sql: Vec<String> = vec![
        format!(
            "COALESCE(julianday({}), julianday('2000-01-01')) >= julianday(?)",
            spec.date_expr
        ),
        format!(
            "COALESCE(julianday({}), julianday('2000-01-01')) < julianday(?, '+1 day')",
            spec.date_expr
        ),
    ];
    for w in spec.extra_where {
        if *w == "tr.to_state = ?" {
            params.push(SqlParam::Text(
                config.filters.state_key.clone().unwrap_or_default(),
            ));
        }
        where_sql.push((*w).to_string());
    }
    apply_conversation_filters(&config.filters, &mut where_sql, &mut params);
    if let Some(gk) = group_key {
        let gk_expr = dim.group_by.replace("{DATE}", spec.date_expr);
        if matches!(gk, rusqlite::types::Value::Null) {
            where_sql.push(format!("{gk_expr} IS NULL"));
        } else {
            where_sql.push(format!("{gk_expr} = ?"));
            params.push(gk.clone());
        }
    }
    let joins = dim.joins.join(" ");
    let sql = format!(
        "SELECT DISTINCT c.id FROM {} {} WHERE {} ORDER BY c.id DESC LIMIT 3",
        spec.from,
        joins,
        where_sql.join(" AND ")
    );
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return vec![];
    };
    let Ok(rows) = stmt.query_map(rusqlite::params_from_iter(params.iter()), |r| {
        r.get::<_, i64>(0)
    }) else {
        return vec![];
    };
    rows.filter_map(std::result::Result::ok).collect()
}

fn execute_builder_query(
    conn: &Connection,
    config: &ReportBuilderConfig,
    spec: &MetricSpec,
    date_from: &str,
    date_to: &str,
) -> std::result::Result<Vec<ReportBuilderRow>, String> {
    let dim = dimension_spec(config.dimension);
    let date_expr = spec.date_expr;
    let dim_expr = dim.expr.replace("{DATE}", date_expr);
    let group_expr = dim.group_by.replace("{DATE}", date_expr);
    let joins: Vec<&str> = if spec.has_conversation_alias() {
        dim.joins.to_vec()
    } else {
        Vec::new()
    };

    let mut select_params: Vec<SqlParam> = Vec::new();
    let mut where_params: Vec<SqlParam> = vec![
        SqlParam::Text(date_from.to_string()),
        SqlParam::Text(date_to.to_string()),
    ];
    let mut where_sql: Vec<String> = vec![
        format!(
            "COALESCE(julianday({}), julianday('2000-01-01')) >= julianday(?)",
            date_expr
        ),
        format!(
            "COALESCE(julianday({}), julianday('2000-01-01')) < julianday(?, '+1 day')",
            date_expr
        ),
    ];

    let mut value_expr = spec.value_expr.to_string();
    if spec.requires_attribute {
        let attr_value = config.filters.attribute_value.as_deref();
        if attr_value.is_none_or(|v| v.is_empty() || v.to_lowercase() == "unknown") {
            value_expr = attribute_unknown_expr().to_string();
            select_params.push(SqlParam::Text(
                config.filters.attribute_key.clone().unwrap_or_default(),
            ));
        } else {
            value_expr = attribute_share_expr().to_string();
            select_params.push(SqlParam::Text(
                config.filters.attribute_key.clone().unwrap_or_default(),
            ));
            select_params.push(SqlParam::Text(attr_value.unwrap_or_default().to_string()));
        }
    }

    for w in spec.extra_where {
        if *w == "tr.to_state = ?" {
            where_params.push(SqlParam::Text(
                config.filters.state_key.clone().unwrap_or_default(),
            ));
        }
        where_sql.push((*w).to_string());
    }
    let mut params: Vec<SqlParam> = select_params;
    params.extend(where_params);

    if spec.applies_conversation_filters() {
        apply_conversation_filters(&config.filters, &mut where_sql, &mut params);
    }

    let sql = safe_sql(&format!(
        "SELECT {dim_expr} AS dv, {group_expr} AS gk, {value_expr} AS v \
         FROM {} {} WHERE {} GROUP BY {group_expr}",
        spec.from,
        joins.join(" "),
        where_sql.join(" AND ")
    ))?;

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Report execution failed: {e}"))?;
    struct RawRow {
        dv: Option<String>,
        gk: rusqlite::types::Value,
        v: Option<f64>,
    }
    let raw: Vec<RawRow> = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |r| {
            Ok(RawRow {
                dv: r.get(0)?,
                gk: r.get(1)?,
                v: r.get(2)?,
            })
        })
        .map_err(|e| format!("Report execution failed: {e}"))?
        .filter_map(std::result::Result::ok)
        .collect();

    let mut rows: Vec<ReportBuilderRow> = raw
        .iter()
        .map(|r| ReportBuilderRow {
            dimension_value: r.dv.clone().unwrap_or_else(|| "null".to_string()),
            dimension_label: r.dv.clone().unwrap_or_else(|| "null".to_string()),
            value: r.v.unwrap_or(0.0),
            sample_conversation_ids: Vec::new(),
        })
        .collect();

    if spec.is_conversation_anchored() && spec.value_expr == "COUNT(*)" {
        if config.dimension != ReportDimensionKey::None {
            let up_to = rows.len().min(20);
            for i in 0..up_to {
                rows[i].sample_conversation_ids =
                    sample_conversations(conn, config, spec, &dim, Some(&raw[i].gk));
            }
        } else {
            for row in &mut rows {
                row.sample_conversation_ids = sample_conversations(conn, config, spec, &dim, None);
            }
        }
    }

    let limit = config.limit.map_or(50, |l| l.clamp(1, 200));
    match config.sort {
        ReportSort::MetricAsc => rows.sort_by(|a, b| {
            a.value
                .partial_cmp(&b.value)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.dimension_label.cmp(&b.dimension_label))
        }),
        ReportSort::DimensionAsc => rows.sort_by(|a, b| a.dimension_label.cmp(&b.dimension_label)),
        ReportSort::MetricDesc => rows.sort_by(|a, b| {
            b.value
                .partial_cmp(&a.value)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.dimension_label.cmp(&b.dimension_label))
        }),
    }
    rows.truncate(usize::try_from(limit).unwrap_or(50));
    Ok(rows)
}

pub fn run_builder_report(
    conn: &Connection,
    config: &ReportBuilderConfig,
) -> std::result::Result<ReportRunOutcome, String> {
    let metric = crate::catalog::reporting::metric_entry(config.metric);
    let dimension = crate::catalog::reporting::dimension_entry(config.dimension);
    let spec = metric_spec(config.metric);
    if (spec.total_only || spec.requires_attribute || spec.requires_state)
        && config.dimension != ReportDimensionKey::None
    {
        return Err(format!(
            "Metric '{}' supports no grouping (total only).",
            config.metric.as_str()
        ));
    }
    if spec.requires_attribute && config.filters.attribute_key.is_none() {
        return Err(format!(
            "Metric '{}' requires an attribute key filter.",
            config.metric.as_str()
        ));
    }
    if spec.requires_state && config.filters.state_key.is_none() {
        return Err(format!(
            "Metric '{}' requires a state key filter.",
            config.metric.as_str()
        ));
    }
    if spec.requires_attribute {
        let key = config.filters.attribute_key.as_deref().unwrap_or_default();
        let in_catalog = crate::catalog::AiAttributeKey::ALL
            .iter()
            .any(|k| k.as_str() == key);
        if !in_catalog {
            return Err(format!(
                "Attribute key '{key}' is not in the closed catalog."
            ));
        }
    }
    if spec.requires_state {
        let key = config.filters.state_key.as_deref().unwrap_or_default();
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM ticket_states WHERE key = ?1)",
                params![key],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if !exists {
            return Err(format!("State key '{key}' does not exist."));
        }
    }
    let _ = ensure_mailbox_business_hours(conn);

    let rows = execute_builder_query(conn, config, &spec, &config.date_from, &config.date_to)?;

    let mut comparison_rows: Option<Vec<ReportBuilderRow>> = None;
    let mut comparison_range: Option<ReportDateRange> = None;
    if config.comparison == ReportComparison::PreviousPeriod {
        let from_date = chrono::NaiveDate::parse_from_str(&config.date_from, "%Y-%m-%d")
            .map_err(|_| "Invalid time value".to_string())?;
        let to_date = chrono::NaiveDate::parse_from_str(&config.date_to, "%Y-%m-%d")
            .map_err(|_| "Invalid time value".to_string())?;
        let days = ((to_date - from_date).num_days() + 1).max(1);
        let prev_to = from_date - chrono::Duration::days(1);
        let prev_from = from_date - chrono::Duration::days(days);
        comparison_range = Some(ReportDateRange {
            date_from: prev_from.format("%Y-%m-%d").to_string(),
            date_to: prev_to.format("%Y-%m-%d").to_string(),
        });
        comparison_rows = Some(execute_builder_query(
            conn,
            config,
            &spec,
            &prev_from.format("%Y-%m-%d").to_string(),
            &prev_to.format("%Y-%m-%d").to_string(),
        )?);
    }

    let mut notes: Vec<String> = vec![
        format!("Metric definition: {}", metric.definition),
        format!("Limitations: {}", metric.limitations),
    ];
    if config.comparison == ReportComparison::PreviousPeriod {
        notes.push(
            "Comparison values are differences over an earlier window of the same length; they describe change, not cause."
                .to_string(),
        );
    }
    if spec.requires_attribute && config.filters.attribute_value.is_none() {
        notes.push(
            "A blank attribute value means 'is unknown' (no current attribute row).".to_string(),
        );
    }

    Ok(ReportRunOutcome {
        metric,
        dimension,
        rows,
        comparison_rows,
        comparison_range,
        date_range: ReportDateRange {
            date_from: config.date_from.clone(),
            date_to: config.date_to.clone(),
        },
        notes,
        origin: "local",
    })
}

pub fn config_to_json(config: &ReportBuilderConfig) -> serde_json::Value {
    let filters = {
        let mut f = serde_json::Map::new();
        if let Some(v) = &config.filters.mailbox_local_ids {
            f.insert("mailboxLocalIds".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.channel {
            f.insert("channel".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.tags_any {
            f.insert("tagsAny".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.tags_none {
            f.insert("tagsNone".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.statuses {
            f.insert("statuses".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.assignee_local_ids {
            f.insert("assigneeLocalIds".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.min_priority {
            f.insert("minPriority".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.attribute_key {
            f.insert("attributeKey".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.attribute_value {
            f.insert("attributeValue".into(), serde_json::json!(v));
        }
        if let Some(v) = &config.filters.state_key {
            f.insert("stateKey".into(), serde_json::json!(v));
        }
        f
    };
    let mut v = serde_json::json!({
        "metric": config.metric.as_str(),
        "dimension": config.dimension.as_str(),
        "dateFrom": config.date_from,
        "dateTo": config.date_to,
        "comparison": match config.comparison {
            ReportComparison::None => "none",
            ReportComparison::PreviousPeriod => "previous_period",
        },
        "sort": match config.sort {
            ReportSort::MetricDesc => "metric_desc",
            ReportSort::MetricAsc => "metric_asc",
            ReportSort::DimensionAsc => "dimension_asc",
        },
    });
    if !filters.is_empty() {
        v["filters"] = serde_json::Value::Object(filters);
    }
    if let Some(limit) = config.limit {
        v["limit"] = serde_json::json!(limit);
    }
    v
}

pub fn list_saved_reports(conn: &Connection) -> Result<Vec<SavedReport>> {
    ensure_report_definitions_table(conn)?;
    let mut stmt = conn.prepare(
        "SELECT id, name, config, created_at, updated_at FROM report_definitions
         ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let config_text: String = r.get(2)?;
            Ok(SavedReport {
                id: r.get(0)?,
                name: r.get(1)?,
                config: serde_json::from_str(&config_text).unwrap_or(serde_json::Value::Null),
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn save_report(
    conn: &Connection,
    name: &str,
    config: &ReportBuilderConfig,
) -> Result<SavedReport> {
    ensure_report_definitions_table(conn)?;
    let config_json =
        serde_json::to_string(&config_to_json(config)).unwrap_or_else(|_| "{}".to_string());
    conn.execute(
        "INSERT INTO report_definitions (name, config) VALUES (?1, ?2)",
        params![name, config_json],
    )?;
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    Ok(SavedReport {
        id: conn.last_insert_rowid(),
        name: name.to_string(),
        config: config_to_json(config),
        created_at: now.clone(),
        updated_at: now,
    })
}

pub fn delete_saved_report(conn: &Connection, id: i64) -> Result<bool> {
    ensure_report_definitions_table(conn)?;
    let rows = conn.execute("DELETE FROM report_definitions WHERE id = ?1", params![id])?;
    Ok(rows > 0)
}

pub fn parse_report_config(
    body: &serde_json::Value,
) -> std::result::Result<ReportBuilderConfig, Vec<(String, String)>> {
    parse_report_config_inner(body).map_err(|mut issues| {
        issues.truncate(10);
        issues
    })
}

fn enum_error(variants: &[&str], received: &str) -> String {
    let quoted: Vec<String> = variants.iter().map(|v| format!("'{v}'")).collect();
    format!(
        "Invalid enum value. Expected {}, received '{received}'",
        quoted.join(" | ")
    )
}

fn metric_variants() -> Vec<&'static str> {
    ReportMetricKey::ALL.iter().map(|k| k.as_str()).collect()
}

fn dimension_variants() -> Vec<&'static str> {
    ReportDimensionKey::ALL.iter().map(|k| k.as_str()).collect()
}

fn parse_report_config_inner(
    body: &serde_json::Value,
) -> std::result::Result<ReportBuilderConfig, Vec<(String, String)>> {
    let mut issues: Vec<(String, String)> = Vec::new();

    let metric = body.get("metric").and_then(|v| v.as_str());
    let metric = match metric {
        None => {
            issues.push(("metric".to_string(), "Required".to_string()));
            None
        }
        Some(m) => match ReportMetricKey::ALL.iter().find(|k| k.as_str() == m) {
            Some(k) => Some(*k),
            None => {
                issues.push(("metric".to_string(), enum_error(&metric_variants(), m)));
                None
            }
        },
    };

    let dimension = body.get("dimension").and_then(|v| v.as_str());
    let dimension = match dimension {
        None => {
            issues.push(("dimension".to_string(), "Required".to_string()));
            None
        }
        Some(d) => match ReportDimensionKey::ALL.iter().find(|k| k.as_str() == d) {
            Some(k) => Some(*k),
            None => {
                issues.push((
                    "dimension".to_string(),
                    enum_error(&dimension_variants(), d),
                ));
                None
            }
        },
    };

    let date_from = match body.get("dateFrom") {
        None | Some(serde_json::Value::Null) => {
            issues.push(("dateFrom".to_string(), "Required".to_string()));
            String::new()
        }
        Some(serde_json::Value::String(s)) => {
            let shape_ok = s.len() == 10
                && s.as_bytes().iter().enumerate().all(|(i, b)| match i {
                    4 | 7 => *b == b'-',
                    _ => b.is_ascii_digit(),
                });
            if shape_ok {
                s.clone()
            } else {
                issues.push(("dateFrom".to_string(), "Invalid".to_string()));
                String::new()
            }
        }
        Some(_) => {
            issues.push((
                "dateFrom".to_string(),
                "Expected string, received non-string".to_string(),
            ));
            String::new()
        }
    };

    let date_to = match body.get("dateTo") {
        None | Some(serde_json::Value::Null) => {
            issues.push(("dateTo".to_string(), "Required".to_string()));
            String::new()
        }
        Some(serde_json::Value::String(s)) => {
            let shape_ok = s.len() == 10
                && s.as_bytes().iter().enumerate().all(|(i, b)| match i {
                    4 | 7 => *b == b'-',
                    _ => b.is_ascii_digit(),
                });
            if shape_ok {
                s.clone()
            } else {
                issues.push(("dateTo".to_string(), "Invalid".to_string()));
                String::new()
            }
        }
        Some(_) => {
            issues.push((
                "dateTo".to_string(),
                "Expected string, received non-string".to_string(),
            ));
            String::new()
        }
    };

    let comparison = match body.get("comparison") {
        None | Some(serde_json::Value::Null) => ReportComparison::None,
        Some(serde_json::Value::String(s)) => match s.as_str() {
            "none" => ReportComparison::None,
            "previous_period" => ReportComparison::PreviousPeriod,
            _ => {
                issues.push((
                    "comparison".to_string(),
                    enum_error(&["none", "previous_period"], s),
                ));
                ReportComparison::None
            }
        },
        Some(_) => {
            issues.push((
                "comparison".to_string(),
                "Expected string, received non-string".to_string(),
            ));
            ReportComparison::None
        }
    };

    let sort = match body.get("sort") {
        None | Some(serde_json::Value::Null) => ReportSort::MetricDesc,
        Some(serde_json::Value::String(s)) => match s.as_str() {
            "metric_desc" => ReportSort::MetricDesc,
            "metric_asc" => ReportSort::MetricAsc,
            "dimension_asc" => ReportSort::DimensionAsc,
            _ => {
                issues.push((
                    "sort".to_string(),
                    enum_error(&["metric_desc", "metric_asc", "dimension_asc"], s),
                ));
                ReportSort::MetricDesc
            }
        },
        Some(_) => {
            issues.push((
                "sort".to_string(),
                "Expected string, received non-string".to_string(),
            ));
            ReportSort::MetricDesc
        }
    };

    let limit = match body.get("limit") {
        None | Some(serde_json::Value::Null) => None,
        Some(v) => match v.as_i64() {
            Some(n) if (1..=200).contains(&n) => Some(n),
            Some(n) if n < 1 => {
                issues.push((
                    "limit".to_string(),
                    "Number must be greater than or equal to 1".to_string(),
                ));
                None
            }
            Some(_) => {
                issues.push((
                    "limit".to_string(),
                    "Number must be less than or equal to 200".to_string(),
                ));
                None
            }
            None => {
                issues.push((
                    "limit".to_string(),
                    "Expected number, received non-number".to_string(),
                ));
                None
            }
        },
    };

    let filters = match body.get("filters") {
        None => ReportFilters::default(),
        Some(serde_json::Value::Object(f)) => parse_report_filters(f, &mut issues),
        Some(_) => {
            issues.push((
                "filters".to_string(),
                "Expected object, received non-object".to_string(),
            ));
            ReportFilters::default()
        }
    };

    match (metric, dimension) {
        (Some(metric), Some(dimension)) if issues.is_empty() => Ok(ReportBuilderConfig {
            metric,
            dimension,
            date_from,
            date_to,
            comparison,
            filters,
            sort,
            limit,
        }),
        _ => Err(issues),
    }
}

fn parse_report_filters(
    f: &serde_json::Map<String, serde_json::Value>,
    issues: &mut Vec<(String, String)>,
) -> ReportFilters {
    let mut out = ReportFilters::default();

    for key in ["mailboxLocalIds", "assigneeLocalIds"] {
        if let Some(v) = f.get(key) {
            match v.as_array() {
                Some(arr) if arr.len() > 20 => {
                    issues.push((
                        format!("filters.{key}"),
                        "Array must contain at most 20 item(s)".to_string(),
                    ));
                }
                Some(arr) => {
                    let mut ids = Vec::new();
                    let mut ok = true;
                    for item in arr {
                        match item.as_i64() {
                            Some(n) if n > 0 => ids.push(n),
                            _ => {
                                issues.push((
                                    format!("filters.{key}"),
                                    "Expected positive integer, received non-integer".to_string(),
                                ));
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok {
                        if key == "mailboxLocalIds" {
                            out.mailbox_local_ids = Some(ids);
                        } else {
                            out.assignee_local_ids = Some(ids);
                        }
                    }
                }
                None => {
                    issues.push((
                        format!("filters.{key}"),
                        "Expected array, received non-array".to_string(),
                    ));
                }
            }
        }
    }

    if let Some(v) = f.get("channel") {
        match v {
            serde_json::Value::Null => {}
            serde_json::Value::String(s) if s.len() <= 40 => {
                out.channel = Some(s.clone());
            }
            serde_json::Value::String(_) => {
                issues.push((
                    "filters.channel".to_string(),
                    "String must contain at most 40 character(s)".to_string(),
                ));
            }
            _ => {
                issues.push((
                    "filters.channel".to_string(),
                    "Expected string, received non-string".to_string(),
                ));
            }
        }
    }

    let string_lists = [
        ("tagsAny", 10usize, 80usize, "tagsAny"),
        ("tagsNone", 10, 80, "tagsNone"),
        ("statuses", 10, 40, "statuses"),
    ];
    for (key, max_items, max_len, field) in string_lists {
        if let Some(v) = f.get(key) {
            match v.as_array() {
                Some(arr) if arr.len() > max_items => {
                    issues.push((
                        format!("filters.{field}"),
                        format!("Array must contain at most {max_items} item(s)"),
                    ));
                }
                Some(arr) => {
                    let mut items = Vec::new();
                    let mut ok = true;
                    for item in arr {
                        match item.as_str() {
                            Some(s) if !s.is_empty() && s.len() <= max_len => {
                                items.push(s.to_string());
                            }
                            Some("") => {
                                issues.push((
                                    format!("filters.{field}"),
                                    "String must contain at least 1 character(s)".to_string(),
                                ));
                                ok = false;
                                break;
                            }
                            Some(_) => {
                                issues.push((
                                    format!("filters.{field}"),
                                    format!("String must contain at most {max_len} character(s)"),
                                ));
                                ok = false;
                                break;
                            }
                            None => {
                                issues.push((
                                    format!("filters.{field}"),
                                    "Expected string, received non-string".to_string(),
                                ));
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok {
                        match field {
                            "tagsAny" => out.tags_any = Some(items),
                            "tagsNone" => out.tags_none = Some(items),
                            _ => out.statuses = Some(items),
                        }
                    }
                }
                None => {
                    issues.push((
                        format!("filters.{field}"),
                        "Expected array, received non-array".to_string(),
                    ));
                }
            }
        }
    }

    if let Some(v) = f.get("minPriority") {
        match v {
            serde_json::Value::Null => {}
            serde_json::Value::String(s)
                if ["low", "normal", "high", "urgent"].contains(&s.as_str()) =>
            {
                out.min_priority = Some(s.clone());
            }
            serde_json::Value::String(s) => {
                issues.push((
                    "filters.minPriority".to_string(),
                    enum_error(&["low", "normal", "high", "urgent"], s),
                ));
            }
            _ => {
                issues.push((
                    "filters.minPriority".to_string(),
                    "Expected string, received non-string".to_string(),
                ));
            }
        }
    }

    let nullable_strings = [
        ("attributeKey", 60usize, "attribute_key"),
        ("attributeValue", 200, "attribute_value"),
        ("stateKey", 80, "state_key"),
    ];
    for (key, max_len, field) in nullable_strings {
        if let Some(v) = f.get(key) {
            match v {
                serde_json::Value::Null => {}
                serde_json::Value::String(s) if s.len() <= max_len => match field {
                    "attribute_key" => out.attribute_key = Some(s.clone()),
                    "attribute_value" => out.attribute_value = Some(s.clone()),
                    _ => out.state_key = Some(s.clone()),
                },
                serde_json::Value::String(_) => {
                    issues.push((
                        format!("filters.{key}"),
                        format!("String must contain at most {max_len} character(s)"),
                    ));
                }
                _ => {
                    issues.push((
                        format!("filters.{key}"),
                        "Expected string, received non-string".to_string(),
                    ));
                }
            }
        }
    }

    out
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectivenessReport {
    pub avg_qa_score: Option<f64>,
    pub total_qa_checks: u32,
    pub score_distribution: Vec<(String, u32)>,
}

pub fn get_effectiveness_report(conn: &Connection) -> Result<EffectivenessReport> {
    let avg_score: Option<f64> = conn
        .query_row(
            "SELECT AVG(qa_score) FROM post_resolution_qa WHERE qa_score IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    let total: i64 = conn.query_row("SELECT COUNT(*) FROM post_resolution_qa", [], |r| r.get(0))?;

    let mut stmt = conn.prepare(
        "SELECT
            CASE
                WHEN qa_score < 0.5 THEN 'low'
                WHEN qa_score < 0.75 THEN 'medium'
                ELSE 'high'
            END as bucket,
            COUNT(*) as cnt
         FROM post_resolution_qa
         WHERE qa_score IS NOT NULL
         GROUP BY bucket
         ORDER BY bucket",
    )?;
    let distribution: Vec<(String, u32)> = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u32))
        })?
        .filter_map(std::result::Result::ok)
        .collect();

    Ok(EffectivenessReport {
        avg_qa_score: avg_score,
        total_qa_checks: u32::try_from(total).unwrap_or(0),
        score_distribution: distribution,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrictionScore {
    pub conversation_id: i64,
    pub effort_score: f64,
    pub factors: FrictionFactors,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrictionFactors {
    pub message_count: u32,
    pub resolution_hours: Option<f64>,
    pub reopen_count: u32,
    pub escalated: bool,
}

pub const HIGH_FRICTION_THRESHOLD: f64 = 0.6;

pub fn compute_friction(conn: &Connection, conversation_id: i64) -> Result<FrictionScore> {
    let message_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM activity_events WHERE conversation_id = ?1 AND event_type = 'message'",
            params![conversation_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let resolution_hours: Option<f64> = conn
        .query_row(
            "SELECT (julianday(closed_at) - julianday(created_at)) * 24
             FROM conversations WHERE remote_id = ?1 AND closed_at IS NOT NULL",
            params![conversation_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    let reopen_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM ticket_state_transitions
             WHERE conversation_remote_id = ?1 AND to_state = 'active'
             AND from_state = 'closed'",
            params![conversation_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let escalated: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM ai_attributes
             WHERE conversation_id = (SELECT id FROM conversations WHERE remote_id = ?1)
               AND attribute = 'escalation_signal'
               AND value = 'true'
               AND superseded_at IS NULL)",
            params![conversation_id],
            |r| r.get::<_, i64>(0),
        )
        .map(|v: i64| v != 0)
        .unwrap_or(false);

    let factors = FrictionFactors {
        message_count: u32::try_from(message_count).unwrap_or(0),
        resolution_hours,
        reopen_count: u32::try_from(reopen_count).unwrap_or(0),
        escalated,
    };

    let mut score = 0.0_f64;
    if factors.message_count > 20 {
        score += 0.3;
    }
    if let Some(hours) = resolution_hours
        && hours > 48.0
    {
        score += 0.3;
    }
    score += (factors.reopen_count as f64 * 0.2).min(0.4);
    if factors.escalated {
        score += 0.2;
    }
    let effort_score = score.min(1.0);

    let factors_json = serde_json::to_string(&factors).unwrap_or_default();
    let _ = conn.execute(
        "INSERT OR REPLACE INTO friction_scores (conversation_id, effort_score, factors_json)
         VALUES (?1, ?2, ?3)",
        params![conversation_id, effort_score, factors_json],
    );

    Ok(FrictionScore {
        conversation_id,
        effort_score,
        factors,
    })
}

pub fn count_high_friction(conn: &Connection) -> Result<u32> {
    let count: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM friction_scores WHERE effort_score >= {HIGH_FRICTION_THRESHOLD}"
        ),
        [],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(count).unwrap_or(0))
}

pub fn friction_findings(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, effort_score, factors_json, created_at
         FROM friction_scores
         WHERE conversation_id = ?1
         ORDER BY id",
    )?;
    let rows = stmt
        .query_map(params![conversation_id], |r| {
            let factors_text: Option<String> = r.get(3)?;
            Ok(serde_json::json!({
                "id": r.get::<_, i64>(0)?,
                "conversation_id": r.get::<_, i64>(1)?,
                "effort_score": r.get::<_, f64>(2)?,
                "factors": factors_text
                    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()),
                "created_at": r.get::<_, String>(4)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthFacts {
    pub total_conversations: u32,
    pub sla_breach_count: u32,
    pub sla_breach_rate: Option<f64>,
    pub high_friction_count: u32,
    pub high_friction_rate: Option<f64>,
    pub avg_first_response_minutes: Option<f64>,
    pub resolution_rate: Option<f64>,
}

pub fn get_health_facts(conn: &Connection, days_back: u32) -> Result<HealthFacts> {
    let metrics = get_dashboard_metrics(conn, None, days_back)?;
    let high_friction = count_high_friction(conn).unwrap_or(0);

    let total = metrics.total_conversations;
    let sla_breach_rate = if total > 0 {
        Some(metrics.sla_breach_count as f64 / total as f64)
    } else {
        None
    };
    let high_friction_rate = if total > 0 {
        Some(high_friction as f64 / total as f64)
    } else {
        None
    };
    let resolution_rate = if total > 0 {
        Some(metrics.closed_conversations as f64 / total as f64)
    } else {
        None
    };

    Ok(HealthFacts {
        total_conversations: total,
        sla_breach_count: metrics.sla_breach_count,
        sla_breach_rate,
        high_friction_count: high_friction,
        high_friction_rate,
        avg_first_response_minutes: metrics.avg_first_response_minutes,
        resolution_rate,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: Option<i64>,
    pub customer_id: i64,
    pub event_type: String,
    pub event_data: Option<String>,
    pub occurred_at: String,
}

pub fn record_timeline_event(
    conn: &Connection,
    customer_id: i64,
    event_type: &str,
    event_data: Option<&str>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO customer_timeline (customer_id, event_type, event_data_json)
         VALUES (?1, ?2, ?3)",
        params![customer_id, event_type, event_data],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_timeline(conn: &Connection, customer_id: i64, limit: u32) -> Result<Vec<TimelineEvent>> {
    let limit = limit.clamp(1, 500);
    let mut stmt = conn.prepare(
        "SELECT id, customer_id, event_type, event_data_json, occurred_at
         FROM customer_timeline
         WHERE customer_id = ?1
         ORDER BY julianday(occurred_at) DESC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![customer_id, limit], |r| {
            Ok(TimelineEvent {
                id: r.get(0)?,
                customer_id: r.get(1)?,
                event_type: r.get(2)?,
                event_data: r.get(3)?,
                occurred_at: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: Option<i64>,
    pub kind: String,
    pub entity_id: Option<i64>,
    pub label: Option<String>,
    pub properties: Option<String>,
    pub created_at: String,
}

pub fn validate_graph_node_kind(kind: &str) -> Result<GraphNodeKind> {
    for k in GraphNodeKind::ALL {
        if k.as_str() == kind {
            return Ok(k);
        }
    }
    Err(crate::error::Error::Config(format!(
        "unknown graph node kind: {kind}"
    )))
}

pub fn add_graph_node(
    conn: &Connection,
    kind: &str,
    entity_id: Option<i64>,
    label: Option<&str>,
    properties: Option<&str>,
) -> Result<i64> {
    validate_graph_node_kind(kind)?;
    conn.execute(
        "INSERT INTO graph_nodes (kind, entity_id, label, properties_json)
         VALUES (?1, ?2, ?3, ?4)",
        params![kind, entity_id, label, properties],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_graph_neighbors(
    conn: &Connection,
    kind: &str,
    local_id: i64,
) -> Result<Vec<crate::support_graph::GraphNodeRef>> {
    let Some(kind) = validate_graph_node_kind(kind).ok() else {
        return Ok(Vec::new());
    };
    let edges = crate::support_graph::human_edges_touching(conn, kind, local_id)?;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for edge in &edges {
        for side in ["source", "target"] {
            if let Some(v) = edge.get(side) {
                let key = serde_json::to_string(v).unwrap_or_default();
                if seen.insert(key)
                    && let Ok(r) =
                        serde_json::from_value::<crate::support_graph::GraphNodeRef>(v.clone())
                    && (r.kind != kind.as_str() || r.local_id != local_id)
                {
                    out.push(r);
                }
            }
        }
    }
    Ok(out)
}

pub fn get_graph_node(conn: &Connection, node_id: i64) -> Result<Option<GraphNode>> {
    struct NodeRow {
        id: i64,
        kind: String,
        entity_id: Option<i64>,
        label: Option<String>,
        properties: Option<String>,
        created_at: String,
    }
    let row: Option<NodeRow> = conn
        .query_row(
            "SELECT id, kind, entity_id, label, properties_json, created_at
             FROM graph_nodes WHERE id = ?1",
            params![node_id],
            |r| {
                Ok(NodeRow {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    entity_id: r.get(2)?,
                    label: r.get(3)?,
                    properties: r.get(4)?,
                    created_at: r.get(5)?,
                })
            },
        )
        .ok();
    match row {
        None => Ok(None),
        Some(r) => Ok(Some(GraphNode {
            id: Some(r.id),
            kind: r.kind,
            entity_id: r.entity_id,
            label: r.label,
            properties: r.properties,
            created_at: r.created_at,
        })),
    }
}

pub fn count_graph_nodes_by_kind(conn: &Connection, kind: &str) -> Result<u32> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM graph_nodes WHERE kind = ?1",
        params![kind],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(count).unwrap_or(0))
}

pub fn list_graph_nodes(conn: &Connection, limit: Option<u32>) -> Result<Vec<GraphNode>> {
    let limit = limit.unwrap_or(100).min(500) as i64;
    let mut stmt = conn.prepare(
        "SELECT id, kind, entity_id, label, properties_json, created_at
         FROM graph_nodes ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |r| {
        Ok(GraphNode {
            id: r.get(0)?,
            kind: r.get(1)?,
            entity_id: r.get(2)?,
            label: r.get(3)?,
            properties: r.get(4)?,
            created_at: r.get(5)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.into())
}

// ---------------------------------------------------------------------------
// P1: the happiness / CSAT readout (webhook-sourced local truth)
// ---------------------------------------------------------------------------

/// The local happiness overview P1 renders: rating counts in range, the
/// CSAT share (great / all ratings), and the recent ratings list. Every
/// number comes from `ratings` rows the satisfaction webhook delivered —
/// Help Scout exposes no list-ratings API to poll, so an empty table means
/// "no ratings received yet", never "zero ratings happened".
pub fn happiness_overview(conn: &Connection, days: i64) -> serde_json::Value {
    let days = days.clamp(1, 365);
    let since = format!(
        "{}",
        (chrono::Utc::now() - chrono::Duration::try_days(days).unwrap_or_default())
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
    );
    let count = |rating: &str| {
        conn.query_row(
            "SELECT COUNT(*) FROM ratings
              WHERE rating = ?1 AND remote_created_at >= ?2",
            params![rating, since],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
    };
    let great = count("great");
    let okay = count("okay");
    let not_good = count("not-good");
    let total = great + okay + not_good;
    let csat = if total > 0 {
        ((great as f64 / total as f64) * 100.0).round() as i64
    } else {
        0
    };
    let recent: Vec<serde_json::Value> = conn
        .prepare(
            "SELECT r.rating, r.comments, r.remote_created_at,
                    c.number, COALESCE(cu.first_name || ' ' || cu.last_name, '')
               FROM ratings r
               LEFT JOIN conversations c ON c.id = r.conversation_id
               LEFT JOIN customers cu ON cu.id = r.customer_local_id
              WHERE r.remote_created_at >= ?1
              ORDER BY r.remote_created_at DESC LIMIT 25",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![since], |r| {
                Ok(serde_json::json!({
                    "rating": r.get::<_, Option<String>>(0)?,
                    "comments": r.get::<_, Option<String>>(1)?,
                    "created_at": r.get::<_, Option<String>>(2)?,
                    "conversation_number": r.get::<_, Option<i64>>(3)?,
                    "customer": r.get::<_, Option<String>>(4)?.filter(|s| !s.is_empty()),
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    serde_json::json!({
        "range_days": days,
        "great": great,
        "okay": okay,
        "not_good": not_good,
        "total": total,
        "csat_percent": csat,
        "recent": recent,
        "source": "satisfaction webhook ratings (Help Scout has no list-ratings API)",
    })
}

#[cfg(test)]
mod p1_tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    #[test]
    fn happiness_overview_counts_and_csat_share() {
        let conn = db();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, remote_created_at)
             VALUES (900, 900, 'Rated conversation', 'p', 'closed', '2026-01-01T00:00:00.000Z')",
            [],
        )
        .expect("conversation");
        let now = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        conn.execute(
            "INSERT INTO ratings (remote_id, conversation_id, rating, comments, remote_created_at)
             VALUES (801, 1, 'great', 'Fast help', ?1),
                    (802, 1, 'great', NULL, ?1),
                    (803, 1, 'okay', NULL, ?1)",
            params![now],
        )
        .expect("ratings");

        let v = happiness_overview(&conn, 30);
        assert_eq!(v["great"], 2);
        assert_eq!(v["okay"], 1);
        assert_eq!(v["not_good"], 0);
        assert_eq!(v["total"], 3);
        assert_eq!(v["csat_percent"], 67, "2/3 great rounds to 67%");
        let recent = v["recent"].as_array().expect("recent");
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0]["conversation_number"], 900);
    }

    #[test]
    fn happiness_overview_empty_is_absent_not_zero() {
        let conn = db();
        let v = happiness_overview(&conn, 30);
        assert_eq!(v["total"], 0);
        assert!(
            v["source"]
                .as_str()
                .unwrap_or_default()
                .contains("no list-ratings API"),
            "the honest source note must survive"
        );
        // A rating outside the range never counts.
        conn.execute(
            "INSERT INTO ratings (remote_id, rating, remote_created_at)
             VALUES (804, 'great', '2020-01-01T00:00:00.000Z')",
            [],
        )
        .expect("old rating");
        assert_eq!(happiness_overview(&conn, 30)["total"], 0);
    }
}

#[cfg(test)]
mod batch30_tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    /// Audit item 109: the "new conversations" window keyed on
    /// local_created_at — the local INSERT stamp — so a backfilled
    /// conversation created 5 days ago but synced today counted as new,
    /// while a genuinely new conversation synced late still counted. The
    /// window keys on the platform creation stamp now and skips deleted
    /// rows.
    #[test]
    fn dashboard_new_conversations_key_on_creation_not_local_insert() {
        let conn = db();
        // Created 5 days ago on the platform, synced today
        // (local_created_at takes its datetime('now') default).
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, created_at)
             VALUES (10, 10, 'backfilled', 'p', 'active', datetime('now', '-5 days'))",
            [],
        )
        .expect("backfilled conversation");
        // Created 60 days ago on the platform, also synced today — not new.
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, created_at)
             VALUES (11, 11, 'old backfill', 'p', 'active', datetime('now', '-60 days'))",
            [],
        )
        .expect("old conversation");
        // Created now but soft-deleted — never counts.
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, created_at, deleted_at)
             VALUES (12, 12, 'deleted', 'p', 'active', datetime('now'), datetime('now'))",
            [],
        )
        .expect("deleted conversation");

        let m = get_dashboard_metrics(&conn, None, 30).expect("metrics");
        assert_eq!(m.new_conversations, 1, "only the 5-day-old creation counts");
    }

    /// Audit item 110: %Y+%W split one natural week at the year boundary
    /// (2025-12-30 reported as "2025-W53"-style, 2026-01-01 as "2026-W00").
    /// ISO %G-%V keeps both in 2026-W01. Buckets also key on the creation
    /// stamp and exclude deleted rows (item 109).
    #[test]
    fn week_dimension_is_iso_and_keeps_year_boundary_together() {
        let conn = db();
        for (remote, day) in [
            (20_i64, "2025-12-30T12:00:00.000Z"),
            (21, "2026-01-01T09:00:00.000Z"),
        ] {
            conn.execute(
                "INSERT INTO conversations (remote_id, number, subject, preview, status, created_at)
                 VALUES (?1, ?1, 'year boundary', 'p', 'active', ?2)",
                params![remote, day],
            )
            .expect("boundary conversation");
        }
        // Same week window, but deleted — must not join the bucket.
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, created_at, deleted_at)
             VALUES (22, 22, 'deleted boundary', 'p', 'active',
                     '2026-01-02T09:00:00.000Z', datetime('now'))",
            [],
        )
        .expect("deleted boundary conversation");

        let report = build_report(
            &conn,
            ReportMetricKey::Conversations,
            ReportDimensionKey::Week,
            400,
        )
        .expect("report");
        assert_eq!(
            report.rows.len(),
            1,
            "both boundary days share one ISO week"
        );
        assert_eq!(report.rows[0].dimension_value, "2026-W01");
        assert_eq!(report.rows[0].metric_value, 2.0);
    }

    /// Audit item 109: the Conversations metric's limitation claims
    /// "Does not include deleted conversations" — nothing filtered them.
    #[test]
    fn build_report_excludes_soft_deleted_conversations() {
        let conn = db();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, created_at)
             VALUES (30, 30, 'live', 'p', 'active', datetime('now', '-1 days'))",
            [],
        )
        .expect("live conversation");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, created_at, deleted_at)
             VALUES (31, 31, 'deleted', 'p', 'active', datetime('now', '-1 days'), datetime('now'))",
            [],
        )
        .expect("deleted conversation");

        let report = build_report(
            &conn,
            ReportMetricKey::Conversations,
            ReportDimensionKey::None,
            30,
        )
        .expect("report");
        assert!(report.metric_limitations.contains("deleted"));
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.rows[0].metric_value, 1.0);
    }
}
