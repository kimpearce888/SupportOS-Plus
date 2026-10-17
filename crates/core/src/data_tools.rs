use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::catalog::{ConnectorAuthMode, ConnectorKind, CustomFieldType};
use crate::error::{Error, Result};

pub const M026_TO_M027_SQL: &str = r#"
    -- M026: custom_object_types + custom_object_fields
    CREATE TABLE IF NOT EXISTS custom_object_types (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        name        TEXT NOT NULL,
        slug        TEXT NOT NULL UNIQUE,
        created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE TABLE IF NOT EXISTS custom_object_fields (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        type_id         INTEGER NOT NULL REFERENCES custom_object_types (id) ON DELETE CASCADE,
        name            TEXT NOT NULL,
        field_type      TEXT NOT NULL,
        required        INTEGER NOT NULL DEFAULT 0,
        options_json    TEXT
    );
    CREATE INDEX IF NOT EXISTS idx_custom_object_fields_type
        ON custom_object_fields (type_id);

    -- M027: connectors
    CREATE TABLE IF NOT EXISTS connectors (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        name            TEXT NOT NULL,
        kind            TEXT NOT NULL,
        config_json     TEXT,
        auth_mode       TEXT NOT NULL DEFAULT 'none',
        created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    UPDATE app_state SET schema_version = 27 WHERE id = 1;
"#;

pub fn apply_m026_to_m027(conn: &Connection) -> Result<()> {
    conn.execute_batch(M026_TO_M027_SQL)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomObjectType {
    pub id: Option<i64>,
    pub name: String,
    pub slug: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomObjectField {
    pub id: Option<i64>,
    pub type_id: i64,
    pub name: String,
    pub field_type: String,
    pub required: bool,
    pub options: Option<String>,
}

pub fn create_object_type(conn: &Connection, name: &str, slug: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO custom_object_types (name, slug) VALUES (?1, ?2)",
        params![name, slug],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn add_object_field(
    conn: &Connection,
    type_id: i64,
    name: &str,
    field_type: CustomFieldType,
    required: bool,
    options: Option<&str>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO custom_object_fields (type_id, name, field_type, required, options_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            type_id,
            name,
            field_type_as_str(field_type),
            required as i64,
            options
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_object_types(conn: &Connection) -> Result<Vec<CustomObjectType>> {
    let mut stmt = conn
        .prepare("SELECT id, name, slug, created_at FROM custom_object_types ORDER BY id DESC")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(CustomObjectType {
                id: r.get(0)?,
                name: r.get(1)?,
                slug: r.get(2)?,
                created_at: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn list_object_fields(conn: &Connection, type_id: i64) -> Result<Vec<CustomObjectField>> {
    let mut stmt = conn.prepare(
        "SELECT id, type_id, name, field_type, required, options_json
         FROM custom_object_fields WHERE type_id = ?1 ORDER BY id ASC",
    )?;
    let rows = stmt
        .query_map(params![type_id], |r| {
            Ok(CustomObjectField {
                id: r.get(0)?,
                type_id: r.get(1)?,
                name: r.get(2)?,
                field_type: r.get(3)?,
                required: r.get::<_, i64>(4)? != 0,
                options: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn delete_object_type(conn: &Connection, id: i64) -> Result<bool> {
    let rows = conn.execute("DELETE FROM custom_object_types WHERE id = ?1", params![id])?;
    Ok(rows > 0)
}

fn field_type_as_str(ft: CustomFieldType) -> &'static str {
    match ft {
        CustomFieldType::Text => "text",
        CustomFieldType::LongText => "long_text",
        CustomFieldType::Number => "number",
        CustomFieldType::Date => "date",
        CustomFieldType::Boolean => "boolean",
        CustomFieldType::Select => "select",
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connector {
    pub id: Option<i64>,
    pub name: String,
    pub kind: String,
    pub config: Option<String>,
    pub auth_mode: String,
    pub created_at: String,
}

pub fn validate_ssrf(url: &str) -> Result<()> {
    crate::security::validate_ssrf_full(url)
}

pub fn create_connector(
    conn: &Connection,
    name: &str,
    kind: ConnectorKind,
    config: Option<&str>,
    auth_mode: ConnectorAuthMode,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO connectors (name, kind, config_json, auth_mode)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            name,
            connector_kind_as_str(kind),
            config,
            connector_auth_as_str(auth_mode)
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_connectors(conn: &Connection) -> Result<Vec<Connector>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, config_json, auth_mode, created_at
         FROM connectors ORDER BY id DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Connector {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                config: r.get(3)?,
                auth_mode: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn delete_connector(conn: &Connection, id: i64) -> Result<bool> {
    let rows = conn.execute("DELETE FROM connectors WHERE id = ?1", params![id])?;
    Ok(rows > 0)
}

fn connector_kind_as_str(k: ConnectorKind) -> &'static str {
    match k {
        ConnectorKind::LocalJson => "local_json",
        ConnectorKind::Csv => "csv",
        ConnectorKind::Sqlite => "sqlite",
        ConnectorKind::Http => "http",
    }
}

fn connector_auth_as_str(a: ConnectorAuthMode) -> &'static str {
    match a {
        ConnectorAuthMode::None => "none",
        ConnectorAuthMode::Header => "header",
        ConnectorAuthMode::Bearer => "bearer",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbBackup {
    pub version: u8,
    pub schema_version: u32,
    pub tables: Vec<TableDump>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDump {
    pub name: String,
    pub rows: Vec<Vec<serde_json::Value>>,
}

pub fn export_db(conn: &Connection) -> Result<DbBackup> {
    let table_names = get_user_tables(conn)?;
    let mut tables = Vec::new();
    for name in &table_names {
        let rows = dump_table(conn, name)?;
        tables.push(TableDump {
            name: name.clone(),
            rows,
        });
    }
    let schema_version: u32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _migrations",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|v| v as u32)
        .unwrap_or(0);
    Ok(DbBackup {
        version: 1,
        schema_version,
        tables,
    })
}

fn get_user_tables(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master
         WHERE type = 'table'
           AND name NOT LIKE 'sqlite_%'
           AND name NOT LIKE '_migrations'
           AND name NOT LIKE 'idx_%'
           AND name != 'oauth_tokens'
         ORDER BY name",
    )?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn dump_table(conn: &Connection, name: &str) -> Result<Vec<Vec<serde_json::Value>>> {
    let sql = format!("SELECT * FROM {name}");
    let mut stmt = conn.prepare(&sql)?;
    let col_count = stmt.column_count();
    let rows = stmt
        .query_map([], |r| {
            let mut row = Vec::with_capacity(col_count);
            for i in 0..col_count {
                let val: rusqlite::types::Value = r.get(i)?;
                row.push(match val {
                    rusqlite::types::Value::Null => serde_json::Value::Null,
                    rusqlite::types::Value::Integer(n) => serde_json::Value::from(n),
                    rusqlite::types::Value::Real(f) => serde_json::Value::from(f),
                    rusqlite::types::Value::Text(s) => serde_json::Value::from(s),
                    rusqlite::types::Value::Blob(b) => serde_json::Value::from(hex_encode(&b)),
                });
            }
            Ok(row)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn export_settings(conn: &Connection, password: &str) -> Result<Vec<u8>> {
    let settings_rows: Vec<(String, String)> = conn
        .prepare("SELECT key, value FROM application_settings")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let json = serde_json::to_vec(&settings_rows)
        .map_err(|e| Error::Config(format!("settings export serialization failed: {e}")))?;

    use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
    use rand::RngCore;
    use scrypt::scrypt;

    const SALT_LEN: usize = 16;
    const NONCE_LEN: usize = 12;
    const KEY_LEN: usize = 32;

    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let mut key = [0u8; KEY_LEN];
    scrypt(
        password.as_bytes(),
        &salt,
        &scrypt::Params::new(17, 8, 1, KEY_LEN)
            .map_err(|e| Error::Config(format!("scrypt: {e}")))?,
        &mut key,
    )
    .map_err(|e| Error::Config(format!("scrypt: {e}")))?;

    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| Error::Config(format!("AES key: {e}")))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, json.as_ref())
        .map_err(|e| Error::Config(format!("AES encrypt: {e}")))?;

    let mut result = Vec::new();
    result.extend_from_slice(b"SOSYNC1");
    result.push(2);
    result.extend_from_slice(&salt);
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

pub fn import_settings(conn: &Connection, data: &[u8], password: &str) -> Result<()> {
    if data.len() < 7 + 1 + 16 + 12 {
        return Err(Error::Config("encrypted settings data too short".into()));
    }
    if &data[..7] != b"SOSYNC1" {
        return Err(Error::Config("invalid magic header for settings".into()));
    }
    let version = data[7];
    if version != 2 {
        return Err(Error::Config(format!(
            "unsupported settings version {version} (expected 2)"
        )));
    }

    const SALT_LEN: usize = 16;
    const NONCE_LEN: usize = 12;
    const KEY_LEN: usize = 32;

    let salt = &data[8..8 + SALT_LEN];
    let nonce_bytes = &data[8 + SALT_LEN..8 + SALT_LEN + NONCE_LEN];
    let ciphertext = &data[8 + SALT_LEN + NONCE_LEN..];

    use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
    use scrypt::scrypt;

    let mut key = [0u8; KEY_LEN];
    scrypt(
        password.as_bytes(),
        salt,
        &scrypt::Params::new(17, 8, 1, KEY_LEN)
            .map_err(|e| Error::Config(format!("scrypt: {e}")))?,
        &mut key,
    )
    .map_err(|e| Error::Config(format!("scrypt: {e}")))?;

    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| Error::Config(format!("AES key: {e}")))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| Error::Config("settings decryption failed — wrong password".into()))?;

    let settings_rows: Vec<(String, String)> = serde_json::from_slice(&plaintext)
        .map_err(|e| Error::Config(format!("settings deserialization failed: {e}")))?;

    for (key, value) in &settings_rows {
        conn.execute(
            "INSERT OR REPLACE INTO application_settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
    }

    Ok(())
}
