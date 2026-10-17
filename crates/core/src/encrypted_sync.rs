use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::crypto;
use crate::error::Result;

pub const MAGIC: &[u8; 6] = b"SOSYNC";
pub const FORMAT_VERSION: u32 = 1;
const SCRYPT_N: u32 = 1 << 15;
const LOG_N: u8 = SCRYPT_N.trailing_zeros() as u8;
const KEY_LEN: usize = crypto::KEY_LEN;

#[derive(Debug, Clone, Serialize)]
pub struct SyncBundleInfo {
    pub version: u32,
    pub created_at: String,
    pub conversations: i64,
    pub customers: i64,
    pub app_version: Option<String>,
    pub sha256: String,
}

#[derive(Serialize)]
pub struct ExportResult {
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<SyncBundleInfo>,
}

#[derive(Serialize)]
pub struct ImportResult {
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_restart: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BundleHeader {
    v: u32,
    kdf: KdfParams,
    cipher: String,
    iv: String,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    app_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conversations: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    customers: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KdfParams {
    name: String,
    #[serde(rename = "N")]
    n: u32,
    r: u32,
    p: u32,
    salt: String,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn app_version() -> Option<String> {
    Some(env!("CARGO_PKG_VERSION").to_string())
}

fn stamp() -> String {
    now_iso().replace([':', '.'], "-")
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; KEY_LEN]> {
    crypto::derive_key(passphrase.as_bytes(), salt, LOG_N)
}

pub fn ensure_log_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS encrypted_sync_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            direction TEXT NOT NULL,
            file_path TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            conversations INTEGER,
            customers INTEGER,
            sha256 TEXT,
            at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;
    Ok(())
}

fn snapshot_info(snapshot_path: &Path) -> Result<SyncBundleInfo> {
    let test =
        Connection::open_with_flags(snapshot_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let conversations: i64 =
        test.query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))?;
    let customers: i64 = test.query_row("SELECT COUNT(*) FROM customers", [], |r| r.get(0))?;
    drop(test);
    let bytes = fs::read(snapshot_path)?;
    let sha256 = crypto::hex(&Sha256::digest(&bytes));
    Ok(SyncBundleInfo {
        version: FORMAT_VERSION,
        created_at: now_iso(),
        conversations,
        customers,
        app_version: app_version(),
        sha256,
    })
}

fn schema_version_of_file(path: &Path) -> i64 {
    let Ok(test) = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
    else {
        return 0;
    };
    for (table, col) in [
        ("schema_migrations", "id"),
        ("migrations", "id"),
        ("_migrations", "version"),
    ] {
        let exists: bool = test
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
                [table],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false);
        if exists
            && let Ok(v) = test.query_row(
                &format!("SELECT COALESCE(MAX({col}), 0) FROM {table}"),
                [],
                |r| r.get(0),
            )
        {
            return v;
        }
    }
    0
}

const FTS_SHADOW_SUFFIXES: [&str; 5] = ["_data", "_idx", "_docsize", "_content", "_config"];

fn virtual_tables(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master
         WHERE type = 'table' AND sql LIKE 'CREATE VIRTUAL%'",
    )?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn fingerprint_excluded(name: &str, virtual_tables: &[String]) -> bool {
    if name.starts_with("sqlite_") {
        return true;
    }
    if name == "_migrations" || name == "schema_migrations" || name == "migrations" {
        return true;
    }
    for vt in virtual_tables {
        for suffix in FTS_SHADOW_SUFFIXES {
            if name == format!("{vt}{suffix}") {
                return true;
            }
        }
    }
    false
}

pub fn schema_fingerprint(conn: &Connection) -> Result<String> {
    let vtabs = virtual_tables(conn)?;
    let mut objects: Vec<(String, String, String)> = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT type, name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (typ, name, sql) in rows {
            if fingerprint_excluded(&name, &vtabs) {
                continue;
            }
            objects.push((typ, name, sql));
        }
    }
    objects.sort();
    let mut canonical = String::new();
    for (typ, name, sql) in &objects {
        let normalized: String = sql.split_whitespace().collect::<Vec<_>>().join(" ");
        canonical.push_str(&format!("{typ}\u{1f}{name}\u{1f}{normalized}\n"));
    }
    Ok(crypto::hex(&Sha256::digest(canonical.as_bytes())))
}

