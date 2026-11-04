use rusqlite::Connection;

use crate::error::Result;

/// AU2-13c: tables created by migrations but never read or written by the
/// program (the AU-1 code-truth census, adjudicated per table at HEAD).
/// support_graph's permanently-empty side-thread reads were removed in the
/// same batch, so the whole side-thread family can go too. Dropping here (not
/// editing the old migrations) keeps upgraded databases and fresh installs
/// converging on the same final shape.
pub(crate) fn apply_m063_drop_orphan_tables(conn: &Connection) -> Result<()> {
    const ORPHAN_TABLES: &[&str] = &[
        // m024 outreach draft tables, superseded by outreach_campaigns /
        // outreach_recipients (m031) — zero program references.
        "campaign_recipients",
        "campaigns",
        // m006 internal-discussion family: never written, support_graph's
        // reads were permanently empty.
        "side_thread_mentions",
        "side_thread_messages",
        "side_thread_participants",
        "side_threads",
        // m032 draft: the live transition table is ticket_state_transitions.
        "state_transitions",
        // remaining zero-reference creations from the breadth migrations.
        "verified_drafts",
        // m014: the live signal store is the interaction_engine's own table.
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
        // encrypted_sync was a name-only stub, deleted in this batch.
        "encrypted_sync_log",
    ];
    for table in ORPHAN_TABLES {
        conn.execute(&format!("DROP TABLE IF EXISTS \"{table}\""), [])?;
    }
    let _ = conn.execute("UPDATE app_state SET schema_version = 54 WHERE id = 1", []);
    Ok(())
}
