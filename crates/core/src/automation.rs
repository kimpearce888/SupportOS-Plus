use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::migrations::add_column_if_missing;

pub fn apply_m007(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS automation_rules (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            name            TEXT NOT NULL,
            trigger_json    TEXT NOT NULL,
            action_json     TEXT NOT NULL,
            enabled         INTEGER NOT NULL DEFAULT 1,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );

        CREATE TABLE IF NOT EXISTS automation_approvals (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            rule_id             INTEGER NOT NULL REFERENCES automation_rules (id) ON DELETE CASCADE,
            conversation_id     INTEGER NOT NULL,
            proposed_action_json TEXT NOT NULL,
            status              TEXT NOT NULL DEFAULT 'pending',
            decided_by_user_id  INTEGER,
            decided_at          TEXT,
            created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_automation_approvals_status
            ON automation_approvals (status, created_at);",
    )?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 7 WHERE id = 1", []);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    StatusChanged {
        from_status: Option<String>,
        to_status: Option<String>,
    },
    TagAdded {
        tag: String,
    },
    SlaRisk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Assign { assignee_remote_id: i64 },
    AddTag { tag: String },
    SendNote { body: String },
    ChangeStatus { new_status: String },
    SetPriority { new_priority: String },
}

impl Action {
    #[must_use]
    pub fn requires_approval(&self) -> bool {
        match self {
            Self::Assign { .. } | Self::ChangeStatus { .. } => true,
            Self::SetPriority { new_priority } => new_priority == "urgent",
            Self::AddTag { .. } | Self::SendNote { .. } => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationRule {
    pub id: Option<i64>,
    pub name: String,
    pub trigger: Trigger,
    pub action: Action,
    pub enabled: bool,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationApproval {
    pub id: Option<i64>,
    pub rule_id: i64,
    pub conversation_id: i64,
    pub proposed_action_json: String,
    pub status: String,
    pub decided_by_user_id: Option<i64>,
    pub decided_at: Option<String>,
    pub created_at: String,
}

pub fn create_rule(conn: &Connection, rule: &AutomationRule) -> Result<i64> {
    let trigger_json = serde_json::to_string(&rule.trigger)
        .map_err(|e| crate::error::Error::Config(format!("trigger serialization failed: {e}")))?;
    let action_json = serde_json::to_string(&rule.action)
        .map_err(|e| crate::error::Error::Config(format!("action serialization failed: {e}")))?;
    conn.execute(
        "INSERT INTO automation_rules (name, trigger_json, action_json, enabled)
         VALUES (?1, ?2, ?3, ?4)",
        params![rule.name, trigger_json, action_json, rule.enabled as i64],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_rules(conn: &Connection) -> Result<Vec<AutomationRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, trigger_json, action_json, enabled, created_at
         FROM automation_rules ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    })?;
    let mut rules = Vec::new();
    for row in rows {
        let (id, name, trigger_json, action_json, enabled, created_at) = row?;
        let trigger: Trigger = serde_json::from_str(&trigger_json).map_err(|e| {
            crate::error::Error::Config(format!("trigger deserialization failed: {e}"))
        })?;
        let action: Action = serde_json::from_str(&action_json).map_err(|e| {
            crate::error::Error::Config(format!("action deserialization failed: {e}"))
        })?;
        rules.push(AutomationRule {
            id: Some(id),
            name,
            trigger,
            action,
            enabled: enabled != 0,
            created_at,
        });
    }
    Ok(rules)
}

pub fn set_rule_enabled(conn: &Connection, rule_id: i64, enabled: bool) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE automation_rules SET enabled = ?1 WHERE id = ?2",
        params![enabled as i64, rule_id],
    )?;
    Ok(rows > 0)
}

pub fn delete_rule(conn: &Connection, rule_id: i64) -> Result<bool> {
    let rows = conn.execute(
        "DELETE FROM automation_rules WHERE id = ?1",
        params![rule_id],
    )?;
    Ok(rows > 0)
}

pub fn approve(conn: &Connection, approval_id: i64, decided_by_user_id: i64) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE automation_approvals
         SET status = 'approved', decided_by_user_id = ?1,
             decided_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?2 AND status = 'pending'",
        params![decided_by_user_id, approval_id],
    )?;
    Ok(rows > 0)
}

pub fn reject(conn: &Connection, approval_id: i64, decided_by_user_id: i64) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE automation_approvals
         SET status = 'rejected', decided_by_user_id = ?1,
             decided_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?2 AND status = 'pending'",
        params![decided_by_user_id, approval_id],
    )?;
    Ok(rows > 0)
}

