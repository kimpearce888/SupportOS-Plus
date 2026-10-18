use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde_json::{Map, Value, json};

use crate::error::{Error, Result};

pub const MAX_ROWS_PER_CONNECTOR: usize = 5000;
const MAX_ROW_BYTES: usize = 32_000;
const HTTP_TIMEOUT_MS: u64 = 10_000;
const HTTP_MAX_BYTES: u64 = 10 * 1024 * 1024;

pub fn apply_m038(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS connector_rows (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            connector_id INTEGER NOT NULL REFERENCES connectors (id) ON DELETE CASCADE,
            row_key      TEXT NOT NULL,
            data         TEXT NOT NULL,
            fetched_at   TEXT NOT NULL,
            UNIQUE (connector_id, row_key)
        );
        CREATE INDEX IF NOT EXISTS idx_connector_rows_connector
            ON connector_rows (connector_id);",
    )?;
    let _ = add_column_if_missing(conn, "connectors", "auth", "TEXT NOT NULL DEFAULT '{}'");
    let _ = add_column_if_missing(
        conn,
        "connectors",
        "refresh_method",
        "TEXT NOT NULL DEFAULT 'manual'",
    );
    add_column_if_missing(
        conn,
        "connectors",
        "refresh_seconds",
        "INTEGER NOT NULL DEFAULT 300",
    )?;
    add_column_if_missing(
        conn,
        "connectors",
        "allowed_ai",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(conn, "connectors", "enabled", "INTEGER NOT NULL DEFAULT 1")?;
    add_column_if_missing(conn, "connectors", "schema_json", "TEXT")?;
    add_column_if_missing(conn, "connectors", "last_sync_at", "TEXT")?;
    add_column_if_missing(conn, "connectors", "last_sync_status", "TEXT")?;
    add_column_if_missing(conn, "connectors", "last_sync_error", "TEXT")?;
    add_column_if_missing(conn, "connectors", "last_sync_rows", "INTEGER")?;
    add_column_if_missing(
        conn,
        "connectors",
        "health",
        "TEXT NOT NULL DEFAULT 'never'",
    )?;
    add_column_if_missing(conn, "connectors", "updated_at", "TEXT")?;
    let _ = add_column_if_missing(
        conn,
        "connectors",
        "provenance",
        "TEXT NOT NULL DEFAULT 'local_ui'",
    );
    let _ = conn.execute("UPDATE app_state SET schema_version = 38 WHERE id = 1", []);
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

fn parse_json_obj(text: Option<String>) -> Map<String, Value> {
    text.and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<Value>> {
    let row = conn
        .query_row(
            "SELECT id, name, kind, config_json, auth, refresh_method, refresh_seconds,
                    allowed_ai, enabled, schema_json, last_sync_at, last_sync_status,
                    last_sync_error, last_sync_rows, health, created_at, updated_at, provenance
             FROM connectors WHERE id = ?1",
            params![id],
            hydrate_row,
        )
        .ok();
    Ok(row)
}

fn get_by_name(conn: &Connection, name: &str) -> Result<Option<Value>> {
    let row = conn
        .query_row(
            "SELECT id, name, kind, config_json, auth, refresh_method, refresh_seconds,
                    allowed_ai, enabled, schema_json, last_sync_at, last_sync_status,
                    last_sync_error, last_sync_rows, health, created_at, updated_at, provenance
             FROM connectors WHERE name = ?1",
            params![name],
            hydrate_row,
        )
        .ok();
    Ok(row)
}

fn hydrate_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let config_json: Option<String> = row.get("config_json")?;
    let auth: Option<String> = row.get("auth")?;
    let schema_json: Option<String> = row.get("schema_json")?;
    let schema = schema_json
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .filter(|v| !v.is_null());
    Ok(json!({
        "id": row.get::<_, i64>("id")?,
        "name": row.get::<_, String>("name")?,
        "kind": row.get::<_, String>("kind")?,
        "config": Value::Object(parse_json_obj(config_json)),
        "auth": Value::Object(parse_json_obj(auth)),
        "refresh_method": row.get::<_, String>("refresh_method")?,
        "refresh_seconds": row.get::<_, i64>("refresh_seconds")?,
        "allowed_ai": row.get::<_, i64>("allowed_ai")?,
        "enabled": row.get::<_, i64>("enabled")?,
        "schema_json": schema.unwrap_or(Value::Null),
        "last_sync_at": row.get::<_, Option<String>>("last_sync_at")?,
        "last_sync_status": row.get::<_, Option<String>>("last_sync_status")?,
        "last_sync_error": row.get::<_, Option<String>>("last_sync_error")?,
        "last_sync_rows": row.get::<_, Option<i64>>("last_sync_rows")?,
        "health": row.get::<_, String>("health")?,
        "created_at": row.get::<_, String>("created_at")?,
        "updated_at": row.get::<_, Option<String>>("updated_at")?,
        "provenance": row.get::<_, String>("provenance")?,
    }))
}

pub fn list(conn: &Connection) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, config_json, auth, refresh_method, refresh_seconds,
                allowed_ai, enabled, schema_json, last_sync_at, last_sync_status,
                last_sync_error, last_sync_rows, health, created_at, updated_at, provenance
         FROM connectors ORDER BY name",
    )?;
    let rows = stmt
        .query_map([], hydrate_row)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn redacted_auth(record: &Value) -> Value {
    let mut auth = record
        .get("auth")
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    if auth.get("mode").and_then(|m| m.as_str()) == Some("header") {
        auth.insert("headerValue".into(), json!("••••••"));
    }
    if auth.get("mode").and_then(|m| m.as_str()) == Some("bearer") {
        auth.insert("token".into(), json!("••••••"));
    }
    Value::Object(auth)
}

