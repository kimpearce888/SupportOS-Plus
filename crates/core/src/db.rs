use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

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

/// Number of reader connections in the WAL reader pool.
///
/// The pool lane is the migration-map "writer + 2 WAL readers" design: one
/// exclusive writer connection (SQLite WAL permits exactly one writer at a
/// time — serializing it with its own mutex is the same contract an OS
/// writer thread would enforce, without forcing every write closure to be
/// `'static`), plus [`READER_POOL_SIZE`] dedicated reader connections that
/// serve concurrent reads while a write is in flight.
const READER_POOL_SIZE: usize = 2;

enum Lane {
    /// In-memory / temporary databases: a single shared connection. Every
    /// `:memory:` connection is a private database, so separate readers
    /// would see none of the writer's data.
    Single(Arc<Mutex<Connection>>),
    /// File-backed databases: an exclusive writer connection plus a small
    /// pool of WAL reader connections on the same file.
    Pooled {
        writer: Arc<Mutex<Connection>>,
        readers: Vec<Arc<Mutex<Connection>>>,
    },
}

fn recover(m: &Mutex<Connection>) -> MutexGuard<'_, Connection> {
    m.lock().unwrap_or_else(|poisoned| {
        tracing::warn!("DbHandle mutex was poisoned — recovering");
        poisoned.into_inner()
    })
}

/// Pooled connection facade (decisions D3/D4-era "DB pool inside DbHandle").
///
/// All UI-side and background code binds to `DbHandle` instead of a raw
/// `Arc<Mutex<Connection>>`: reads go through [`Self::with_read`] (the WAL
/// reader pool on file-backed databases, the shared connection on
/// in-memory ones), writes through [`Self::with_write`] (the exclusive
/// writer lane). Code that needs a scoped guard instead of a closure uses
/// [`Self::write_guard`]. `lock()` poisoning is recovered from, matching
/// the pre-pool facade behavior.
#[derive(Clone)]
pub struct DbHandle {
    lane: Arc<Lane>,
}

impl DbHandle {
    /// Wrap a freshly opened connection, upgrading file-backed databases
    /// to the pooled lane (writer + readers on the same file).
    ///
    /// Migrations must run on `conn` **before** it is wrapped: the reader
    /// connections are opened immediately and only ever read committed
    /// state, so they rely on the schema already being in place.
    #[must_use]
    pub fn new(conn: Connection) -> Self {
        let Some(path) = db_file_path(&conn) else {
            return Self {
                lane: Arc::new(Lane::Single(Arc::new(Mutex::new(conn)))),
            };
        };
        let mut readers = Vec::with_capacity(READER_POOL_SIZE);
        for _ in 0..READER_POOL_SIZE {
            match open(&path) {
                Ok(reader) => readers.push(Arc::new(Mutex::new(reader))),
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        path = %path.display(),
                        "reader connection could not be opened — degrading the pool"
                    );
                    break;
                }
            }
        }
        if readers.is_empty() {
            // No readers could be opened (e.g. transient IO error): behave
            // exactly like the pre-pool single-connection facade.
            tracing::warn!("DbHandle degraded to the single-connection lane");
            Self {
                lane: Arc::new(Lane::Single(Arc::new(Mutex::new(conn)))),
            }
        } else {
            Self {
                lane: Arc::new(Lane::Pooled {
                    writer: Arc::new(Mutex::new(conn)),
                    readers,
                }),
            }
        }
    }

    /// Size of the WAL reader pool (0 for the single-connection lane).
    /// Exposed for diagnostics and tests.
    #[must_use]
    pub fn reader_pool_size(&self) -> usize {
        match &*self.lane {
            Lane::Single(_) => 0,
            Lane::Pooled { readers, .. } => readers.len(),
        }
    }

    /// Run `f` with a read connection.
    ///
    /// On file-backed databases this takes a connection from the reader
    /// pool (an idle one when available) so reads run concurrently with
    /// each other and with an in-flight writer transaction, per WAL.
    pub fn with_read<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        match &*self.lane {
            Lane::Single(conn) => {
                let guard = recover(conn);
                f(&guard)
            }
            Lane::Pooled { readers, .. } => {
                let guard = acquire_reader(readers);
                f(&guard)
            }
        }
    }

    /// Run `f` with the exclusive writer connection.
    pub fn with_write<T>(&self, f: impl FnOnce(&mut Connection) -> T) -> T {
        let mut guard = self.write_guard();
        f(&mut guard)
    }

    /// Scoped exclusive (writer-lane) guard for code that cannot be
    /// expressed as a closure — the direct replacement for the old
    /// `lock()` accessor. Deref/DerefMut to [`Connection`].
    pub fn write_guard(&self) -> DbWriteGuard<'_> {
        let guard = match &*self.lane {
            Lane::Single(conn) => recover(conn),
            Lane::Pooled { writer, .. } => recover(writer),
        };
        DbWriteGuard { guard }
    }

    /// Filesystem path of the wrapped database, or `None` for
    /// in-memory/temporary databases.
    #[must_use]
    pub fn file_path(&self) -> Option<PathBuf> {
        db_file_path(&self.write_guard())
    }

    /// Whether two handles share the exact same connection pool.
    #[must_use]
    pub fn is_shared_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.lane, &other.lane)
    }
}

