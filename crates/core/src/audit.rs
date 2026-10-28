use rusqlite::Connection;
use serde_json::{Value, json};

use crate::error::Result;

/// Keep at most this many events; `log_event` prunes older rows so the
/// Diagnostics page stays fast on long-running installs.
const EVENT_LOG_CAP: i64 = 500;

/// Migration m050: the diagnostics event log grows a `level` column
/// (existing rows default to `error`) plus a level-aware index, and the
/// retired demo-mode settings rows are dropped for good.
///
/// The column guarantee is shared with the runtime [`ensure_tables`] on
/// purpose: a database whose migration history skipped or lost this step
/// (a renumbered boot step on an upgraded install, a restored backup)
/// must still end up with the column both [`log_event`] and
/// [`list_recent_events`] require — without it, every write dies with
/// "no column named level" (swallowed by the call sites' `let _ =`) and
/// every read returns an honest-looking, permanently empty log.
pub fn apply_m050(conn: &Connection) -> Result<()> {
    ensure_tables(conn)?;
    let retired = conn.execute(
        "DELETE FROM application_settings WHERE key IN ('demo_mode', 'demo_content_purged')",
        [],
    )?;
    if retired > 0 {
        tracing::info!(
            rows = retired,
            "m050: removed retired demo-mode settings rows"
        );
    }
    Ok(())
}

pub fn ensure_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL DEFAULT (datetime('now')),
            actor TEXT NOT NULL,
            action TEXT NOT NULL,
            conversation_id INTEGER,
            before_state TEXT,
            after_state TEXT,
            remote_operation TEXT,
            remote_result TEXT,
            ai_involvement INTEGER DEFAULT 0,
            job_id INTEGER,
            correlation_id TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_audit_conversation ON audit_log(conversation_id);
        CREATE INDEX IF NOT EXISTS idx_audit_time ON audit_log(timestamp DESC);

        CREATE TABLE IF NOT EXISTS application_errors (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL DEFAULT (datetime('now')),
            service TEXT,
            message TEXT,
            stack TEXT,
            context TEXT,
            level TEXT NOT NULL DEFAULT 'error'
        );",
    )?;
    ensure_event_level_column(conn)
}

/// The one canonical guarantee behind the diagnostics log: the
/// `application_errors` table carries the `level` column (`log_event`
/// writes it, `list_recent_events` filters on it) plus its index. Runs
/// on every log/read/clear call, so a database that reached the current
/// version with the pre-m050 table shape — a skipped or renumbered boot
/// step, a restored backup — self-heals here instead of failing every
/// future write silently.
fn ensure_event_level_column(conn: &Connection) -> Result<()> {
    let has_level: bool = {
        let mut stmt = conn.prepare("PRAGMA table_info(application_errors)")?;
        let names: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        names.iter().any(|n| n == "level")
    };
    if !has_level {
        tracing::info!(
            "application_errors is missing the level column (pre-m050 shape) — adding it"
        );
        conn.execute_batch(
            "ALTER TABLE application_errors ADD COLUMN level TEXT NOT NULL DEFAULT 'error';",
        )?;
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_application_errors_level_id
            ON application_errors(level, id DESC);",
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub actor: &'static str,
    pub action: String,
    pub conversation_id: Option<i64>,
    pub before_state: Option<Value>,
    pub after_state: Option<Value>,
    pub remote_operation: Option<String>,
    pub remote_result: Option<Value>,
    pub ai_involvement: bool,
    pub job_id: Option<i64>,
    pub correlation_id: Option<String>,
}

impl AuditEntry {
    pub fn user(action: &str) -> Self {
        Self {
            actor: "user",
            action: action.to_string(),
            conversation_id: None,
            before_state: None,
            after_state: None,
            remote_operation: None,
            remote_result: None,
            ai_involvement: false,
            job_id: None,
            correlation_id: None,
        }
    }

    pub fn with_after_state(mut self, state: Value) -> Self {
        self.after_state = Some(state);
        self
    }

    pub fn with_before_state(mut self, state: Value) -> Self {
        self.before_state = Some(state);
        self
    }

    pub fn with_conversation_id(mut self, conversation_id: i64) -> Self {
        self.conversation_id = Some(conversation_id);
        self
    }
}

pub fn audit(conn: &Connection, entry: &AuditEntry) -> Result<()> {
    ensure_tables(conn)?;
    let before = entry
        .before_state
        .as_ref()
        .map(|v| v.to_string())
        .map(|s| s.chars().take(4000).collect::<String>());
    let after = entry
        .after_state
        .as_ref()
        .map(|v| v.to_string())
        .map(|s| s.chars().take(4000).collect::<String>());
    let remote_result = entry.remote_result.as_ref().map(|v| v.to_string());
    conn.execute(
        "INSERT INTO audit_log (timestamp, actor, action, conversation_id, before_state, after_state, remote_operation, remote_result, ai_involvement, job_id, correlation_id)
         VALUES (datetime('now'), ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            entry.actor,
            entry.action,
            entry.conversation_id,
            before,
            after,
            entry.remote_operation,
            remote_result,
            i64::from(entry.ai_involvement),
            entry.job_id,
            entry.correlation_id,
        ],
    )?;
    Ok(())
}

pub fn list_audit(
    conn: &Connection,
    conversation_id: Option<i64>,
    limit: i64,
) -> Result<Vec<Value>> {
    ensure_tables(conn)?;
    let mut out = Vec::new();
    if let Some(cid) = conversation_id {
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, actor, action, conversation_id, before_state, after_state, remote_operation, remote_result, ai_involvement, job_id, correlation_id
             FROM audit_log WHERE conversation_id = ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![cid, limit], audit_row_to_json)?;
        out.extend(rows.flatten());
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, actor, action, conversation_id, before_state, after_state, remote_operation, remote_result, ai_involvement, job_id, correlation_id
             FROM audit_log ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit], audit_row_to_json)?;
        out.extend(rows.flatten());
    }
    Ok(out)
}

