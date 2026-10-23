use rusqlite::Connection;
use serde_json::{Value, json};

use crate::error::Result;

pub fn ensure_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS inbox_fields (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE NOT NULL,
            mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            type TEXT,
            system_type TEXT,
            required INTEGER DEFAULT 0,
            sort_order INTEGER DEFAULT 0,
            raw_json TEXT,
            last_synced_at TEXT,
            deleted_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_inbox_fields_mailbox ON inbox_fields(mailbox_id);
        CREATE TABLE IF NOT EXISTS inbox_field_options (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE NOT NULL,
            field_id INTEGER NOT NULL REFERENCES inbox_fields(id) ON DELETE CASCADE,
            label TEXT,
            sort_order INTEGER DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS saved_replies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE,
            mailbox_local_id INTEGER REFERENCES mailboxes(id),
            name TEXT NOT NULL,
            preview TEXT,
            text TEXT,
            chat_text TEXT,
            raw_json TEXT,
            remote_updated_at TEXT,
            last_synced_at TEXT,
            deleted_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_saved_replies_name ON saved_replies(name);
        CREATE TABLE IF NOT EXISTS workflows (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE,
            mailbox_local_id INTEGER REFERENCES mailboxes(id),
            name TEXT NOT NULL,
            type TEXT,
            status TEXT,
            sort_order INTEGER,
            raw_json TEXT,
            remote_updated_at TEXT,
            last_synced_at TEXT,
            deleted_at TEXT
        );
        CREATE TABLE IF NOT EXISTS user_statuses (
            user_local_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
            email_status TEXT,
            email_updated_at TEXT,
            chat_status TEXT,
            mailbox_statuses TEXT,
            raw_json TEXT,
            last_synced_at TEXT
        );
        CREATE TABLE IF NOT EXISTS webhook_configs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE,
            url TEXT,
            events TEXT,
            status TEXT,
            raw_json TEXT,
            last_synced_at TEXT
        );",
    )?;
    Ok(())
}

pub fn inbox_fields(conn: &Connection) -> Result<Vec<Value>> {
    ensure_tables(conn)?;
    let mut stmt = conn.prepare(
        "SELECT f.id FROM inbox_fields f WHERE f.deleted_at IS NULL ORDER BY f.mailbox_id, f.sort_order",
    )?;
    let ids: Vec<i64> = stmt
        .query_map([], |r| r.get::<_, i64>(0))?
        .flatten()
        .collect();
    drop(stmt);
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let field = conn.query_row(
            "SELECT f.id, f.remote_id, f.mailbox_id, f.name, f.type, f.system_type, f.required, f.sort_order AS \"order\"
             FROM inbox_fields f WHERE f.id = ?1",
            [id],
            field_row,
        )?;
        let mut opt_stmt = conn.prepare(
            "SELECT id, remote_id, label, sort_order FROM inbox_field_options WHERE field_id = ?1 ORDER BY sort_order",
        )?;
        let options: Vec<Value> = opt_stmt
            .query_map([id], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "remote_id": r.get::<_, Option<i64>>(1)?,
                    "label": r.get::<_, Option<String>>(2)?,
                    "sort_order": r.get::<_, i64>(3)?,
                }))
            })?
            .flatten()
            .collect();
        let mut with_options = field;
        with_options["options"] = Value::Array(options);
        out.push(with_options);
    }
    Ok(out)
}

fn field_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, i64>(0)?,
        "remote_id": r.get::<_, Option<i64>>(1)?,
        "mailbox_id": r.get::<_, i64>(2)?,
        "name": r.get::<_, String>(3)?,
        "type": r.get::<_, Option<String>>(4)?,
        "system_type": r.get::<_, Option<String>>(5)?,
        "required": r.get::<_, i64>(6)?,
        "order": r.get::<_, i64>(7)?,
    }))
}

pub fn workflows(conn: &Connection) -> Result<Vec<Value>> {
    ensure_tables(conn)?;
    let mut stmt = conn.prepare(
        "SELECT id, remote_id, mailbox_local_id AS mailbox_id, name, type, status, sort_order AS \"order\"
         FROM workflows WHERE deleted_at IS NULL ORDER BY sort_order",
    )?;
    let rows: Vec<Value> = stmt
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "remote_id": r.get::<_, Option<i64>>(1)?,
                "mailbox_id": r.get::<_, Option<i64>>(2)?,
                "name": r.get::<_, String>(3)?,
                "type": r.get::<_, Option<String>>(4)?,
                "status": r.get::<_, Option<String>>(5)?,
                "order": r.get::<_, Option<i64>>(6)?,
            }))
        })?
        .flatten()
        .collect();
    Ok(rows)
}