fn fingerprint_of_file(path: &Path) -> Result<String> {
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    schema_fingerprint(&conn)
}

fn table_columns(conn: &Connection, table: &str) -> Option<Vec<(String, String, i64, i64)>> {
    let escaped = table.replace('"', "\"\"");
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info(\"{escaped}\")"))
        .ok()?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_lowercase(),
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(5)?,
            ))
        })
        .ok()?;
    Some(rows.flatten().collect())
}

fn snapshot_is_compatible_ancestor(local: &Connection, snapshot: &Connection) -> bool {
    let vtabs = match virtual_tables(snapshot) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let names: Vec<String> = match snapshot
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .and_then(|mut s| {
            s.query_map([], |r| r.get::<_, String>(0))
                .map(|rows| rows.flatten().collect::<Vec<_>>())
        }) {
        Ok(v) => v,
        Err(_) => return false,
    };
    for name in names {
        if fingerprint_excluded(&name, &vtabs) {
            continue;
        }
        let Some(snap_cols) = table_columns(snapshot, &name) else {
            return false;
        };
        let Some(local_cols) = table_columns(local, &name) else {
            return false;
        };
        let local_set: HashSet<(String, String, i64, i64)> = local_cols.into_iter().collect();
        for col in snap_cols {
            if !local_set.contains(&col) {
                return false;
            }
        }
    }
    true
}

enum SchemaCheck {
    Identical,
    CompatibleAncestor,
    Incompatible { bundle: String, local: String },
}

fn check_schema_compatibility(local: &Connection, snapshot: &Connection) -> Result<SchemaCheck> {
    let local_fp = schema_fingerprint(local)?;
    let bundle_fp = schema_fingerprint(snapshot)?;
    if local_fp == bundle_fp {
        return Ok(SchemaCheck::Identical);
    }
    if snapshot_is_compatible_ancestor(local, snapshot) {
        Ok(SchemaCheck::CompatibleAncestor)
    } else {
        Ok(SchemaCheck::Incompatible {
            bundle: bundle_fp,
            local: local_fp,
        })
    }
}

fn fp_short(fp: &str) -> &str {
    &fp[..fp.len().min(12)]
}

pub fn export_bundle(conn: &Connection, bundles_dir: &Path, passphrase: &str) -> ExportResult {
    try_export_bundle(conn, bundles_dir, passphrase).unwrap_or_else(|e| ExportResult {
        ok: false,
        message: format!("Export failed: {e}"),
        path: None,
        size_bytes: None,
        info: None,
    })
}

