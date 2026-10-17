use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::error::Result;

pub fn apply_m030(conn: &Connection) -> Result<()> {
    ensure_column(
        conn,
        "conversations",
        "snoozed_until",
        "ALTER TABLE conversations ADD COLUMN snoozed_until TEXT",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS conversation_tags (
            conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            tag_id         INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (conversation_id, tag_id)
        );
        CREATE INDEX IF NOT EXISTS idx_conversation_tags_tag
            ON conversation_tags(tag_id);
        CREATE TABLE IF NOT EXISTS conversation_fields (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            field_id        INTEGER NOT NULL,
            value           TEXT,
            text_value      TEXT,
            UNIQUE (conversation_id, field_id)
        );
        CREATE INDEX IF NOT EXISTS idx_conversation_fields_conv
            ON conversation_fields(conversation_id);
        CREATE TABLE IF NOT EXISTS attachments (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id       INTEGER UNIQUE,
            thread_id       INTEGER,
            conversation_id INTEGER NOT NULL,
            filename        TEXT,
            mime_type       TEXT,
            size            INTEGER,
            local_path      TEXT,
            hash            TEXT,
            downloaded_at   TEXT,
            state           TEXT DEFAULT 'metadata',
            raw_json        TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_attachments_conversation
            ON attachments(conversation_id);",
    )?;
    ensure_column(
        conn,
        "conversation_threads",
        "state",
        "ALTER TABLE conversation_threads ADD COLUMN state TEXT DEFAULT 'published'",
    )?;
    ensure_column(
        conn,
        "conversation_threads",
        "scheduled_for",
        "ALTER TABLE conversation_threads ADD COLUMN scheduled_for TEXT",
    )?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 30 WHERE id = 1", []);
    Ok(())
}

fn ensure_column(conn: &Connection, table: &str, column: &str, ddl: &str) -> Result<()> {
    let present = conn
        .prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|c| c.ok())
        .any(|c| c == column);
    if !present {
        conn.execute_batch(ddl)?;
    }
    Ok(())
}

pub fn js_number(raw: &str) -> Option<f64> {
    let t = raw.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    t.parse::<f64>().ok()
}

pub fn zod_422(path: &str, message: &str) -> Response {
    zod_422_multi(&[(path, message)])
}

pub fn zod_422_multi(issues: &[(&str, &str)]) -> Response {
    let first = issues
        .first()
        .map(|(p, m)| {
            let location = if p.is_empty() {
                String::new()
            } else {
                format!(" ({p})")
            };
            format!("Invalid request{location}: {m}")
        })
        .unwrap_or_else(|| "Invalid request: request body failed validation.".into());
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "statusCode": 422,
            "error": "ValidationError",
            "message": first,
            "issues": issues.iter().take(10).map(|(p, m)| json!({
                "path": p, "message": m
            })).collect::<Vec<_>>()
        })),
    )
        .into_response()
}

pub fn zod_enum_message(variants: &[&str], received: &str) -> String {
    let expected = variants
        .iter()
        .map(|v| format!("'{v}'"))
        .collect::<Vec<_>>()
        .join(" | ");
    format!("Invalid enum value. Expected {expected}, received '{received}'")
}

pub fn zod_int_msg(body: &Value, field: &str) -> std::result::Result<i64, String> {
    match body.get(field) {
        None | Some(Value::Null) => Err("Required".into()),
        Some(v) if v.is_number() => {
            let f = v.as_f64().unwrap();
            if f.fract() != 0.0 || f < i64::MIN as f64 || f > i64::MAX as f64 {
                Err("Expected integer, received float".into())
            } else {
                Ok(f as i64)
            }
        }
        Some(v) => Err(format!("Expected number, received {}", zod_typeof(v))),
    }
}

pub fn zod_string_msg(body: &Value, field: &str) -> std::result::Result<String, String> {
    match body.get(field) {
        None | Some(Value::Null) => Err("Required".into()),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(v) => Err(format!("Expected string, received {}", zod_typeof(v))),
    }
}

pub fn zod_bool_msg(body: &Value, field: &str) -> std::result::Result<bool, String> {
    match body.get(field) {
        None | Some(Value::Null) => Err("Required".into()),
        Some(Value::Bool(b)) => Ok(*b),
        Some(v) => Err(format!("Expected boolean, received {}", zod_typeof(v))),
    }
}

fn zod_typeof(v: &Value) -> &'static str {
    match v {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        Value::Null => "null",
    }
}

pub fn eval_mode_blocked(conn: &Connection) -> Option<&'static str> {
    let on: Option<String> = crate::settings::get_string(conn, "ai_evaluation_mode")
        .ok()
        .flatten();
    matches!(on.as_deref(), Some("1") | Some("true") | Some("on"))
        .then_some(
            "AI evaluation mode is ON: no replies, notes, status changes or other updates are sent to Help Scout.",
        )
}

pub fn create_outbound_job(
    conn: &Connection,
    kind: &str,
    payload: &Value,
    conversation_id: Option<i64>,
    thread_id: Option<i64>,
) -> i64 {
    let payload = serde_json::to_string(payload).unwrap_or_else(|_| "{}".into());
    let res = conn.execute(
        "INSERT INTO outbound_jobs (kind, conversation_id, thread_id, payload,
             status, requires_confirmation, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'queued', 0, datetime('now'), datetime('now'))",
        params![kind, conversation_id, thread_id, payload],
    );
    if res.is_err() {
        return 0;
    }
    conn.last_insert_rowid()
}

pub fn set_outbound_status(conn: &Connection, id: i64, status: &str, error: Option<&str>) {
    if id == 0 {
        return;
    }
    let _ = conn.execute(
        "UPDATE outbound_jobs
            SET status = ?1, error = ?2, attempts = attempts + 1,
                updated_at = datetime('now')
          WHERE id = ?3",
        params![status, error, id],
    );
}

pub struct ConvRef {
    pub id: i64,
    pub remote_id: i64,
    pub mailbox_id: Option<i64>,
}

pub fn conv_by_local_id(conn: &Connection, id: i64) -> Option<ConvRef> {
    conn.query_row(
        "SELECT id, remote_id, mailbox_local_id FROM conversations WHERE id = ?1",
        params![id],
        |r| {
            Ok(ConvRef {
                id: r.get(0)?,
                remote_id: r.get(1)?,
                mailbox_id: r.get(2)?,
            })
        },
    )
    .ok()
}

pub fn rejected(message: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({ "ok": false, "message": message })),
    )
        .into_response()
}

pub fn ok_result(message: &str, data: Option<Value>) -> Response {
    let mut body = json!({ "ok": true, "message": message });
    if let Some(d) = data {
        body["data"] = d;
    }
    (StatusCode::OK, Json(body)).into_response()
}

