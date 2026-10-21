use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;

pub const M007_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS automation_rules (
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
        ON automation_approvals (status, created_at);

    UPDATE app_state SET schema_version = 7 WHERE id = 1;
"#;

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

pub fn load_rule(conn: &Connection, rule_id: i64) -> Result<Option<AutomationRule>> {
    let row: Option<(i64, String, String, String, i64, Option<String>)> = conn
        .query_row(
            "SELECT id, name, trigger_json, action_json, enabled, created_at
             FROM automation_rules WHERE id = ?1",
            params![rule_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .ok();
    match row {
        None => Ok(None),
        Some((id, name, trigger_json, action_json, enabled, created_at)) => {
            let trigger: Trigger = serde_json::from_str(&trigger_json).map_err(|e| {
                crate::error::Error::Config(format!("trigger deserialization failed: {e}"))
            })?;
            let action: Action = serde_json::from_str(&action_json).map_err(|e| {
                crate::error::Error::Config(format!("action deserialization failed: {e}"))
            })?;
            Ok(Some(AutomationRule {
                id: Some(id),
                name,
                trigger,
                action,
                enabled: enabled != 0,
                created_at,
            }))
        }
    }
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

pub fn trigger_matches(trigger: &Trigger, event: &AutomationEvent) -> bool {
    match (trigger, event) {
        (
            Trigger::StatusChanged {
                from_status,
                to_status,
            },
            AutomationEvent::StatusChanged { from, to },
        ) => {
            let from_ok =
                from_status.as_deref().is_none() || from_status.as_deref() == Some(from.as_str());
            let to_ok = to_status.as_deref().is_none() || to_status.as_deref() == Some(to.as_str());
            from_ok && to_ok
        }
        (Trigger::TagAdded { tag }, AutomationEvent::TagAdded { tag: added_tag }) => {
            tag == added_tag
        }
        (Trigger::SlaRisk, AutomationEvent::SlaRisk) => true,
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationEvent {
    StatusChanged { from: String, to: String },
    TagAdded { tag: String },
    SlaRisk,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvaluateResult {
    pub executed_directly: u32,
    pub approval_pending: u32,
    pub no_match: u32,
}

pub fn evaluate_rules(
    conn: &Connection,
    event: &AutomationEvent,
    conversation_id: i64,
) -> Result<EvaluateResult> {
    let mut result = EvaluateResult::default();
    for rule in list_rules(conn)? {
        if !rule.enabled {
            continue;
        }
        if !trigger_matches(&rule.trigger, event) {
            result.no_match += 1;
            continue;
        }
        if rule.action.requires_approval() {
            let action_json = serde_json::to_string(&rule.action).map_err(|e| {
                crate::error::Error::Config(format!("action serialization failed: {e}"))
            })?;
            conn.execute(
                "INSERT INTO automation_approvals (rule_id, conversation_id, proposed_action_json, status)
                 VALUES (?1, ?2, ?3, 'pending')",
                params![rule.id, conversation_id, action_json],
            )?;
            result.approval_pending += 1;
        } else {
            result.executed_directly += 1;
        }
    }
    Ok(result)
}

pub fn count_pending_approvals(conn: &Connection) -> Result<u32> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM automation_approvals WHERE status = 'pending'",
        [],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(count).unwrap_or(0))
}

pub fn list_pending_approvals(conn: &Connection) -> Result<Vec<AutomationApproval>> {
    let mut stmt = conn.prepare(
        "SELECT id, rule_id, conversation_id, proposed_action_json, status, decided_by_user_id, decided_at, created_at
         FROM automation_approvals
         WHERE status = 'pending'
         ORDER BY julianday(created_at) DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(AutomationApproval {
            id: Some(r.get(0)?),
            rule_id: r.get(1)?,
            conversation_id: r.get(2)?,
            proposed_action_json: r.get(3)?,
            status: r.get(4)?,
            decided_by_user_id: r.get(5)?,
            decided_at: r.get(6)?,
            created_at: r.get(7)?,
        })
    })?;
    let mut approvals = Vec::new();
    for row in rows {
        approvals.push(row?);
    }
    Ok(approvals)
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

fn execute_action(
    conn: &mut Connection,
    rule_id: i64,
    action: &Action,
    conversation_remote_id: i64,
    conversation_local_id: i64,
) -> &'static str {
    use crate::ticket_ops::{OperationResult, TicketOperation, execute};
    if action.requires_approval() {
        if let Ok(action_json) = serde_json::to_string(action) {
            let _ = conn.execute(
                "INSERT INTO automation_approvals (rule_id, conversation_id, proposed_action_json, status)
                 VALUES (?1, ?2, ?3, 'pending')",
                params![rule_id, conversation_remote_id, action_json],
            );
        }
        return "awaiting_approval";
    }
    let op = match action {
        Action::Assign { assignee_remote_id } => {
            let assignee_local_id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM users WHERE remote_id = ?1",
                    params![assignee_remote_id],
                    |r| r.get(0),
                )
                .ok();
            TicketOperation::Assign {
                conversation_remote_id,
                assignee_local_id,
                actor_type: "automation".to_string(),
                actor_id: None,
            }
        }
        Action::ChangeStatus { new_status } => TicketOperation::ChangeStatus {
            conversation_remote_id,
            new_status: new_status.clone(),
            actor_type: "automation".to_string(),
            actor_id: None,
        },
        Action::SetPriority { new_priority } => {
            match crate::ticket_states::TicketPriority::parse(new_priority) {
                Some(p) => TicketOperation::SetPriority {
                    conversation_remote_id,
                    new_priority: p,
                    actor_type: "automation".to_string(),
                    actor_id: None,
                },
                None => return "failed",
            }
        }
        Action::SendNote { body } => TicketOperation::AddNote {
            conversation_remote_id,
            body: body.clone(),
            actor_type: "automation".to_string(),
            actor_id: None,
        },
        Action::AddTag { tag } => {
            let mut tags =
                crate::conversation_ops::read_conversation_tags(conn, conversation_local_id);
            if !tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                tags.push(tag.clone());
                crate::conversation_ops::write_conversation_tags(
                    conn,
                    conversation_local_id,
                    &tags,
                );
            }
            return "completed";
        }
    };
    match execute(conn, &op) {
        Ok(OperationResult::Success { .. }) => "completed",
        Ok(OperationResult::Rejected { .. }) => "failed",
        Err(_) => "failed",
    }
}

pub fn fire_trigger(
    conn: &mut Connection,
    fired: &Trigger,
    conversation_id: i64,
) -> Result<Vec<serde_json::Value>> {
    if !crate::settings::get_bool(conn, "automation_enabled", false)? {
        return Ok(Vec::new());
    }
    let conversation: Option<(i64, i64)> = conn
        .query_row(
            "SELECT id, remote_id FROM conversations WHERE id = ?1 OR remote_id = ?1",
            params![conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let Some((conversation_local_id, conversation_remote_id)) = conversation else {
        return Ok(Vec::new());
    };
    crate::intelligence_features::apply_m035(conn)?;
    let mut runs = Vec::new();
    for rule in list_rules(conn)? {
        if !rule.enabled || &rule.trigger != fired {
            continue;
        }
        let rule_id = rule.id.unwrap_or_default();
        let outcome = execute_action(
            conn,
            rule_id,
            &rule.action,
            conversation_remote_id,
            conversation_local_id,
        );
        conn.execute(
            "INSERT INTO automation_runs (rule_id, conversation_id, triggered_by, outcome)
             VALUES (?1, ?2, 'manual', ?3)",
            params![rule_id, conversation_id, outcome],
        )?;
        let last = conn.last_insert_rowid();
        if let Ok(v) = conn.query_row(
            "SELECT id, rule_id, conversation_id, triggered_by, outcome, created_at
             FROM automation_runs WHERE id = ?1",
            params![last],
            |r| {
                Ok(serde_json::json!({
                    "id": r.get::<_, i64>(0)?,
                    "rule_id": r.get::<_, i64>(1)?,
                    "conversation_id": r.get::<_, Option<i64>>(2)?,
                    "triggered_by": r.get::<_, String>(3)?,
                    "outcome": r.get::<_, String>(4)?,
                    "created_at": r.get::<_, String>(5)?,
                }))
            },
        ) {
            runs.push(v);
        }
    }
    Ok(runs)
}

pub const MAIN_TRIGGERS: [&str; 4] = [
    "new_conversation",
    "customer_reply",
    "ai_low_confidence",
    "manual",
];

pub const MAIN_ACTION_KINDS: [&str; 9] = [
    "analyze_ticket",
    "search_similar",
    "check_known_issues",
    "create_ai_note",
    "create_ai_draft",
    "add_tag",
    "set_status",
    "assign",
    "manual_review_queue",
];

pub const MAIN_CONDITION_FIELDS: [&str; 8] = [
    "subject",
    "body",
    "tag",
    "mailbox",
    "confidence",
    "known_issue_match",
    "ai_attribute",
    "ai_verification",
];

pub const MAIN_CONDITION_OPERATORS: [&str; 7] =
    ["contains", "equals", "not_equals", "gt", "gte", "lt", "lte"];

pub const MAIN_AI_ATTRIBUTE_KEYS: [&str; 14] = [
    "intent",
    "product",
    "feature",
    "issue",
    "urgency",
    "frustration_cues",
    "technical_familiarity",
    "customer_goal",
    "question_count",
    "risk",
    "known_issue",
    "issue_cluster",
    "response_style",
    "escalation_signal",
];

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

fn add_column_if_missing(conn: &Connection, table: &str, column: &str, decl: &str) -> Result<()> {
    let exists: bool = conn
        .prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|c| c == column);
    if !exists {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
            [],
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub path: String,
    pub message: String,
}

impl ValidationIssue {
    fn new(path: &str, message: impl Into<String>) -> Self {
        Self {
            path: path.to_string(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleInput {
    pub name: String,
    pub enabled: bool,
    pub trigger: String,
    pub conditions: serde_json::Value,
    pub actions: serde_json::Value,
    pub priority: i64,
    pub requires_approval: bool,
}

fn received(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(_) => "boolean".into(),
        serde_json::Value::Number(_) => "number".into(),
        serde_json::Value::String(_) => "string".into(),
        serde_json::Value::Array(_) => "array".into(),
        serde_json::Value::Object(_) => "object".into(),
    }
}

fn received_literal(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => format!("'{s}'"),
        other => other.to_string(),
    }
}

fn enum_message(expected: &[&str], received_value: &serde_json::Value) -> String {
    let list = expected
        .iter()
        .map(|e| format!("'{e}'"))
        .collect::<Vec<_>>()
        .join(" | ");
    format!(
        "Invalid enum value. Expected {list}, received {}",
        received_literal(received_value)
    )
}

pub fn validate_rule(
    body: &serde_json::Value,
) -> std::result::Result<RuleInput, Vec<ValidationIssue>> {
    let mut issues: Vec<ValidationIssue> = Vec::new();
    let obj: &serde_json::Map<String, serde_json::Value> = match body.as_object() {
        Some(o) => o,
        None => {
            issues.push(ValidationIssue::new(
                "",
                format!("Expected object, received {}", received(body)),
            ));
            return Err(issues);
        }
    };

    let name = match obj.get("name") {
        None => {
            issues.push(ValidationIssue::new("name", "Required"));
            String::new()
        }
        Some(serde_json::Value::String(s)) => {
            if s.is_empty() {
                issues.push(ValidationIssue::new(
                    "name",
                    "String must contain at least 1 character(s)",
                ));
            }
            s.clone()
        }
        Some(other) => {
            issues.push(ValidationIssue::new(
                "name",
                format!("Expected string, received {}", received(other)),
            ));
            String::new()
        }
    };

    let enabled = match obj.get("enabled") {
        None => false,
        Some(serde_json::Value::Bool(b)) => *b,
        Some(other) => {
            issues.push(ValidationIssue::new(
                "enabled",
                format!("Expected boolean, received {}", received(other)),
            ));
            false
        }
    };

    let trigger = match obj.get("trigger") {
        None => {
            issues.push(ValidationIssue::new("trigger", "Required"));
            String::new()
        }
        Some(serde_json::Value::String(s)) if MAIN_TRIGGERS.contains(&s.as_str()) => s.clone(),
        Some(other) => {
            issues.push(ValidationIssue::new(
                "trigger",
                enum_message(&MAIN_TRIGGERS, other),
            ));
            String::new()
        }
    };

    let conditions = match obj.get("conditions") {
        None => serde_json::Value::Array(Vec::new()),
        Some(serde_json::Value::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                let base = format!("conditions.{i}");
                match item {
                    serde_json::Value::Object(_) => {}
                    serde_json::Value::Null => {
                        issues.push(ValidationIssue::new(
                            &base,
                            "Expected object, received null",
                        ));
                        continue;
                    }
                    other => {
                        issues.push(ValidationIssue::new(
                            &base,
                            format!("Expected object, received {}", received(other)),
                        ));
                        continue;
                    }
                }
                let cond_field = match item_field(item, "field") {
                    FieldGet::Missing => {
                        issues.push(ValidationIssue::new(&format!("{base}.field"), "Required"));
                        String::new()
                    }
                    FieldGet::Value(serde_json::Value::String(s))
                        if MAIN_CONDITION_FIELDS.contains(&s.as_str()) =>
                    {
                        s.clone()
                    }
                    FieldGet::Value(other) => {
                        issues.push(ValidationIssue::new(
                            &format!("{base}.field"),
                            enum_message(&MAIN_CONDITION_FIELDS, other),
                        ));
                        String::new()
                    }
                };
                let operator = match item_field(item, "operator") {
                    FieldGet::Missing => {
                        issues.push(ValidationIssue::new(
                            &format!("{base}.operator"),
                            "Required",
                        ));
                        String::new()
                    }
                    FieldGet::Value(serde_json::Value::String(s))
                        if MAIN_CONDITION_OPERATORS.contains(&s.as_str()) =>
                    {
                        s.clone()
                    }
                    FieldGet::Value(other) => {
                        issues.push(ValidationIssue::new(
                            &format!("{base}.operator"),
                            enum_message(&MAIN_CONDITION_OPERATORS, other),
                        ));
                        String::new()
                    }
                };
                let value = match item_field(item, "value") {
                    FieldGet::Missing => {
                        issues.push(ValidationIssue::new(&format!("{base}.value"), "Required"));
                        String::new()
                    }
                    FieldGet::Value(serde_json::Value::String(s)) => {
                        if s.chars().count() > 200 {
                            issues.push(ValidationIssue::new(
                                &format!("{base}.value"),
                                "String must contain at most 200 character(s)",
                            ));
                        }
                        s.clone()
                    }
                    FieldGet::Value(other) => {
                        issues.push(ValidationIssue::new(
                            &format!("{base}.value"),
                            format!("Expected string, received {}", received(other)),
                        ));
                        String::new()
                    }
                };
                let attribute = match item_field(item, "attribute") {
                    FieldGet::Missing => None,
                    FieldGet::Value(serde_json::Value::Null) => None,
                    FieldGet::Value(serde_json::Value::String(s))
                        if MAIN_AI_ATTRIBUTE_KEYS.contains(&s.as_str()) =>
                    {
                        Some(s.clone())
                    }
                    FieldGet::Value(other) => {
                        issues.push(ValidationIssue::new(
                            &format!("{base}.attribute"),
                            enum_message(&MAIN_AI_ATTRIBUTE_KEYS, other),
                        ));
                        None
                    }
                };
                if cond_field == "ai_attribute" && attribute.is_none() {
                    issues.push(ValidationIssue::new(
                        &format!("{base}.attribute"),
                        "field 'ai_attribute' requires the catalog attribute key (e.g. urgency).",
                    ));
                }
                if cond_field == "ai_verification"
                    && !["failed", "passed", "none"].contains(&value.as_str())
                {
                    issues.push(ValidationIssue::new(
                        &format!("{base}.value"),
                        "field 'ai_verification' value must be 'failed', 'passed' or 'none'.",
                    ));
                }
                let mut c = serde_json::Map::new();
                c.insert("field".into(), serde_json::Value::String(cond_field));
                c.insert("operator".into(), serde_json::Value::String(operator));
                c.insert("value".into(), serde_json::Value::String(value));
                if let Some(a) = attribute {
                    c.insert("attribute".into(), serde_json::Value::String(a));
                }
                out.push(serde_json::Value::Object(c));
            }
            serde_json::Value::Array(out)
        }
        Some(other) => {
            issues.push(ValidationIssue::new(
                "conditions",
                format!("Expected array, received {}", received(other)),
            ));
            serde_json::Value::Array(Vec::new())
        }
    };

    let actions = match obj.get("actions") {
        None => {
            issues.push(ValidationIssue::new("actions", "Required"));
            serde_json::Value::Array(Vec::new())
        }
        Some(serde_json::Value::Array(items)) => {
            if items.is_empty() {
                issues.push(ValidationIssue::new(
                    "actions",
                    "Array must contain at least 1 element(s)",
                ));
            }
            let mut out = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                let base = format!("actions.{i}");
                match item {
                    serde_json::Value::Null => {
                        issues.push(ValidationIssue::new(
                            &base,
                            "Expected object, received null",
                        ));
                        continue;
                    }
                    serde_json::Value::Object(_) => {}
                    other => {
                        issues.push(ValidationIssue::new(
                            &base,
                            format!("Expected object, received {}", received(other)),
                        ));
                        continue;
                    }
                }
                let kind = match item_field(item, "kind") {
                    FieldGet::Missing => {
                        issues.push(ValidationIssue::new(&format!("{base}.kind"), "Required"));
                        String::new()
                    }
                    FieldGet::Value(serde_json::Value::String(s))
                        if MAIN_ACTION_KINDS.contains(&s.as_str()) =>
                    {
                        s.clone()
                    }
                    FieldGet::Value(other) => {
                        issues.push(ValidationIssue::new(
                            &format!("{base}.kind"),
                            enum_message(&MAIN_ACTION_KINDS, other),
                        ));
                        String::new()
                    }
                };
                let params = match obj_get(item, "params") {
                    None => serde_json::Map::new(),
                    Some(serde_json::Value::Null) => {
                        issues.push(ValidationIssue::new(
                            &format!("{base}.params"),
                            "Expected object, received null",
                        ));
                        serde_json::Map::new()
                    }
                    Some(serde_json::Value::Object(map)) => {
                        for (k, pv) in map {
                            if !pv.is_string() {
                                issues.push(ValidationIssue::new(
                                    &format!("{base}.params.{k}"),
                                    format!("Expected string, received {}", received(pv)),
                                ));
                            }
                        }
                        map.clone()
                    }
                    Some(other) => {
                        issues.push(ValidationIssue::new(
                            &format!("{base}.params"),
                            format!("Expected object, received {}", received(other)),
                        ));
                        serde_json::Map::new()
                    }
                };
                let mut a = serde_json::Map::new();
                a.insert("kind".into(), serde_json::Value::String(kind));
                a.insert("params".into(), serde_json::Value::Object(params));
                out.push(serde_json::Value::Object(a));
            }
            serde_json::Value::Array(out)
        }
        Some(other) => {
            issues.push(ValidationIssue::new(
                "actions",
                format!("Expected array, received {}", received(other)),
            ));
            serde_json::Value::Array(Vec::new())
        }
    };

    let priority = match obj.get("priority") {
        None => 100,
        Some(v @ serde_json::Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                i
            } else if let Some(f) = n.as_f64() {
                if f.fract() == 0.0 && f.abs() < 9.0e15 {
                    f as i64
                } else {
                    issues.push(ValidationIssue::new(
                        "priority",
                        "Expected int, received float",
                    ));
                    100
                }
            } else {
                issues.push(ValidationIssue::new(
                    "priority",
                    format!("Expected number, received {}", received(v)),
                ));
                100
            }
        }
        Some(other) => {
            issues.push(ValidationIssue::new(
                "priority",
                format!("Expected number, received {}", received(other)),
            ));
            100
        }
    };

    let requires_approval = match obj.get("requires_approval") {
        None => true,
        Some(serde_json::Value::Bool(b)) => *b,
        Some(other) => {
            issues.push(ValidationIssue::new(
                "requires_approval",
                format!("Expected boolean, received {}", received(other)),
            ));
            true
        }
    };

    if issues.is_empty() {
        Ok(RuleInput {
            name,
            enabled,
            trigger,
            conditions,
            actions,
            priority,
            requires_approval,
        })
    } else {
        Err(issues)
    }
}

enum FieldGet<'a> {
    Missing,
    Value(&'a serde_json::Value),
}

fn item_field<'a>(item: &'a serde_json::Value, key: &str) -> FieldGet<'a> {
    match item.as_object().and_then(|o| o.get(key)) {
        None => FieldGet::Missing,
        Some(v) => FieldGet::Value(v),
    }
}

fn obj_get<'a>(item: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    item.as_object().and_then(|o| o.get(key))
}

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
    let trigger: String = serde_json::from_str(&trigger_json)
        .map_err(|e| crate::error::Error::Config(format!("trigger deserialization failed: {e}")))?;
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