#[must_use]
pub fn risk_tier(kind: &str) -> Option<&'static str> {
    match kind {
        "analyze_ticket" | "search_similar" | "check_known_issues" => Some("read"),
        "create_ai_note" | "create_ai_draft" | "add_tag" | "manual_review_queue" => {
            Some("non_destructive")
        }
        "set_status" | "assign" => Some("higher_risk"),
        _ => None,
    }
}

pub fn ensure_main_rule_columns(conn: &Connection) -> Result<()> {
    add_column_if_missing(
        conn,
        "automation_rules",
        "conditions",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    add_column_if_missing(conn, "automation_rules", "priority", "INTEGER DEFAULT 100")?;
    add_column_if_missing(
        conn,
        "automation_rules",
        "requires_approval",
        "INTEGER DEFAULT 1",
    )?;
    add_column_if_missing(conn, "automation_rules", "last_run_at", "TEXT")?;
    add_column_if_missing(conn, "automation_rules", "run_count", "INTEGER DEFAULT 0")?;
    crate::intelligence_features::apply_m035(conn)?;
    add_column_if_missing(conn, "automation_runs", "detail", "TEXT")
}

pub fn apply_m060_automation_once_guard(conn: &Connection) -> Result<()> {
    add_column_if_missing(
        conn,
        "conversations",
        "automation_last_customer_thread_id",
        "INTEGER",
    )
}

#[must_use]
pub fn customer_thread_trigger_decision(
    last_fired: Option<i64>,
    newest_customer_thread: Option<i64>,
) -> Option<&'static str> {
    match newest_customer_thread {
        None => None,
        Some(thread) if last_fired == Some(thread) => None,
        Some(_) if last_fired.is_none() => Some("new_conversation"),
        Some(_) => Some("customer_reply"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub path: String,
    pub message: String,
}

impl ValidationIssue {}

#[derive(Debug, Clone)]
pub struct RuleRecord {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub trigger: String,
    pub conditions: serde_json::Value,
    pub actions: serde_json::Value,
    pub priority: i64,
    pub requires_approval: bool,
    pub last_run_at: Option<String>,
    pub run_count: i64,
}

impl RuleRecord {
    #[must_use]
    pub fn to_main_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "name": self.name,
            "enabled": if self.enabled { 1 } else { 0 },
            "trigger": self.trigger,
            "conditions": self.conditions,
            "actions": self.actions,
            "priority": self.priority,
            "requires_approval": if self.requires_approval { 1 } else { 0 },
            "last_run_at": self.last_run_at,
            "run_count": self.run_count,
        })
    }
}

const RULE_RECORD_SELECT: &str =
    "SELECT id, name, trigger_json, action_json, COALESCE(enabled, 0), conditions,
            COALESCE(priority, 0), COALESCE(requires_approval, 0), last_run_at,
            COALESCE(run_count, 0)
     FROM automation_rules";

type RuleRow = (
    i64,
    String,
    String,
    String,
    i64,
    String,
    i64,
    i64,
    Option<String>,
    i64,
);

fn read_rule_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<RuleRow> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
        r.get(8)?,
        r.get(9)?,
    ))
}

fn parse_trigger_json(raw: &str) -> Result<String> {
    if let Ok(s) = serde_json::from_str::<String>(raw) {
        return Ok(s);
    }
    if let Ok(t) = serde_json::from_str::<Trigger>(raw) {
        return Ok(match t {
            Trigger::StatusChanged { .. } => "status_changed".to_string(),
            Trigger::TagAdded { .. } => "tag_added".to_string(),
            Trigger::SlaRisk => "sla_risk".to_string(),
        });
    }
    Err(crate::error::Error::Config(format!(
        "trigger deserialization failed: {raw}"
    )))
}

fn parse_rule_record(row: RuleRow) -> Result<RuleRecord> {
    let (
        id,
        name,
        trigger_json,
        action_json,
        enabled,
        conditions_json,
        priority,
        requires_approval,
        last_run_at,
        run_count,
    ) = row;
    let trigger: String = parse_trigger_json(&trigger_json)?;
    let actions: serde_json::Value = serde_json::from_str(&action_json)
        .map_err(|e| crate::error::Error::Config(format!("actions deserialization failed: {e}")))?;
    let conditions: serde_json::Value = serde_json::from_str(&conditions_json).map_err(|e| {
        crate::error::Error::Config(format!("conditions deserialization failed: {e}"))
    })?;
    Ok(RuleRecord {
        id,
        name,
        enabled: enabled == 1,
        trigger,
        conditions,
        actions,
        priority,
        requires_approval: requires_approval == 1,
        last_run_at,
        run_count,
    })
}