pub fn op_move_to_inbox(
    conn: &Connection,
    conversation_local_id: i64,
    mailbox_id: i64,
) -> Response {
    if let Some(msg) = eval_mode_blocked(conn) {
        return rejected(msg);
    }
    let Some(conv) = conv_by_local_id(conn, conversation_local_id) else {
        return rejected("Conversation not found locally.");
    };
    let job_id = create_outbound_job(
        conn,
        "move",
        &json!({ "conversationId": conv.id, "mailboxRemoteId": mailbox_id }),
        Some(conv.id),
        None,
    );
    let mailbox_name: Option<String> = conn
        .query_row(
            "SELECT name FROM mailboxes WHERE remote_id = ?1",
            params![mailbox_id],
            |r| r.get(0),
        )
        .ok();
    let before_mailbox = conv.mailbox_id;
    let updated = conn.execute(
        "UPDATE conversations SET mailbox_local_id =
             (SELECT id FROM mailboxes WHERE remote_id = ?1),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
           WHERE id = ?2",
        params![mailbox_id, conv.id],
    );
    match updated {
        Ok(_) if mailbox_name.is_some() || updated.unwrap_or(0) > 0 => {
            set_outbound_status(conn, job_id, "confirmed", None);
            let _ = crate::jobs::audit(
                conn,
                "user",
                "moved_inbox",
                Some(conv.id),
                Some(&json!({ "mailbox": before_mailbox }).to_string()),
                Some(&json!({ "mailbox": mailbox_id }).to_string()),
                Some(&format!(
                    "PATCH /v2/conversations/{}/mailboxId",
                    conv.remote_id
                )),
                Some(&json!({ "job_id": job_id }).to_string()),
                false,
            );
            let name = mailbox_name.unwrap_or_else(|| "the new inbox".into());
            ok_result(&format!("Moved to {name}."), None)
        }
        _ => {
            let msg = "Conversation not found".to_string();
            set_outbound_status(conn, job_id, "failed", Some(&msg));
            rejected(&format!("Conversation was NOT moved. {msg}"))
        }
    }
}

pub fn op_update_tags(conn: &Connection, conversation_local_id: i64, change: &Value) -> Response {
    if let Some(msg) = eval_mode_blocked(conn) {
        return rejected(msg);
    }
    let Some(conv) = conv_by_local_id(conn, conversation_local_id) else {
        return rejected("Conversation not found locally.");
    };
    let job_id = create_outbound_job(
        conn,
        "update_tags",
        &json!({ "conversationId": conv.id, "change": change }),
        Some(conv.id),
        None,
    );
    let current: Vec<String> = read_conversation_tags(conn, conv.id);
    let desired: Vec<String> = if let Some(set) = change.get("set").and_then(|v| v.as_array()) {
        let mut seen = std::collections::HashSet::new();
        set.iter()
            .filter_map(|t| t.as_str())
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty() && seen.insert(t.clone()))
            .collect()
    } else {
        let remove: Vec<String> = change
            .get("remove")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str())
                    .map(|t| t.to_lowercase())
                    .collect()
            })
            .unwrap_or_default();
        let add: Vec<String> = change
            .get("add")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str())
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        current
            .iter()
            .filter(|t| !remove.contains(&t.to_lowercase()))
            .chain(add.iter())
            .filter(|t| seen.insert((*t).clone()))
            .cloned()
            .collect()
    };
    write_conversation_tags(conn, conv.id, &desired);
    set_outbound_status(conn, job_id, "confirmed", None);
    let _ = crate::jobs::audit(
        conn,
        "user",
        "tags_changed",
        Some(conv.id),
        Some(&json!({ "tags": current }).to_string()),
        Some(&json!({ "tags": desired }).to_string()),
        Some(&format!("PUT /v2/conversations/{}/tags", conv.remote_id)),
        Some(&json!({ "job_id": job_id }).to_string()),
        false,
    );
    ok_result("Tags updated.", Some(json!({ "tags": desired })))
}

pub fn read_conversation_tags(conn: &Connection, conversation_id: i64) -> Vec<String> {
    let mut stmt = match conn.prepare(
        "SELECT t.name FROM conversation_tags ct
            JOIN tags t ON t.id = ct.tag_id
           WHERE ct.conversation_id = ?1
           ORDER BY t.name",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    stmt.query_map(params![conversation_id], |r| r.get::<_, String>(0))
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

pub fn merge_tags(current: &[String], change: &Value) -> Vec<String> {
    if let Some(set) = change.get("set").and_then(|v| v.as_array()) {
        let mut seen = std::collections::HashSet::new();
        return set
            .iter()
            .filter_map(|t| t.as_str())
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty() && seen.insert(t.clone()))
            .collect();
    }
    let remove: Vec<String> = change
        .get("remove")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|t| t.as_str())
                .map(|t| t.to_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let add: Vec<String> = change
        .get("add")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|t| t.as_str())
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    current
        .iter()
        .filter(|t| !remove.contains(&t.to_lowercase()))
        .chain(add.iter())
        .filter(|t| seen.insert((*t).clone()))
        .cloned()
        .collect()
}

pub fn write_conversation_tags(conn: &Connection, conversation_id: i64, tags: &[String]) {
    let _ = conn.execute(
        "DELETE FROM conversation_tags WHERE conversation_id = ?1",
        params![conversation_id],
    );
    for name in tags {
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE",
                params![name],
                |r| r.get(0),
            )
            .ok();
        let tag_id = existing.unwrap_or_else(|| {
            let next_local: i64 = conn
                .query_row("SELECT COALESCE(MAX(id), 0) + 1 AS n FROM tags", [], |r| {
                    r.get(0)
                })
                .unwrap_or(1);
            let _ = conn.execute(
                "INSERT INTO tags (remote_id, name, slug, ticket_count)
                 VALUES (?1, ?2, ?3, 0)",
                params![-next_local, name, slugify(name)],
            );
            conn.last_insert_rowid()
        });
        let _ = conn.execute(
            "INSERT OR IGNORE INTO conversation_tags (conversation_id, tag_id)
             VALUES (?1, ?2)",
            params![conversation_id, tag_id],
        );
    }
}

fn slugify(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut out = String::new();
    let mut dash = false;
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}

