use rusqlite::Connection;

use crate::error::Result;

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

// Clean-audit item 156: the `data_tools` runtime below the migration is
// gone — zero callers anywhere. The custom-object CRUD is duplicated
// (and owned) by `custom_objects.rs`, the connector CRUD by
// `connectors.rs`, and the backup/settings export-import path by
// `backup_service.rs` + `data_reset.rs`; this module's copies drifted
// from the live schemas and were never wired into the shell.