pub fn redacted(conn: &Connection, record: &Value) -> Value {
    let id = record.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let row_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM connector_rows WHERE connector_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    json!({
        "id": record.get("id"),
        "name": record.get("name"),
        "kind": record.get("kind"),
        "config": record.get("config"),
        "auth": redacted_auth(record),
        "refresh_method": record.get("refresh_method"),
        "refresh_seconds": record.get("refresh_seconds"),
        "allowed_ai": record.get("allowed_ai").and_then(|v| v.as_i64()).unwrap_or(0) == 1,
        "enabled": record.get("enabled").and_then(|v| v.as_i64()).unwrap_or(0) == 1,
        "schema_json": record.get("schema_json"),
        "last_sync_at": record.get("last_sync_at"),
        "last_sync_status": record.get("last_sync_status"),
        "last_sync_error": record.get("last_sync_error"),
        "last_sync_rows": record.get("last_sync_rows"),
        "health": record.get("health"),
        "row_count": row_count,
        "created_at": record.get("created_at"),
        "updated_at": record.get("updated_at"),
    })
}

#[allow(clippy::too_many_arguments)]
pub fn create(
    conn: &Connection,
    name: &str,
    kind: &str,
    config: &Value,
    auth: &Value,
    refresh_method: &str,
    refresh_seconds: i64,
    allowed_ai: bool,
) -> Result<Value> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM connectors WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )
        .ok();
    if exists.is_some() {
        return Err(Error::Config(format!(
            "Connector \"{name}\" already exists"
        )));
    }
    conn.execute(
        "INSERT INTO connectors (name, kind, config_json, auth, refresh_method, refresh_seconds, allowed_ai, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)",
        params![
            name,
            kind,
            serde_json::to_string(config).unwrap_or_else(|_| "{}".into()),
            serde_json::to_string(auth).unwrap_or_else(|_| "{}".into()),
            refresh_method,
            refresh_seconds,
            i64::from(allowed_ai),
        ],
    )?;
    let id = conn.last_insert_rowid();
    get(conn, id)?.ok_or_else(|| Error::Config("connector vanished".into()))
}

