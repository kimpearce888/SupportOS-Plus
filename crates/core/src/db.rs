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

pub(crate) fn query_rows<T>(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
    map: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params, map)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub(crate) fn rows_ok<T, F>(rows: rusqlite::MappedRows<'_, F>) -> Vec<T>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    rows.filter_map(|r| r.ok()).collect()
}

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

const READER_POOL_SIZE: usize = 2;

enum Lane {
    Single(Arc<Mutex<Connection>>),
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

#[derive(Clone)]
pub struct DbHandle {
    lane: Arc<Lane>,
}

impl DbHandle {
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

    pub fn with_write<T>(&self, f: impl FnOnce(&mut Connection) -> T) -> T {
        let mut guard = self.write_guard();
        f(&mut guard)
    }

    pub fn write_guard(&self) -> DbWriteGuard<'_> {
        let guard = match &*self.lane {
            Lane::Single(conn) => recover(conn),
            Lane::Pooled { writer, .. } => recover(writer),
        };
        DbWriteGuard { guard }
    }

    pub fn long_op_guard(&self) -> DbLongOpGuard<'_> {
        let path = self.with_read(db_file_path);
        if let Some(path) = path
            && let Ok(conn) = open(&path)
        {
            return DbLongOpGuard {
                lane: LongOpLane::Owned(conn),
            };
        }
        DbLongOpGuard {
            lane: LongOpLane::Shared(self.write_guard()),
        }
    }

    #[must_use]
    pub fn file_path(&self) -> Option<PathBuf> {
        db_file_path(&self.write_guard())
    }
}

fn acquire_reader(readers: &[Arc<Mutex<Connection>>]) -> MutexGuard<'_, Connection> {
    for reader in readers {
        if let Ok(guard) = reader.try_lock() {
            return guard;
        }
    }
    recover(&readers[0])
}

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

pub struct DbLongOpGuard<'a> {
    lane: LongOpLane<'a>,
}

enum LongOpLane<'a> {
    Owned(Connection),
    Shared(DbWriteGuard<'a>),
}

impl Deref for DbLongOpGuard<'_> {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        match &self.lane {
            LongOpLane::Owned(conn) => conn,
            LongOpLane::Shared(guard) => guard,
        }
    }
}

impl DerefMut for DbLongOpGuard<'_> {
    fn deref_mut(&mut self) -> &mut Connection {
        match &mut self.lane {
            LongOpLane::Owned(conn) => conn,
            LongOpLane::Shared(guard) => guard,
        }
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

        let (tx_hold, rx_hold) = std::sync::mpsc::channel::<()>();
        let (tx_done, rx_done) = std::sync::mpsc::channel::<()>();
        let holder = handle.clone();
        std::thread::spawn(move || {
            holder.with_read(|conn| {
                let _count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM probe", [], |r| r.get(0))
                    .expect("count under held reader");
                rx_hold.recv().expect("hold signal");
            });
            tx_done.send(()).expect("done signal");
        });

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
