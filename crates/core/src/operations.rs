use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::catalog::OperationsTileKey;
use crate::error::{Error, Result};
use crate::saved_views::RESPONSE_STATE_SQL;

pub const OPS_WAITING_THRESHOLD_SETTING: &str = "ops_waiting_threshold_minutes";

pub const OPS_WAITING_THRESHOLD_DEFAULT: i64 = 240;

pub const OPS_TILE_WHITELIST: [&str; 9] = [
    "unassigned",
    "needs_first_response",
    "customer_waiting",
    "waiting_over_threshold",
    "urgent",
    "high_effort",
    "repeated_issue",
    "known_issue",
    "ai_escalation",
];

pub const OPS_INVALID_MESSAGE: &str = "ops must be one of: unassigned, needs_first_response, customer_waiting, waiting_over_threshold, urgent, high_effort, repeated_issue, known_issue, ai_escalation.";

#[must_use]
pub fn is_conversation_ops_tile(key: &str) -> bool {
    OPS_TILE_WHITELIST.contains(&key)
}

const NOT_DELETED: &str = "c.deleted_at IS NULL AND c.merged_into_conversation_id IS NULL";

#[must_use]
pub fn tile_fragment(
    key: &str,
    waiting_threshold_minutes: i64,
) -> Option<(String, Vec<rusqlite::types::Value>)> {
    let sql: String = match key {
        "unassigned" => {
            format!("{NOT_DELETED} AND c.status IN ('active','pending') AND c.assignee_local_id IS NULL")
        }
        "needs_first_response" => {
            format!("{NOT_DELETED} AND ({RESPONSE_STATE_SQL}) = 'needs_first_response'")
        }
        "customer_waiting" => {
            format!("{NOT_DELETED} AND ({RESPONSE_STATE_SQL}) = 'customer_waiting'")
        }
        "waiting_over_threshold" => {
            format!(
                "{NOT_DELETED} AND ({RESPONSE_STATE_SQL}) = 'customer_waiting' \
                 AND c.customer_waiting_since IS NOT NULL \
                 AND (julianday('now') - julianday(c.customer_waiting_since)) * 1440 >= ?"
            )
        }
        "urgent" => format!(
            "{NOT_DELETED} AND c.status IN ('active','pending') \
             AND c.supportos_priority IN ('high','urgent')"
        ),
        "high_effort" => {
            format!(
                "{NOT_DELETED} AND c.status IN ('active','pending') AND EXISTS (
          SELECT 1 FROM client_current_signals s
          WHERE s.conversation_id = c.id
            AND (
              EXISTS (SELECT 1 FROM json_each(s.signals_json) je
                      WHERE json_extract(je.value, '$.dimension') = 'frustration'
                        AND json_extract(je.value, '$.value') = 'strong')
              OR CAST(json_extract(s.message_stats_json, '$.customer_messages') AS INTEGER) >= 5
            )
        )"
            )
        }
        "repeated_issue" => {
            format!(
                "{NOT_DELETED} AND c.status IN ('active','pending') AND EXISTS (
          SELECT 1 FROM known_issue_links kic
          WHERE kic.conversation_id = c.id
            AND (SELECT COUNT(*) FROM known_issue_links kic2
                 JOIN conversations c2 ON c2.id = kic2.conversation_id
                 WHERE kic2.known_issue_id = kic.known_issue_id
                   AND c2.customer_local_id = c.customer_local_id
                   AND c2.deleted_at IS NULL) >= 2
        )"
            )
        }
        "known_issue" => {
            format!(
                "{NOT_DELETED} AND c.status IN ('active','pending') AND EXISTS (
          SELECT 1 FROM known_issue_links kic
          JOIN known_issues ki ON ki.id = kic.known_issue_id
          WHERE kic.conversation_id = c.id AND ki.status != 'resolved'
        )"
            )
        }
        "ai_escalation" => {
            format!(
                "{NOT_DELETED} AND c.status IN ('active','pending') AND EXISTS (
          SELECT 1 FROM ai_runs a
          WHERE a.conversation_id = c.id AND a.type = 'ticket_analysis' AND a.status = 'completed'
            AND a.id = (SELECT MAX(a2.id) FROM ai_runs a2
                        WHERE a2.conversation_id = c.id AND a2.type = 'ticket_analysis' AND a2.status = 'completed')
            AND (json_extract(a.response_json, '$.urgency') IN ('high','critical')
                 OR json_extract(a.response_json, '$.sentiment') = 'frustrated')
            AND json_extract(a.response_json, '$.confidence') IN ('medium','high')
        )"
            )
        }
        _ => return None,
    };
    let params = if key == "waiting_over_threshold" {
        vec![rusqlite::types::Value::Integer(
            waiting_threshold_minutes.max(1),
        )]
    } else {
        Vec::new()
    };
    Some((sql, params))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OperationsTile {
    pub key: String,
    pub label: String,
    pub count: u32,
    pub severity: String,
    pub drill: Value,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationsSnapshot {
    pub generated_at: String,
    pub mailbox_scope: Option<Vec<i64>>,
    pub tiles: Vec<OperationsTile>,
    pub waiting_threshold_minutes: i64,
}

impl OperationsSnapshot {
    #[must_use]
    pub fn count_of(&self, key: &str) -> Option<u32> {
        self.tiles.iter().find(|t| t.key == key).map(|t| t.count)
    }
}

#[must_use]
pub fn waiting_threshold_minutes(conn: &Connection) -> i64 {
    let raw = crate::settings::get_i64(
        conn,
        OPS_WAITING_THRESHOLD_SETTING,
        OPS_WAITING_THRESHOLD_DEFAULT,
    )
    .unwrap_or(OPS_WAITING_THRESHOLD_DEFAULT);
    if (1..=20_160).contains(&raw) {
        raw
    } else {
        OPS_WAITING_THRESHOLD_DEFAULT
    }
}

pub fn set_waiting_threshold_minutes(conn: &Connection, minutes: i64) -> Result<()> {
    let clamped = minutes.clamp(1, 20_160);
    crate::settings::set_i64(conn, OPS_WAITING_THRESHOLD_SETTING, clamped)
}

fn sanitize_scope(mailbox_ids: Option<&[i64]>) -> Option<Vec<i64>> {
    let ids: Vec<i64> = mailbox_ids?
        .iter()
        .copied()
        .filter(|id| *id > 0)
        .take(50)
        .collect();
    if ids.is_empty() {
        None
    } else {
        Some(ids)
    }
}

pub fn snapshot(conn: &Connection, mailbox_ids: Option<&[i64]>) -> Result<OperationsSnapshot> {
    let scope = sanitize_scope(mailbox_ids);
    let scope_sql = scope.as_ref().map(|ids| {
        format!(
            "c.mailbox_local_id IN ({})",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        )
    });
    let threshold = waiting_threshold_minutes(conn);

    let conv_count = |key: &str| -> Result<u32> {
        let (frag_sql, frag_params) = tile_fragment(key, threshold).ok_or_else(|| {
            Error::Config(format!("conversation tile {key} must have a fragment"))
        })?;
        let mut sql = format!("SELECT COUNT(*) FROM conversations c WHERE {frag_sql}");
        let mut params = frag_params;
        if let Some(scope_sql) = scope_sql.as_ref() {
            sql.push_str(" AND ");
            sql.push_str(scope_sql);
            params.extend(
                scope
                    .iter()
                    .flatten()
                    .map(|id| rusqlite::types::Value::Integer(*id)),
            );
        }
        let n: i64 = conn.query_row(&sql, rusqlite::params_from_iter(params.iter()), |r| {
            r.get(0)
        })?;
        Ok(u32::try_from(n).unwrap_or(0))
    };

    let inbox_drill = |ops: Option<&str>, view: &str| -> Value {
        match ops {
            Some(ops) => json!({ "type": "inbox", "params": { "view": view, "ops": ops } }),
            None => json!({ "type": "inbox", "params": { "view": view } }),
        }
    };
    let page_drill = |page: &str| -> Value { json!({ "type": "page", "page": page }) };
    let tile = |key: OperationsTileKey,
                label: String,
                count: u32,
                severity: &str,
                drill: Value,
                note: Option<String>| {
        OperationsTile {
            key: key.as_str().to_string(),
            label,
            count,
            severity: severity.to_string(),
            drill,
            note,
        }
    };

    let mut tiles = Vec::with_capacity(OperationsTileKey::ALL.len());

    tiles.push(tile(
        OperationsTileKey::Unassigned,
        "Unassigned".into(),
        conv_count("unassigned")?,
        "info",
        inbox_drill(None, "unassigned"),
        None,
    ));
    tiles.push(tile(
        OperationsTileKey::NeedsFirstResponse,
        "Needs first response".into(),
        conv_count("needs_first_response")?,
        "warning",
        inbox_drill(Some("needs_first_response"), "active"),
        None,
    ));
    tiles.push(tile(
        OperationsTileKey::CustomerWaiting,
        "Customer waiting".into(),
        conv_count("customer_waiting")?,
        "info",
        inbox_drill(Some("customer_waiting"), "active"),
        None,
    ));
    tiles.push(tile(
        OperationsTileKey::WaitingOverThreshold,
        format!("Waiting > {threshold} min"),
        conv_count("waiting_over_threshold")?,
        "warning",
        inbox_drill(Some("waiting_over_threshold"), "active"),
        Some(
            "Wall-clock minutes since the customer's last message (business hours not applied). Threshold is configurable in Settings."
                .to_string(),
        ),
    ));
    tiles.push(tile(
        OperationsTileKey::Urgent,
        "Urgent / high priority".into(),
        conv_count("urgent")?,
        "warning",
        inbox_drill(Some("urgent"), "active"),
        Some("SupportOS priority (local field), high + urgent.".to_string()),
    ));

    let alerts = crate::sla::sla_alerts(conn)?;
    let scoped_alerts: Vec<&crate::sla::SlaAlertRow> = alerts
        .alerts
        .iter()
        .filter(|a| scope.as_ref().is_none_or(|ids| ids.contains(&a.mailbox_id)))
        .collect();
    let at_risk = scoped_alerts
        .iter()
        .filter(|a| a.state == crate::sla::SlaAlertState::AtRisk)
        .count();
    let breached = scoped_alerts
        .iter()
        .filter(|a| a.state == crate::sla::SlaAlertState::Breached)
        .count();
    tiles.push(tile(
        OperationsTileKey::SlaAtRisk,
        "SLA at risk".into(),
        at_risk as u32,
        "warning",
        page_drill("issues"),
        Some(if alerts.unconfigured_mailboxes.is_empty() {
            "Business minutes at 80%+ of the mailbox target (Issue Radar holds the detail)."
                .to_string()
        } else {
            format!(
                "{} mailbox(es) have no business hours/targets configured and are not monitored.",
                alerts.unconfigured_mailboxes.len()
            )
        }),
    ));
    tiles.push(tile(
        OperationsTileKey::SlaBreached,
        "SLA breached".into(),
        breached as u32,
        "critical",
        page_drill("issues"),
        Some(
            "Business minutes past the mailbox target (Issue Radar holds the detail).".to_string(),
        ),
    ));

    tiles.push(tile(
        OperationsTileKey::HighEffort,
        "High customer effort".into(),
        conv_count("high_effort")?,
        "info",
        inbox_drill(Some("high_effort"), "active"),
        Some(
            "Heuristic: strong frustration signal or 5+ customer messages in the conversation."
                .to_string(),
        ),
    ));
    tiles.push(tile(
        OperationsTileKey::RepeatedIssue,
        "Repeated issue".into(),
        conv_count("repeated_issue")?,
        "info",
        inbox_drill(Some("repeated_issue"), "active"),
        Some("The same customer has 2+ conversations linked to one known issue.".to_string()),
    ));
    tiles.push(tile(
        OperationsTileKey::KnownIssue,
        "Known issue (open tickets)".into(),
        conv_count("known_issue")?,
        "info",
        inbox_drill(Some("known_issue"), "active"),
        Some("Open conversations linked to unresolved known issues.".to_string()),
    ));
    tiles.push(tile(
        OperationsTileKey::AiEscalation,
        "AI escalation".into(),
        conv_count("ai_escalation")?,
        "warning",
        inbox_drill(Some("ai_escalation"), "active"),
        Some(
            "Latest AI analysis: urgency high/critical or frustrated sentiment, medium/high confidence."
                .to_string(),
        ),
    ));
    tiles.push(tile(
        OperationsTileKey::IssueSpike,
        "Issue spike".into(),
        count_issue_spikes(conn)?,
        "warning",
        page_drill("issues"),
        Some("Issue clusters trending up (Issue Radar detail).".to_string()),
    ));
    tiles.push(tile(
        OperationsTileKey::AutomationApprovals,
        "Automation approvals".into(),
        count_automation_approvals(conn)?,
        "warning",
        page_drill("automation"),
        Some("Actions parked awaiting explicit human approval.".to_string()),
    ));
    tiles.push(tile(
        OperationsTileKey::FailedJobs,
        "Failed jobs (7d)".into(),
        count_failed_jobs(conn)?,
        "warning",
        page_drill("automation"),
        Some("Background jobs that exhausted their retries in the last 7 days.".to_string()),
    ));
    let sync_problems = sync_problems(conn)?;
    tiles.push(tile(
        OperationsTileKey::SyncProblems,
        "Sync problems".into(),
        sync_problems.count,
        if sync_problems.state == "ERROR" {
            "critical"
        } else {
            "info"
        },
        page_drill("sync-health"),
        Some(sync_problems.note),
    ));
    tiles.push(tile(
        OperationsTileKey::CampaignActivity,
        "Campaign activity".into(),
        count_campaign_activity(conn)?,
        "info",
        page_drill("outreach"),
        Some("Campaigns queued/sending/paused right now.".to_string()),
    ));

    Ok(OperationsSnapshot {
        generated_at: iso_now(),
        mailbox_scope: scope,
        tiles,
        waiting_threshold_minutes: threshold,
    })
}

fn iso_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    crate::saved_views::iso_utc_pub(now)
}