fn try_export_bundle(
    conn: &Connection,
    bundles_dir: &Path,
    passphrase: &str,
) -> Result<ExportResult> {
    if passphrase.len() < 8 {
        return Ok(ExportResult {
            ok: false,
            message: "Passphrase must be at least 8 characters.".into(),
            path: None,
            size_bytes: None,
            info: None,
        });
    }
    fs::create_dir_all(bundles_dir)?;
    ensure_log_table(conn)?;
    let stamp = stamp();
    let snapshot_path = bundles_dir.join(format!(".snapshot-{stamp}.tmp"));
    let target = bundles_dir.join(format!("supportos-sync-{stamp}.sosync"));

    let escaped = snapshot_path.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{escaped}'"))?;

    let info = snapshot_info(&snapshot_path)?;
    let fingerprint = fingerprint_of_file(&snapshot_path)?;

    let salt = crypto::random_salt();
    let key = derive_key(passphrase, &salt)?;
    let (iv, ciphertext) = crypto::encrypt(&key, &fs::read(&snapshot_path)?)?;
    let header = BundleHeader {
        v: FORMAT_VERSION,
        kdf: KdfParams {
            name: "scrypt".into(),
            n: SCRYPT_N,
            r: 8,
            p: 1,
            salt: B64.encode(salt),
        },
        cipher: "aes-256-gcm".into(),
        iv: B64.encode(iv),
        created_at: now_iso(),
        app_version: app_version(),
        conversations: Some(info.conversations),
        customers: Some(info.customers),
        sha256: Some(info.sha256.clone()),
        schema_fingerprint: Some(fingerprint.clone()),
    };
    let header_bytes = serde_json::to_vec(&header)?;
    let mut header_len = [0u8; 4];
    header_len[0] = (header_bytes.len() >> 24) as u8;
    header_len[1] = (header_bytes.len() >> 16) as u8;
    header_len[2] = (header_bytes.len() >> 8) as u8;
    header_len[3] = header_bytes.len() as u8;

    let mut out = Vec::with_capacity(MAGIC.len() + 4 + header_bytes.len() + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&header_len);
    out.extend_from_slice(&header_bytes);
    out.extend_from_slice(&ciphertext);
    fs::write(&target, &out)?;
    let _ = fs::remove_file(&snapshot_path);

    let size = fs::metadata(&target)?.len();
    log_sync(conn, "export", &target, size, &info)?;
    prune_old_bundles(bundles_dir, 5);
    Ok(ExportResult {
        ok: true,
        message: format!(
            "Encrypted sync bundle created ({:.1} MB). Move it to the other device and import it there with the same passphrase.",
            size as f64 / 1024.0 / 1024.0
        ),
        path: Some(target.to_string_lossy().into_owned()),
        size_bytes: Some(size),
        info: Some(info),
    })
}

fn decrypt_to_temp(
    bundles_dir: &Path,
    bundle_path: &Path,
    passphrase: &str,
) -> std::result::Result<PathBuf, String> {
    if !bundle_path.exists() {
        return Err("Bundle file not found.".into());
    }
    let raw = fs::read(bundle_path).map_err(|e| e.to_string())?;
    if raw.len() < MAGIC.len() + 4 {
        return Err("File is too small to be a SupportOS sync bundle.".into());
    }
    if &raw[0..MAGIC.len()] != MAGIC {
        return Err("This is not a SupportOS encrypted sync bundle (.sosync).".into());
    }
    let mut offset = MAGIC.len();
    let header_len = u32::from_be_bytes([
        raw[offset],
        raw[offset + 1],
        raw[offset + 2],
        raw[offset + 3],
    ]);
    offset += 4;
    if header_len == 0
        || header_len > 65536
        || (offset as u64) + u64::from(header_len) >= raw.len() as u64
    {
        return Err("Bundle header is corrupt.".into());
    }
    let header: BundleHeader = serde_json::from_slice(&raw[offset..offset + header_len as usize])
        .map_err(|_| "Bundle header is corrupt.".to_string())?;
    if header.v != FORMAT_VERSION {
        return Err(format!("Unsupported bundle format version {}.", header.v));
    }
    if header.cipher != "aes-256-gcm"
        || header.kdf.name != "scrypt"
        || header.kdf.r != 8
        || header.kdf.p != 1
    {
        return Err("Bundle uses an unsupported cipher or KDF.".into());
    }
    let ciphertext = &raw[offset + header_len as usize..];

    let salt = B64
        .decode(&header.kdf.salt)
        .map_err(|_| "Bundle header is corrupt.".to_string())?;
    let key = crypto::derive_key(
        passphrase.as_bytes(),
        &salt,
        header.kdf.n.trailing_zeros() as u8,
    )
    .map_err(|e| format!("Key derivation failed: {e}"))?;
    let iv = B64
        .decode(&header.iv)
        .map_err(|_| "Bundle header is corrupt.".to_string())?;
    let plain = crypto::decrypt(&key, &iv, ciphertext).map_err(|_| {
        "Wrong passphrase (or the bundle was modified). Nothing was changed.".to_string()
    })?;
    let temp_path = bundles_dir.join(format!(
        ".import-{}.tmp",
        chrono::Utc::now().timestamp_millis()
    ));
    fs::create_dir_all(bundles_dir).map_err(|e| e.to_string())?;
    fs::write(&temp_path, plain).map_err(|e| e.to_string())?;
    Ok(temp_path)
}