pub fn list_rule_records(conn: &Connection) -> Result<Vec<RuleRecord>> {
    let mut stmt = conn.prepare(&format!("{RULE_RECORD_SELECT} ORDER BY priority, id"))?;
    let rows = stmt
        .query_map([], read_rule_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|r| match parse_rule_record(r) {
            Ok(rec) => Some(rec),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "skipping unparsable automation rule; the engine continues"
                );
                None
            }
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

#[derive(Debug, Clone)]
pub struct RunRecord {
    pub id: i64,
    pub rule_id: i64,
    pub conversation_id: Option<i64>,
    pub triggered_at: String,
    pub status: String,
    pub detail: Option<String>,
}

impl RunRecord {
    #[must_use]
    pub fn to_main_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "rule_id": self.rule_id,
            "conversation_id": self.conversation_id,
            "triggered_at": self.triggered_at,
            "status": self.status,
            "detail": self.detail,
        })
    }

    fn read(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            rule_id: r.get(1)?,
            conversation_id: r.get(2)?,
            triggered_at: r.get(3)?,
            status: r.get(4)?,
            detail: r.get(5)?,
        })
    }
}

const RUN_SELECT: &str =
    "SELECT id, rule_id, conversation_id, created_at, outcome, detail FROM automation_runs";

fn record_run(
    conn: &Connection,
    rule_id: i64,
    conversation_id: Option<i64>,
    status: &str,
    detail: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO automation_runs (rule_id, conversation_id, triggered_by, outcome, detail)
         VALUES (?1, ?2, 'manual', ?3, ?4)",
        params![rule_id, conversation_id, status, detail],
    )?;
    Ok(())
}

fn enqueue_awaiting_approval(
    conn: &Connection,
    rule_id: i64,
    conversation_id: i64,
    action: &serde_json::Value,
) -> Result<()> {
    let payload = serde_json::json!({
        "ruleId": rule_id,
        "conversationId": conversation_id,
        "action": action,
    });
    crate::jobs::enqueue_on(
        conn,
        "ai",
        "automation_action_awaiting_approval",
        &payload.to_string(),
        2,
        1,
    )?;
    Ok(())
}

fn execute_non_destructive(
    conn: &Connection,
    kind: &str,
    params: &serde_json::Value,
    conversation_id: i64,
    ai_enabled: bool,
) -> Result<bool> {
    match kind {
        "create_ai_note" => {
            if !ai_enabled {
                return Ok(false);
            }
            crate::jobs::enqueue_on(
                conn,
                "ai",
                "create_ai_note",
                &format!("{{\"conversationId\":{conversation_id}}}"),
                2,
                1,
            )?;
            Ok(true)
        }
        "create_ai_draft" => {
            if !ai_enabled {
                return Ok(false);
            }
            crate::jobs::enqueue_on(
                conn,
                "ai",
                "generate_draft",
                &format!("{{\"conversationId\":{conversation_id}}}"),
                2,
                1,
            )?;
            Ok(true)
        }
        "add_tag" => {
            if let Some(tag) = params.get("tag").and_then(|t| t.as_str()) {
                let mut tags =
                    crate::conversation_ops::read_conversation_tags(conn, conversation_id);
                if !tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                    tags.push(tag.to_string());
                    crate::conversation_ops::write_conversation_tags(conn, conversation_id, &tags);
                    crate::conversation_ops::create_outbound_job_keyed(
                        conn,
                        "update_tags",
                        &serde_json::json!({
                            "conversationId": conversation_id,
                            "tags": tags,
                        }),
                        Some(conversation_id),
                        Some(&format!("automation_tags_{conversation_id}")),
                    )?;
                }
            }
            Ok(true)
        }
        "manual_review_queue" => {
            conn.execute(
                "UPDATE conversations SET is_unread = 1 WHERE id = ?1",
                params![conversation_id],
            )?;
            Ok(true)
        }
        _ => Ok(true),
    }
}