fn count_issue_spikes(conn: &Connection) -> Result<u32> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM issue_clusters WHERE trend = 'rising'",
        [],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(n).unwrap_or(0))
}

fn count_automation_approvals(conn: &Connection) -> Result<u32> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM jobs WHERE type = 'automation_action_awaiting_approval' AND status IN ('queued','awaiting_approval')",
        [],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(n).unwrap_or(0))
}

fn count_failed_jobs(conn: &Connection) -> Result<u32> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM jobs WHERE status = 'failed' AND julianday(COALESCE(completed_at, created_at)) >= julianday('now', '-7 days')",
        [],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(n).unwrap_or(0))
}

fn sync_problems(conn: &Connection) -> Result<SyncProblems> {
    let state: String = conn
        .query_row(
            "SELECT value FROM application_settings WHERE key = 'sync_state'",
            [],
            |r| r.get(0),
        )
        .map(|v: String| v.replace('"', ""))
        .unwrap_or_else(|_| "NEW".to_string());
    let recent_errors: i64 = conn.query_row(
        "SELECT COUNT(*) FROM application_errors WHERE julianday(timestamp) >= julianday('now', '-1 day')",
        [],
        |r| r.get(0),
    )?;
    let recent_errors = u32::try_from(recent_errors).unwrap_or(0);
    let problematic = u32::from(state == "ERROR");
    let count = problematic + recent_errors;
    let note = if state == "ERROR" {
        if recent_errors > 0 {
            format!("Sync state is ERROR plus {recent_errors} application error(s) in 24h.")
        } else {
            "Sync state is ERROR.".to_string()
        }
    } else if recent_errors > 0 {
        format!("{recent_errors} application error(s) logged in the last 24h (state: {state}).")
    } else {
        format!("Sync state: {state}.")
    };
    Ok(SyncProblems { count, state, note })
}

struct SyncProblems {
    count: u32,
    state: String,
    note: String,
}

fn count_campaign_activity(conn: &Connection) -> Result<u32> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM outreach_campaigns WHERE status IN ('queued','sending','paused')",
        [],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(n).unwrap_or(0))
}