fn acquire_reader(readers: &[Arc<Mutex<Connection>>]) -> MutexGuard<'_, Connection> {
    for reader in readers {
        if let Ok(guard) = reader.try_lock() {
            return guard;
        }
    }
    // Every reader is busy: queue behind the first one (blocking lock
    // preserves FIFO fairness; the pool only has to absorb bursts).
    recover(&readers[0])
}

/// RAII guard over the exclusive writer connection (or the single shared
/// connection on the single-lane mode). Returned by
/// [`DbHandle::write_guard`]; dereferences to [`Connection`] so existing
/// `let conn = ...; conn.execute(...)` call sites keep working unchanged.
pub struct DbWriteGuard<'a> {
    guard: MutexGuard<'a, Connection>,
}

impl Deref for DbWriteGuard<'_> {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        &self.guard
    }
}

impl DerefMut for DbWriteGuard<'_> {
    fn deref_mut(&mut self) -> &mut Connection {
        &mut self.guard
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

    fn temp_db_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dbhandle-test-{tag}-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    fn cleanup(path: &Path) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
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
    fn in_memory_handle_has_no_file_path() {
        let handle = setup();
        assert_eq!(handle.file_path(), None);
        assert_eq!(handle.reader_pool_size(), 0);
    }

    #[test]
    fn file_backed_handle_reports_path() {
        let path = temp_db_path("path");
        let conn = open(&path).expect("open file-backed db");
        let handle = DbHandle::new(conn);
        let reported = handle.file_path().expect("file path");
        assert!(reported.ends_with(&path));
        drop(handle);
        cleanup(&path);
    }

    #[test]
    fn file_backed_handle_uses_the_reader_pool() {
        let path = temp_db_path("pool");
        let conn = open(&path).expect("open file-backed db");
        conn.execute_batch("CREATE TABLE probe (v INTEGER);")
            .expect("create probe table");
        let handle = DbHandle::new(conn);
        assert_eq!(handle.reader_pool_size(), READER_POOL_SIZE);

        // Readers see the writer's committed state (separate connections,
        // same WAL file).
        handle.with_write(|conn| {
            conn.execute("INSERT INTO probe (v) VALUES (5)", [])
                .expect("insert");
        });
        let v: i64 = handle
            .with_read(|conn| conn.query_row("SELECT v FROM probe", [], |r| r.get(0)))
            .expect("select from a reader connection");
        assert_eq!(v, 5);
        drop(handle);
        cleanup(&path);
    }

    #[test]
    fn reader_pool_serves_concurrent_reads() {
        let path = temp_db_path("concurrent");
        let conn = open(&path).expect("open file-backed db");
        conn.execute_batch("CREATE TABLE probe (v INTEGER);")
            .expect("create probe table");
        let handle = DbHandle::new(conn);
        handle.with_write(|conn| {
            conn.execute("INSERT INTO probe (v) VALUES (1)", [])
                .expect("insert");
        });

        // Both readers must be usable simultaneously: hold one read open
        // (guarded by a channel) and prove a second with_read completes
        // without waiting for it.
        let (tx_hold, rx_hold) = std::sync::mpsc::channel::<()>();
        let (tx_done, rx_done) = std::sync::mpsc::channel::<()>();
        let holder = handle.clone();
        std::thread::spawn(move || {
            holder.with_read(|conn| {
                // Hold the reader connection while the main thread reads.
                let _count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM probe", [], |r| r.get(0))
                    .expect("count under held reader");
                rx_hold.recv().expect("hold signal");
            });
            tx_done.send(()).expect("done signal");
        });

        // The holder's reader is busy inside with_read; the main thread
        // must land on the other pool connection.
        let v: i64 = handle
            .with_read(|conn| conn.query_row("SELECT v FROM probe", [], |r| r.get(0)))
            .expect("second concurrent read");
        assert_eq!(v, 1);
        tx_hold.send(()).expect("release holder");
        rx_done
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("holder finished");
        drop(handle);
        cleanup(&path);
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

    #[test]
    fn write_guard_supports_deref_mut_transactions() {
        let handle = setup();
        {
            let mut guard = handle.write_guard();
            let tx = guard.transaction().expect("transaction via guard");
            tx.execute("INSERT INTO probe (v) VALUES (9)", [])
                .expect("insert via guard tx");
            tx.commit().expect("commit guard tx");
        }
        let count: i64 = handle
            .with_read(|conn| conn.query_row("SELECT COUNT(*) FROM probe", [], |r| r.get(0)))
            .expect("count");
        assert_eq!(count, 1);
    }
}
