//! Conversation write operations (promoted from the retired axum
//! `http/routes/conversations.rs` at the Phase 4 cutover — this module
//! predates the cutover and always held the shared logic; the axum
//! response wrappers and AppState handlers are gone, the pure SQL
//! operations the sync engine, workers, automation, and the native shell
//! call remain).

use rusqlite::{Connection, params};
use serde_json::Value;

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

// `create_outbound_job` (the unkeyed variant) was removed in v1.4.0 R4:
// it had no callers since the Phase 4 cutover and its silent
// `return 0` on INSERT failure was defect D8's root. Everything
// enqueues through `create_outbound_job_keyed`, which now returns a
// truthful Result.

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
    // 1.4.0 R7 (D13): the token itself lives in the encrypted envelope now,
    // so the existence check goes through the same reader the provider uses
    // (envelope first, pre-m053 plaintext columns as a transitional fallback).
    let connected = crate::helpscout_real::has_stored_tokens(conn).unwrap_or(false);
    if connected { None } else { Some(NOT_CONNECTED) }
}

/// Create an outbound job row (v1.4.0 R4 / D8: the id is now a real
/// `Result` — an INSERT failure used to return job id `0` and every
/// caller reported success with it, so a queued reply could evaporate
/// while the composer said it was sent).
///
/// With an idempotency key the insert is an upsert: an existing row for
/// the same key is re-queued (attempts reset, error cleared) and its id
/// returned — the duplicate-send retry path.
///
/// # Errors
/// Propagates the database write failure verbatim.
pub fn create_outbound_job_keyed(
    conn: &Connection,
    kind: &str,
    payload: &Value,
    conversation_id: Option<i64>,
    idempotency_key: Option<&str>,
) -> crate::error::Result<i64> {
    let payload = serde_json::to_string(payload).unwrap_or_else(|_| "{}".into());
    conn.execute(
        "INSERT INTO outbound_jobs (kind, conversation_id, thread_id, payload,
             status, requires_confirmation, idempotency_key, created_at, updated_at)
         VALUES (?1, ?2, NULL, ?3, 'queued', 0, ?4, datetime('now'), datetime('now'))
         ON CONFLICT(idempotency_key) DO UPDATE SET
            kind = excluded.kind, payload = excluded.payload,
            status = 'queued', error = NULL, remote_result = NULL,
            attempts = 0, updated_at = datetime('now')",
        params![kind, conversation_id, payload, idempotency_key],
    )?;
    Ok(conn
        .query_row(
            "SELECT id FROM outbound_jobs WHERE idempotency_key = ?1",
            params![idempotency_key],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| conn.last_insert_rowid()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