fn integrity_check(path: &Path) -> bool {
    Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .and_then(|test| test.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)))
        .map(|v| v == "ok")
        .unwrap_or(false)
}

pub fn verify_bundle(
    conn: &Connection,
    bundles_dir: &Path,
    bundle_path: &Path,
    passphrase: &str,
) -> Value {
    let temp = match decrypt_to_temp(bundles_dir, bundle_path, passphrase) {
        Ok(t) => t,
        Err(msg) => return json!({ "ok": false, "message": format!("Verification failed: {msg}") }),
    };
    let result = (|| -> Result<Value> {
        ensure_log_table(conn)?;
        let info = snapshot_info(&temp)?;
        let schema = schema_version_of_file(&temp);
        let snap = Connection::open_with_flags(&temp, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let check = check_schema_compatibility(conn, &snap)?;
        let _ = fs::remove_file(&temp);
        match check {
            SchemaCheck::Incompatible { bundle, local } => Ok(json!({
                "ok": false,
                "message": format!(
                    "The bundle was created by an incompatible database schema (bundle fingerprint {}, local {}). Importing it could corrupt your data - nothing was changed.",
                    fp_short(&bundle),
                    fp_short(&local)
                )
            })),
            SchemaCheck::CompatibleAncestor => Ok(json!({
                "ok": true,
                "message": format!(
                    "Bundle verified: {} conversations, {} customers, schema {} — an older compatible schema that will be brought up to date on restart.",
                    info.conversations, info.customers, schema
                ),
                "info": info
            })),
            SchemaCheck::Identical => Ok(json!({
                "ok": true,
                "message": format!(
                    "Bundle verified: {} conversations, {} customers, schema {}.",
                    info.conversations, info.customers, schema
                ),
                "info": info
            })),
        }
    })();
    result
        .unwrap_or_else(|e| json!({ "ok": false, "message": format!("Verification failed: {e}") }))
}

pub fn import_bundle(
    conn: &Connection,
    db_path: &Path,
    bundles_dir: &Path,
    bundle_path: &Path,
    passphrase: &str,
) -> ImportResult {
    let temp = match decrypt_to_temp(bundles_dir, bundle_path, passphrase) {
        Ok(t) => t,
        Err(msg) => {
            return ImportResult {
                ok: false,
                message: format!("Import failed: {msg}"),
                require_restart: None,
            };
        }
    };
    if !integrity_check(&temp) {
        let _ = fs::remove_file(&temp);
        return ImportResult {
            ok: false,
            message: "The decrypted database failed the integrity check. The bundle is corrupt - nothing was changed.".into(),
            require_restart: None,
        };
    }
    let schema_guard = (|| -> Result<SchemaCheck> {
        ensure_log_table(conn)?;
        let snap = Connection::open_with_flags(&temp, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        check_schema_compatibility(conn, &snap)
    })();
    match schema_guard {
        Ok(SchemaCheck::Identical) | Ok(SchemaCheck::CompatibleAncestor) => {}
        Ok(SchemaCheck::Incompatible { bundle, local }) => {
            let _ = fs::remove_file(&temp);
            return ImportResult {
                ok: false,
                message: format!(
                    "The bundle's database schema is incompatible with this build (bundle fingerprint {}, local {}). Importing it could corrupt your data - nothing was changed.",
                    fp_short(&bundle),
                    fp_short(&local)
                ),
                require_restart: None,
            };
        }
        Err(e) => {
            let _ = fs::remove_file(&temp);
            return ImportResult {
                ok: false,
                message: format!("Import failed: schema check failed: {e}"),
                require_restart: None,
            };
        }
    }
    let info = match snapshot_info(&temp) {
        Ok(i) => i,
        Err(e) => {
            let _ = fs::remove_file(&temp);
            return ImportResult {
                ok: false,
                message: format!("Import failed: {e}"),
                require_restart: None,
            };
        }
    };

    let backup_path = format!(
        "{}.pre-import-{}.db",
        db_path.to_string_lossy(),
        chrono::Utc::now().timestamp_millis()
    );
    let escaped = backup_path.replace('\'', "''");
    if conn
        .execute_batch(&format!("VACUUM INTO '{escaped}'"))
        .is_err()
    {
        let _ = fs::remove_file(&temp);
        return ImportResult {
            ok: false,
            message: "Import failed: could not write the safety backup.".into(),
            require_restart: None,
        };
    }

    let restore_tmp = format!("{}.restore-tmp", db_path.to_string_lossy());
    if fs::copy(&temp, &restore_tmp).is_err() {
        let _ = fs::remove_file(&temp);
        return ImportResult {
            ok: false,
            message: "Import failed: could not stage the restored database.".into(),
            require_restart: None,
        };
    }
    let _ = fs::remove_file(&temp);
    if fs::rename(&restore_tmp, db_path).is_err() {
        return ImportResult {
            ok: false,
            message: "Import failed: could not swap the database into place.".into(),
            require_restart: None,
        };
    }
    for ext in ["-wal", "-shm"] {
        let p = format!("{}{ext}", db_path.to_string_lossy());
        if Path::new(&p).exists() {
            let _ = fs::remove_file(&p);
        }
    }
    let size = fs::metadata(bundle_path).map(|m| m.len()).unwrap_or(0);
    let _ = log_sync(conn, "import", bundle_path, size, &info);
    ImportResult {
        ok: true,
        message: format!(
            "Encrypted bundle imported ({} conversations, {} customers). A safety backup of the previous data was written next to the database. Restart SupportOS to use the restored mirror.",
            info.conversations, info.customers
        ),
        require_restart: Some(true),
    }
}

pub fn list_bundles(bundles_dir: &Path) -> Vec<Value> {
    let mut out: Vec<(String, Value)> = Vec::new();
    let Ok(entries) = fs::read_dir(bundles_dir) else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".sosync") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        let created_at = modified
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| {
                chrono::DateTime::<chrono::Utc>::from_timestamp(
                    d.as_secs() as i64,
                    d.subsec_nanos(),
                )
                .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
                .unwrap_or_default()
            })
            .unwrap_or_default();
        out.push((
            created_at.clone(),
            json!({
                "file": name,
                "size_bytes": meta.len(),
                "created_at": created_at,
            }),
        ));
    }
    out.sort_by_key(|(k, _)| std::cmp::Reverse(k.clone()));
    out.into_iter().map(|(_, v)| v).collect()
}