pub fn execute_approved_action(
    conn: &mut Connection,
    conversation_id: i64,
    action: &serde_json::Value,
) -> Result<bool> {
    let kind = action
        .get("kind")
        .and_then(|k| k.as_str())
        .unwrap_or_default();
    let params = action
        .get("params")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let ai_enabled = !matches!(
        crate::ai_hub::backend_for_role(conn, crate::ai_hub::AiRole::Classify),
        crate::ai_pipeline::AiBackend::Disabled
    );
    match kind {
        "set_status" | "assign" => {
            let remote: Option<i64> = conn
                .query_row(
                    "SELECT remote_id FROM conversations WHERE id = ?1",
                    params![conversation_id],
                    |r| r.get(0),
                )
                .ok();
            let Some(conversation_remote_id) = remote else {
                return Ok(false);
            };
            let assignee_local_id: Option<i64> = if kind == "set_status" {
                None
            } else {
                params
                    .get("userId")
                    .and_then(|u| u.as_i64())
                    .and_then(|id| {
                        conn.query_row(
                            "SELECT id FROM users WHERE id = ?1 OR remote_id = ?1",
                            params![id],
                            |r| r.get(0),
                        )
                        .ok()
                    })
            };
            let op = if kind == "set_status" {
                let new_status = params
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("closed")
                    .to_string();
                crate::ticket_ops::TicketOperation::ChangeStatus {
                    conversation_remote_id,
                    new_status,
                    actor_type: "automation".to_string(),
                    actor_id: None,
                }
            } else {
                crate::ticket_ops::TicketOperation::Assign {
                    conversation_remote_id,
                    assignee_local_id,
                    actor_type: "automation".to_string(),
                    actor_id: None,
                }
            };
            match crate::ticket_ops::execute(conn, &op) {
                Ok(crate::ticket_ops::OperationResult::Success { .. }) => {
                    let (job_kind, payload, key) = if kind == "set_status" {
                        (
                            "update_status",
                            serde_json::json!({
                                "conversationId": conversation_id,
                                "status": params
                                    .get("status")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("closed"),
                            }),
                            format!("automation_status_{conversation_id}"),
                        )
                    } else {
                        (
                            "assign",
                            serde_json::json!({
                                "conversationId": conversation_id,
                                "userId": assignee_local_id,
                            }),
                            format!("automation_assign_{conversation_id}"),
                        )
                    };
                    crate::conversation_ops::create_outbound_job_keyed(
                        conn,
                        job_kind,
                        &payload,
                        Some(conversation_id),
                        Some(&key),
                    )?;
                    Ok(true)
                }
                Ok(crate::ticket_ops::OperationResult::Rejected { .. }) => Ok(false),
                Err(_) => Ok(false),
            }
        }
        _ => execute_non_destructive(conn, kind, &params, conversation_id, ai_enabled),
    }
}

pub fn record_approved_run(
    conn: &Connection,
    rule_id: i64,
    conversation_id: i64,
    kind: &str,
) -> Result<()> {
    record_run(
        conn,
        rule_id,
        Some(conversation_id),
        "completed",
        &format!("Approved by human: executed {kind}"),
    )
}

#[derive(Debug, Clone, Default)]
struct FireContext {
    subject: Option<String>,
    preview: Option<String>,
    tags: Option<String>,
    mailbox_name: Option<String>,
}

fn load_fire_context(conn: &Connection, conversation_id: i64) -> Result<Option<FireContext>> {
    let row = conn
        .query_row(
            "SELECT c.subject, c.preview,
                    (SELECT GROUP_CONCAT(t.name) FROM conversation_tags ct
                      JOIN tags t ON t.id = ct.tag_id
                     WHERE ct.conversation_id = c.id),
                    (SELECT m.name FROM mailboxes m WHERE m.id = c.mailbox_local_id)
               FROM conversations c WHERE c.id = ?1",
            params![conversation_id],
            |r| {
                Ok(FireContext {
                    subject: r.get(0)?,
                    preview: r.get(1)?,
                    tags: r.get(2)?,
                    mailbox_name: r.get(3)?,
                })
            },
        )
        .ok();
    Ok(row)
}

fn current_attributes(
    conn: &Connection,
    conversation_id: i64,
) -> std::collections::HashMap<String, (String, String)> {
    let mut map = std::collections::HashMap::new();
    let Ok(mut stmt) = conn.prepare(
        "SELECT attribute, value, value_type FROM ai_attributes
          WHERE conversation_id = ?1 AND superseded_at IS NULL",
    ) else {
        return map;
    };
    let Ok(rows) = stmt.query_map(params![conversation_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    }) else {
        return map;
    };
    for (k, v, t) in rows.flatten() {
        map.insert(k, (v, t));
    }
    map
}

fn latest_verification(conn: &Connection, conversation_id: i64) -> &'static str {
    let row: Option<String> = conn
        .query_row(
            "SELECT verification FROM ai_drafts
              WHERE conversation_id = ?1 AND verification IS NOT NULL
              ORDER BY id DESC LIMIT 1",
            params![conversation_id],
            |r| r.get(0),
        )
        .ok();
    let Some(payload) = row else {
        return "none";
    };
    match serde_json::from_str::<serde_json::Value>(&payload)
        .ok()
        .and_then(|v| v.get("verified").and_then(|x| x.as_bool()))
    {
        Some(true) => "passed",
        Some(false) => "failed",
        None => "none",
    }
}