pub fn patch(conn: &Connection, id: i64, changes: &Value) -> Result<Option<Value>> {
    let Some(existing) = get(conn, id)? else {
        return Ok(None);
    };
    if let Some(name) = changes.get("name").and_then(|v| v.as_str()) {
        if name != existing.get("name").and_then(|v| v.as_str()).unwrap_or("") {
            let dupe: Option<i64> = conn
                .query_row(
                    "SELECT 1 FROM connectors WHERE name = ?1 AND id != ?2",
                    params![name, id],
                    |r| r.get(0),
                )
                .ok();
            if dupe.is_some() {
                return Err(Error::Config(format!(
                    "Connector \"{name}\" already exists"
                )));
            }
        }
        conn.execute(
            "UPDATE connectors SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
    }
    if let Some(config) = changes.get("config") {
        conn.execute(
            "UPDATE connectors SET config_json = ?1 WHERE id = ?2",
            params![
                serde_json::to_string(config).unwrap_or_else(|_| "{}".into()),
                id
            ],
        )?;
    }
    if let Some(auth) = changes.get("auth") {
        conn.execute(
            "UPDATE connectors SET auth = ?1 WHERE id = ?2",
            params![
                serde_json::to_string(auth).unwrap_or_else(|_| "{}".into()),
                id
            ],
        )?;
    }
    if let Some(rm) = changes.get("refreshMethod").and_then(|v| v.as_str()) {
        conn.execute(
            "UPDATE connectors SET refresh_method = ?1 WHERE id = ?2",
            params![rm, id],
        )?;
    }
    if let Some(rs) = changes.get("refreshSeconds").and_then(|v| v.as_i64()) {
        conn.execute(
            "UPDATE connectors SET refresh_seconds = ?1 WHERE id = ?2",
            params![rs, id],
        )?;
    }
    if let Some(ai) = changes.get("allowedAi").and_then(|v| v.as_bool()) {
        conn.execute(
            "UPDATE connectors SET allowed_ai = ?1 WHERE id = ?2",
            params![i64::from(ai), id],
        )?;
    }
    if let Some(en) = changes.get("enabled").and_then(|v| v.as_bool()) {
        conn.execute(
            "UPDATE connectors SET enabled = ?1 WHERE id = ?2",
            params![i64::from(en), id],
        )?;
    }
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<bool> {
    conn.execute(
        "DELETE FROM connector_rows WHERE connector_id = ?1",
        params![id],
    )?;
    let n = conn.execute("DELETE FROM connectors WHERE id = ?1", params![id])?;
    Ok(n > 0)
}

fn connector_row_json(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let data: String = row.get(3)?;
    Ok(json!({
        "id": row.get::<_, i64>(0)?,
        "connector_id": row.get::<_, i64>(1)?,
        "row_key": row.get::<_, String>(2)?,
        "data": serde_json::from_str::<Value>(&data).unwrap_or(Value::Null),
        "fetched_at": row.get::<_, String>(4)?,
    }))
}

pub fn list_rows(
    conn: &Connection,
    connector_id: i64,
    q: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Value>, i64)> {
    let limit = limit.clamp(1, 200);
    let offset = offset.max(0);
    let like: Option<String> = q.filter(|q| !q.is_empty()).map(|q| format!("%{q}%"));
    let rows: Vec<Value> = match &like {
        Some(like) => {
            let mut stmt = conn.prepare(
                "SELECT id, connector_id, row_key, data, fetched_at FROM connector_rows
                  WHERE connector_id = ?1 AND data LIKE ?2 ORDER BY id LIMIT ?3 OFFSET ?4",
            )?;
            let mapped: Vec<Value> = stmt
                .query_map(
                    params![connector_id, like, limit, offset],
                    connector_row_json,
                )?
                .filter_map(|r| r.ok())
                .collect();
            mapped
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, connector_id, row_key, data, fetched_at FROM connector_rows
                  WHERE connector_id = ?1 ORDER BY id LIMIT ?2 OFFSET ?3",
            )?;
            let mapped: Vec<Value> = stmt
                .query_map(params![connector_id, limit, offset], connector_row_json)?
                .filter_map(|r| r.ok())
                .collect();
            mapped
        }
    };
    let total: i64 = match &like {
        Some(like) => conn.query_row(
            "SELECT COUNT(*) FROM connector_rows WHERE connector_id = ?1 AND data LIKE ?2",
            params![connector_id, like],
            |r| r.get(0),
        )?,
        None => conn.query_row(
            "SELECT COUNT(*) FROM connector_rows WHERE connector_id = ?1",
            params![connector_id],
            |r| r.get(0),
        )?,
    };
    Ok((rows, total))
}

#[derive(Debug, Clone)]
pub struct RefreshResult {
    pub ok: bool,
    pub rows: usize,
    pub pruned: usize,
    pub schema: Option<Value>,
    pub error: Option<String>,
}

pub fn due_for_refresh(conn: &Connection) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT id, enabled, refresh_method, refresh_seconds, last_sync_at FROM connectors",
    )?;
    let rows: Vec<(i64, i64, String, i64, Option<String>)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok(rows
        .into_iter()
        .filter(|(_, enabled, method, seconds, last_sync_at)| {
            if *enabled == 0 || method != "interval" {
                return false;
            }
            match last_sync_at {
                None => true,
                Some(at) => match parse_sqlite_ts(at) {
                    Some(ts) => now - ts >= *seconds,
                    None => true,
                },
            }
        })
        .map(|(id, ..)| id)
        .collect())
}

