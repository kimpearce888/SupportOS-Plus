//! One-click workspace reset (Settings → Help Scout → Workspace data).
//!
//! "Workspace data" is every business record the app holds: the full
//! Help Scout mirror (conversations, customers, threads, ratings, ...),
//! the knowledge base, incidents, reports, AI records, and the
//! sync/webhook bookkeeping that tracked how it got there — including
//! anything left over from the demo dataset that shipped before 1.2.0.
//! The reset walks the **live schema** instead of a hand-maintained
//! delete list: every table not on the [`KEEP_TABLES`] allowlist is
//! emptied. A test pins the allowlist against the migrated schema, so a
//! name typo fails the build, and a table added by a future migration
//! lands in the purge set by default — its author must make a
//! conscious KEEP decision here to preserve it.
//!
//! The reset also writes `sync_state = NEW` inside the same
//! transaction: the next boot with credentials configured immediately
//! starts a fresh initial sync (since 1.2.0 `NEW` means "pull
//! everything", not "do nothing"), and without credentials the app
//! stays empty — nothing is ever simulated.
//!
//! Foreign keys make the dance delicate: deleting mirror rows while
//! child tables still hold rows would fire immediate cascades or
//! constraint errors under live enforcement, so the reset runs with
//! enforcement suspended (the same pattern as boot migrations since
//! 1.1.2) and restores consistency itself: a `PRAGMA foreign_key_check`
//! sweep deletes every row left dangling by the bulk delete —
//! repeatedly, until the schema checks clean — so the commit is either
//! fully consistent or the whole reset rolls back. KEEP rows that
//! reference purged content (a business-hours row for a mailbox that
//! no longer exists) die in the sweep: config tied to deleted content
//! is stale by definition.

use std::path::Path;

use rusqlite::Connection;

use crate::error::{Error, Result};

/// Tables that survive the reset: migration bookkeeping, application
/// settings and diagnostics, encrypted credentials, connection state,
/// and every user-configured artifact (views, rules, templates,
/// definitions, connector configs). Everything else is business
/// content — synced or left over from the pre-1.2.0 demo — and is
/// deleted.
const KEEP_TABLES: &[&str] = &[
    // Migration bookkeeping and app state
    "_migrations",
    "app_state",
    // Application configuration and diagnostics
    "application_settings",
    "application_errors",
    // Encrypted credentials and connection state
    "secrets",
    "oauth_tokens",
    "oauth_states",
    // User-configured views, rules, templates, definitions
    "inbox_views",
    "inbox_fields",
    "inbox_field_options",
    "notification_prefs",
    "mailbox_business_hours",
    "automation_rules",
    "routing_configurations",
    "sla_configs",
    "saved_replies",
    "saved_segments",
    "custom_object_types",
    "custom_object_fields",
    "customer_property_definitions",
    "organization_property_definitions",
    "connectors",
    "ai_settings",
    "metric_definitions",
    "products",
];

/// What the reset did: rows deleted, tables emptied (only tables that
/// actually held rows), vector collections and payload entries dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResetReport {
    pub rows_deleted: u64,
    pub tables_cleared: usize,
    pub vectors_dropped: u64,
}

/// Delete all business content from the database (workspace reset).
///
/// Keeps the tables on [`KEEP_TABLES`], resets the vector store next to
/// the database (`data_dir/vectors/vectors.db`) when `data_dir` is
/// given, and leaves credentials, settings and connection tokens
/// untouched. Runs in one transaction — a failure rolls the database
/// back to its pre-reset state. `sync_state` is set to `NEW` so the
/// next boot either pulls the account fresh (credentials configured)
/// or stays honestly empty.
///
/// # Errors
/// Returns [`Error::Migration`]-style failures from SQLite, or a
/// `Config` error when the post-reset `foreign_key_check` sweep fails
/// to converge (should be impossible; the transaction rolls back).
pub fn reset_workspace_data(conn: &mut Connection, data_dir: Option<&Path>) -> Result<ResetReport> {
    // Enforcement is suspended for the whole reset; the sweep below
    // restores a verified-consistent state before the commit. The
    // pragma is a no-op inside a transaction, so it must run here,
    // before `purge_in_transaction` begins one.
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    let report = purge_in_transaction(conn);
    conn.pragma_update(None, "foreign_keys", "ON")?;
    let mut report = report?;

    if let Some(dir) = data_dir {
        match crate::vectorstore_sqlitevec::SqliteVecStore::purge_all(dir) {
            Ok(dropped) => report.vectors_dropped = dropped,
            Err(e) => tracing::warn!(
                error = %e,
                "vector store purge failed; stale vectors are harmless \
                 (content is re-embedded through the pending pipeline)"
            ),
        }
    }
    Ok(report)
}

