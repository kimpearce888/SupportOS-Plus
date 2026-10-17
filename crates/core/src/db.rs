use std::path::Path;

use rusqlite::Connection;

use crate::error::{Error, Result};

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::Io)?;
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

pub fn open_with_migrations(path: &Path) -> Result<Connection> {
    let mut conn = open(path)?;
    ensure_migrations_table(&conn)?;
    crate::migrations::run_all(&mut conn)?;
    Ok(conn)
}

pub fn ensure_migrations_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version    INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            label      TEXT
        );",
    )?;
    Ok(())
}

pub fn latest_version(conn: &Connection) -> Result<u32> {
    let v: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM _migrations",
        [],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(v).unwrap_or(0))
}

pub fn apply_migration(conn: &mut Connection, version: u32, label: &str, sql: &str) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch(sql).map_err(|e| Error::Migration {
        version,
        message: e.to_string(),
    })?;
    tx.execute(
        "INSERT INTO _migrations (version, label) VALUES (?1, ?2)",
        rusqlite::params![i64::from(version as i32), label],
    )?;
    tx.commit()?;
    Ok(())
}