fn condition_matches(
    ctx: &FireContext,
    analysis: Option<&crate::ai_pipeline::LatestAnalysis>,
    cond: &serde_json::Value,
    attributes: &std::collections::HashMap<String, (String, String)>,
    verification: &str,
) -> bool {
    let field = cond
        .get("field")
        .and_then(|f| f.as_str())
        .unwrap_or_default();
    let operator = cond
        .get("operator")
        .and_then(|o| o.as_str())
        .unwrap_or_default();
    let raw_value = cond
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let value = raw_value.to_lowercase();
    match field {
        "subject" => {
            let subject = ctx.subject.as_deref().unwrap_or_default().to_lowercase();
            match operator {
                "contains" => subject.contains(&value),
                "equals" => subject == value,
                "not_equals" => subject != value,
                _ => false,
            }
        }
        "body" => {
            let preview = ctx.preview.as_deref().unwrap_or_default().to_lowercase();
            match operator {
                "contains" => preview.contains(&value),
                "not_equals" => !preview.contains(&value),
                _ => false,
            }
        }
        "tag" => {
            let tags = ctx.tags.as_deref().unwrap_or_default().to_lowercase();
            match operator {
                "contains" => tags.split(',').any(|t| t.contains(&value)),
                "equals" => tags.split(',').any(|t| t.trim() == value),
                "not_equals" => tags.split(',').all(|t| t.trim() != value),
                _ => false,
            }
        }
        "mailbox" => {
            let mailbox = ctx
                .mailbox_name
                .as_deref()
                .unwrap_or_default()
                .to_lowercase();
            match operator {
                "contains" => mailbox.contains(&value),
                "not_equals" => !mailbox.contains(&value),
                _ => false,
            }
        }
        "confidence" => {
            const LEVELS: [&str; 4] = ["unknown", "low", "medium", "high"];
            let current = analysis
                .and_then(|a| a.analysis.confidence.as_deref())
                .unwrap_or("unknown");
            let idx = LEVELS.iter().position(|l| *l == current);
            let target = LEVELS.iter().position(|l| *l == value);
            match (idx, target) {
                (Some(i), Some(t)) => match operator {
                    "lt" => i < t,
                    "gt" => i > t,
                    _ => i == t,
                },
                _ => false,
            }
        }
        "known_issue_match" => analysis
            .and_then(|a| a.analysis.known_issue_candidate.as_deref())
            .map(|k| k.to_lowercase().contains(&value))
            .unwrap_or(false),
        "ai_attribute" => attribute_matches(cond, raw_value, attributes),
        "ai_verification" => {
            if operator == "not_equals" {
                verification != value
            } else {
                verification == value
            }
        }
        _ => false,
    }
}

fn attribute_matches(
    cond: &serde_json::Value,
    wanted: &str,
    attributes: &std::collections::HashMap<String, (String, String)>,
) -> bool {
    use crate::catalog::AiAttributeKey;
    let key = cond
        .get("attribute")
        .and_then(|a| a.as_str())
        .unwrap_or_default();
    let Some(def) = AiAttributeKey::parse(key) else {
        return false;
    };
    let operator = cond
        .get("operator")
        .and_then(|o| o.as_str())
        .unwrap_or_default();
    let current = attributes.get(key);
    let actual = current.map(|(v, _)| v.as_str()).unwrap_or("unknown");
    if wanted.to_lowercase() == "unknown" {
        return operator == "equals" && actual == "unknown";
    }
    if actual == "unknown" {
        return false;
    }
    match def.value_type() {
        crate::catalog::AttributeValueType::Number => {
            let (Some(a), Some(b)) = (actual.parse::<f64>().ok(), wanted.parse::<f64>().ok())
            else {
                return false;
            };
            match operator {
                "gt" => a > b,
                "gte" => a >= b,
                "lt" => a < b,
                "lte" => a <= b,
                "equals" => a == b,
                "not_equals" => a != b,
                _ => false,
            }
        }
        crate::catalog::AttributeValueType::Boolean => {
            if wanted != "true" && wanted != "false" {
                return false;
            }
            match operator {
                "equals" => actual == wanted,
                "not_equals" => actual != wanted,
                _ => false,
            }
        }
        crate::catalog::AttributeValueType::Enum => {
            let vocab = def.values();
            let ai = vocab.iter().position(|v| v.eq_ignore_ascii_case(actual));
            let bi = vocab.iter().position(|v| v.eq_ignore_ascii_case(wanted));
            match operator {
                "equals" => actual.eq_ignore_ascii_case(wanted),
                "not_equals" => !actual.eq_ignore_ascii_case(wanted),
                "contains" => actual.to_lowercase().contains(&wanted.to_lowercase()),
                "gt" => matches!((ai, bi), (Some(x), Some(y)) if x > y),
                "gte" => matches!((ai, bi), (Some(x), Some(y)) if x >= y),
                "lt" => matches!((ai, bi), (Some(x), Some(y)) if x < y),
                "lte" => matches!((ai, bi), (Some(x), Some(y)) if x <= y),
                _ => false,
            }
        }
        _ => match operator {
            "equals" => actual.eq_ignore_ascii_case(wanted),
            "not_equals" => !actual.eq_ignore_ascii_case(wanted),
            "contains" => actual.to_lowercase().contains(&wanted.to_lowercase()),
            _ => false,
        },
    }
}

