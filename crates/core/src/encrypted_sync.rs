use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::business_hours::now_iso;
use crate::crypto;
use crate::error::Result;

pub const MAGIC: &[u8; 6] = b"SOSYNC";
pub const FORMAT_VERSION: u32 = 1;
const SCRYPT_N: u32 = 1 << 15;
const LOG_N: u8 = SCRYPT_N.trailing_zeros() as u8;
const SCRYPT_N_MIN: u32 = 1 << 14;
const SCRYPT_N_MAX: u32 = 1 << 25;
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

struct SnapshotTempGuard<'a> {
    path: &'a Path,
    armed: bool,
}

impl SnapshotTempGuard<'_> {
    fn defuse(&mut self) {
        self.armed = false;
    }
}

impl Drop for SnapshotTempGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(self.path);
        }
    }
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
    crypto::restrict_permissions(&snapshot_path);
    let mut snapshot_guard = SnapshotTempGuard {
        path: &snapshot_path,
        armed: true,
    };

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
    crypto::restrict_permissions(&target);
    snapshot_guard.defuse();

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
    sweep_stale_import_temps(bundles_dir);
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
    if header.kdf.n < SCRYPT_N_MIN || header.kdf.n > SCRYPT_N_MAX || !header.kdf.n.is_power_of_two()
    {
        return Err(format!(
            "Bundle declares an out-of-range scrypt work factor (N = {}).",
            header.kdf.n
        ));
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
    crypto::restrict_permissions(&temp_path);
    Ok(temp_path)
}

