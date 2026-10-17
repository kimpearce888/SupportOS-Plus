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
         WHERE julianday(created_at) < julianday('now', ?1)
            OR (read_at IS NOT NULL AND julianday(read_at) < julianday('now', ?1))",
        [format!("-{retention_days} days")],
    ) {
        removed += n;
    }
    Ok(removed)
}