pub fn fire_trigger_for_conversation(
    conn: &mut Connection,
    fired: &str,
    conversation_id: i64,
) -> Result<Vec<RunRecord>> {
    if !crate::settings::get_bool(conn, "automation_enabled", false)? {
        return Ok(Vec::new());
    }
    let Some(ctx) = load_fire_context(conn, conversation_id)? else {
        return Ok(Vec::new());
    };
    ensure_main_rule_columns(conn)?;
    let analysis = crate::ai_pipeline::get_latest_analysis(conn, conversation_id)
        .ok()
        .flatten();
    let attributes = current_attributes(conn, conversation_id);
    let verification = latest_verification(conn, conversation_id);
    let write_enabled = crate::settings::get_bool(conn, "automation_write_actions_enabled", false)?;
    let ai_enabled = !matches!(
        crate::ai_hub::backend_for_role(conn, crate::ai_hub::AiRole::Classify),
        crate::ai_pipeline::AiBackend::Disabled
    );
    let mut runs = Vec::new();
    for rule in list_rule_records(conn)? {
        if !rule.enabled || rule.trigger != fired {
            continue;
        }
        let matched = rule
            .conditions
            .as_array()
            .map(|conds| {
                conds.iter().all(|c| {
                    condition_matches(&ctx, analysis.as_ref(), c, &attributes, verification)
                })
            })
            .unwrap_or(true);
        if !matched {
            record_run(
                conn,
                rule.id,
                Some(conversation_id),
                "skipped",
                "Conditions not matched",
            )?;
            continue;
        }
        if let Some(actions) = rule.actions.as_array() {
            for action in actions {
                let kind = action
                    .get("kind")
                    .and_then(|k| k.as_str())
                    .unwrap_or_default();
                let params = action
                    .get("params")
                    .cloned()
                    .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
                let tier = risk_tier(kind).unwrap_or("read");
                match tier {
                    "read" => {
                        if kind == "analyze_ticket" {
                            if ai_enabled {
                                let _ = crate::jobs::enqueue_on(
                                    conn,
                                    "ai",
                                    "analyze_ticket",
                                    &format!("{{\"conversationId\":{conversation_id}}}"),
                                    3,
                                    2,
                                );
                                record_run(
                                    conn,
                                    rule.id,
                                    Some(conversation_id),
                                    "completed",
                                    &format!("Executed read action {kind}"),
                                )?;
                            } else {
                                record_run(
                                    conn,
                                    rule.id,
                                    Some(conversation_id),
                                    "skipped",
                                    "AI action analyze_ticket skipped: AI is disabled",
                                )?;
                            }
                        } else {
                            record_run(
                                conn,
                                rule.id,
                                Some(conversation_id),
                                "completed",
                                &format!("Executed read action {kind}"),
                            )?;
                        }
                    }
                    "non_destructive" => {
                        if rule.requires_approval && !write_enabled {
                            enqueue_awaiting_approval(conn, rule.id, conversation_id, action)?;
                            record_run(
                                conn,
                                rule.id,
                                Some(conversation_id),
                                "awaiting_approval",
                                &format!("Action {kind} requires approval (non-destructive)"),
                            )?;
                        } else {
                            let executed = execute_non_destructive(
                                conn,
                                kind,
                                &params,
                                conversation_id,
                                ai_enabled,
                            )?;
                            if !executed {
                                record_run(
                                    conn,
                                    rule.id,
                                    Some(conversation_id),
                                    "skipped",
                                    &format!("AI action {kind} skipped: AI is disabled"),
                                )?;
                            }
                            if executed {
                                record_run(
                                    conn,
                                    rule.id,
                                    Some(conversation_id),
                                    "completed",
                                    &format!("Executed {kind}"),
                                )?;
                            }
                        }
                    }
                    _ => {
                        enqueue_awaiting_approval(conn, rule.id, conversation_id, action)?;
                        record_run(
                            conn,
                            rule.id,
                            Some(conversation_id),
                            "awaiting_approval",
                            &format!(
                                "Action {kind} is a write action and requires explicit approval"
                            ),
                        )?;
                    }
                }
            }
        }
        conn.execute(
            "UPDATE automation_rules
             SET run_count = COALESCE(run_count, 0) + 1,
                 last_run_at = datetime('now')
             WHERE id = ?1",
            params![rule.id],
        )?;
        let last: Option<RunRecord> = conn
            .query_row(
                &format!(
                    "{RUN_SELECT} WHERE id = (SELECT MAX(id) FROM automation_runs WHERE rule_id = ?1)"
                ),
                params![rule.id],
                RunRecord::read,
            )
            .ok();
        if let Some(r) = last {
            runs.push(r);
        }
    }
    Ok(runs)
}