pub fn create_rule_record(conn: &Connection, input: &RuleInput) -> Result<i64> {
    let trigger_json = serde_json::to_string(&input.trigger)
        .map_err(|e| crate::error::Error::Config(format!("trigger serialization failed: {e}")))?;
    let action_json = serde_json::to_string(&input.actions)
        .map_err(|e| crate::error::Error::Config(format!("actions serialization failed: {e}")))?;
    let conditions_json = serde_json::to_string(&input.conditions).map_err(|e| {
        crate::error::Error::Config(format!("conditions serialization failed: {e}"))
    })?;
    conn.execute(
        "INSERT INTO automation_rules
             (name, trigger_json, action_json, enabled, conditions, priority, requires_approval)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            input.name,
            trigger_json,
            action_json,
            input.enabled as i64,
            conditions_json,
            input.priority,
            input.requires_approval as i64
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn load_rule_record(conn: &Connection, rule_id: i64) -> Result<Option<RuleRecord>> {
    use rusqlite::OptionalExtension;
    let row: Option<RuleRow> = conn
        .query_row(
            &format!("{RULE_RECORD_SELECT} WHERE id = ?1"),
            params![rule_id],
            read_rule_row,
        )
        .optional()?;
    match row {
        None => Ok(None),
        Some(row) => Ok(Some(parse_rule_record(row)?)),
    }
}

pub fn list_rule_records(conn: &Connection) -> Result<Vec<RuleRecord>> {
    let mut stmt = conn.prepare(&format!("{RULE_RECORD_SELECT} ORDER BY priority, id"))?;
    let rows = stmt
        .query_map([], read_rule_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter().map(parse_rule_record).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

#[derive(Debug, Default)]
pub struct RuleUpdate {
    pub name: Option<String>,
    pub enabled: Option<i64>,
    pub trigger: Option<String>,
    pub conditions: Option<serde_json::Value>,
    pub actions: Option<serde_json::Value>,
    pub priority: Option<SqlValue>,
    pub requires_approval: Option<i64>,
}

pub fn update_rule_record(conn: &Connection, rule_id: i64, u: &RuleUpdate) -> Result<usize> {
    let mut sets: Vec<String> = Vec::new();
    let mut vals: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(v) = &u.name {
        sets.push(format!("name = ?{}", vals.len() + 1));
        vals.push(rusqlite::types::Value::Text(v.clone()));
    }
    if let Some(v) = u.enabled {
        sets.push(format!("enabled = ?{}", vals.len() + 1));
        vals.push(rusqlite::types::Value::Integer(v));
    }
    if let Some(v) = &u.trigger {
        let trigger_json = serde_json::to_string(v).map_err(|e| {
            crate::error::Error::Config(format!("trigger serialization failed: {e}"))
        })?;
        sets.push(format!("trigger_json = ?{}", vals.len() + 1));
        vals.push(rusqlite::types::Value::Text(trigger_json));
    }
    if let Some(v) = &u.conditions {
        let s = serde_json::to_string(v).map_err(|e| {
            crate::error::Error::Config(format!("conditions serialization failed: {e}"))
        })?;
        sets.push(format!("conditions = ?{}", vals.len() + 1));
        vals.push(rusqlite::types::Value::Text(s));
    }
    if let Some(v) = &u.actions {
        let s = serde_json::to_string(v).map_err(|e| {
            crate::error::Error::Config(format!("actions serialization failed: {e}"))
        })?;
        sets.push(format!("action_json = ?{}", vals.len() + 1));
        vals.push(rusqlite::types::Value::Text(s));
    }
    if let Some(v) = &u.priority {
        sets.push(format!("priority = ?{}", vals.len() + 1));
        vals.push(match v {
            SqlValue::Null => rusqlite::types::Value::Null,
            SqlValue::Int(i) => rusqlite::types::Value::Integer(*i),
            SqlValue::Real(f) => rusqlite::types::Value::Real(*f),
            SqlValue::Text(t) => rusqlite::types::Value::Text(t.clone()),
        });
    }
    if let Some(v) = u.requires_approval {
        sets.push(format!("requires_approval = ?{}", vals.len() + 1));
        vals.push(rusqlite::types::Value::Integer(v));
    }
    if sets.is_empty() {
        return Ok(0);
    }
    let sql = format!(
        "UPDATE automation_rules SET {} WHERE id = ?{}",
        sets.join(", "),
        vals.len() + 1
    );
    vals.push(rusqlite::types::Value::Integer(rule_id));
    let n = conn.execute(&sql, rusqlite::params_from_iter(vals))?;
    Ok(n)
}

pub fn delete_rule_record(conn: &Connection, rule_id: i64) -> Result<usize> {
    let n = conn.execute(
        "DELETE FROM automation_rules WHERE id = ?1",
        params![rule_id],
    )?;
    Ok(n)
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

pub fn list_run_records(conn: &Connection, limit: i64) -> Result<Vec<RunRecord>> {
    let mut stmt = conn.prepare(&format!("{RUN_SELECT} ORDER BY id DESC LIMIT ?1"))?;
    let rows = stmt
        .query_map(params![limit], RunRecord::read)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[must_use]
pub fn js_truthy(v: &serde_json::Value) -> i64 {
    match v {
        serde_json::Value::Null => 0,
        serde_json::Value::Bool(b) => i64::from(*b),
        serde_json::Value::Number(n) => i64::from(n.as_f64().unwrap_or(0.0) != 0.0),
        serde_json::Value::String(s) => i64::from(!s.is_empty()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => 1,
    }
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
    // A1: automation's AI passes run on the Classify role.
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
                let assignee_local_id: Option<i64> = params
                    .get("userId")
                    .and_then(|u| u.as_i64())
                    .and_then(|id| {
                        conn.query_row(
                            "SELECT id FROM users WHERE id = ?1 OR remote_id = ?1",
                            params![id],
                            |r| r.get(0),
                        )
                        .ok()
                    });
                crate::ticket_ops::TicketOperation::Assign {
                    conversation_remote_id,
                    assignee_local_id,
                    actor_type: "automation".to_string(),
                    actor_id: None,
                }
            };
            match crate::ticket_ops::execute(conn, &op) {
                Ok(crate::ticket_ops::OperationResult::Success { .. }) => Ok(true),
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
                // The UI accepts not_equals for every field; treating it
                // as equality fired rules on exactly the excluded value.
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
                // "no tag equals the value": `any` here would fire on the
                // first non-matching tag instead.
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
    // A1: automation's AI passes run on the Classify role.
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