pub fn user_statuses(conn: &Connection) -> Result<Vec<Value>> {
    ensure_tables(conn)?;
    let mut stmt = conn.prepare(
        "SELECT user_local_id AS user_id, email_status, chat_status, mailbox_statuses FROM user_statuses
         JOIN users ON users.id = user_statuses.user_local_id WHERE users.deleted_at IS NULL",
    )?;
    let rows: Vec<Value> = stmt
        .query_map([], |r| {
            let mailbox_statuses: Option<String> = r.get(3)?;
            let parsed: Value = mailbox_statuses
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_else(|| json!({}));
            Ok(json!({
                "user_id": r.get::<_, i64>(0)?,
                "email_status": r.get::<_, Option<String>>(1)?,
                "chat_status": r.get::<_, Option<String>>(2)?,
                "mailbox_statuses": parsed,
            }))
        })?
        .flatten()
        .collect();
    Ok(rows)
}

pub fn webhook_configs(conn: &Connection) -> Result<Vec<Value>> {
    ensure_tables(conn)?;
    let mut stmt =
        conn.prepare("SELECT id, remote_id, url, events, status FROM webhook_configs")?;
    let rows: Vec<Value> = stmt
        .query_map([], |r| {
            let events: Option<String> = r.get(3)?;
            let parsed: Value = events
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_else(|| json!([]));
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "remote_id": r.get::<_, Option<i64>>(1)?,
                "url": r.get::<_, Option<String>>(2)?,
                "events": parsed,
                "status": r.get::<_, Option<String>>(4)?,
            }))
        })?
        .flatten()
        .collect();
    Ok(rows)
}

pub fn saved_replies(conn: &Connection, q: Option<&str>) -> Result<Vec<Value>> {
    ensure_tables(conn)?;
    let base_sql = "SELECT id, remote_id, mailbox_local_id, name, preview, text, chat_text FROM saved_replies WHERE deleted_at IS NULL";
    if let Some(q) = q.filter(|s| !s.is_empty()) {
        // Clean-audit item 83: the search text is a LITERAL — the
        // pattern body is escaped and the ESCAPE clause declared (the
        // pre-fix query had neither, so `%`/`_` in the input acted as
        // wildcards over the mirror).
        let like = crate::search::escape_like(q);
        let mut stmt = conn.prepare(&format!(
            "{base_sql} AND lower(name) LIKE '%' || lower(?1) || '%' ESCAPE '\\' ORDER BY name"
        ))?;
        let out: Vec<Value> = stmt.query_map([&like], reply_row)?.flatten().collect();
        Ok(out)
    } else {
        let mut stmt = conn.prepare(&format!("{base_sql} ORDER BY name"))?;
        let out: Vec<Value> = stmt.query_map([], reply_row)?.flatten().collect();
        Ok(out)
    }
}

fn reply_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, i64>(0)?,
        "remote_id": r.get::<_, Option<i64>>(1)?,
        "mailbox_id": r.get::<_, Option<i64>>(2)?,
        "name": r.get::<_, String>(3)?,
        "preview": r.get::<_, Option<String>>(4)?,
        "text": r.get::<_, Option<String>>(5)?,
        "chat_text": r.get::<_, Option<String>>(6)?,
    }))
}

/// Clean-audit item 83: the saved-replies mirror search had neither an
/// ESCAPE clause nor escaping — `%`/`_` in the input acted as wildcards
/// over the mirror.
#[cfg(test)]
mod batch27_audit83_tests {
    use super::*;

    fn seeded() -> rusqlite::Connection {
        let mut conn = rusqlite::Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        ensure_tables(&conn).expect("tables");
        conn.execute(
            "INSERT INTO mailboxes (id, remote_id, name) VALUES (1, 101, 'Support')",
            [],
        )
        .expect("mailbox");
        for (id, name) in [
            (1, "Refund policy"),
            (2, "50% off apology"),
            (3, "Greetings_z"),
        ] {
            conn.execute(
                "INSERT INTO saved_replies (id, remote_id, mailbox_local_id, name)
                 VALUES (?1, ?1, 1, ?2)",
                rusqlite::params![id, name],
            )
            .expect("reply");
        }
        conn
    }

    fn names_matching(conn: &rusqlite::Connection, q: &str) -> Vec<String> {
        saved_replies(conn, Some(q))
            .expect("replies")
            .into_iter()
            .filter_map(|v| v.get("name").and_then(|n| n.as_str().map(str::to_string)))
            .collect::<Vec<_>>()
    }

    #[test]
    fn percent_and_underscore_match_literally() {
        let conn = seeded();
        assert_eq!(
            names_matching(&conn, "50%"),
            vec!["50% off apology".to_string()],
            "a literal % finds only the discount reply"
        );
        assert_eq!(
            names_matching(&conn, "Greetings_z"),
            vec!["Greetings_z".to_string()],
            "a literal _ is not a wildcard"
        );
    }

    #[test]
    fn plain_searches_still_match() {
        let conn = seeded();
        assert_eq!(
            names_matching(&conn, "refund"),
            vec!["Refund policy".to_string()],
            "case-insensitive substring matching is unchanged"
        );
    }
}