#[cfg(test)]
mod batch19_tests {
    use super::*;
    use crate::db::DbHandle;

    fn db() -> DbHandle {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        DbHandle::new(conn)
    }

    fn seed_conversation(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (9101, 91, 'Export broken', 'preview', 'active')",
            [],
        )
        .expect("conversation");
        conn.last_insert_rowid()
    }

    fn insert_rule(conn: &Connection, name: &str, trigger_json: &str) -> i64 {
        conn.execute(
            "INSERT INTO automation_rules (name, trigger_json, action_json, enabled, conditions)
             VALUES (?1, ?2, '[]', 1, '[]')",
            params![name, trigger_json],
        )
        .expect("rule");
        conn.last_insert_rowid()
    }

    #[test]
    fn ui_created_legacy_rules_parse_instead_of_poisoning_the_engine() {
        let db = db();
        db.with_write(|c| {
            insert_rule(c, "modern", "\"customer_reply\"");
            insert_rule(c, "ui legacy", r#"{"kind":"tag_added","tag":"vip"}"#);
        });

        let records = db
            .with_read(crate::automation::list_rule_records)
            .expect("the engine must survive every stored rule shape");
        assert_eq!(records.len(), 2, "both real generations parse: {records:?}");
        let legacy = records
            .iter()
            .find(|r| r.name == "ui legacy")
            .expect("the UI-created rule is visible to the engine");
        assert_eq!(
            legacy.trigger, "tag_added",
            "the legacy M007 enum maps to its kind name"
        );
    }

    #[test]
    fn a_single_poisoned_rule_no_longer_kills_the_whole_engine() {
        let db = db();
        let conversation = db.with_write(|c| seed_conversation(c));
        db.with_write(|c| {
            crate::settings::set_bool(c, "automation_enabled", true).expect("enable");
            c.execute(
                "INSERT INTO automation_rules (name, trigger_json, action_json, enabled, conditions)
                 VALUES ('healthy', '\"customer_reply\"',
                         '[{\"kind\":\"manual_review_queue\",\"params\":{}}]', 1, '[]')",
                [],
            )
            .expect("healthy rule");
            insert_rule(c, "poison", "this is not valid json {{{");
        });

        let runs =
            db.with_write(|c| fire_trigger_for_conversation(c, "customer_reply", conversation));
        assert!(
            runs.is_ok(),
            "one malformed rule must not abort the engine: {runs:?}"
        );
        let names: Vec<String> = db.with_read(|c| {
            let mut stmt = c
                .prepare(
                    "SELECT name FROM automation_runs r
                          JOIN automation_rules ar ON ar.id = r.rule_id
                          WHERE ar.name = 'healthy'",
                )
                .expect("runs query");
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).expect("map");
            rows.filter_map(|x| x.ok()).collect()
        });
        assert_eq!(names.len(), 1, "the healthy rule was evaluated: {names:?}");
    }

    #[test]
    fn an_approved_set_status_queues_the_platform_push() {
        let db = db();
        let conversation = db.with_write(|c| seed_conversation(c));
        let executed = db
            .with_write(|c| {
                execute_approved_action(
                    c,
                    conversation,
                    &serde_json::json!({
                        "kind": "set_status",
                        "params": { "status": "closed" },
                    }),
                )
            })
            .expect("approved action");
        assert!(executed, "the local half succeeds");

        let (_kind, key, payload): (String, String, String) = db
            .with_read(|c| {
                c.query_row(
                    "SELECT kind, idempotency_key, payload FROM outbound_jobs
                      WHERE kind = 'update_status'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
            })
            .expect("the outbound push is queued");
        assert_eq!(key, format!("automation_status_{conversation}"));
        let payload: serde_json::Value = serde_json::from_str(&payload).expect("payload json");
        assert_eq!(
            payload["conversationId"], conversation,
            "the lane gets the LOCAL id"
        );
        assert_eq!(payload["status"], "closed");
        let status: String = db
            .with_read(|c| {
                c.query_row(
                    "SELECT status FROM conversations WHERE id = ?1",
                    params![conversation],
                    |r| r.get(0),
                )
            })
            .expect("conversation");
        assert_eq!(status, "closed", "the typed local write happened too");
    }

    #[test]
    fn an_approved_assign_queues_the_platform_push() {
        let db = db();
        let conversation = db.with_write(|c| {
            let id = seed_conversation(c);
            c.execute(
                "INSERT INTO users (remote_id, first_name, last_name, email)
                 VALUES (77, 'Al', 'Agent', 'al@example.com')",
                [],
            )
            .expect("user");
            id
        });
        let user_local: i64 = db
            .with_read(|c| {
                c.query_row("SELECT id FROM users WHERE remote_id = 77", [], |r| {
                    r.get(0)
                })
            })
            .expect("user");

        let executed = db
            .with_write(|c| {
                execute_approved_action(
                    c,
                    conversation,
                    &serde_json::json!({
                        "kind": "assign",
                        "params": { "userId": 77 },
                    }),
                )
            })
            .expect("approved action");
        assert!(executed);

        let (_kind, payload): (String, String) = db
            .with_read(|c| {
                c.query_row(
                    "SELECT kind, payload FROM outbound_jobs WHERE kind = 'assign'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
            })
            .expect("the assign push is queued");
        let payload: serde_json::Value = serde_json::from_str(&payload).expect("payload json");
        assert_eq!(payload["conversationId"], conversation);
        assert_eq!(
            payload["userId"], user_local,
            "the push carries the LOCAL assignee id for the lane's remote resolution"
        );
        let assignee: Option<i64> = db
            .with_read(|c| {
                c.query_row(
                    "SELECT assignee_local_id FROM conversations WHERE id = ?1",
                    params![conversation],
                    |r| r.get(0),
                )
            })
            .expect("conversation");
        assert_eq!(assignee, Some(user_local), "the local write agrees");
    }

    #[test]
    fn an_automation_add_tag_queues_the_platform_push() {
        let db = db();
        let conversation = db.with_write(|c| seed_conversation(c));

        let executed = db
            .with_write(|c| {
                execute_non_destructive(
                    c,
                    "add_tag",
                    &serde_json::json!({ "tag": "vip" }),
                    conversation,
                    false,
                )
            })
            .expect("add_tag");
        assert!(executed);

        let tags =
            db.with_read(|c| crate::conversation_ops::read_conversation_tags(c, conversation));
        assert_eq!(tags, vec!["vip".to_string()], "the local write happened");

        let (_kind, payload): (String, String) = db
            .with_read(|c| {
                c.query_row(
                    "SELECT kind, payload FROM outbound_jobs WHERE kind = 'update_tags'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
            })
            .expect("the tag push is queued — otherwise sync reverts the tag");
        let payload: serde_json::Value = serde_json::from_str(&payload).expect("payload json");
        assert_eq!(payload["conversationId"], conversation);
        assert_eq!(
            payload["tags"],
            serde_json::json!(["vip"]),
            "the full tag list reaches the platform push"
        );
    }
}

#[cfg(test)]
mod batch25_item57_tests {
    use super::*;

    #[test]
    fn the_first_customer_thread_ever_seen_fires_new_conversation() {
        assert_eq!(
            customer_thread_trigger_decision(None, Some(41)),
            Some("new_conversation"),
            "no thread has ever fired for this conversation — the first customer thread is THE new conversation"
        );
    }

    #[test]
    fn a_later_customer_thread_fires_customer_reply_only() {
        assert_eq!(
            customer_thread_trigger_decision(Some(41), Some(57)),
            Some("customer_reply"),
            "a thread the guard has not consumed is a reply, never a second new_conversation"
        );
    }

    #[test]
    fn a_consumed_customer_thread_fires_nothing() {
        assert_eq!(
            customer_thread_trigger_decision(Some(57), Some(57)),
            None,
            "a re-sync / tag change / status flip that delivers no new customer thread must stay silent — the pre-fix code re-fired both triggers here"
        );
    }

    #[test]
    fn a_conversation_without_customer_threads_fires_nothing() {
        assert_eq!(
            customer_thread_trigger_decision(None, None),
            None,
            "no customer thread at all — nothing to fire"
        );
        assert_eq!(
            customer_thread_trigger_decision(Some(12), None),
            None,
            "guard set but the customer thread vanished (deleted): stay silent"
        );
    }

    #[test]
    fn m060_adds_the_once_guard_column_idempotently() {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        let has_column = |c: &Connection| {
            let mut stmt = c
                .prepare("PRAGMA table_info(conversations)")
                .expect("pragma");
            stmt.query_map([], |r| r.get::<_, String>(1))
                .expect("map")
                .filter_map(|x| x.ok())
                .any(|col| col == "automation_last_customer_thread_id")
        };
        assert!(
            has_column(&conn),
            "the once-guard column exists after bootstrap"
        );
        apply_m060_automation_once_guard(&conn).expect("second apply is a no-op");
        assert!(has_column(&conn));
    }
}