fn audit_row_to_json(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let before: Option<String> = r.get(5)?;
    let after: Option<String> = r.get(6)?;
    let remote_result: Option<String> = r.get(8)?;
    // Clean-audit item 68: a stored payload that is not valid JSON used to
    // decode to `null` — the reader silently dropped the recorded state.
    // Non-JSON payloads now surface verbatim as JSON strings.
    let parse_json = |s: Option<&str>| -> Option<Value> {
        s.map(|v| serde_json::from_str::<Value>(v).unwrap_or_else(|_| Value::String(v.to_string())))
    };
    Ok(json!({
        "id": r.get::<_, i64>(0)?,
        "timestamp": r.get::<_, String>(1)?,
        "actor": r.get::<_, String>(2)?,
        "action": r.get::<_, String>(3)?,
        "conversation_id": r.get::<_, Option<i64>>(4)?,
        "before_state": parse_json(before.as_deref()),
        "after_state": parse_json(after.as_deref()),
        "remote_operation": r.get::<_, Option<String>>(7)?,
        "remote_result": parse_json(remote_result.as_deref()),
        "ai_involvement": r.get::<_, i64>(9)? != 0,
        "job_id": r.get::<_, Option<i64>>(10)?,
        "correlation_id": r.get::<_, Option<String>>(11)?,
    }))
}

pub fn log_error(
    conn: &Connection,
    service: &str,
    message: &str,
    stack: Option<&str>,
    context: Option<&Value>,
) -> Result<()> {
    log_event(conn, "error", service, message, stack, context)
}

/// Append one event to the diagnostics log (the Diagnostics page reads
/// it). `level` is one of `"info"`, `"warn"`, `"error"`; anything else
/// is stored as `"error"` so a typo can never hide a failure.
pub fn log_event(
    conn: &Connection,
    level: &str,
    service: &str,
    message: &str,
    stack: Option<&str>,
    context: Option<&Value>,
) -> Result<()> {
    ensure_tables(conn)?;
    let level = match level {
        "info" | "warn" | "error" => level,
        _ => "error",
    };
    let msg: String = message.chars().take(2000).collect();
    let stk: Option<String> = stack.map(|s| s.chars().take(8000).collect());
    let written = conn.execute(
        "INSERT INTO application_errors (timestamp, service, message, stack, context, level)
         VALUES (datetime('now'), ?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![service, msg, stk, context.map(|c| c.to_string()), level],
    );
    if let Err(e) = written {
        // The call sites swallow this error on purpose (diagnostics must
        // never take the app down) — but a losing log has to be audible
        // somewhere, or "the event log reports nothing" stays a mystery.
        tracing::warn!(
            error = %e,
            service,
            "diagnostics event could not be recorded — the event log is losing entries"
        );
        return Err(e.into());
    }
    // Cap the log: newest EVENT_LOG_CAP events survive.
    let _ = conn.execute(
        "DELETE FROM application_errors WHERE id <= \
         (SELECT COALESCE(MAX(id), 0) FROM application_errors) - ?1",
        [EVENT_LOG_CAP],
    );
    Ok(())
}

/// One diagnostics event as the Diagnostics page shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct DiagEvent {
    pub id: i64,
    pub timestamp: String,
    pub level: String,
    pub service: String,
    pub message: String,
    pub stack: Option<String>,
    pub context: Option<Value>,
}

