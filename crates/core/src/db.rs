use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

/// Filesystem path of the connection's `main` database, or `None` for
/// in-memory or temporary databases.
#[must_use]
pub fn db_file_path(conn: &Connection) -> Option<PathBuf> {
    let mut stmt = conn.prepare("PRAGMA database_list").ok()?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?))
        })
        .ok()?;
    for (name, file) in rows.flatten() {
        if name == "main" {
            return file.filter(|p| !p.is_empty()).map(PathBuf::from);
        }
    }
    None
}

/// Shared-connection facade over `Arc<Mutex<Connection>>` (decision D3).
///
/// All new UI-side code binds to `DbHandle` instead of the raw
/// `Arc<Mutex<Connection>>` used by the HTTP layer, so the connection
/// strategy can change behind this seam — the single shared connection
/// today, a writer thread plus WAL readers after the Phase 4 cutover —
/// without touching call sites. `lock()` poisoning is recovered from,
/// mirroring [`crate::http::server::lock_or_recover`].
#[derive(Clone)]
pub struct DbHandle {
    conn: Arc<Mutex<Connection>>,
}

impl DbHandle {
    /// Wrap a freshly opened connection.
    #[must_use]
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    /// Adopt an existing shared connection (e.g. one also held by the
    /// HTTP `AppState` during the side-by-side port period).
    #[must_use]
    pub fn from_shared(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Lock the connection, recovering from a poisoned mutex.
    ///
    /// Prefer [`Self::with_read`] / [`Self::with_write`] in new code:
    /// closure-based access is what the post-cutover pool will offer.
    pub fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("DbHandle mutex was poisoned — recovering");
            poisoned.into_inner()
        })
    }

    /// Run `f` with shared access to the connection.
    pub fn with_read<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        let guard = self.lock();
        f(&guard)
    }

    /// Run `f` with exclusive access to the connection.
    pub fn with_write<T>(&self, f: impl FnOnce(&mut Connection) -> T) -> T {
        let mut guard = self.lock();
        f(&mut guard)
    }

    /// Filesystem path of the wrapped database, or `None` for
    /// in-memory/temporary databases.
    #[must_use]
    pub fn file_path(&self) -> Option<PathBuf> {
        db_file_path(&self.lock())
    }

    /// Whether two handles share the exact same connection instance.
    #[must_use]
    pub fn is_shared_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.conn, &other.conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> DbHandle {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch("CREATE TABLE probe (v INTEGER);")
            .expect("create probe table");
        DbHandle::new(conn)
    }

    #[test]
    fn with_read_returns_query_result() {
        let handle = setup();
        handle.with_write(|conn| {
            conn.execute("INSERT INTO probe (v) VALUES (42)", [])
                .expect("insert");
        });
        let v: i64 = handle
            .with_read(|conn| conn.query_row("SELECT v FROM probe", [], |r| r.get(0)))
            .expect("select");
        assert_eq!(v, 42);
    }

    #[test]
    fn with_write_can_open_transactions() {
        let handle = setup();
        handle.with_write(|conn| {
            let tx = conn.transaction().expect("begin tx");
            tx.execute("INSERT INTO probe (v) VALUES (7)", [])
                .expect("insert in tx");
            tx.commit().expect("commit");
        });
        let count: i64 = handle
            .with_read(|conn| conn.query_row("SELECT COUNT(*) FROM probe", [], |r| r.get(0)))
            .expect("count");
        assert_eq!(count, 1);
    }

    #[test]
    fn clones_share_the_connection() {
        let handle = setup();
        let clone = handle.clone();
        assert!(handle.is_shared_with(&clone));
        clone.with_write(|conn| {
            conn.execute("INSERT INTO probe (v) VALUES (1)", [])
                .expect("insert");
        });
        let count: i64 = handle
            .with_read(|conn| conn.query_row("SELECT COUNT(*) FROM probe", [], |r| r.get(0)))
            .expect("count");
        assert_eq!(count, 1);
    }

    #[test]
    fn from_shared_adopts_existing_arc() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        let shared = Arc::new(Mutex::new(conn));
        let a = DbHandle::from_shared(Arc::clone(&shared));
        let b = DbHandle::from_shared(shared);
        assert!(a.is_shared_with(&b));
    }

    #[test]
    fn in_memory_handle_has_no_file_path() {
        let handle = setup();
        assert_eq!(handle.file_path(), None);
    }

    #[test]
    fn file_backed_handle_reports_path() {
        let path = std::env::temp_dir().join(format!("dbhandle-test-{}.db", std::process::id()));
        let conn = open(&path).expect("open file-backed db");
        let handle = DbHandle::new(conn);
        let reported = handle.file_path().expect("file path");
        assert!(reported.ends_with(&path));
        drop(handle);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    #[test]
    fn recovers_from_poisoned_mutex() {
        let handle = setup();
        let handle_ptr = &handle;
        let boom = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            handle_ptr.with_write(|_| panic!("poison the DbHandle mutex"));
        }));
        assert!(boom.is_err());
        // The next access must recover instead of panicking.
        handle
            .with_read(|conn| {
                conn.query_row("SELECT COUNT(*) FROM probe", [], |r| r.get::<_, i64>(0))
            })
            .expect("read after poison recovery");
    }
}
