use rusqlite::Connection;

use crate::error::Result;

fn iso_utc(ts_ms: i64) -> String {
    use chrono::TimeZone;
    chrono::Utc
        .timestamp_millis_opt(ts_ms)
        .single()
        .map(|t| t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
        .unwrap_or_default()
}

#[must_use]
pub(crate) fn iso_utc_pub(ts_ms: i64) -> String {
    iso_utc(ts_ms)
}

pub(crate) const RESPONSE_STATE_SQL: &str = "CASE
  WHEN c.status = 'closed' THEN 'closed'
  WHEN c.status = 'spam' THEN 'closed'
  WHEN c.status = 'active' AND c.snoozed_until IS NOT NULL AND c.snoozed_until > datetime('now') THEN 'snoozed'
  WHEN c.status = 'active' AND c.first_customer_message_at IS NOT NULL AND c.first_response_at IS NULL THEN 'needs_first_response'
  WHEN c.status = 'active'
       AND c.last_customer_reply_at IS NOT NULL
       AND (c.last_human_agent_response_at IS NULL OR c.last_customer_reply_at > c.last_human_agent_response_at) THEN 'customer_waiting'
  WHEN c.status IN ('active', 'pending')
       AND c.last_human_agent_response_at IS NOT NULL
       AND (c.last_customer_reply_at IS NULL OR c.last_human_agent_response_at > c.last_customer_reply_at)
       AND (julianday('now') - julianday(c.last_human_agent_response_at)) * 1440 <= 1440 THEN 'recently_responded'
  WHEN c.status IN ('active', 'pending')
       AND c.last_human_agent_response_at IS NOT NULL
       AND (c.last_customer_reply_at IS NULL OR c.last_human_agent_response_at > c.last_customer_reply_at) THEN 'agent_waiting'
  WHEN c.activity_history_complete = 0
       AND c.first_customer_message_at IS NULL
       AND c.first_response_at IS NULL THEN 'unknown'
  ELSE 'never_responded'
END";

pub(crate) fn ensure_inbox_views_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS inbox_views (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT,
            definition TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            folder TEXT,
            version INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;
    Ok(())
}