/// Newest-first slice of the diagnostics log. `level = None` returns all
/// levels; `Some("error")` returns only errors, `Some("warn")` warnings,
/// and so on.
pub fn list_recent_events(
    conn: &Connection,
    limit: i64,
    level: Option<&str>,
) -> Result<Vec<DiagEvent>> {
    ensure_tables(conn)?;
    let mut out = Vec::new();
    let has_level = {
        let mut stmt = conn.prepare("PRAGMA table_info(application_errors)")?;
        stmt.query_map([], |r| r.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .any(|n| n == "level")
    };
    if !has_level {
        // ensure_tables just ran and self-heals the column, so this is
        // unreachable in practice — if it ever fires, say why the log
        // looks empty instead of returning silence.
        tracing::warn!(
            "application_errors still has no level column after ensure_tables — the diagnostics log will read empty"
        );
        return Ok(out);
    }
    let mut stmt = if level.is_some() {
        conn.prepare(
            "SELECT id, timestamp, level, service, message, stack, context \
             FROM application_errors WHERE level = ?1 ORDER BY id DESC LIMIT ?2",
        )?
    } else {
        conn.prepare(
            "SELECT id, timestamp, level, service, message, stack, context \
             FROM application_errors ORDER BY id DESC LIMIT ?1",
        )?
    };
    let map_row = |r: &rusqlite::Row<'_>| -> rusqlite::Result<DiagEvent> {
        let context: Option<String> = r.get(6)?;
        Ok(DiagEvent {
            id: r.get(0)?,
            timestamp: r.get(1)?,
            level: r.get(2)?,
            service: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            message: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            stack: r.get(5)?,
            context: context
                .as_deref()
                .and_then(|c| serde_json::from_str::<Value>(c).ok()),
        })
    };
    if let Some(lvl) = level {
        let rows = stmt.query_map(rusqlite::params![lvl, limit], map_row)?;
        out.extend(rows.flatten());
    } else {
        let rows = stmt.query_map(rusqlite::params![limit], map_row)?;
        out.extend(rows.flatten());
    }
    Ok(out)
}

/// Delete every diagnostics event. Used by the Diagnostics page's clear
/// button. Returns the number of rows removed.
pub fn clear_events(conn: &Connection) -> Result<usize> {
    ensure_tables(conn)?;
    let n = conn.execute("DELETE FROM application_errors", [])?;
    Ok(n)
}