fn sweep_stale_import_temps(bundles_dir: &Path) {
    const STALE_AFTER_MS: i64 = 60 * 60 * 1000;
    const SNAPSHOT_STALE_AFTER_MS: i64 = 24 * 60 * 60 * 1000;
    let Ok(entries) = fs::read_dir(bundles_dir) else {
        return;
    };
    let cutoff = chrono::Utc::now().timestamp_millis() - STALE_AFTER_MS;
    let snapshot_cutoff = chrono::Utc::now().timestamp_millis() - SNAPSHOT_STALE_AFTER_MS;
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if let Some(stem) = name
            .strip_prefix(".import-")
            .and_then(|s| s.strip_suffix(".tmp"))
        {
            let Ok(created_ms) = stem.parse::<i64>() else {
                continue;
            };
            if created_ms < cutoff {
                let _ = fs::remove_file(entry.path());
            }
        } else if name.starts_with(".snapshot-") && name.ends_with(".tmp") {
            let modified_ms = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64);
            if modified_ms.is_some_and(|ms| ms < snapshot_cutoff) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
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
        let check = check_schema_compatibility(conn, &snap);
        drop(snap);
        let check = check?;
        Ok(match check {
            SchemaCheck::Incompatible { bundle, local } => json!({
                "ok": false,
                "message": format!(
                    "The bundle was created by an incompatible database schema (bundle fingerprint {}, local {}). Importing it could corrupt your data - nothing was changed.",
                    fp_short(&bundle),
                    fp_short(&local)
                )
            }),
            SchemaCheck::CompatibleAncestor => json!({
                "ok": true,
                "message": format!(
                    "Bundle verified: {} conversations, {} customers, schema {} — an older compatible schema that will be brought up to date on restart.",
                    info.conversations, info.customers, schema
                ),
                "info": info
            }),
            SchemaCheck::Identical => json!({
                "ok": true,
                "message": format!(
                    "Bundle verified: {} conversations, {} customers, schema {}.",
                    info.conversations, info.customers, schema
                ),
                "info": info
            }),
        })
    })();
    let _ = fs::remove_file(&temp);
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

    let db_str = db_path.to_string_lossy();
    let restore_tmp = format!("{db_str}.restore-tmp");
    let prev = format!("{db_str}.restore-prev");
    if fs::copy(&temp, &restore_tmp).is_err() {
        let _ = fs::remove_file(&temp);
        return ImportResult {
            ok: false,
            message: "Import failed: could not stage the restored database.".into(),
            require_restart: None,
        };
    }
    crypto::restrict_permissions(Path::new(&restore_tmp));
    if fs::File::open(&restore_tmp)
        .and_then(|f| f.sync_all())
        .is_err()
    {
        let _ = fs::remove_file(&temp);
        let _ = fs::remove_file(&restore_tmp);
        return ImportResult {
            ok: false,
            message: "Import failed: could not flush the staged database.".into(),
            require_restart: None,
        };
    }
    let _ = fs::remove_file(&temp);
    let _ = fs::remove_file(&prev);
    let _ = fs::remove_file(format!("{prev}-wal"));
    let _ = fs::remove_file(format!("{prev}-shm"));
    if db_path.exists() && fs::rename(db_path, &prev).is_err() {
        let _ = fs::remove_file(&restore_tmp);
        return ImportResult {
            ok: false,
            message: "Import failed: could not set the live database aside.".into(),
            require_restart: None,
        };
    }
    for ext in ["-wal", "-shm"] {
        let live = format!("{db_str}{ext}");
        if Path::new(&live).exists() {
            let _ = fs::rename(&live, format!("{prev}{ext}"));
        }
    }
    if fs::rename(&restore_tmp, db_path).is_err() {
        if Path::new(&prev).exists() {
            let _ = fs::rename(&prev, db_path);
        }
        for ext in ["-wal", "-shm"] {
            let side = format!("{prev}{ext}");
            if Path::new(&side).exists() {
                let _ = fs::rename(&side, format!("{db_str}{ext}"));
            }
        }
        return ImportResult {
            ok: false,
            message: "Import failed: could not swap the database into place.".into(),
            require_restart: None,
        };
    }
    let _ = fs::File::open(db_path).and_then(|f| f.sync_all());
    if let Some(parent) = db_path.parent() {
        let _ = fs::File::open(parent).and_then(|d| d.sync_all());
    }
    let _ = fs::remove_file(&prev);
    let _ = fs::remove_file(format!("{prev}-wal"));
    let _ = fs::remove_file(format!("{prev}-shm"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spp-sync-test-{tag}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn schema_db(path: &Path) -> Connection {
        let conn = Connection::open(path).expect("file db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS widget (id INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE IF NOT EXISTS conversations (id INTEGER PRIMARY KEY);
             CREATE TABLE IF NOT EXISTS customers (id INTEGER PRIMARY KEY);
             CREATE TABLE IF NOT EXISTS application_settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
             );",
        )
        .expect("schema");
        conn
    }

    #[test]
    fn export_rejects_short_passphrase() {
        let dir = temp_dir("short");
        let conn = Connection::open_in_memory().expect("memory db");
        let result = export_bundle(&conn, &dir, "short");
        assert!(!result.ok);
        assert!(result.message.contains("at least 8 characters"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_then_verify_round_trip() {
        let dir = temp_dir("roundtrip");
        let src = dir.join("a.db");
        let conn = schema_db(&src);
        conn.execute("INSERT INTO widget (name) VALUES ('alpha')", [])
            .expect("seed");

        let exported = export_bundle(&conn, &dir, "correct horse battery");
        assert!(exported.ok, "export failed: {}", exported.message);
        let bundle = PathBuf::from(exported.path.expect("bundle path"));
        assert!(bundle.exists());

        let raw = fs::read(&bundle).expect("bundle bytes");
        assert!(raw.starts_with(MAGIC));
        assert!(
            !raw.windows(16)
                .any(|w| w == b"SQLite format 3\0".as_slice())
        );

        let ok = verify_bundle(&conn, &dir, &bundle, "correct horse battery");
        assert_eq!(ok["ok"], json!(true), "verify: {}", ok["message"]);

        let wrong = verify_bundle(&conn, &dir, &bundle, "a wrong passphrase");
        assert_eq!(wrong["ok"], json!(false));
        assert!(
            wrong["message"]
                .as_str()
                .unwrap()
                .contains("Wrong passphrase")
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_rejects_tampered_ciphertext() {
        let dir = temp_dir("tamper");
        let src = dir.join("a.db");
        let conn = schema_db(&src);
        let exported = export_bundle(&conn, &dir, "passphrase-ok");
        assert!(exported.ok, "export failed: {}", exported.message);
        let bundle = PathBuf::from(exported.path.expect("bundle path"));

        let mut raw = fs::read(&bundle).expect("bundle bytes");
        let last = raw.len() - 1;
        raw[last] ^= 0xFF;
        fs::write(&bundle, raw).expect("rewrite tampered");

        let result = verify_bundle(&conn, &dir, &bundle, "passphrase-ok");
        assert_eq!(result["ok"], json!(false));
        assert!(result["message"].as_str().unwrap().contains("modified"));
        let _ = fs::remove_dir_all(&dir);
    }

    fn craft_bundle_with_plaintext(dir: &Path, passphrase: &str, plain: &[u8], n: u32) -> PathBuf {
        let salt = crypto::random_salt();
        let key = derive_key(passphrase, &salt).expect("key");
        let (iv, ciphertext) = crypto::encrypt(&key, plain).expect("encrypt");
        let header = BundleHeader {
            v: FORMAT_VERSION,
            kdf: KdfParams {
                name: "scrypt".into(),
                n,
                r: 8,
                p: 1,
                salt: B64.encode(salt),
            },
            cipher: "aes-256-gcm".into(),
            iv: B64.encode(iv),
            created_at: now_iso(),
            app_version: None,
            conversations: None,
            customers: None,
            sha256: None,
            schema_fingerprint: None,
        };
        let header_bytes = serde_json::to_vec(&header).expect("header");
        let mut out = Vec::with_capacity(MAGIC.len() + 4 + header_bytes.len() + ciphertext.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&(header_bytes.len() as u32).to_be_bytes());
        out.extend_from_slice(&header_bytes);
        out.extend_from_slice(&ciphertext);
        let path = dir.join("crafted.sosync");
        fs::write(&path, out).expect("write bundle");
        path
    }

    fn import_temps(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .expect("read dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(".import-") && n.ends_with(".tmp"))
            .collect()
    }

    #[test]
    fn audit48_verify_error_path_removes_the_plaintext_temp() {
        let dir = temp_dir("leak");
        let conn = Connection::open_in_memory().expect("memory db");
        let bundle = craft_bundle_with_plaintext(
            &dir,
            "passphrase-ok",
            b"this is definitely not a sqlite database",
            SCRYPT_N,
        );
        let result = verify_bundle(&conn, &dir, &bundle, "passphrase-ok");
        assert_eq!(result["ok"], json!(false), "verify must fail on garbage");
        assert!(
            import_temps(&dir).is_empty(),
            "the decrypted plaintext temp must be removed on error paths"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit48_verify_success_path_leaves_no_temp() {
        let dir = temp_dir("clean");
        let src = dir.join("a.db");
        let conn = schema_db(&src);
        let exported = export_bundle(&conn, &dir, "passphrase-ok");
        assert!(exported.ok, "export failed: {}", exported.message);
        let bundle = PathBuf::from(exported.path.expect("bundle path"));
        let ok = verify_bundle(&conn, &dir, &bundle, "passphrase-ok");
        assert_eq!(ok["ok"], json!(true), "verify: {}", ok["message"]);
        assert!(
            import_temps(&dir).is_empty(),
            "no plaintext temp survives a successful verify"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit48_sweep_removes_only_stale_temps() {
        let dir = temp_dir("sweep");
        let now = chrono::Utc::now().timestamp_millis();
        let stale = dir.join(format!(".import-{}.tmp", now - 2 * 60 * 60 * 1000));
        let fresh = dir.join(format!(".import-{}.tmp", now));
        fs::write(&stale, b"stale plaintext").expect("stale");
        fs::write(&fresh, b"fresh plaintext").expect("fresh");
        sweep_stale_import_temps(&dir);
        assert!(!stale.exists(), "the two-hour-old temp is swept");
        assert!(fresh.exists(), "the fresh temp survives the sweep");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_m11_sweep_removes_stale_export_snapshots() {
        let dir = temp_dir("snap-sweep");
        let stale = dir.join(".snapshot-2020-01-01T00-00-00-000Z.tmp");
        let fresh = dir.join(".snapshot-latest.tmp");
        fs::write(&stale, b"stale plaintext db").expect("stale");
        fs::write(&fresh, b"fresh plaintext db").expect("fresh");
        let two_days_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 86400);
        fs::File::options()
            .write(true)
            .open(&stale)
            .expect("open stale")
            .set_modified(two_days_ago)
            .expect("mtime");
        sweep_stale_import_temps(&dir);
        assert!(!stale.exists(), "the two-day-old snapshot is swept");
        assert!(fresh.exists(), "the fresh snapshot survives the sweep");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn audit_m15_rejects_out_of_range_header_scrypt_n() {
        let dir = temp_dir("m15");
        let conn = Connection::open_in_memory().expect("memory db");
        let bundle = craft_bundle_with_plaintext(&dir, "passphrase-ok", b"x", 1 << 31);
        let result = verify_bundle(&conn, &dir, &bundle, "passphrase-ok");
        assert_eq!(result["ok"], json!(false));
        let msg = result["message"].as_str().unwrap();
        assert!(
            msg.contains("scrypt work factor"),
            "must reject the work factor honestly, got: {msg}"
        );
        assert!(import_temps(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_round_trip_restores_data_with_safety_backup() {
        let dir = temp_dir("import");
        let src = dir.join("a.db");
        let conn_a = schema_db(&src);
        for name in ["alpha", "beta"] {
            conn_a
                .execute("INSERT INTO widget (name) VALUES (?1)", [name])
                .expect("seed a");
        }
        let exported = export_bundle(&conn_a, &dir, "passphrase-ok");
        assert!(exported.ok, "export failed: {}", exported.message);
        let bundle = PathBuf::from(exported.path.expect("bundle path"));

        let dst = dir.join("b.db");
        let conn_b = schema_db(&dst);
        conn_b
            .execute("INSERT INTO widget (name) VALUES ('local-only')", [])
            .expect("seed b");

        let imported = import_bundle(&conn_b, &dst, &dir, &bundle, "passphrase-ok");
        assert!(imported.ok, "import failed: {}", imported.message);
        assert_eq!(imported.require_restart, Some(true));

        let check = Connection::open(&dst).expect("reopened db");
        let names: Vec<String> = {
            let mut stmt = check
                .prepare("SELECT name FROM widget ORDER BY name")
                .expect("select");
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .expect("query map");
            rows.flatten().collect()
        };
        assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);

        let has_safety_backup = fs::read_dir(&dir)
            .expect("read dir")
            .flatten()
            .any(|e| e.file_name().to_string_lossy().contains(".pre-import-"));
        assert!(has_safety_backup);

        let _ = fs::remove_dir_all(&dir);
    }
}
