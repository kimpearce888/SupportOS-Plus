use rusqlite::Connection;

use crate::error::Result;

pub const M010_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS ai_attributes (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        conversation_id INTEGER NOT NULL,
        attribute_key   TEXT NOT NULL,
        value           TEXT NOT NULL,
        evidence_excerpt TEXT NOT NULL,
        thread_ref      TEXT NOT NULL,
        confidence      REAL NOT NULL DEFAULT 0.0,
        created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_ai_attributes_conv
        ON ai_attributes (conversation_id, attribute_key);

    UPDATE app_state SET schema_version = 10 WHERE id = 1;
"#;

pub fn apply_m010(conn: &Connection) -> Result<()> {
    {
        let mut stmt = conn.prepare("PRAGMA table_info(ai_attributes)")?;
        let has_new_shape = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|c| c.ok())
            .any(|c| c == "attribute");
        if has_new_shape {
            return Ok(());
        }
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ai_attributes (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL,
            attribute_key   TEXT NOT NULL,
            value           TEXT NOT NULL,
            evidence_excerpt TEXT NOT NULL,
            thread_ref      TEXT NOT NULL,
            confidence      REAL NOT NULL DEFAULT 0.0,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_ai_attributes_conv
            ON ai_attributes (conversation_id, attribute_key);",
    )?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 10 WHERE id = 1", []);
    Ok(())
}
