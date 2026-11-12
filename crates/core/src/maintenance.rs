use rusqlite::Connection;

use crate::error::Result;

pub fn apply_m037(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS products (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            name         TEXT NOT NULL UNIQUE COLLATE NOCASE,
            description  TEXT,
            pinned       INTEGER NOT NULL DEFAULT 0,
            source       TEXT NOT NULL DEFAULT 'derived'
                CHECK (source IN ('derived','human')),
            first_seen_at TEXT,
            last_seen_at TEXT,
            created_at   TEXT NOT NULL DEFAULT (datetime('now')),
            provenance   TEXT NOT NULL DEFAULT 'deterministic_local'
        );",
    )?;
    add_column_if_missing(conn, "incidents", "product", "TEXT")?;
    add_column_if_missing(conn, "known_issues", "product", "TEXT")?;
    add_column_if_missing(conn, "issue_clusters", "product", "TEXT")?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 37 WHERE id = 1", []);
    Ok(())
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

pub fn compute_trends(conn: &Connection) -> Result<()> {
    conn.execute(
        "UPDATE issue_clusters SET trend = CASE
           WHEN julianday(first_seen_at) >= julianday('now', '-14 days') THEN 'new'
           ELSE 'stable'
         END",
        [],
    )?;
    let clusters: Vec<(i64, i64, i64)> = {
        let mut stmt = conn.prepare(
            "SELECT ic.id,
               (SELECT COUNT(*) FROM issue_cluster_conversations icm
                  JOIN conversations c ON c.id = icm.conversation_id
                 WHERE icm.cluster_id = ic.id
                   AND julianday(c.created_at) >= julianday('now', '-14 days')) AS recent,
               (SELECT COUNT(*) FROM issue_cluster_conversations icm
                  JOIN conversations c ON c.id = icm.conversation_id
                 WHERE icm.cluster_id = ic.id
                   AND julianday(c.created_at) >= julianday('now', '-28 days')
                   AND julianday(c.created_at) <  julianday('now', '-14 days')) AS previous
             FROM issue_clusters ic",
        )?;

        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect()
    };
    let tx = conn.unchecked_transaction()?;
    for (id, recent, previous) in clusters {
        let trend = if recent as f64 > previous as f64 * 1.3 && recent >= 3 {
            "rising"
        } else if previous > 0 && (recent as f64) < previous as f64 * 0.7 {
            "falling"
        } else if previous == 0 && recent > 0 {
            "new"
        } else {
            "stable"
        };
        tx.execute(
            "UPDATE issue_clusters SET trend = ?1 WHERE id = ?2",
            rusqlite::params![trend, id],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn refresh_products(conn: &Connection) -> Result<usize> {
    let before: i64 = conn.query_row("SELECT COUNT(*) FROM products", [], |r| r.get(0))?;
    conn.execute(
        "INSERT OR IGNORE INTO products (name, source, first_seen_at, last_seen_at, provenance)
         SELECT DISTINCT TRIM(p.product), 'derived', datetime('now'), datetime('now'), 'deterministic_local'
           FROM (
             SELECT product FROM incidents WHERE product IS NOT NULL AND TRIM(product) != '' AND LENGTH(TRIM(product)) <= 120
             UNION ALL
             SELECT product FROM known_issues WHERE product IS NOT NULL AND TRIM(product) != '' AND LENGTH(TRIM(product)) <= 120
             UNION ALL
             SELECT product FROM issue_clusters WHERE product IS NOT NULL AND TRIM(product) != '' AND LENGTH(TRIM(product)) <= 120
           ) AS p
          WHERE TRIM(p.product) != ''",
        [],
    )?;
    let after: i64 = conn.query_row("SELECT COUNT(*) FROM products", [], |r| r.get(0))?;
    Ok((after - before).max(0) as usize)
}

pub fn enforce_retention(conn: &Connection, retention_days: i64) -> Result<usize> {
    if retention_days <= 0 {
        return Ok(0);
    }
    let mut removed = 0usize;
    for (table, column) in [
        ("webhook_events", "received_at"),
        ("application_errors", "timestamp"),
        ("audit_log", "timestamp"),
        ("ai_runs", "created_at"),
        ("translation_cache", "created_at"),
    ] {
        let sql = format!(
            "DELETE FROM {table} WHERE julianday({column}) < julianday('now', '-{retention_days} days')"
        );
        if let Ok(n) = conn.execute(&sql, []) {
            removed += n;
        }
    }
    if let Ok(n) = conn.execute(
        "DELETE FROM notifications
         WHERE (read_at IS NULL AND julianday(created_at) < julianday('now', ?1))
            OR (read_at IS NOT NULL AND julianday(read_at) < julianday('now', ?1))",
        [format!("-{retention_days} days")],
    ) {
        removed += n;
    }
    Ok(removed)
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
    fn retention_prunes_stale_translation_cache_rows() {
        let conn = migrated();
        conn.execute(
            "INSERT INTO translation_cache
                 (cache_key, source_lang, target_lang, purpose, source_text,
                  translated_text, model, created_at)
             VALUES ('fresh', 'en', 'de', 'general', 'hello', 'hallo', 'm',
                      datetime('now', '-1 day')),
                    ('stale', 'en', 'de', 'general', 'bye', 'tschüss', 'm',
                      datetime('now', '-400 day'))",
            [],
        )
        .expect("seed cache");

        let removed = enforce_retention(&conn, 365).expect("retention");
        assert!(removed >= 1, "the stale row is removed");
        let remaining: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM translation_cache WHERE cache_key = 'stale'",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(remaining, 0, "the 400-day-old cache row is gone");
        let fresh: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM translation_cache WHERE cache_key = 'fresh'",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(fresh, 1, "the 1-day-old cache row survives");
    }

    #[test]
    fn retention_disabled_prunes_nothing() {
        let conn = migrated();
        conn.execute(
            "INSERT INTO translation_cache
                 (cache_key, source_lang, target_lang, purpose, source_text,
                  translated_text, model, created_at)
             VALUES ('old', 'en', 'de', 'general', 'x', 'y', 'm',
                      datetime('now', '-400 day'))",
            [],
        )
        .expect("seed cache");
        let removed = enforce_retention(&conn, 0).expect("retention");
        assert_eq!(removed, 0);
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM translation_cache", [], |r| r.get(0))
            .expect("count");
        assert_eq!(n, 1, "retention off keeps every cached row");
    }

    #[test]
    fn audp2_3_d8_retention_deletes_audit_history_and_never_domain_data() {
        let conn = migrated();
        conn.execute(
            "INSERT INTO audit_log (timestamp, actor, action)
             VALUES (datetime('now', '-400 day'), 'agent', 'status.change'),
                    (datetime('now', '-1 day'), 'agent', 'status.change')",
            [],
        )
        .expect("seed audit_log");
        conn.execute(
            "INSERT INTO webhook_events (event_hash, event_type, received_at, payload)
             VALUES ('h1', 'convo.created', datetime('now', '-400 day'), '{}')",
            [],
        )
        .expect("seed webhook_events");
        conn.execute(
            "INSERT INTO application_errors (timestamp, service, message)
             VALUES (datetime('now', '-400 day'), 'test', 'boom')",
            [],
        )
        .expect("seed application_errors");
        conn.execute(
            "INSERT INTO ai_runs (input_hash, prompt_version, model, response_json, created_at)
             VALUES ('k1', 'v1', 'm', '{}', strftime('%Y-%m-%dT%H:%M:%fZ','now','-400 day'))",
            [],
        )
        .expect("seed ai_runs");
        conn.execute(
            "INSERT INTO notifications (type, title, dedup_key, created_at)
             VALUES ('sync_failure', 't', 'd8-n1', datetime('now', '-400 day')),
                    ('sync_failure', 't', 'd8-n2', datetime('now', '-400 day'))",
            [],
        )
        .expect("seed notifications");
        conn.execute(
            "UPDATE notifications SET read_at = datetime('now', '-1 day')
             WHERE dedup_key = 'd8-n2'",
            [],
        )
        .expect("mark read");
        conn.execute(
            "INSERT INTO customers (id, remote_id, email, created_at, updated_at)
             VALUES (1, 900, 'kept@example.com', datetime('now'), datetime('now'))",
            [],
        )
        .expect("seed customer");
        conn.execute(
            "INSERT INTO mailboxes (id, remote_id, name, slug, email, created_at, updated_at)
             VALUES (1, 1, 'kept', 'kept', 'kept@example.com',
                     datetime('now'), datetime('now'))",
            [],
        )
        .expect("seed mailbox");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, status, mailbox_local_id, customer_local_id, created_at, updated_at)
             VALUES (900, 900, 'kept', 'open', 1, 1,
                     datetime('now', '-400 day'), datetime('now', '-400 day'))",
            [],
        )
        .expect("seed conversation");

        let removed = enforce_retention(&conn, 365).expect("retention");
        assert!(removed >= 5, "each stale row counts: got {removed}");

        let stale_audit: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_log
                  WHERE julianday(timestamp) < julianday('now', '-365 day')",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(
            stale_audit, 0,
            "the stale audit-history row is deleted (D8)"
        );
        let fresh_audit: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_log
                  WHERE julianday(timestamp) >= julianday('now', '-365 day')",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(fresh_audit, 1, "audit history inside the horizon survives");

        let stale_hooks: i64 = conn
            .query_row("SELECT COUNT(*) FROM webhook_events", [], |r| r.get(0))
            .expect("count");
        assert_eq!(stale_hooks, 0, "stale webhook events are deleted");
        let stale_errors: i64 = conn
            .query_row("SELECT COUNT(*) FROM application_errors", [], |r| r.get(0))
            .expect("count");
        assert_eq!(stale_errors, 0, "stale error reports are deleted");
        let stale_runs: i64 = conn
            .query_row("SELECT COUNT(*) FROM ai_runs", [], |r| r.get(0))
            .expect("count");
        assert_eq!(stale_runs, 0, "stale AI run records are deleted");
        let kept_read_notification: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notifications WHERE dedup_key = 'd8-n2'",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(
            kept_read_notification, 1,
            "a read notification's clock runs from read_at — 1 day old survives a 365-day horizon"
        );

        let kept_conversation: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM conversations WHERE remote_id = 900",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(
            kept_conversation, 1,
            "conversations are never pruned by retention"
        );
        let kept_customer: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM customers WHERE remote_id = 900",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(kept_customer, 1, "customers are never pruned by retention");
    }
}