pub fn op_update_custom_fields(
    conn: &Connection,
    conversation_local_id: i64,
    fields: &[(i64, Option<String>)],
) -> Response {
    if let Some(msg) = eval_mode_blocked(conn) {
        return rejected(msg);
    }
    let Some(conv) = conv_by_local_id(conn, conversation_local_id) else {
        return rejected("Conversation not found locally.");
    };
    let job_id = create_outbound_job(
        conn,
        "update_fields",
        &json!({ "conversationId": conv.id, "fields": fields }),
        Some(conv.id),
        None,
    );
    let current = read_conversation_fields(conn, conv.id);
    let changed_ids: std::collections::HashSet<i64> = fields.iter().map(|f| f.0).collect();
    let mut merged: Vec<Value> = Vec::new();
    for f in &current {
        let is_system = f
            .get("system_type")
            .and_then(|s| s.as_str())
            .map(|s| !s.is_empty() && s != "null")
            .unwrap_or(false);
        let changed = changed_ids.contains(&f["field_id"].as_i64().unwrap_or(-1));
        if is_system || !changed {
            merged.push(json!({ "id": f["field_id"], "value": f["value"] }));
        }
    }
    for (id, value) in fields {
        merged.push(json!({ "id": id, "value": value.clone().unwrap_or_default() }));
    }
    for (id, value) in fields {
        let text = resolve_field_text(conn, *id, value);
        let _ = conn.execute(
            "INSERT INTO conversation_fields (conversation_id, field_id, value, text_value)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(conversation_id, field_id)
             DO UPDATE SET value = excluded.value, text_value = excluded.text_value",
            params![conv.id, id, value.clone().unwrap_or_default(), text],
        );
    }
    set_outbound_status(conn, job_id, "confirmed", None);
    let _ = crate::jobs::audit(
        conn,
        "user",
        "fields_changed",
        Some(conv.id),
        Some(&json!({ "fields": current }).to_string()),
        Some(&json!({ "fields": merged }).to_string()),
        Some(&format!("PUT /v2/conversations/{}/fields", conv.remote_id)),
        Some(&json!({ "job_id": job_id }).to_string()),
        false,
    );
    ok_result("Custom fields updated.", None)
}

