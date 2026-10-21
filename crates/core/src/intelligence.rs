use rusqlite::Connection;

use crate::error::Result;

pub const RECENCY_HALF_LIFE_DAYS: f64 = 90.0;

pub const MIN_OBSERVATIONS_FOR_PREFERENCE: u32 = 3;

pub const CHANGE_SIGNIFICANCE_THRESHOLD: f64 = 0.34;

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

pub fn apply_m014(conn: &Connection) -> Result<()> {
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

// 1.4.0 R7 (D14): the legacy interaction-signals CRUD that used to live
// here (SignalKey, record/get/count/confirm, derive_signal) was removed —
// it had zero production callers since the interaction profile moved to
// `interaction_engine`/`interaction_current` over the `interaction_profile`
// tables. The constants above stay live (the engine reads them); the
// `interaction_signals` table remains as inert schema (created by m014).
