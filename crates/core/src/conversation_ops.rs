use rusqlite::{Connection, params};
use serde_json::Value;

use crate::crypto::hex;
use crate::error::Result;

pub(crate) fn apply_m030(conn: &Connection) -> Result<()> {
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

pub(crate) fn conv_exists(conn: &Connection, id: i64) -> bool {
    conn.query_row(
        "SELECT 1 FROM conversations WHERE id = ?1",
        params![id],
        |_| Ok(()),
    )
    .is_ok()
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

pub(crate) struct AttachmentRow {
    pub conversation_id: i64,
    pub filename: Option<String>,
}

pub(crate) fn attachment_by_id(conn: &Connection, id: i64) -> Option<AttachmentRow> {
    conn.query_row(
        "SELECT conversation_id, filename FROM attachments WHERE id = ?1",
        params![id],
        |r| {
            Ok(AttachmentRow {
                conversation_id: r.get(0)?,
                filename: r.get(1)?,
            })
        },
    )
    .ok()
}

pub(crate) fn persist_attachment_bytes(
    conn: &Connection,
    attachments_dir: &std::path::Path,
    attachment_id: i64,
    data: &[u8],
) -> std::result::Result<String, String> {
    let Some(att) = attachment_by_id(conn, attachment_id) else {
        return Err("Attachment not found.".into());
    };
    if !conv_exists(conn, att.conversation_id) {
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

pub(crate) fn set_attachment_state(
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

pub const NOT_CONNECTED: &str = "Help Scout is not connected. Remote actions are disabled - connect Help Scout in Settings first.";

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