fn read_conversation_fields(conn: &Connection, conversation_id: i64) -> Vec<Value> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT cf.field_id, cf.value, cf.text_value,
                (SELECT system_type FROM inbox_fields WHERE remote_id = cf.field_id)
           FROM conversation_fields cf
          WHERE cf.conversation_id = ?1
          ORDER BY cf.field_id",
    ) else {
        return Vec::new();
    };

    stmt.query_map(params![conversation_id], |r| {
        Ok(json!({
            "field_id": r.get::<_, i64>(0)?,
            "value": r.get::<_, Option<String>>(1)?,
            "text_value": r.get::<_, Option<String>>(2)?,
            "system_type": r.get::<_, Option<String>>(3)?,
        }))
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

fn resolve_field_text(conn: &Connection, field_id: i64, value: &Option<String>) -> Option<String> {
    let raw = value.clone()?;
    let label: Option<String> = conn
        .query_row(
            "SELECT o.label FROM inbox_field_options o
              JOIN inbox_fields f ON f.id = o.field_id
              WHERE f.remote_id = ?1 AND CAST(o.id AS TEXT) = ?2",
            params![field_id, raw],
            |r| r.get(0),
        )
        .ok();
    label.or(Some(raw))
}

pub fn op_snooze(conn: &Connection, conversation_local_id: i64, snoozed_until: &str) -> Response {
    if let Some(msg) = eval_mode_blocked(conn) {
        return rejected(msg);
    }
    let Some(conv) = conv_by_local_id(conn, conversation_local_id) else {
        return rejected("Conversation not found locally.");
    };
    let job_id = create_outbound_job(
        conn,
        "snooze",
        &json!({ "conversationId": conv.id, "snoozedUntil": snoozed_until }),
        Some(conv.id),
        None,
    );
    let _ = conn.execute(
        "UPDATE conversations SET snoozed_until = ?1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
           WHERE id = ?2",
        params![snoozed_until, conv.id],
    );
    set_outbound_status(conn, job_id, "confirmed", None);
    let _ = crate::jobs::audit(
        conn,
        "user",
        "snoozed",
        Some(conv.id),
        None,
        Some(&json!({ "snoozedUntil": snoozed_until }).to_string()),
        Some(&format!("PUT /v2/conversations/{}/snooze", conv.remote_id)),
        Some(&json!({ "job_id": job_id }).to_string()),
        false,
    );
    ok_result(&format!("Snoozed until {snoozed_until}."), None)
}

pub fn op_unsnooze(conn: &Connection, conversation_local_id: i64) -> Response {
    if let Some(msg) = eval_mode_blocked(conn) {
        return rejected(msg);
    }
    let Some(conv) = conv_by_local_id(conn, conversation_local_id) else {
        return rejected("Conversation not found locally.");
    };
    let _ = conn.execute(
        "UPDATE conversations SET snoozed_until = NULL,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
           WHERE id = ?1",
        params![conv.id],
    );
    let _ = crate::jobs::audit(
        conn,
        "user",
        "unsnoozed",
        Some(conv.id),
        None,
        None,
        Some(&format!(
            "DELETE /v2/conversations/{}/snooze",
            conv.remote_id
        )),
        None,
        false,
    );
    ok_result("Snooze removed.", None)
}

struct ThreadRef {
    #[allow(dead_code)]
    id: i64,
    conversation_id: i64,
}

fn thread_by_local_id(conn: &Connection, thread_id: i64) -> Option<ThreadRef> {
    conn.query_row(
        "SELECT id, conversation_id FROM conversation_threads WHERE id = ?1",
        params![thread_id],
        |r| {
            Ok(ThreadRef {
                id: r.get(0)?,
                conversation_id: r.get(1)?,
            })
        },
    )
    .ok()
}

pub fn op_schedule_reply(
    conn: &Connection,
    conversation_local_id: i64,
    thread_id: i64,
    scheduled_for: &str,
) -> Response {
    if let Some(msg) = eval_mode_blocked(conn) {
        return rejected(msg);
    }
    let Some(conv) = conv_by_local_id(conn, conversation_local_id) else {
        return rejected("Conversation or draft thread not found locally.");
    };
    let Some(thread) = thread_by_local_id(conn, thread_id) else {
        return rejected("Conversation or draft thread not found locally.");
    };
    if thread.conversation_id != conv.id {
        return rejected("That thread does not belong to this conversation.");
    }
    let job_id = create_outbound_job(
        conn,
        "schedule",
        &json!({ "conversationId": conv.id, "threadId": thread_id, "scheduledFor": scheduled_for }),
        Some(conv.id),
        Some(thread_id),
    );
    let _ = conn.execute(
        "UPDATE conversation_threads SET scheduled_for = ?1, state = 'scheduled'
           WHERE id = ?2",
        params![scheduled_for, thread_id],
    );
    set_outbound_status(conn, job_id, "confirmed", None);
    let _ = crate::jobs::audit(
        conn,
        "user",
        "reply_scheduled",
        Some(conv.id),
        None,
        Some(&json!({ "scheduledFor": scheduled_for }).to_string()),
        Some(&format!(
            "PUT /v2/conversations/{}/threads/{thread_id}/schedule",
            conv.remote_id
        )),
        Some(&json!({ "job_id": job_id }).to_string()),
        false,
    );
    ok_result(&format!("Reply scheduled for {scheduled_for}."), None)
}

pub fn op_publish_schedule(
    conn: &Connection,
    conversation_local_id: i64,
    thread_id: i64,
) -> Response {
    if let Some(msg) = eval_mode_blocked(conn) {
        return rejected(msg);
    }
    let Some(conv) = conv_by_local_id(conn, conversation_local_id) else {
        return rejected("Conversation or thread not found locally.");
    };
    let Some(thread) = thread_by_local_id(conn, thread_id) else {
        return rejected("Conversation or thread not found locally.");
    };
    if thread.conversation_id != conv.id {
        return rejected("That thread does not belong to this conversation.");
    }
    let job_id = create_outbound_job(
        conn,
        "schedule_publish",
        &json!({ "conversationId": conv.id, "threadId": thread_id }),
        Some(conv.id),
        Some(thread_id),
    );
    let _ = conn.execute(
        "UPDATE conversation_threads SET state = 'published', scheduled_for = NULL
           WHERE id = ?1",
        params![thread_id],
    );
    set_outbound_status(conn, job_id, "confirmed", None);
    let _ = crate::jobs::audit(
        conn,
        "user",
        "scheduled_reply_published",
        Some(conv.id),
        None,
        None,
        Some("PATCH .../schedule (publish)"),
        Some(&json!({ "job_id": job_id }).to_string()),
        false,
    );
    ok_result("Scheduled reply published (sent now).", None)
}

pub fn op_delete_schedule(
    conn: &Connection,
    conversation_local_id: i64,
    thread_id: i64,
) -> Response {
    if let Some(msg) = eval_mode_blocked(conn) {
        return rejected(msg);
    }
    let Some(conv) = conv_by_local_id(conn, conversation_local_id) else {
        return rejected("Conversation or thread not found locally.");
    };
    let Some(thread) = thread_by_local_id(conn, thread_id) else {
        return rejected("Conversation or thread not found locally.");
    };
    if thread.conversation_id != conv.id {
        return rejected("That thread does not belong to this conversation.");
    }
    let _ = conn.execute(
        "UPDATE conversation_threads SET state = 'draft', scheduled_for = NULL
           WHERE id = ?1",
        params![thread_id],
    );
    let _ = crate::jobs::audit(
        conn,
        "user",
        "schedule_deleted",
        Some(conv.id),
        None,
        None,
        Some("DELETE .../schedule"),
        None,
        false,
    );
    ok_result("Schedule deleted; draft kept.", None)
}

pub fn op_bulk_action(
    conn: &Connection,
    conversation_ids: &[i64],
    action: &str,
    params_body: &Value,
) -> Response {
    if let Some(msg) = eval_mode_blocked(conn) {
        return rejected(msg);
    }
    let mut queued = 0usize;
    for id in conversation_ids {
        let mut payload = json!({ "conversationId": id });
        if let Some(obj) = params_body.as_object() {
            for (k, v) in obj {
                payload[k] = v.clone();
            }
        }
        let payload_str = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".into());
        let _ = crate::jobs::enqueue_on(conn, "api", &format!("bulk_{action}"), &payload_str, 1, 3);
        queued += 1;
    }
    let _ = crate::jobs::audit(
        conn,
        "user",
        &format!("bulk_{action}"),
        None,
        Some(&json!({ "count": conversation_ids.len() }).to_string()),
        Some(&json!({ "queued": queued }).to_string()),
        None,
        None,
        false,
    );
    ok_result(
        &format!("{queued} operations queued."),
        Some(json!({ "queued": queued })),
    )
}

pub fn op_run_workflow(
    conn: &Connection,
    conversation_local_id: i64,
    workflow_id: i64,
) -> Response {
    if let Some(msg) = eval_mode_blocked(conn) {
        return rejected(msg);
    }
    let Some(conv) = conv_by_local_id(conn, conversation_local_id) else {
        return rejected("Conversation not found locally.");
    };
    let wf_name: Option<String> = conn
        .query_row(
            "SELECT name FROM workflows WHERE remote_id = ?1",
            params![workflow_id],
            |r| r.get(0),
        )
        .ok();
    if let Some(name) = wf_name.as_deref()
        && name.contains("Tier 1")
    {
        let _ = conn.execute(
            "UPDATE conversations SET assignee_local_id =
                     (SELECT id FROM users WHERE remote_id = 1001)
                   WHERE id = ?1",
            params![conv.id],
        );
    }
    let _ = crate::jobs::audit(
        conn,
        "user",
        "helpscout_workflow_run",
        Some(conv.id),
        None,
        None,
        Some(&format!("POST /v2/workflows/{workflow_id}/run")),
        None,
        false,
    );
    ok_result("Help Scout workflow executed.", None)
}

pub struct AttachmentRow {
    pub id: i64,
    pub remote_id: Option<i64>,
    pub thread_id: Option<i64>,
    pub conversation_id: i64,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub local_path: Option<String>,
    pub state: Option<String>,
}

pub fn attachment_by_id(conn: &Connection, id: i64) -> Option<AttachmentRow> {
    conn.query_row(
        "SELECT id, remote_id, thread_id, conversation_id, filename, mime_type,
                local_path, state
           FROM attachments WHERE id = ?1",
        params![id],
        |r| {
            Ok(AttachmentRow {
                id: r.get(0)?,
                remote_id: r.get(1)?,
                thread_id: r.get(2)?,
                conversation_id: r.get(3)?,
                filename: r.get(4)?,
                mime_type: r.get(5)?,
                local_path: r.get(6)?,
                state: r.get(7)?,
            })
        },
    )
    .ok()
}

pub async fn op_download_attachment(
    state: &crate::http::server::AppState,
    attachments_dir: &std::path::Path,
    attachment_id: i64,
) -> Response {
    let att = {
        let conn = state.conn_lock();
        attachment_by_id(&conn, attachment_id)
    };
    let Some(att) = att else {
        return rejected("Attachment not found.");
    };
    let (conv_ok, conv_remote, thread_remote): (bool, i64, Option<i64>) = {
        let conn = state.conn_lock();
        let conv = conv_by_local_id(&conn, att.conversation_id);
        let t_remote: Option<i64> = att
            .thread_id
            .and_then(|tid| {
                conn.query_row(
                    "SELECT remote_id FROM conversation_threads WHERE id = ?1",
                    params![tid],
                    |r| r.get(0),
                )
                .ok()
            })
            .flatten();
        (
            conv.is_some(),
            conv.map(|c| c.remote_id).unwrap_or_default(),
            t_remote,
        )
    };
    if !conv_ok {
        return rejected("Parent conversation not found.");
    }
    let Some(provider) = op_provider(state) else {
        return internal_error("no provider available");
    };
    match provider
        .get_attachment_data(
            conv_remote,
            thread_remote.unwrap_or(0),
            att.remote_id.unwrap_or(0),
        )
        .await
    {
        Ok(Some(bytes)) => {
            let conn = state.conn_lock();
            match persist_attachment_bytes(&conn, attachments_dir, attachment_id, &bytes.data) {
                Ok(path) => ok_result("Attachment downloaded.", Some(json!({ "path": path }))),
                Err(msg) => rejected(&msg),
            }
        }
        Ok(None) => {
            let conn = state.conn_lock();
            set_attachment_state(&conn, attachment_id, "failed", None, None);
            rejected(
                "Attachment is no longer available in Help Scout (it may have expired or been removed).",
            )
        }
        Err(e) => {
            let conn = state.conn_lock();
            set_attachment_state(&conn, attachment_id, "failed", None, None);
            rejected(&format!(
                "Attachment download failed. {}",
                failure_message(&e)
            ))
        }
    }
}

pub fn persist_attachment_bytes(
    conn: &Connection,
    attachments_dir: &std::path::Path,
    attachment_id: i64,
    data: &[u8],
) -> std::result::Result<String, String> {
    let Some(att) = attachment_by_id(conn, attachment_id) else {
        return Err("Attachment not found.".into());
    };
    if conv_by_local_id(conn, att.conversation_id).is_none() {
        return Err("Parent conversation not found.".into());
    }
    let safe_name = att
        .filename
        .as_deref()
        .unwrap_or("attachment")
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let _ = std::fs::create_dir_all(attachments_dir);
    let target = attachments_dir.join(format!(
        "{}-{}-{}",
        att.conversation_id, attachment_id, safe_name
    ));
    if let Err(e) = std::fs::write(&target, data) {
        set_attachment_state(conn, attachment_id, "failed", None, None);
        return Err(format!("Attachment download failed. {e}"));
    }
    use sha2::{Digest, Sha256};
    let hash = hex(&Sha256::digest(data));
    let path_str = target.to_string_lossy().to_string();
    set_attachment_state(
        conn,
        attachment_id,
        "downloaded",
        Some(&path_str),
        Some(&hash),
    );
    Ok(path_str)
}

pub async fn download_attachment_via_provider(
    state: &crate::http::server::AppState,
    attachments_dir: &std::path::Path,
    attachment_id: i64,
) -> std::result::Result<String, String> {
    let att = {
        let conn = state.conn_lock();
        attachment_by_id(&conn, attachment_id)
    };
    let Some(att) = att else {
        return Err("Attachment not found.".into());
    };
    let (conv_remote, thread_remote): (i64, Option<i64>) = {
        let conn = state.conn_lock();
        let conv = conv_by_local_id(&conn, att.conversation_id)
            .map(|c| c.remote_id)
            .unwrap_or_default();
        let t_remote: Option<i64> = att
            .thread_id
            .and_then(|tid| {
                conn.query_row(
                    "SELECT remote_id FROM conversation_threads WHERE id = ?1",
                    params![tid],
                    |r| r.get(0),
                )
                .ok()
            })
            .flatten();
        (conv, t_remote)
    };
    let Some(provider) = op_provider(state) else {
        return Err("no provider available".into());
    };
    match provider
        .get_attachment_data(
            conv_remote,
            thread_remote.unwrap_or(0),
            att.remote_id.unwrap_or(0),
        )
        .await
    {
        Ok(Some(bytes)) => {
            let conn = state.conn_lock();
            persist_attachment_bytes(&conn, attachments_dir, attachment_id, &bytes.data)
        }
        Ok(None) => {
            let conn = state.conn_lock();
            set_attachment_state(&conn, attachment_id, "failed", None, None);
            Err("Attachment is no longer available in Help Scout.".into())
        }
        Err(e) => {
            let conn = state.conn_lock();
            set_attachment_state(&conn, attachment_id, "failed", None, None);
            Err(format!(
                "Attachment download failed. {}",
                failure_message(&e)
            ))
        }
    }
}

pub fn set_attachment_state(
    conn: &Connection,
    id: i64,
    state: &str,
    path: Option<&str>,
    hash: Option<&str>,
) {
    let _ = conn.execute(
        "UPDATE attachments SET state = ?1, local_path = COALESCE(?2, local_path),
             hash = COALESCE(?3, hash), downloaded_at = datetime('now')
           WHERE id = ?4",
        params![state, path, hash, id],
    );
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

use std::time::Instant;

use crate::helpscout::{ConversationPatch, CreateThreadInput, HelpScoutProvider};

pub fn eval_mode_on(conn: &Connection) -> bool {
    let on: Option<String> = crate::settings::get_string(conn, "ai_evaluation_mode")
        .ok()
        .flatten();
    matches!(on.as_deref(), Some("1") | Some("true") | Some("on"))
}

pub struct ConvFull {
    pub id: i64,
    pub remote_id: i64,
    pub status: String,
    pub subject: Option<String>,
    pub closed_at: Option<String>,
    pub assignee_local_id: Option<i64>,
    pub merged_into_conversation_id: Option<i64>,
}

pub fn conv_full_by_local_id(conn: &Connection, id: i64) -> Option<ConvFull> {
    conn.query_row(
        "SELECT id, remote_id, status, subject, closed_at, assignee_local_id,
                merged_into_conversation_id
           FROM conversations WHERE id = ?1",
        params![id],
        |r| {
            Ok(ConvFull {
                id: r.get(0)?,
                remote_id: r.get(1)?,
                status: r.get(2)?,
                subject: r.get(3)?,
                closed_at: r.get(4)?,
                assignee_local_id: r.get(5)?,
                merged_into_conversation_id: r.get(6)?,
            })
        },
    )
    .ok()
}

pub const NOT_CONNECTED: &str = "Help Scout is not connected. Remote actions are disabled - connect Help Scout in Settings first.";

pub fn require_auth(conn: &Connection, provider_kind: &str) -> Option<&'static str> {
    if provider_kind != "real" {
        return None;
    }
    let has_cols: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('oauth_tokens')
              WHERE name IN ('account', 'revoked')",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n == 2)
        .unwrap_or(false);
    let connected = if has_cols {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM oauth_tokens
               WHERE (account = 'default' OR id = 1)
                 AND access_token IS NOT NULL AND access_token != ''
                 AND COALESCE(revoked, 0) = 0)",
            [],
            |r| r.get::<_, bool>(0),
        )
        .unwrap_or(false)
    } else {
        crate::oauth::has_token(conn).unwrap_or(false)
    };
    if connected { None } else { Some(NOT_CONNECTED) }
}