fn parse_sqlite_ts(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let mo: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    let h: i64 = s.get(11..13)?.parse().ok()?;
    let mi: i64 = s.get(14..16)?.parse().ok()?;
    let se: i64 = s.get(17..19)?.parse().ok()?;
    let yy = if mo <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + se)
}

fn sqlite_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

pub async fn refresh(
    conn: &crate::db::DbHandle,
    connector_id: i64,
    data_dir: &Path,
) -> std::result::Result<RefreshResult, Error> {
    let record = {
        let c = conn.write_guard();
        get(&c, connector_id)?
    };
    let Some(connector) = record else {
        return Ok(RefreshResult {
            ok: false,
            rows: 0,
            pruned: 0,
            schema: None,
            error: Some("connector not found".into()),
        });
    };
    match refresh_inner(conn, &connector, data_dir).await {
        Ok(result) => Ok(result),
        Err(e) => {
            let message = e.to_string().chars().take(500).collect::<String>();
            let c = conn.write_guard();
            record_sync(&c, connector_id, "error", None, Some(&message), None)?;
            Ok(RefreshResult {
                ok: false,
                rows: 0,
                pruned: 0,
                schema: None,
                error: Some(message),
            })
        }
    }
}

async fn refresh_inner(
    conn: &crate::db::DbHandle,
    connector: &Value,
    data_dir: &Path,
) -> Result<RefreshResult> {
    let rows = fetch_rows(connector, data_dir).await?;
    let fetched_at = sqlite_now();
    let schema = infer_schema(&rows);
    let connector_id = connector.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let keys: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
    let pruned = {
        let mut c = conn.write_guard();
        let tx = c.transaction()?;
        for (row_key, data) in &rows {
            tx.execute(
                "INSERT INTO connector_rows (connector_id, row_key, data, fetched_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (connector_id, row_key) DO UPDATE SET
                    data = excluded.data, fetched_at = excluded.fetched_at",
                params![
                    connector_id,
                    row_key,
                    serde_json::to_string(data).unwrap_or_else(|_| "{}".into()),
                    fetched_at,
                ],
            )?;
        }
        let mut pruned = 0usize;
        {
            let mut stmt =
                tx.prepare("SELECT row_key FROM connector_rows WHERE connector_id = ?1")?;
            let existing: Vec<String> = stmt
                .query_map(params![connector_id], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            drop(stmt);
            for key in existing {
                if !keys.contains(&key) {
                    pruned += tx.execute(
                        "DELETE FROM connector_rows WHERE connector_id = ?1 AND row_key = ?2",
                        params![connector_id, key],
                    )?;
                }
            }
        }
        let schema_json = schema
            .as_ref()
            .map(|s| serde_json::to_string(s).unwrap_or_default());
        tx.execute(
            "UPDATE connectors SET last_sync_at = ?1, last_sync_status = 'ok',
                last_sync_error = NULL, last_sync_rows = ?2, health = 'ok',
                schema_json = COALESCE(?3, schema_json), updated_at = ?1
              WHERE id = ?4",
            params![fetched_at, rows.len() as i64, schema_json, connector_id],
        )?;
        tx.commit()?;
        pruned
    };
    Ok(RefreshResult {
        ok: true,
        rows: rows.len(),
        pruned,
        schema,
        error: None,
    })
}

fn record_sync(
    conn: &Connection,
    id: i64,
    status: &str,
    rows: Option<usize>,
    error: Option<&str>,
    schema: Option<&Value>,
) -> Result<()> {
    conn.execute(
        "UPDATE connectors SET last_sync_at = ?1, last_sync_status = ?2, last_sync_error = ?3,
            last_sync_rows = ?4, health = ?5,
            schema_json = COALESCE(?6, schema_json), updated_at = ?1
          WHERE id = ?7",
        params![
            sqlite_now(),
            status,
            error,
            rows.map(|n| n as i64),
            status,
            schema.map(|s| serde_json::to_string(s).unwrap_or_default()),
            id,
        ],
    )?;
    Ok(())
}

fn connectors_root(data_dir: &Path) -> PathBuf {
    data_dir.join("connectors")
}

fn jail_resolve(data_dir: &Path, file_name: &str) -> Result<PathBuf> {
    let root = connectors_root(data_dir);
    let abs = root.join(file_name);
    let abs = abs.clean_path();
    if abs != root && !abs.starts_with(&root) {
        return Err(Error::Other(
            "File must live inside the connectors folder".into(),
        ));
    }
    Ok(abs)
}

trait CleanPath {
    fn clean_path(&self) -> PathBuf;
}
impl CleanPath for Path {
    fn clean_path(&self) -> PathBuf {
        let mut out = PathBuf::new();
        for comp in self.components() {
            match comp {
                std::path::Component::ParentDir => {
                    out.pop();
                }
                std::path::Component::CurDir => {}
                other => out.push(other),
            }
        }
        out
    }
}

pub async fn validate_config(
    data_dir: &Path,
    kind: &str,
    file: Option<&str>,
    url: Option<&str>,
) -> Option<String> {
    if kind == "http" {
        let url = url.unwrap_or("");
        return match crate::security::validate_ssrf_full(url) {
            Ok(()) => None,
            Err(e) => Some(format!("URL refused: {e}")),
        };
    }
    let file = file.unwrap_or("");
    match jail_resolve(data_dir, file) {
        Ok(abs) => {
            if !abs.exists() {
                Some(format!(
                    "File not found: create \"connectors/{file}\" first (the connectors folder is {})",
                    connectors_root(data_dir).display()
                ))
            } else {
                None
            }
        }
        Err(e) => Some(e.to_string()),
    }
}

type NormalizedRow = (String, Map<String, Value>);

async fn fetch_rows(connector: &Value, data_dir: &Path) -> Result<Vec<NormalizedRow>> {
    let kind = connector.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let config = connector.get("config").cloned().unwrap_or(Value::Null);
    let key_column = config
        .get("keyColumn")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    match kind {
        "local_json" => {
            let file = config.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let abs = jail_resolve(data_dir, file)?;
            let stat =
                std::fs::metadata(&abs).map_err(|e| Error::Config(format!("file error: {e}")))?;
            if stat.len() > HTTP_MAX_BYTES {
                return Err(Error::Config(format!(
                    "file too large ({}MB > 10MB cap)",
                    stat.len() / 1024 / 1024
                )));
            }
            let text = std::fs::read_to_string(&abs)
                .map_err(|e| Error::Config(format!("read error: {e}")))?;
            let payload: Value = serde_json::from_str(&text)
                .map_err(|e| Error::Config(format!("invalid JSON: {e}")))?;
            normalize_rows(payload, key_column.as_deref(), "the JSON root")
        }
        "csv" => {
            let file = config.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let abs = jail_resolve(data_dir, file)?;
            let stat =
                std::fs::metadata(&abs).map_err(|e| Error::Config(format!("file error: {e}")))?;
            if stat.len() > HTTP_MAX_BYTES {
                return Err(Error::Config(format!(
                    "file too large ({}MB > 10MB cap)",
                    stat.len() / 1024 / 1024
                )));
            }
            let text = std::fs::read_to_string(&abs)
                .map_err(|e| Error::Config(format!("read error: {e}")))?;
            let rows = parse_csv(&text, MAX_ROWS_PER_CONNECTOR);
            normalize_rows(Value::Array(rows), key_column.as_deref(), "CSV rows")
        }
        "sqlite" => {
            let file = config.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let table = config.get("table").and_then(|v| v.as_str()).unwrap_or("");
            let abs = jail_resolve(data_dir, file)?;
            let valid = !table.is_empty()
                && table
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic())
                && table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !valid {
                return Err(Error::Config("invalid table name".into()));
            }
            let sqlite =
                Connection::open_with_flags(&abs, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                    .map_err(|e| Error::Config(format!("sqlite open: {e}")))?;
            let count: i64 = sqlite
                .query_row(&format!("SELECT COUNT(*) FROM \"{table}\""), [], |r| {
                    r.get(0)
                })
                .map_err(|e| Error::Config(format!("sqlite read: {e}")))?;
            let limit = count.min(MAX_ROWS_PER_CONNECTOR as i64);
            let mut stmt = sqlite.prepare(&format!("SELECT * FROM \"{table}\" LIMIT {limit}"))?;
            let col_count = stmt.column_count();
            let names: Vec<String> = (0..col_count)
                .filter_map(|i| stmt.column_name(i).ok().map(str::to_string))
                .collect();
            let rows: Vec<Value> = stmt
                .query_map([], |row| {
                    let mut obj = Map::new();
                    for (i, name) in names.iter().enumerate() {
                        let v: Value = match row.get_ref(i) {
                            Ok(rusqlite::types::ValueRef::Null) => Value::Null,
                            Ok(rusqlite::types::ValueRef::Integer(n)) => json!(n),
                            Ok(rusqlite::types::ValueRef::Real(f)) => json!(f),
                            Ok(rusqlite::types::ValueRef::Text(t)) => {
                                json!(String::from_utf8_lossy(t))
                            }
                            Ok(rusqlite::types::ValueRef::Blob(b)) => {
                                json!(String::from_utf8_lossy(b))
                            }
                            Err(_) => Value::Null,
                        };
                        obj.insert(name.clone(), v);
                    }
                    Ok(Value::Object(obj))
                })?
                .filter_map(|r| r.ok())
                .collect();
            normalize_rows(Value::Array(rows), key_column.as_deref(), "table rows")
        }
        "http" => {
            let url = config.get("url").and_then(|v| v.as_str()).unwrap_or("");
            crate::security::validate_ssrf_full(url)?;
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| Error::Config(format!("http client: {e}")))?;
            let mut request = client
                .get(url)
                .header("Accept", "application/json, text/csv, text/plain")
                .timeout(std::time::Duration::from_millis(HTTP_TIMEOUT_MS));
            let auth = connector.get("auth").cloned().unwrap_or(Value::Null);
            let mode = auth.get("mode").and_then(|v| v.as_str()).unwrap_or("");
            if mode == "header"
                && let (Some(name), Some(value)) = (
                    auth.get("headerName").and_then(|v| v.as_str()),
                    auth.get("headerValue").and_then(|v| v.as_str()),
                )
            {
                request = request.header(name, value);
            }
            if mode == "bearer"
                && let Some(token) = auth.get("token").and_then(|v| v.as_str())
            {
                request = request.header("Authorization", format!("Bearer {token}"));
            }
            let response = request
                .send()
                .await
                .map_err(|e| Error::Config(format!("http: {e}")))?;
            if !response.status().is_success() {
                return Err(Error::Config(format!(
                    "HTTP {}",
                    response.status().as_u16()
                )));
            }
            if let Some(len) = response.content_length()
                && len > HTTP_MAX_BYTES
            {
                return Err(Error::Config(format!(
                    "response too large ({}MB > 10MB cap)",
                    len / 1024 / 1024
                )));
            }
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_lowercase();
            let mut body: Vec<u8> = Vec::new();
            let mut stream = response;
            while let Some(chunk) = stream
                .chunk()
                .await
                .map_err(|e| Error::Config(format!("read: {e}")))?
            {
                if body.len() + chunk.len() > HTTP_MAX_BYTES as usize {
                    return Err(Error::Config(
                        "response exceeded the 10MB cap mid-stream".into(),
                    ));
                }
                body.extend_from_slice(&chunk);
            }
            let body_text = String::from_utf8_lossy(&body).to_string();
            let trimmed = body_text.trim_start();
            if content_type.contains("json") || trimmed.starts_with('{') || trimmed.starts_with('[')
            {
                let payload: Value = serde_json::from_str(&body_text)
                    .map_err(|e| Error::Config(format!("invalid JSON: {e}")))?;
                normalize_rows(payload, key_column.as_deref(), "the JSON response")
            } else if content_type.contains("csv") || content_type.contains("plain") {
                let rows = parse_csv(&body_text, MAX_ROWS_PER_CONNECTOR);
                normalize_rows(Value::Array(rows), key_column.as_deref(), "CSV rows")
            } else {
                Err(Error::Config(format!(
                    "unsupported content type: {}",
                    if content_type.is_empty() {
                        "(none)"
                    } else {
                        &content_type
                    }
                )))
            }
        }
        _ => Err(Error::Config(format!(
            "unsupported connector kind: {}",
            if kind.is_empty() { "unknown" } else { kind }
        ))),
    }
}

