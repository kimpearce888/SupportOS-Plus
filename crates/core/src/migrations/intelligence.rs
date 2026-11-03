use rusqlite::Connection;

use crate::error::Result;

pub(crate) const RECENCY_HALF_LIFE_DAYS: f64 = 90.0;

pub(crate) const MIN_OBSERVATIONS_FOR_PREFERENCE: u32 = 3;

pub(crate) const CHANGE_SIGNIFICANCE_THRESHOLD: f64 = 0.34;

pub const M014_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS interaction_signals (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        customer_id     INTEGER NOT NULL,
        signal_key      TEXT NOT NULL,
        signal_value    TEXT NOT NULL,
        observed_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        source_conversation_id INTEGER
    );
    CREATE INDEX IF NOT EXISTS idx_interaction_signals_customer
        ON interaction_signals (customer_id, signal_key, observed_at);

    UPDATE app_state SET schema_version = 14 WHERE id = 1;
"#;

pub(crate) fn apply_m014(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS interaction_signals (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_id     INTEGER NOT NULL,
            signal_key      TEXT NOT NULL,
            signal_value    TEXT NOT NULL,
            observed_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            source_conversation_id INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_interaction_signals_customer
            ON interaction_signals (customer_id, signal_key, observed_at);",
    )?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 14 WHERE id = 1", []);
    Ok(())
}