pub fn create_outbound_job_keyed(
    conn: &Connection,
    kind: &str,
    payload: &Value,
    conversation_id: Option<i64>,
    idempotency_key: Option<&str>,
) -> i64 {
    let payload = serde_json::to_string(payload).unwrap_or_else(|_| "{}".into());
    let res = conn.execute(
        "INSERT INTO outbound_jobs (kind, conversation_id, thread_id, payload,
             status, requires_confirmation, idempotency_key, created_at, updated_at)
         VALUES (?1, ?2, NULL, ?3, 'queued', 0, ?4, datetime('now'), datetime('now'))
         ON CONFLICT(idempotency_key) DO UPDATE SET
            kind = excluded.kind, payload = excluded.payload,
            status = 'queued', error = NULL, remote_result = NULL,
            attempts = 0, updated_at = datetime('now')",
        params![kind, conversation_id, payload, idempotency_key],
    );
    if res.is_err() {
        return 0;
    }
    conn.query_row(
        "SELECT id FROM outbound_jobs WHERE idempotency_key = ?1",
        params![idempotency_key],
        |r| r.get(0),
    )
    .unwrap_or_else(|_| conn.last_insert_rowid())
}

fn sha256_24(text: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(text.as_bytes());
    hex(&digest)[..24].to_string()
}

