use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::catalog::{GraphNodeKind, ReportDimensionKey, ReportMetricKey};
use crate::error::Result;

mod config;
pub(crate) use config::config_to_json;
pub use config::parse_report_config;

pub(crate) const M020_TO_M022_SQL: &str = r#"
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

pub(crate) fn apply_m020_to_m022(conn: &Connection) -> Result<()> {
    conn.execute_batch(M020_TO_M022_SQL)?;
    crate::support_graph::ensure_graph_edges_schema(conn)?;
    Ok(())
}

pub(crate) const M034_SQL: &str = r#"
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

pub(crate) fn apply_m034(conn: &Connection) -> Result<()> {
    conn.execute_batch(M034_SQL)?;
    ensure_mailbox_business_hours(conn)?;
    Ok(())
}

pub(crate) fn ensure_mailbox_business_hours(conn: &Connection) -> Result<()> {
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

pub(crate) fn ensure_report_definitions_table(conn: &Connection) -> Result<()> {
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
pub(crate) struct DashboardMetrics {
    pub total_conversations: u32,
    pub new_conversations: u32,
    pub closed_conversations: u32,
    pub active_conversations: u32,
    pub customer_waiting: u32,
    pub avg_first_response_minutes: Option<f64>,
    pub avg_resolution_minutes: Option<f64>,
    pub sla_breach_count: u32,
}

pub(crate) fn get_dashboard_metrics(
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
                "(t.type = 'reply' OR (t.type IN ('message','chat','phone') AND t.from_type IN ('user','agent')))",
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
pub(crate) struct FrictionScore {
    pub conversation_id: i64,
    pub effort_score: f64,
    pub factors: FrictionFactors,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct FrictionFactors {
    pub message_count: u32,
    pub resolution_hours: Option<f64>,
    pub reopen_count: u32,
    pub escalated: bool,
}

pub(crate) const HIGH_FRICTION_THRESHOLD: f64 = 0.6;

pub(crate) fn compute_friction(conn: &Connection, conversation_id: i64) -> Result<FrictionScore> {
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

pub(crate) fn record_timeline_event(
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

pub(crate) fn validate_graph_node_kind(kind: &str) -> Result<GraphNodeKind> {
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

pub fn delete_graph_node(conn: &Connection, node_id: i64) -> Result<bool> {
    let Some(node) = get_graph_node(conn, node_id)? else {
        return Ok(false);
    };
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM graph_edges
          WHERE (source_kind = ?1 AND source_local_id = ?2)
             OR (target_kind = ?1 AND target_local_id = ?2)",
        params![node.kind.as_str(), node.entity_id],
    )?;
    let changed = tx.execute("DELETE FROM graph_nodes WHERE id = ?1", params![node_id])?;
    tx.commit()?;
    Ok(changed > 0)
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
                    c.number, COALESCE(cu.first_name || ' ' || cu.last_name, ''), c.id
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
                    "conversation_id": r.get::<_, Option<i64>>(5)?,
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

    #[test]
    fn dashboard_new_conversations_key_on_creation_not_local_insert() {
        let conn = db();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, created_at)
             VALUES (10, 10, 'backfilled', 'p', 'active', datetime('now', '-5 days'))",
            [],
        )
        .expect("backfilled conversation");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, created_at)
             VALUES (11, 11, 'old backfill', 'p', 'active', datetime('now', '-60 days'))",
            [],
        )
        .expect("old conversation");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, created_at, deleted_at)
             VALUES (12, 12, 'deleted', 'p', 'active', datetime('now'), datetime('now'))",
            [],
        )
        .expect("deleted conversation");

        let m = get_dashboard_metrics(&conn, None, 30).expect("metrics");
        assert_eq!(m.new_conversations, 1, "only the 5-day-old creation counts");
    }
}