fn purge_in_transaction(conn: &mut Connection) -> Result<ResetReport> {
    let mut report = ResetReport::default();
    let tx = conn.transaction()?;

    // (name, create-sql) of every user table, in schema order.
    let tables: Vec<(String, String)> = {
        let mut stmt = tx.prepare(
            "SELECT name, COALESCE(sql, '') FROM sqlite_master \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )?;
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .filter_map(|r| r.ok())
            .collect()
    };

    // Virtual tables (FTS5, vec0): DELETE FROM goes through the module
    // and maintains the shadow tables; the shadows themselves must
    // never be touched directly.
    let virtual_tables: Vec<String> = tables
        .iter()
        .filter(|(_, sql)| sql.contains("USING fts5") || sql.contains("USING vec0"))
        .map(|(name, _)| name.clone())
        .collect();

    for (name, _) in &tables {
        let is_shadow = virtual_tables
            .iter()
            .any(|v| name.as_str() != v.as_str() && name.starts_with(&format!("{v}_")));
        if is_shadow {
            continue; // owned by the virtual table module
        }
        if virtual_tables.iter().any(|v| v == name) || !KEEP_TABLES.contains(&name.as_str()) {
            let deleted = tx
                .execute(&format!("DELETE FROM \"{name}\""), [])
                .map_err(|e| Error::Config(format!("purge of {name} failed: {e}")))?;
            if deleted > 0 {
                report.tables_cleared += 1;
                report.rows_deleted += deleted as u64;
            }
        }
    }

    // Fresh sync state, atomically with the delete: the next boot with
    // credentials starts a clean initial pull (`NEW`), and without
    // credentials the app stays empty (nothing is ever simulated).
    crate::sync_engine::set_state(&tx, "NEW");

    // Restore consistency: delete rows that dangle after the bulk
    // delete (children of purged parents, recursively). Convergence is
    // guaranteed — every pass strictly removes rows — but the loop is
    // bounded so a pathological schema cannot spin forever.
    for pass in 0..64 {
        let dangling: Vec<(String, Option<i64>)> = {
            let mut stmt = tx.prepare("SELECT \"table\", rowid FROM pragma_foreign_key_check")?;
            stmt.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect()
        };
        if dangling.is_empty() {
            tx.commit()?;
            return Ok(report);
        }
        tracing::debug!(pass, count = dangling.len(), "purge: pruning dangling rows");
        for (table, rowid) in dangling {
            match rowid {
                Some(id) => {
                    tx.execute(&format!("DELETE FROM \"{table}\" WHERE rowid = ?1"), [id])
                        .map_err(|e| Error::Config(format!("prune of {table}#{id} failed: {e}")))?;
                    report.rows_deleted += 1;
                }
                None => {
                    // WITHOUT ROWID children report NULL; empty the whole
                    // child table (none exist today; this is the safe
                    // fallback if one ever appears).
                    let deleted = tx
                        .execute(&format!("DELETE FROM \"{table}\""), [])
                        .map_err(|e| Error::Config(format!("prune of {table} failed: {e}")))?;
                    report.rows_deleted += deleted as u64;
                }
            }
        }
    }
    Err(Error::Config(
        "foreign_key_check did not converge after the purge; rolled back".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn migrated_conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM \"{table}\""), [], |r| {
            r.get(0)
        })
        .unwrap_or_else(|e| panic!("count {table}: {e}"))
    }

    #[test]
    fn every_keep_table_exists_in_the_migrated_schema() {
        let conn = migrated_conn();
        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT name FROM sqlite_master \
                     WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                )
                .expect("prepare");
            stmt.query_map([], |r| r.get(0))
                .expect("query")
                .filter_map(|r| r.ok())
                .collect()
        };
        let unknown: Vec<&str> = KEEP_TABLES
            .iter()
            .filter(|k| !tables.iter().any(|t| t == *k))
            .copied()
            .collect();
        assert!(
            unknown.is_empty(),
            "KEEP_TABLES names missing from the schema (typo? table renamed?): {unknown:?}\n\
             all tables: {tables:?}"
        );
        // And the sweep target exists: at least one content table is
        // outside the keep list.
        let purged_tables = tables
            .iter()
            .filter(|t| !KEEP_TABLES.contains(&t.as_str()))
            .count();
        assert!(
            purged_tables > 50,
            "expected a large content sweep set, got {purged_tables}"
        );
    }

    #[test]
    fn purge_clears_content_preserves_config_and_is_repeatable() {
        let mut conn = migrated_conn();

        // Business content in representative tables.
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject) VALUES (1, 5001, 'demo')",
            [],
        )
        .expect("conversation");
        conn.execute(
            "INSERT INTO conversation_threads (conversation_id, type, state, body_text) \
             VALUES (1, 'customer', 'published', 'hello')",
            [],
        )
        .expect("thread");
        conn.execute(
            "INSERT INTO ratings (conversation_id, rating) VALUES (1, 'great')",
            [],
        )
        .expect("rating");
        conn.execute(
            "INSERT INTO sync_cursors (resource, cursor) VALUES ('conversations', 'tok-1')",
            [],
        )
        .expect("cursor");
        conn.execute(
            "INSERT INTO webhook_events (event_id, event_hash, event_type, received_at, payload) \
             VALUES ('evt-1', 'hash-1', 'convo.created', '2026-10-20T00:00:00Z', '{}')",
            [],
        )
        .expect("webhook event");
        // A mailbox for the dangling-config sweep case below.
        conn.execute(
            "INSERT INTO mailboxes (remote_id, name) VALUES (7, 'Support')",
            [],
        )
        .expect("mailbox");
        let mailbox_local_id: i64 = conn.last_insert_rowid();

        // User configuration that must survive.
        crate::settings::set_string(&conn, "display_timezone", "Asia/Dhaka").expect("setting");
        // Two saved replies: one standalone (must survive), one tied to
        // the demo mailbox (must die in the dangling sweep with the
        // purged mailboxes table).
        conn.execute(
            "INSERT INTO saved_replies (name, text) VALUES ('greeting', 'Hi there!')",
            [],
        )
        .expect("saved reply");
        conn.execute(
            "INSERT INTO saved_replies (name, text, mailbox_local_id) \
             VALUES ('mailbox-bound', 'Sorry to hear', ?1)",
            [mailbox_local_id],
        )
        .expect("mailbox-bound reply");
        conn.execute("INSERT INTO oauth_tokens (access_token) VALUES ('tok')", [])
            .expect("token");

        let report = reset_workspace_data(&mut conn, None).expect("reset");
        assert!(
            report.rows_deleted >= 5,
            "at least the seeded rows: {report:?}"
        );
        assert!(
            report.tables_cleared >= 5,
            "at least the seeded tables: {report:?}"
        );

        for table in [
            "conversations",
            "conversation_threads",
            "ratings",
            "sync_cursors",
            "sync_checkpoints",
            "sync_runs",
            "webhook_events",
            "customers",
            "knowledge_documents",
            "incidents",
        ] {
            assert_eq!(
                count(&conn, table),
                0,
                "{table} must be empty after the purge"
            );
        }
        for (table, expected) in [
            // display_timezone (seeded) + the sync_state row the reset
            // itself writes (asserted by value below). The retired demo
            // rows are gone: m050 deletes them at migration time.
            ("application_settings", 2),
            // Both replies: the standalone one survives; the mailbox-bound
            // one was pruned by the dangling sweep.
            ("saved_replies", 1),
            ("oauth_tokens", 1),
        ] {
            assert_eq!(
                count(&conn, table),
                expected,
                "{table} config must survive the reset"
            );
        }
        let tz = crate::settings::get_string(&conn, "display_timezone")
            .expect("read")
            .expect("present");
        assert_eq!(tz, "Asia/Dhaka");

        // The reset arms a fresh first pull: sync_state is NEW (boot
        // starts the initial sync immediately when credentials exist,
        // and nothing is simulated otherwise).
        assert_eq!(crate::sync_engine::get_state(&conn), "NEW");
        // And the retired demo settings rows are not resurrected.
        assert!(!crate::settings::get_bool(&conn, "demo_mode", false).unwrap_or(false));

        // The post-reset schema checks foreign-key clean.
        let violations: i64 = conn
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .expect("fk check");
        assert_eq!(violations, 0);

        // Idempotent: a second reset succeeds and deletes nothing.
        let again = reset_workspace_data(&mut conn, None).expect("second reset");
        assert_eq!(
            again.rows_deleted, 0,
            "second reset must be a no-op: {again:?}"
        );
    }

    #[test]
    fn reset_cleans_the_vector_store_when_a_data_dir_is_given() {
        let mut conn = migrated_conn();
        let dir = std::env::temp_dir().join(format!("spp-purge-vec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");

        // A vector collection with one point (via the public store API).
        {
            let store = crate::vectorstore_sqlitevec::SqliteVecStore::open(
                &crate::vectorstore_sqlitevec::store_dir(&dir).join("vectors.db"),
            )
            .expect("store");
            store
                .create_collection(crate::vectorstore_sqlitevec::VECTOR_COLLECTION, Some(4))
                .expect("collection");
            use crate::vectorstore::{Point, VectorStore};
            store
                .upsert(
                    crate::vectorstore_sqlitevec::VECTOR_COLLECTION,
                    Point {
                        id: "1".into(),
                        dense: Some(vec![0.1, 0.2, 0.3, 0.4]),
                        sparse: None,
                        payload: serde_json::json!({"entity_type": "knowledge_chunk", "entity_id": 1}),
                    },
                )
                .expect("point");
        }

        let report = reset_workspace_data(&mut conn, Some(&dir)).expect("reset with vectors");
        assert!(
            report.vectors_dropped >= 2,
            "collection + payload row: {report:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