fn not_connected(message: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "ok": false, "message": message })),
    )
        .into_response()
}

pub fn op_provider(
    state: &crate::http::server::AppState,
) -> Option<std::sync::Arc<dyn HelpScoutProvider>> {
    if let Some(sync) = &state.sync {
        return Some(sync.provider().clone());
    }
    state
        .real
        .clone()
        .map(|r| r as std::sync::Arc<dyn HelpScoutProvider>)
}

fn failure_message(e: &crate::error::Error) -> String {
    crate::helpscout_real::hs_error(e)
        .map(|hs| hs.friendly.clone())
        .unwrap_or_else(|| e.to_string())
}

fn failure_detail(e: &crate::error::Error) -> Option<String> {
    crate::helpscout_real::hs_error(e).map(|hs| format!("HTTP {}", hs.status_code))
}

#[derive(Debug, Clone)]
pub struct ReplyInput {
    pub conversation_id: i64,
    pub text: String,
    pub draft: bool,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub status_after: Option<String>,
    pub assign_to: Option<i64>,
    pub ai_draft_id: Option<i64>,
    pub original_ai_text: Option<String>,
}

pub async fn op_send_reply(state: &crate::http::server::AppState, input: ReplyInput) -> Response {
    let (conv, job_id) = {
        let conn = state.conn_lock();
        if let Some(msg) = require_auth(&conn, &state.provider_kind) {
            return not_connected(msg);
        }
        if let Some(msg) = eval_mode_blocked(&conn) {
            return rejected(msg);
        }
        let Some(conv) = conv_full_by_local_id(&conn, input.conversation_id) else {
            return rejected("Conversation not found locally.");
        };
        if conv.merged_into_conversation_id.is_some() {
            return rejected(
                "This conversation was merged into another conversation. Open the target conversation to reply.",
            );
        }
        let key = format!(
            "reply:{}:{}:{}",
            conv.remote_id,
            if input.draft { "draft" } else { "send" },
            sha256_24(&input.text)
        );
        let existing: Option<String> = conn
            .query_row(
                "SELECT status FROM outbound_jobs WHERE idempotency_key = ?1",
                params![&key],
                |r| r.get(0),
            )
            .ok();
        if existing.is_some_and(|status| status != "failed") {
            return rejected(
                "This exact reply was already sent (duplicate-send protection). Check the conversation history before sending again.",
            );
        }
        let job_id = create_outbound_job_keyed(
            &conn,
            "create_reply",
            &json!({
                "conversationId": input.conversation_id,
                "text": input.text,
                "draft": input.draft,
                "cc": input.cc,
                "bcc": input.bcc,
                "statusAfter": input.status_after,
                "assignTo": input.assign_to,
            }),
            Some(conv.id),
            Some(&key),
        );
        set_outbound_status(&conn, job_id, "sending", None);
        (conv, job_id)
    };

    let Some(provider) = op_provider(state) else {
        return internal_error("no provider available");
    };
    let started = Instant::now();
    match provider
        .create_reply_thread(CreateThreadInput {
            conversation_id: conv.remote_id,
            text: input.text.clone(),
            draft: input.draft,
            cc: input.cc.clone(),
            bcc: input.bcc.clone(),
            status_after: input.status_after.clone(),
            assign_to: input.assign_to,
        })
        .await
    {
        Ok(res) => {
            let elapsed = started.elapsed().as_millis() as i64;
            {
                let conn = state.conn_lock();
                let _ = crate::jobs::record_outbound_attempt(
                    &conn,
                    job_id,
                    1,
                    &format!("POST reply (draft={})", input.draft),
                    Some(201),
                    None,
                    Some(elapsed),
                );
                set_outbound_status_confirmed(
                    &conn,
                    job_id,
                    &json!({ "threadId": res.thread_id, "conversationId": res.conversation_id }),
                );
            }
            if let Some(sync) = &state.sync {
                let _ = sync.sync_single_conversation(conv.remote_id).await;
            }
            if let Some(draft_id) = input.ai_draft_id {
                let conn = state.conn_lock();
                let _ = crate::ai_pipeline::ensure_pipeline_schema(&conn);
                let _ = crate::ai_pipeline::set_draft_state(&conn, draft_id, "sent");
                let _ = crate::ai_pipeline::record_feedback(
                    &conn,
                    draft_id,
                    input.original_ai_text.as_deref().unwrap_or(""),
                    &input.text,
                    true,
                );
            }
            {
                let conn = state.conn_lock();
                let _ = crate::jobs::audit_entry(
                    &conn,
                    "user",
                    if input.draft {
                        "reply_draft_created"
                    } else {
                        "reply_sent"
                    },
                    Some(conv.id),
                    Some(&json!({ "status": conv.status }).to_string()),
                    Some(&json!({ "threadRemoteId": res.thread_id }).to_string()),
                    Some(&format!("POST /v2/conversations/{}/reply", conv.remote_id)),
                    Some(
                        &json!({
                            "threadId": res.thread_id,
                            "conversationId": res.conversation_id
                        })
                        .to_string(),
                    ),
                    input.ai_draft_id.is_some(),
                    Some(job_id),
                    None,
                );
            }
            emit_conversation_updated(state, conv.id);
            ok_result(
                if input.draft {
                    "Draft saved to Help Scout."
                } else {
                    "Reply sent successfully."
                },
                Some(json!({ "threadRemoteId": res.thread_id })),
            )
        }
        Err(e) => {
            let msg = failure_message(&e);
            let detail = failure_detail(&e);
            {
                let conn = state.conn_lock();
                let _ = crate::jobs::record_outbound_attempt(
                    &conn,
                    job_id,
                    1,
                    "POST reply",
                    crate::helpscout_real::hs_status(&e).map(i64::from),
                    Some(&e.to_string()),
                    Some(started.elapsed().as_millis() as i64),
                );
                set_outbound_status(&conn, job_id, "failed", Some(&msg));
                let _ = crate::jobs::audit_entry(
                    &conn,
                    "user",
                    "reply_failed",
                    Some(conv.id),
                    Some(&json!({ "status": conv.status }).to_string()),
                    None,
                    Some(&format!("POST /v2/conversations/{}/reply", conv.remote_id)),
                    Some(&json!({ "error": msg }).to_string()),
                    false,
                    Some(job_id),
                    None,
                );
            }
            let caution = if input.draft {
                String::new()
            } else {
                " The reply may or may not have been delivered - verify in Help Scout before sending again; SupportOS will not automatically resend.".to_string()
            };
            let mut body = json!({ "ok": false, "message": format!("{msg}{caution}") });
            if let Some(d) = detail {
                body["detail"] = json!(d);
            }
            (StatusCode::UNPROCESSABLE_ENTITY, Json(body)).into_response()
        }
    }
}