pub fn parse_csv(text: &str, max_rows: usize) -> Vec<Value> {
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut field = String::new();
    let mut record: Vec<String> = Vec::new();
    let mut in_quotes = false;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if in_quotes {
            if ch == '"' {
                if chars.get(i + 1) == Some(&'"') {
                    field.push('"');
                    i += 1;
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(ch);
            }
            i += 1;
            continue;
        }
        match ch {
            '"' => in_quotes = true,
            ',' => {
                record.push(std::mem::take(&mut field));
            }
            '\r' | '\n' => {
                if ch == '\r' && chars.get(i + 1) == Some(&'\n') {
                    i += 1;
                }
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
                if records.len() > max_rows {
                    break;
                }
            }
            _ => field.push(ch),
        }
        i += 1;
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    if let Some(last) = records.last()
        && last.len() == 1
        && last[0].is_empty()
    {
        records.pop();
    }
    let Some(headers) = records.first() else {
        return Vec::new();
    };
    let headers: Vec<&str> = headers
        .iter()
        .map(|h| h.trim())
        .filter(|h| !h.is_empty())
        .collect();
    let mut rows = Vec::new();
    for rec in records.iter().take(max_rows + 1).skip(1) {
        let mut obj = Map::new();
        for (idx, h) in headers.iter().enumerate() {
            obj.insert(
                (*h).to_string(),
                json!(rec.get(idx).map(String::as_str).unwrap_or("")),
            );
        }
        if !obj.is_empty() {
            rows.push(Value::Object(obj));
        }
    }
    rows
}

fn stable_key(data: &Map<String, Value>) -> String {
    let json = serde_json::to_string(data).unwrap_or_default();
    let mut h1: u32 = 0x811c_9dc5;
    for b in json.as_bytes() {
        h1 ^= u32::from(*b);
        h1 = h1.wrapping_mul(0x0100_0193);
    }
    format!("h{:x}-{}", h1, json.len())
}

fn normalize_rows(
    payload: Value,
    key_column: Option<&str>,
    source_label: &str,
) -> Result<Vec<NormalizedRow>> {
    let list: Vec<Value> = match &payload {
        Value::Array(items) => items.clone(),
        Value::Object(obj) => {
            if let Some(Value::Array(items)) = obj.get("data") {
                items.clone()
            } else if let Some(Value::Array(items)) = obj.get("rows") {
                items.clone()
            } else {
                vec![payload.clone()]
            }
        }
        _ => {
            return Err(Error::Config(format!(
                "{source_label} must be an array of objects (or {{ data: [...] }} / {{ rows: [...] }})"
            )));
        }
    };
    let list: Vec<Value> = list.into_iter().take(MAX_ROWS_PER_CONNECTOR).collect();
    let mut out = Vec::new();
    for item in list {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let mut data = Map::new();
        for (k, v) in obj {
            if v.is_null() {
                continue;
            }
            data.insert(k.clone(), v.clone());
        }
        let json = serde_json::to_string(&data).unwrap_or_default();
        if json.len() > MAX_ROW_BYTES {
            continue;
        }
        let row_key = key_column
            .and_then(|kc| data.get(kc))
            .map(|v| match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_else(|| stable_key(&data));
        out.push((row_key, data));
    }
    Ok(out)
}

fn guess_type(values: &[&Value]) -> &'static str {
    let non_null: Vec<&&Value> = values
        .iter()
        .filter(|v| !v.is_null() && !matches!(**v, Value::String(s) if s.is_empty()))
        .collect();
    if non_null.is_empty() {
        return "text";
    }
    if non_null.iter().all(|v| v.is_number()) {
        return "number";
    }
    if non_null.iter().all(|v| v.is_boolean()) {
        return "boolean";
    }
    "text"
}

fn infer_schema(rows: &[NormalizedRow]) -> Option<Value> {
    if rows.is_empty() {
        return None;
    }
    let mut columns: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (_, data) in rows {
        for (k, v) in data {
            columns.entry(k.clone()).or_default().push(v.clone());
        }
    }
    let schema: Vec<Value> = columns
        .iter()
        .take(60)
        .map(|(name, values)| {
            let refs: Vec<&Value> = values.iter().collect();
            json!({ "name": name, "type": guess_type(&refs) })
        })
        .collect();
    Some(Value::Array(schema))
}

pub fn search_for_ai(
    conn: &Connection,
    connector_name_or_id: &str,
    query: &str,
    limit: usize,
) -> Value {
    let record = match connector_name_or_id.parse::<i64>() {
        Ok(id) => get(conn, id).ok().flatten(),
        Err(_) => get_by_name(conn, connector_name_or_id).ok().flatten(),
    };
    let Some(connector) = record else {
        return json!({ "error": "Connector not found" });
    };
    let allowed = connector
        .get("allowed_ai")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if allowed == 0 {
        let name = connector.get("name").and_then(|v| v.as_str()).unwrap_or("");
        return json!({ "error": format!("Connector \"{name}\" is not marked as AI-visible. Data stays private to the UI.") });
    }
    let enabled = connector
        .get("enabled")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if enabled == 0 {
        let name = connector.get("name").and_then(|v| v.as_str()).unwrap_or("");
        return json!({ "error": format!("Connector \"{name}\" is disabled.") });
    }
    let q = query.to_lowercase();
    let limit = limit.clamp(1, 10);
    let id = connector.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let (rows, _) = list_rows(conn, id, None, limit as i64, 0).unwrap_or_default();
    let results: Vec<Value> = rows
        .into_iter()
        .filter(|r| {
            if q.is_empty() {
                return true;
            }
            r.get("data")
                .and_then(|d| serde_json::to_string(&d).ok())
                .map(|s| s.to_lowercase().contains(&q))
                .unwrap_or(false)
        })
        .take(limit)
        .map(|r| {
            let mut out = Map::new();
            if let Some(data) = r.get("data").and_then(|d| d.as_object()) {
                for (k, v) in data {
                    let redacted = match v {
                        Value::String(s) => {
                            let sliced: String = s.chars().take(300).collect();
                            let (text, _) = crate::security::redact_text(&sliced, true);
                            Value::String(text)
                        }
                        other => other.clone(),
                    };
                    out.insert(k.clone(), redacted);
                }
            }
            Value::Object(out)
        })
        .collect();
    let name = connector.get("name").and_then(|v| v.as_str()).unwrap_or("");
    json!({ "connector": name, "results": results })
}

pub fn ai_visible_connector_names(conn: &Connection) -> Vec<String> {
    let mut stmt = match conn
        .prepare("SELECT name FROM connectors WHERE allowed_ai = 1 AND enabled = 1 ORDER BY name")
    {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    stmt.query_map([], |r| r.get(0))
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}
