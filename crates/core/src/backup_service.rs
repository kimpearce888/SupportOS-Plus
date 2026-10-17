use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;
use serde::Serialize;
use serde_json::{Value, json};

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

    let settings_target = target.to_string_lossy().replace(".db", ".settings.json");
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

pub fn restore(db_path: &Path, backup_path: &Path) -> Value {
    if !backup_path.exists() {
        return json!({ "ok": false, "message": "Backup file not found." });
    }
    if let Some(parent) = db_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let restore_tmp = format!("{}.restore-tmp", db_path.to_string_lossy());
    if fs::copy(backup_path, &restore_tmp).is_err() {
        return json!({ "ok": false, "message": "Restore failed: could not stage the backup." });
    }
    if fs::rename(&restore_tmp, db_path).is_err() {
        return json!({ "ok": false, "message": "Restore failed: could not swap the database into place." });
    }
    for ext in ["-wal", "-shm"] {
        let p = format!("{}{ext}", db_path.to_string_lossy());
        if Path::new(&p).exists() {
            let _ = fs::remove_file(&p);
        }
    }
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
    Ok(json!({
        "ok": true,
        "message": "JSON export created. Note: this export contains customer data - handle it carefully.",
        "path": target.to_string_lossy(),
    }))
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
    let esc = |v: &Option<String>| -> String {
        let s = v.as_deref().unwrap_or("").replace('"', "\"\"");
        if s.contains('"') || s.contains(',') || s.contains('\n') {
            format!("\"{s}\"")
        } else {
            s
        }
    };
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
    Ok(json!({
        "ok": true,
        "message": "CSV export created. Note: this export contains customer data.",
        "path": target.to_string_lossy(),
    }))
}