pub async fn op_add_note(
    state: &crate::http::server::AppState,
    conversation_id: i64,
    text: &str,
    ai_generated: bool,
) -> Response {
    let (conv, job_id) = {
        let conn = state.conn_lock();
        if let Some(msg) = require_auth(&conn, &state.provider_kind) {
            return not_connected(msg);
        }
        let Some(conv) = conv_full_by_local_id(&conn, conversation_id) else {
            return rejected("Conversation not found locally.");
        };
        if eval_mode_on(&conn) {
            return rejected(
                "AI evaluation mode is ON: no notes, replies or status changes are sent to Help Scout.",
            );
        }
        let job_id = create_outbound_job_keyed(
            &conn,
            "create_note",
            &json!({ "conversationId": conversation_id, "text": text, "aiGenerated": ai_generated }),
            Some(conv.id),
            None,
        );
        set_outbound_status(&conn, job_id, "sending", None);
        (conv, job_id)
    };

    let Some(provider) = op_provider(state) else {
        return internal_error("no provider available");
    };
    let started = Instant::now();
    let attempt = provider.create_note_thread(CreateThreadInput {
        conversation_id: conv.remote_id,
        text: text.to_string(),
        draft: false,
        cc: vec![],
        bcc: vec![],
        status_after: None,
        assign_to: None,
    });
    match attempt.await {
        Ok(res) => {
            let elapsed = started.elapsed().as_millis() as i64;
            {
                let conn = state.conn_lock();
                let _ = crate::jobs::record_outbound_attempt(
                    &conn,
                    job_id,
                    1,
                    "POST note",
                    Some(201),
                    None,
                    Some(elapsed),
                );
                set_outbound_status_confirmed(
                    &conn,
                    job_id,
                    &json!({ "threadId": res.thread_id, "conversationId": res.conversation_id }),
                );
            }
            if let Some(sync) = &state.sync {
                let _ = sync.sync_single_conversation(conv.remote_id).await;
            }
            {
                let conn = state.conn_lock();
                let _ = crate::jobs::audit_entry(
                    &conn,
                    if ai_generated { "ai" } else { "user" },
                    "note_added",
                    Some(conv.id),
                    None,
                    None,
                    Some(&format!("POST /v2/conversations/{}/notes", conv.remote_id)),
                    Some(
                        &json!({
                            "threadId": res.thread_id,
                            "conversationId": res.conversation_id
                        })
                        .to_string(),
                    ),
                    ai_generated,
                    Some(job_id),
                    None,
                );
            }
            emit_conversation_updated(state, conv.id);
            ok_result(
                "Internal note added.",
                Some(json!({ "threadRemoteId": res.thread_id })),
            )
        }
        Err(e) => {
            let msg = failure_message(&e);
            let detail = failure_detail(&e);
            {
                let conn = state.conn_lock();
                let _ = crate::jobs::record_outbound_attempt(
                    &conn,
                    job_id,
                    1,
                    "POST note",
                    crate::helpscout_real::hs_status(&e).map(i64::from),
                    Some(&e.to_string()),
                    Some(started.elapsed().as_millis() as i64),
                );
                set_outbound_status(&conn, job_id, "failed", Some(&msg));
            }
            let mut body = json!({ "ok": false, "message": msg });
            if let Some(d) = detail {
                body["detail"] = json!(d);
            }
            (StatusCode::UNPROCESSABLE_ENTITY, Json(body)).into_response()
        }
    }
}

pub async fn op_change_status(
    state: &crate::http::server::AppState,
    conversation_id: i64,
    status: &str,
) -> Response {
    let conv = {
        let conn = state.conn_lock();
        if require_auth(&conn, &state.provider_kind).is_some() {
            return rejected(NOT_CONNECTED);
        }
        if eval_mode_on(&conn) {
            return rejected("AI evaluation mode is ON: status changes are disabled.");
        }
        let Some(conv) = conv_full_by_local_id(&conn, conversation_id) else {
            return rejected("Conversation not found locally.");
        };
        if conv.status == status {
            return conflict_409(&format!("Conversation is already '{status}'."));
        }
        conv
    };
    let Some(provider) = op_provider(state) else {
        return internal_error("no provider available");
    };
    let job_id = {
        let conn = state.conn_lock();
        let job_id = create_outbound_job_keyed(
            &conn,
            "update_status",
            &json!({ "conversationId": conversation_id, "status": status }),
            Some(conv.id),
            None,
        );
        set_outbound_status(&conn, job_id, "sending", None);
        job_id
    };
    match provider
        .update_conversation(
            conv.remote_id,
            ConversationPatch {
                status: Some(status.to_string()),
                ..Default::default()
            },
        )
        .await
    {
        Ok(_) => {
            let mut conn = state.conn_lock();
            set_outbound_status(&conn, job_id, "confirmed", None);
            let tx = match conn.transaction() {
                Ok(tx) => tx,
                Err(e) => return internal_error(&e.to_string()),
            };
            if let Err(e) = tx.execute(
                "UPDATE conversations SET status = ?1,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                   WHERE id = ?2",
                params![status, conv.id],
            ) {
                return internal_error(&e.to_string());
            }
            if status == "closed"
                && conv.closed_at.is_none()
                && let Err(e) = tx.execute(
                    "UPDATE conversations SET closed_at = datetime('now') WHERE id = ?1",
                    params![conv.id],
                )
            {
                return internal_error(&e.to_string());
            }
            if let Err(e) = tx.commit() {
                return internal_error(&e.to_string());
            }
            let _ = crate::jobs::audit_entry(
                &conn,
                "user",
                "status_changed",
                Some(conv.id),
                Some(&json!({ "status": conv.status }).to_string()),
                Some(&json!({ "status": status }).to_string()),
                Some(&format!("PATCH /v2/conversations/{}", conv.remote_id)),
                None,
                false,
                Some(job_id),
                None,
            );
            drop(conn);
            emit_conversation_updated(state, conv.id);
            ok_result(&format!("Status changed to {status}."), None)
        }
        Err(e) => {
            let msg = failure_message(&e);
            let conn = state.conn_lock();
            set_outbound_status(&conn, job_id, "failed", Some(&msg));
            drop(conn);
            rejected(&format!("Status was NOT changed. {msg}"))
        }
    }
}

