use std::path::Path;

use rusqlite::Connection;

use crate::error::{Error, Result};

const KEEP_TABLES: &[&str] = &[
    "_migrations",
    "app_state",
    "application_settings",
    "application_errors",
    "secrets",
    "oauth_tokens",
    "oauth_states",
    "inbox_views",
    "inbox_fields",
    "inbox_field_options",
    "mailbox_business_hours",
    "automation_rules",
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResetReport {
    pub rows_deleted: u64,
    pub tables_cleared: usize,
    pub vectors_dropped: u64,
    pub attachment_files_deleted: u64,
}

pub fn reset_workspace_data(conn: &mut Connection, data_dir: Option<&Path>) -> Result<ResetReport> {
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    let report = purge_in_transaction(conn);
    conn.pragma_update(None, "foreign_keys", "ON")?;
    let mut report = report?;

    if report.rows_deleted > 0
        && let Err(e) = conn.execute_batch("VACUUM")
    {
        tracing::warn!(
            error = %e,
            "post-reset VACUUM failed; free pages keep their (zeroed) \
             space until the next VACUUM"
        );
    }

    if let Some(dir) = data_dir {
        match crate::vectorstore_sqlitevec::SqliteVecStore::purge_all(dir) {
            Ok(dropped) => report.vectors_dropped = dropped,
            Err(e) => tracing::warn!(
                error = %e,
                "vector store purge failed; stale vectors are harmless \
                 (content is re-embedded through the pending pipeline)"
            ),
        }
        report.attachment_files_deleted = purge_attachment_files(&dir.join("attachments"));
    }
    Ok(report)
}

fn purge_attachment_files(attachments_dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(attachments_dir) else {
        return 0;
    };
    let mut removed = 0_u64;
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        match std::fs::remove_file(entry.path()) {
            Ok(()) => removed += 1,
            Err(e) => tracing::warn!(
                path = %entry.path().display(),
                error = %e,
                "attachment file purge failed; remove it manually"
            ),
        }
    }
    removed
}

fn purge_in_transaction(conn: &mut Connection) -> Result<ResetReport> {
    if let Err(e) = conn.pragma_update(None, "secure_delete", "ON") {
        tracing::warn!(
            error = %e,
            "secure_delete could not be enabled; purged rows may remain \
             recoverable until the next VACUUM"
        );
    }
    let result = purge_tables(conn);
    if let Err(e) = conn.pragma_update(None, "secure_delete", "OFF") {
        tracing::warn!(error = %e, "secure_delete could not be restored to OFF");
    }
    result
}

fn purge_tables(conn: &mut Connection) -> Result<ResetReport> {
    let mut report = ResetReport::default();
    let tx = conn.transaction()?;

    let tables: Vec<(String, String)> = {
        let mut stmt = tx.prepare(
            "SELECT name, COALESCE(sql, '') FROM sqlite_master \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )?;
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .filter_map(|r| r.ok())
            .collect()
    };

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
            continue;
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

    crate::sync_engine::set_state(&tx, "NEW");

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
        conn.execute(
            "INSERT INTO mailboxes (remote_id, name) VALUES (7, 'Support')",
            [],
        )
        .expect("mailbox");
        let mailbox_local_id: i64 = conn.last_insert_rowid();

        crate::settings::set_string(&conn, "display_timezone", "Asia/Dhaka").expect("setting");
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
            ("application_settings", 2),
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

        assert_eq!(crate::sync_engine::get_state(&conn), "NEW");
        assert!(!crate::settings::get_bool(&conn, "demo_mode", false).unwrap_or(false));

        let violations: i64 = conn
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .expect("fk check");
        assert_eq!(violations, 0);

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

    #[test]
    fn audit49_reset_removes_downloaded_attachment_files() {
        let mut conn = migrated_conn();
        let dir = std::env::temp_dir().join(format!(
            "spp-purge-att-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let attachments = dir.join("attachments");
        std::fs::create_dir_all(&attachments).expect("attachments dir");
        let file_a = attachments.join("12-34-invoice.pdf");
        let file_b = attachments.join("13-35-screenshot.png");
        std::fs::write(&file_a, b"customer document").expect("write a");
        std::fs::write(&file_b, b"customer image").expect("write b");
        let other = dir.join("vectors");
        std::fs::create_dir_all(&other).expect("other dir");

        let report = reset_workspace_data(&mut conn, Some(&dir)).expect("reset with attachments");
        assert_eq!(
            report.attachment_files_deleted, 2,
            "both downloaded files are removed: {report:?}"
        );
        assert!(
            !file_a.exists() && !file_b.exists(),
            "no attachment survives"
        );
        assert!(
            attachments.exists(),
            "the (now empty) attachments directory itself stays"
        );
        assert!(other.exists(), "unrelated data-dir content is untouched");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
