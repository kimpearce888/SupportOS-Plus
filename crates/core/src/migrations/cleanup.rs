use rusqlite::Connection;

use crate::error::Result;

pub(crate) fn apply_m063_drop_orphan_tables(conn: &Connection) -> Result<()> {
    const ORPHAN_TABLES: &[&str] = &[
        "campaign_recipients",
        "campaigns",
        "side_thread_mentions",
        "side_thread_messages",
        "side_thread_participants",
        "side_threads",
        "state_transitions",
        "verified_drafts",
        "interaction_signals",
        "support_cases",
        "known_issue_refs",
        "thread_participants",
        "notification_prefs",
        "knowledge_doc_usage",
        "report_snapshots",
        "daily_metrics",
        "release_events",
        "routing_configurations",
        "encrypted_sync_log",
    ];
    for table in ORPHAN_TABLES {
        conn.execute(&format!("DROP TABLE IF EXISTS \"{table}\""), [])?;
    }
    let _ = conn.execute("UPDATE app_state SET schema_version = 54 WHERE id = 1", []);
    Ok(())
}