pub fn list_recent_errors(conn: &Connection, limit: i64) -> Result<Vec<Value>> {
    ensure_tables(conn)?;
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, service, message FROM application_errors ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(rusqlite::params![limit], |r| {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "timestamp": r.get::<_, String>(1)?,
            "service": r.get::<_, Option<String>>(2)?,
            "message": r.get::<_, Option<String>>(3)?,
        }))
    })?;
    Ok(rows.flatten().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn migrated() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    #[test]
    fn m050_adds_the_level_column_and_drops_demo_rows() {
        let conn = migrated();
        let has_level: bool = {
            let mut stmt = conn
                .prepare("PRAGMA table_info(application_errors)")
                .expect("pragma");
            stmt.query_map([], |r| r.get::<_, String>(1))
                .expect("columns")
                .filter_map(|r| r.ok())
                .any(|n| n == "level")
        };
        assert!(has_level, "m050 must add the level column");
        // The retired demo rows are gone and cannot resurrect.
        for key in ["demo_mode", "demo_content_purged"] {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM application_settings WHERE key = ?1",
                    [key],
                    |r| r.get(0),
                )
                .expect("count");
            assert_eq!(n, 0, "the {key} settings row must be deleted by m050");
        }
    }

    #[test]
    fn log_event_levels_filter_and_cap() {
        let conn = migrated();
        log_event(&conn, "info", "sync", "first sync finished", None, None).expect("info");
        log_event(&conn, "warn", "webhook", "rejected: no secret", None, None).expect("warn");
        log_event(
            &conn,
            "error",
            "helpscout",
            "login failed",
            Some("stack-frame"),
            Some(&serde_json::json!({ "detail": "401" })),
        )
        .expect("error");
        // Unknown level is stored as error (a typo can never hide a failure).
        log_event(&conn, "bogus", "sync", "stored as error", None, None).expect("bogus level");

        let all = list_recent_events(&conn, 10, None).expect("list all");
        assert_eq!(all.len(), 4);
        assert_eq!(all[0].level, "error", "newest first: bogus -> error row");
        assert_eq!(all[0].service, "sync");
        assert_eq!(
            all[1].context.as_ref().and_then(|c| c["detail"].as_str()),
            Some("401"),
            "context JSON round-trips"
        );
        assert_eq!(all[3].message, "first sync finished");

        let only_errors = list_recent_events(&conn, 10, Some("error")).expect("errors only");
        assert_eq!(only_errors.len(), 2);
        assert!(only_errors.iter().all(|e| e.level == "error"));

        let only_info = list_recent_events(&conn, 10, Some("info")).expect("info only");
        assert_eq!(only_info.len(), 1);
        assert_eq!(only_info[0].message, "first sync finished");

        // log_error keeps its shape and defaults to level = error.
        log_error(&conn, "sync", "legacy writer", None, None).expect("log_error");
        let errors = list_recent_events(&conn, 10, Some("error")).expect("errors");
        assert_eq!(errors.len(), 3);
        assert_eq!(errors[0].message, "legacy writer");

        // Clear empties the log.
        let removed = clear_events(&conn).expect("clear");
        assert_eq!(removed, 5);
        assert!(
            list_recent_events(&conn, 10, None)
                .expect("list")
                .is_empty()
        );
    }

    #[test]
    fn log_event_prunes_old_rows_beyond_the_cap() {
        let conn = migrated();
        for i in 0..600 {
            log_event(&conn, "info", "sync", &format!("event {i}"), None, None).expect("write");
        }
        let remaining = list_recent_events(&conn, 1000, None).expect("list");
        assert_eq!(remaining.len(), 500, "the cap keeps the newest 500 events");
        assert_eq!(remaining[0].message, "event 599");
        assert_eq!(remaining[499].message, "event 100");
    }

    /// Clean-audit item 68: the audit reader never dropped a stored
    /// payload again — non-JSON `before_state`/`after_state`/
    /// `remote_result` values surface verbatim as strings instead of
    /// decoding to `null`.
    #[test]
    fn list_audit_surfaces_non_json_payloads_verbatim() {
        let conn = migrated();
        // A writer that stored plain text (not JSON) in the state columns:
        // the audit table has no CHECK constraint on these payloads.
        conn.execute(
            "INSERT INTO audit_log (timestamp, actor, action, conversation_id,
                                     before_state, after_state, remote_operation,
                                     remote_result, ai_involvement, job_id, correlation_id)
             VALUES (datetime('now'), 'user', 'update_status', 7,
                     'status was open', 'status: closed', 'conversations.update',
                     'HTTP 200 (not json)', 0, NULL, NULL)",
            [],
        )
        .expect("seed non-json audit row");
        // A regular JSON row for contrast.
        audit(
            &conn,
            &AuditEntry::user("tag_added")
                .with_conversation_id(8)
                .with_after_state(json!({"tag": "vip"})),
        )
        .expect("json audit row");

        let rows = list_audit(&conn, None, 10).expect("list");
        assert_eq!(rows.len(), 2, "newest first: the JSON row leads");
        let json_row = &rows[0];
        assert_eq!(json_row["after_state"], json!({"tag": "vip"}));
        let plain = &rows[1];
        assert_eq!(
            plain["before_state"],
            json!("status was open"),
            "plain-text before_state surfaces verbatim, not null"
        );
        assert_eq!(
            plain["after_state"],
            json!("status: closed"),
            "plain-text after_state surfaces verbatim, not null"
        );
        assert_eq!(
            plain["remote_result"],
            json!("HTTP 200 (not json)"),
            "plain-text remote_result surfaces verbatim, not null"
        );
        // The conversation filter still narrows.
        let one = list_audit(&conn, Some(7), 10).expect("list one");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0]["conversation_id"], json!(7));
    }

    /// The live-account failure behind "the event log reports nothing":
    /// a database can reach the current version with the pre-m050 table
    /// shape (a skipped or renumbered boot step, a restored backup).
    /// ensure_tables must self-heal the column on the next call — before
    /// the fix, every write died with "no column named level" (swallowed
    /// by the call sites' `let _ =`) and every read returned empty, in
    /// perfect silence, forever.
    #[test]
    fn a_pre_m050_event_log_self_heals_and_stores_events() {
        let conn = Connection::open_in_memory().expect("in-memory connection");
        conn.execute_batch(
            "CREATE TABLE application_errors (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                service TEXT,
                message TEXT,
                stack TEXT,
                context TEXT
            );",
        )
        .expect("old-shape table");

        // Before the fix this INSERT failed ("no column named level") and
        // the error was swallowed — the test's expect() would have fired.
        log_event(
            &conn,
            "error",
            "webhook",
            "Help Scout rejected the registration",
            None,
            None,
        )
        .expect("log_event must self-heal the old shape and write");
        log_event(&conn, "info", "sync", "sync finished", None, None).expect("second write");

        let events = list_recent_events(&conn, 10, None).expect("list");
        assert_eq!(events.len(), 2, "both events readable after the heal");
        assert_eq!(events[0].message, "sync finished");
        assert_eq!(events[1].level, "error");

        // The healed shape survives a clear/re-write cycle too.
        clear_events(&conn).expect("clear");
        log_event(&conn, "warn", "webhook", "still writable", None, None).expect("post-clear");
        assert_eq!(list_recent_events(&conn, 10, None).expect("list").len(), 1);
    }
}