pub async fn op_assign(
    state: &crate::http::server::AppState,
    conversation_id: i64,
    user_id: Option<i64>,
) -> Response {
    let conv = {
        let conn = state.conn_lock();
        if require_auth(&conn, &state.provider_kind).is_some() {
            return rejected(NOT_CONNECTED);
        }
        if let Some(msg) = eval_mode_blocked(&conn) {
            return rejected(msg);
        }
        let Some(conv) = conv_full_by_local_id(&conn, conversation_id) else {
            return rejected("Conversation not found locally.");
        };
        conv
    };
    let Some(provider) = op_provider(state) else {
        return internal_error("no provider available");
    };
    let job_id = {
        let conn = state.conn_lock();
        let job_id = create_outbound_job_keyed(
            &conn,
            "assign",
            &json!({ "conversationId": conversation_id, "userId": user_id }),
            Some(conv.id),
            None,
        );
        set_outbound_status(&conn, job_id, "sending", None);
        job_id
    };
    match provider
        .update_conversation(
            conv.remote_id,
            ConversationPatch {
                assign_to: Some(user_id),
                ..Default::default()
            },
        )
        .await
    {
        Ok(_) => {
            let conn = state.conn_lock();
            set_outbound_status(&conn, job_id, "confirmed", None);
            let local_user: Option<i64> = user_id.and_then(|uid| {
                conn.query_row(
                    "SELECT id FROM users WHERE remote_id = ?1",
                    params![uid],
                    |r| r.get(0),
                )
                .ok()
            });
            let local_team: Option<i64> = match (user_id, local_user) {
                (Some(uid), None) => conn
                    .query_row(
                        "SELECT id FROM teams WHERE remote_id = ?1",
                        params![uid],
                        |r| r.get(0),
                    )
                    .ok(),
                _ => None,
            };
            let resolved = local_user.or(local_team).or(user_id);
            let _ = conn.execute(
                "UPDATE conversations SET assignee_local_id = ?1,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                   WHERE id = ?2",
                params![resolved, conv.id],
            );
            let _ = crate::jobs::audit_entry(
                &conn,
                "user",
                "assignment_changed",
                Some(conv.id),
                Some(&json!({ "assignee": conv.assignee_local_id }).to_string()),
                Some(&json!({ "assignee": user_id }).to_string()),
                Some(&format!(
                    "PATCH /v2/conversations/{} /assignTo",
                    conv.remote_id
                )),
                None,
                false,
                Some(job_id),
                None,
            );
            drop(conn);
            emit_conversation_updated(state, conv.id);
            ok_result(
                if user_id.is_some() {
                    "Conversation assigned."
                } else {
                    "Conversation unassigned."
                },
                None,
            )
        }
        Err(e) => {
            let msg = failure_message(&e);
            let conn = state.conn_lock();
            set_outbound_status(&conn, job_id, "failed", Some(&msg));
            drop(conn);
            rejected(&format!("Assignment was NOT changed. {msg}"))
        }
    }
}

pub async fn op_change_subject(
    state: &crate::http::server::AppState,
    conversation_id: i64,
    subject: &str,
) -> Response {
    let conv = {
        let conn = state.conn_lock();
        if require_auth(&conn, &state.provider_kind).is_some() {
            return rejected(NOT_CONNECTED);
        }
        if let Some(msg) = eval_mode_blocked(&conn) {
            return rejected(msg);
        }
        let Some(conv) = conv_full_by_local_id(&conn, conversation_id) else {
            return rejected("Conversation not found locally.");
        };
        conv
    };
    let Some(provider) = op_provider(state) else {
        return internal_error("no provider available");
    };
    let job_id = {
        let conn = state.conn_lock();
        let job_id = create_outbound_job_keyed(
            &conn,
            "update_subject",
            &json!({ "conversationId": conversation_id, "subject": subject }),
            Some(conv.id),
            None,
        );
        set_outbound_status(&conn, job_id, "sending", None);
        job_id
    };
    match provider
        .update_conversation(
            conv.remote_id,
            ConversationPatch {
                subject: Some(subject.to_string()),
                ..Default::default()
            },
        )
        .await
    {
        Ok(_) => {
            let conn = state.conn_lock();
            set_outbound_status(&conn, job_id, "confirmed", None);
            let _ = conn.execute(
                "UPDATE conversations SET subject = ?1,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                   WHERE id = ?2",
                params![subject, conv.id],
            );
            let _ = crate::jobs::audit_entry(
                &conn,
                "user",
                "subject_changed",
                Some(conv.id),
                Some(&json!({ "subject": conv.subject }).to_string()),
                Some(&json!({ "subject": subject }).to_string()),
                Some(&format!(
                    "PATCH /v2/conversations/{} /subject",
                    conv.remote_id
                )),
                None,
                false,
                Some(job_id),
                None,
            );
            drop(conn);
            emit_conversation_updated(state, conv.id);
            ok_result("Subject updated.", None)
        }
        Err(e) => {
            let msg = failure_message(&e);
            let conn = state.conn_lock();
            set_outbound_status(&conn, job_id, "failed", Some(&msg));
            drop(conn);
            rejected(&format!("Subject was NOT changed. {msg}"))
        }
    }
}

fn set_outbound_status_confirmed(conn: &Connection, id: i64, remote_result: &Value) {
    if id == 0 {
        return;
    }
    let _ = conn.execute(
        "UPDATE outbound_jobs
            SET status = 'confirmed', error = NULL, remote_result = ?1,
                attempts = attempts + 1, updated_at = datetime('now')
          WHERE id = ?2",
        params![remote_result.to_string(), id],
    );
}

fn internal_error(message: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "statusCode": 500,
            "error": "InternalError",
            "message": message
        })),
    )
        .into_response()
}

fn conflict_409(message: &str) -> Response {
    (
        StatusCode::CONFLICT,
        Json(json!({
            "statusCode": 409,
            "error": "Conflict",
            "message": message
        })),
    )
        .into_response()
}

fn emit_conversation_updated(state: &crate::http::server::AppState, conversation_local_id: i64) {
    crate::http::event_bus::notify_conversation_updated(
        &state.bus,
        &crate::events::ConversationUpdatedEvent {
            conversation_id: Some(conversation_local_id),
            conversation_number: None,
            mailbox_id: None,
            subject: None,
            reason: "sync".into(),
            at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
        },
    );
}