pub fn recompute_recent_friction(conn: &Connection, limit: u32) -> Result<(u32, u32)> {
    let limit = limit.clamp(1, 500) as i64;
    let ids: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM conversations
              WHERE status = 'closed' AND deleted_at IS NULL
              ORDER BY COALESCE(closed_at, remote_updated_at, created_at) DESC
              LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| r.get(0))?;
        rows.flatten().collect()
    };
    let mut high = 0u32;
    for id in &ids {
        let score = compute_friction(conn, *id)?;
        if score.effort_score >= HIGH_FRICTION_THRESHOLD {
            let customer_id: Option<i64> = conn
                .query_row(
                    "SELECT customer_local_id FROM conversations WHERE id = ?1",
                    params![id],
                    |r| r.get(0),
                )
                .ok()
                .flatten();
            if let Some(customer_id) = customer_id {
                let already: i64 = conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM customer_timeline
                          WHERE customer_id = ?1 AND event_type = 'high_friction'
                            AND json_extract(event_data_json, '$.conversation_id') = ?2)",
                        params![customer_id, id],
                        |r| r.get(0),
                    )
                    .unwrap_or(1);
                if already == 0 {
                    let data = format!(
                        "{{\"conversation_id\":{id},\"effort_score\":{:.2}}}",
                        score.effort_score
                    );
                    let _ = record_timeline_event(conn, customer_id, "high_friction", Some(&data));
                }
            }
            high += 1;
        }
    }
    Ok((u32::try_from(ids.len()).unwrap_or(0), high))
}

#[cfg(test)]
mod item56_tests {
    use super::*;
    use crate::reports::config::parse_report_config;
    use rusqlite::Connection;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    #[test]
    fn recompute_friction_persists_scores_and_deduped_milestones() {
        let conn = db();
        conn.execute(
            "INSERT INTO customers (id, remote_id, first_name) VALUES (1, 1, 'Test')",
            [],
        )
        .expect("customer");
        conn.execute(
            "INSERT INTO conversations (id, remote_id, number, subject, preview, status,
                                        customer_local_id, created_at, closed_at)
             VALUES (10, 10, 10, 'rough ride', 'p', 'closed', 1,
                     '2026-01-01T00:00:00.000Z', '2026-01-04T00:00:00.000Z')",
            [],
        )
        .expect("conversation");
        for i in 0..21 {
            conn.execute(
                "INSERT INTO activity_events (conversation_id, event_type, actor_type, occurred_at, dedup_key)
                 VALUES (10, 'message', 'customer', ?1, ?2)",
                params![
                    format!("2026-01-0{}T00:00:{:02}.000Z", 1 + (i % 4), i),
                    format!("m-{i}")
                ],
            )
            .expect("activity event");
        }