pub fn sync_log(conn: &Connection) -> Vec<Value> {
    if ensure_log_table(conn).is_err() {
        return Vec::new();
    }
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, direction, file_path, size_bytes, conversations, customers, at
         FROM encrypted_sync_log ORDER BY id DESC LIMIT 50",
    ) else {
        return Vec::new();
    };
    let rows = stmt.query_map([], |r| {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "direction": r.get::<_, String>(1)?,
            "file_path": r.get::<_, String>(2)?,
            "size_bytes": r.get::<_, i64>(3)?,
            "conversations": r.get::<_, Option<i64>>(4)?,
            "customers": r.get::<_, Option<i64>>(5)?,
            "at": r.get::<_, String>(6)?,
        }))
    });
    match rows {
        Ok(rows) => rows.flatten().collect(),
        Err(_) => Vec::new(),
    }
}

fn log_sync(
    conn: &Connection,
    direction: &str,
    path: &Path,
    size: u64,
    info: &SyncBundleInfo,
) -> Result<()> {
    if ensure_log_table(conn).is_err() {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO encrypted_sync_log (direction, file_path, size_bytes, conversations, customers, sha256)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            direction,
            path.to_string_lossy(),
            size as i64,
            info.conversations,
            info.customers,
            info.sha256,
        ],
    )?;
    Ok(())
}

fn prune_old_bundles(bundles_dir: &Path, keep: usize) {
    let Ok(entries) = fs::read_dir(bundles_dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".sosync"))
        .filter_map(|e| {
            e.metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| (t, e.path()))
        })
        .collect();
    files.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    for (_, path) in files.iter().skip(keep.max(1)) {
        let _ = fs::remove_file(path);
    }
}