        let (computed, high) = recompute_recent_friction(&conn, 100).expect("recompute");
        assert_eq!(computed, 1);
        assert_eq!(high, 1, "0.6 crosses the 0.6 threshold");
        assert_eq!(count_high_friction(&conn).expect("count"), 1);
        let milestones: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM customer_timeline
                  WHERE event_type = 'high_friction' AND customer_id = 1",
                [],
                |r| r.get(0),
            )
            .expect("milestones");
        assert_eq!(milestones, 1);

        let (_again, _high) = recompute_recent_friction(&conn, 100).expect("recompute 2");
        let milestones: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM customer_timeline WHERE event_type = 'high_friction'",
                [],
                |r| r.get(0),
            )
            .expect("milestones 2");
        assert_eq!(milestones, 1, "milestone deduped per conversation");
    }

    #[test]
    fn builder_config_round_trips_through_json() {
        let conn = db();
        let config = ReportBuilderConfig {
            metric: crate::catalog::ReportMetricKey::Conversations,
            dimension: crate::catalog::ReportDimensionKey::Day,
            date_from: "2026-01-01".into(),
            date_to: "2026-01-31".into(),
            comparison: ReportComparison::PreviousPeriod,
            filters: ReportFilters::default(),
            sort: ReportSort::MetricDesc,
            limit: Some(20),
        };
        let outcome = run_builder_report(&conn, &config);
        assert!(outcome.is_ok(), "the builder runs over an empty mirror");
        let json = config_to_json(&config);
        let parsed = parse_report_config(&json).expect("parse");
        assert_eq!(parsed.metric, config.metric);
        assert_eq!(parsed.dimension, config.dimension);
        assert_eq!(parsed.comparison, config.comparison);
        assert_eq!(parsed.sort, config.sort);
        assert_eq!(parsed.limit, config.limit);
    }

    #[test]
    fn saved_reports_crud_round_trips() {
        let conn = db();
        let config = ReportBuilderConfig {
            metric: crate::catalog::ReportMetricKey::Closures,
            dimension: crate::catalog::ReportDimensionKey::None,
            date_from: "2026-02-01".into(),
            date_to: "2026-02-28".into(),
            comparison: ReportComparison::None,
            filters: ReportFilters::default(),
            sort: ReportSort::MetricDesc,
            limit: None,
        };
        let saved = save_report(&conn, "Closures this month", &config).expect("save");
        assert_eq!(saved.name, "Closures this month");
        let listed = list_saved_reports(&conn).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, saved.id);
        let restored = parse_report_config(&listed[0].config).expect("parse saved");
        assert_eq!(restored.metric, config.metric);
        assert!(delete_saved_report(&conn, saved.id).expect("delete"));
        assert!(list_saved_reports(&conn).expect("list").is_empty());
    }

    #[test]
    fn graph_nodes_validate_and_round_trip() {
        let conn = db();
        assert!(add_graph_node(&conn, "not_a_kind", None, None, None).is_err());
        let id = add_graph_node(
            &conn,
            "product",
            None,
            Some("SupportOS+"),
            Some("{\"tier\":\"app\"}"),
        )
        .expect("add node");
        let node = get_graph_node(&conn, id)
            .expect("get")
            .expect("node exists");
        assert_eq!(node.kind, "product");
        assert_eq!(node.label.as_deref(), Some("SupportOS+"));
        let listed = list_graph_nodes(&conn, None).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(
            validate_graph_node_kind("product")
                .expect("valid kind")
                .as_str(),
            "product"
        );
    }
}

#[cfg(test)]
mod audp2_tests {
    use super::*;
    use rusqlite::Connection;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut c).expect("migrations");
        c
    }

    #[test]
    fn audp2_happiness_recent_rows_carry_the_conversation_id() {
        let c = conn();
        c.execute(
            "INSERT INTO conversations (remote_id, number, subject, status) VALUES (31, 531, 's', 'closed')",
            [],
        )
        .expect("conversation");
        let conv = c.last_insert_rowid();
        c.execute(
            "INSERT INTO ratings (conversation_id, customer_local_id, rating, comments, remote_created_at)
             VALUES (?1, NULL, 'great', 'loved it', '2026-11-10T10:00:00.000Z')",
            rusqlite::params![conv],
        )
        .expect("rating");

        let data = happiness_overview(&c, 30);
        let recent = data["recent"].as_array().expect("recent");
        assert_eq!(recent.len(), 1);
        assert_eq!(
            recent[0]["conversation_id"].as_i64(),
            Some(conv),
            "the deep-link target rides the row"
        );
        assert_eq!(recent[0]["conversation_number"].as_i64(), Some(531));
    }

    #[test]
    fn audp2_delete_graph_node_removes_the_node_and_touching_edges() {
        let c = conn();
        let a = add_graph_node(&c, "customer", Some(9), Some("Kim"), None).expect("node a");
        let b = add_graph_node(&c, "organization", Some(3), Some("Globex"), None).expect("node b");
        c.execute(
            "INSERT INTO graph_edges (source_kind, source_local_id, target_kind, target_local_id, relation)
             VALUES ('customer', 9, 'organization', 3, 'related_to')",
            [],
        )
        .expect("edge");

        assert!(delete_graph_node(&c, a).expect("delete"));
        assert!(
            get_graph_node(&c, a).expect("get").is_none(),
            "the node is gone"
        );
        let edges: i64 = c
            .query_row("SELECT COUNT(*) FROM graph_edges", [], |r| r.get(0))
            .expect("edges");
        assert_eq!(edges, 0, "the touching human edge is removed with it");

        assert!(get_graph_node(&c, b).expect("get").is_some());
        assert!(!delete_graph_node(&c, a).expect("delete again"));
    }
}
