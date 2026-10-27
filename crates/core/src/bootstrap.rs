use crate::error::Result;
use rusqlite::Connection;

struct BootStep {
    version: u32,
    label: &'static str,
    apply: fn(&Connection) -> Result<()>,
}

fn apply_m007_full(conn: &Connection) -> Result<()> {
    crate::automation::apply_m007(conn)?;
    crate::automation::ensure_main_rule_columns(conn)
}

fn apply_runtime_guards(conn: &Connection) -> Result<()> {
    crate::saved_views::ensure_inbox_views_table(conn)?;
    crate::oauth_state::ensure_oauth_states_table(conn)?;
    crate::jobs::ensure_jobs_table(conn)?;
    crate::interaction_current::ensure_client_current_signals_table(conn)?;
    crate::interaction_current::ensure_interaction_evidence_table(conn)?;
    crate::interaction_engine::ensure_schema(conn)?;
    crate::sla::ensure_sla_schema(conn)?;
    crate::ai_pipeline::ensure_pipeline_schema(conn)?;
    crate::copilot::ensure_copilot_schema(conn)?;
    crate::quality::ensure_quality_tables(conn)
}

const BOOT_STEPS: &[BootStep] = &[
    BootStep {
        version: 3,
        label: "fts5_reference_004",
        apply: crate::search::apply_fts_migration,
    },
    BootStep {
        version: 4,
        label: "m003_activity",
        apply: crate::activity::apply_m003,
    },
    BootStep {
        version: 5,
        label: "m004_ticket_states",
        apply: crate::ticket_states::apply_m004,
    },
    BootStep {
        version: 6,
        label: "m005_notifications",
        apply: crate::notifications::apply_m005,
    },
    BootStep {
        version: 7,
        label: "m006_side_threads",
        apply: crate::side_threads::apply_m006,
    },
    BootStep {
        version: 8,
        label: "m007_automation_rules",
        apply: apply_m007_full,
    },
    BootStep {
        version: 9,
        label: "m008_embeddings",
        apply: crate::embeddings::apply_m008,
    },
    BootStep {
        version: 10,
        label: "m009_ai_center",
        apply: crate::ai_center::apply_m009,
    },
    BootStep {
        version: 11,
        label: "m010_ai_analysis",
        apply: crate::ai_analysis::apply_m010,
    },
    BootStep {
        version: 12,
        label: "m011_to_m013_ai_features",
        apply: crate::ai_features::apply_m011_to_m013,
    },
    BootStep {
        version: 13,
        label: "m014_intelligence",
        apply: crate::intelligence::apply_m014,
    },
    BootStep {
        version: 14,
        label: "m015_to_m019_intelligence_features",
        apply: crate::intelligence_features::apply_m015_to_m019,
    },
    BootStep {
        version: 15,
        label: "m020_to_m022_reports",
        apply: crate::reports::apply_m020_to_m022,
    },
    BootStep {
        version: 16,
        label: "m023_to_m025_outreach",
        apply: crate::outreach::apply_m023_to_m025,
    },
    BootStep {
        version: 17,
        label: "m026_to_m027_data_tools",
        apply: crate::data_tools::apply_m026_to_m027,
    },
    BootStep {
        version: 18,
        label: "m028_inbox",
        apply: crate::inbox::apply_m028,
    },
    BootStep {
        version: 19,
        label: "m029_sync_schema",
        apply: crate::sync_schema::apply_m029,
    },
    BootStep {
        version: 20,
        label: "m030_conversation_ops",
        apply: crate::conversation_ops::apply_m030,
    },
    BootStep {
        version: 21,
        label: "m031_outreach",
        apply: crate::outreach::apply_m031,
    },
    BootStep {
        version: 22,
        label: "m032_ticket_states",
        apply: crate::ticket_states::apply_m032,
    },
    BootStep {
        version: 23,
        label: "m033_ai_attributes",
        apply: crate::ai_attributes::apply_m033,
    },
    BootStep {
        version: 24,
        label: "m034_reports",
        apply: crate::reports::apply_m034,
    },
    BootStep {
        version: 25,
        label: "m035_incidents",
        apply: crate::intelligence_features::apply_m035,
    },
    BootStep {
        version: 26,
        label: "m036_customer_events",
        apply: crate::customer_events::apply_m036,
    },
    BootStep {
        version: 27,
        label: "m037_maintenance",
        apply: crate::maintenance::apply_m037,
    },
    BootStep {
        version: 28,
        label: "m038_connectors",
        apply: crate::connectors::apply_m038,
    },
    BootStep {
        version: 29,
        label: "m039_mirror_tables",
        apply: crate::mirror_tables::apply_m039,
    },
    BootStep {
        version: 30,
        label: "m040_db_breadth",
        apply: crate::db_breadth::apply_m040,
    },
    BootStep {
        version: 31,
        label: "m041_incident_workspace",
        apply: crate::intelligence_features::apply_m041,
    },
    BootStep {
        version: 32,
        label: "runtime_table_guards",
        apply: apply_runtime_guards,
    },
    BootStep {
        version: 33,
        label: "m042_sync_cursors_reference",
        apply: crate::sync_schema::apply_m042,
    },
    BootStep {
        version: 34,
        label: "m043_hs_rate_limit",
        apply: crate::helpscout_real::apply_m043,
    },
    BootStep {
        version: 35,
        label: "m044_mirror_schema_completion",
        apply: crate::mirror_parity::apply_m044,
    },
    BootStep {
        version: 36,
        label: "m045_threads_actor_model",
        apply: crate::mirror_parity::apply_m045,
    },
    BootStep {
        version: 37,
        label: "m046_base_mirror_fks",
        apply: crate::mirror_parity::apply_m046,
    },
    BootStep {
        version: 38,
        label: "m047_conversations_reference_shape",
        apply: crate::mirror_parity::apply_m047,
    },
    // The webhook_events schema used to be re-ensured on every insert; it
    // now lives here, in its own numbered step (migration-map row 6).
    BootStep {
        version: 39,
        label: "m048_webhook_events_schema",
        apply: crate::webhook::apply_m048_webhook_events,
    },
    // The secrets table gets its first writer: the plaintext webhook_secret
    // moves out of application_settings into the encrypted envelope
    // (migration-map row 13). Requires settings::init_master_key to have run
    // (see the app shell); skips with a warning otherwise.
    BootStep {
        version: 40,
        label: "m049_secret_envelope",
        apply: crate::settings::apply_m049_secret_envelope,
    },
    // The diagnostics event log gains a level column (info/warn/error)
    // so the Diagnostics page can show and filter reports, and the
    // retired demo-mode settings rows are dropped for good.
    BootStep {
        version: 41,
        label: "m050_diagnostics_log",
        apply: crate::audit::apply_m050,
    },
    // 1.3.0: the outbound lane becomes a real dispatcher (replies/notes/
    // status changes/assignments actually reach Help Scout). Existing
    // rows get a retry budget, rows stuck mid-send by an older crash are
    // re-queued, and installs sitting on the old 5-minute default poll
    // move to 1 minute.
    BootStep {
        version: 42,
        label: "m051_outbound_dispatch",
        apply: crate::outbound::apply_m051,
    },
    // 1.4.0 R6: the sweep's silent no-op stages are gone (chats/attachments/
    // ratings have no list endpoints on the platform; report_data never had
    // a fetcher). Their stale checkpoint rows are dropped so Sync Health
    // stops rendering stages that can never run again.
    BootStep {
        version: 43,
        label: "m052_sweep_stage_cleanup",
        apply: crate::sync_schema::apply_m052,
    },
    // 1.4.0 R7 (D13): the OAuth access/refresh tokens move out of the
    // plaintext `oauth_tokens` columns into the AES-256-GCM envelope; the
    // row keeps metadata only. Skips (unstamped) until a master key exists.
    BootStep {
        version: 44,
        label: "m053_oauth_token_envelope",
        apply: crate::helpscout_real::apply_m053_oauth_token_envelope,
    },
    // 1.4.0 A1: ai_runs gains endpoint_id so the provider hub's cost
    // ledger can price every completed run against the endpoint that
    // served it.
    BootStep {
        version: 45,
        label: "m054_ai_runs_endpoint",
        apply: crate::ai_hub::apply_m054_ai_runs_endpoint,
    },
    // Batch 13 A4: conversation_chunks gains chunk_kind ('thread' |
    // 'resolution') so the retrieval spine can prefer resolution excerpts.
    BootStep {
        version: 46,
        label: "m055_chunk_kind",
        apply: crate::embeddings::apply_m055_chunk_kind,
    },
    // Batch 14 K2: the AI article-draft review queue.
    BootStep {
        version: 47,
        label: "m056_article_drafts",
        apply: crate::knowledge_store::apply_m056_article_drafts,
    },
    // Batch 14 K4: the weekly AI digest archive.
    BootStep {
        version: 48,
        label: "m057_ai_digests",
        apply: crate::digest::apply_m057_ai_digests,
    },
    // Batch 14 K5: the fetched Help Scout report cache.
    BootStep {
        version: 49,
        label: "m058_hs_report_cache",
        apply: crate::reports_api::apply_m058_hs_report_cache,
    },
    // Batch 15 P2: the live Beacon chat mirror — chats arrive through the
    // five beacon.chat.* webhook events (the platform has no list
    // endpoint), so the mirror keeps even ended chats.
    BootStep {
        version: 50,
        label: "m059_beacon_chats",
        apply: crate::chats::apply_m059_beacon_chats,
    },
    // Clean-audit item 57: the automation once-guard's memory. The newest
    // customer thread id a conversation has already fired triggers for, so
    // `new_conversation` fires at most once per conversation and
    // `customer_reply` at most once per customer thread — the pre-fix code
    // fired BOTH on every customer-latest sync.
    BootStep {
        version: 51,
        label: "m060_automation_once_guard",
        apply: crate::automation::apply_m060_automation_once_guard,
    },
    // Clean-audit item 100: knowledge_chunks gains the embedding_attempts
    // counter its sibling chunk tables already carry, so the family's
    // failure lane observes the same five-attempt cap instead of retrying
    // a doomed chunk on every embed pass forever.
    BootStep {
        version: 52,
        label: "m061_knowledge_chunk_attempts",
        apply: crate::embeddings::apply_m061_knowledge_chunk_attempts,
    },
    // Audit fix (C4 follow-up): customer_chats was originally delivered by
    // editing the already-applied m039 in place. Databases that had run the
    // released 1.7.3 recorded version 52, so every later boot skipped m039
    // and the customer chat-channel mirror never existed on upgraded
    // installs — the first chat-channel sync failed with "no such table".
    // m062 re-issues the same idempotent CREATE: a no-op where the current
    // m039 already made the table (fresh installs), the only creator where
    // it did not (1.7.3 → 1.8.0 upgrades).
    BootStep {
        version: 53,
        label: "m062_customer_chats_upgrade_guard",
        apply: crate::mirror_tables::apply_m062_customer_chats,
    },
];

fn apply_boot_steps(conn: &mut Connection, steps: &[BootStep]) -> Result<()> {
    let current = crate::db::latest_version(conn)?;
    for step in steps {
        if step.version <= current {
            continue;
        }
        tracing::info!(
            version = step.version,
            label = step.label,
            "applying migration"
        );
        // Foreign-key enforcement is suspended while a step runs. Several
        // steps rebuild tables by dropping and renaming them, and SQLite's
        // live enforcement cannot survive that dance: immediate cascade
        // actions fire against rows that are about to come back under the
        // rebuilt table, and the deferred-violation counter never forgets
        // a violation that the rename has long since repaired (the 1.1.x
        // "migration 36 failed" boot failures on real 0.2.x data).
        // Integrity is enforced instead by an explicit PRAGMA
        // foreign_key_check sweep below, inside the same transaction: a
        // step that would commit dangling references rolls back whole.
        conn.pragma_update(None, "foreign_keys", "OFF")?;
        let applied = apply_one_step(conn, step);
        conn.pragma_update(None, "foreign_keys", "ON")?;
        applied?;
    }
    Ok(())
}

/// Run one boot step in its own transaction, verified foreign-key clean.
fn apply_one_step(conn: &mut Connection, step: &BootStep) -> Result<()> {
    let tx = conn.transaction()?;
    (step.apply)(&tx).map_err(|e| crate::error::Error::Migration {
        version: step.version,
        message: e.to_string(),
    })?;
    let violations: i64 =
        tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })?;
    if violations > 0 {
        let first: Option<(String, i64, String)> = tx
            .query_row(
                "SELECT \"table\", rowid, \"parent\" FROM pragma_foreign_key_check LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok();
        return Err(crate::error::Error::Migration {
            version: step.version,
            message: format!(
                "foreign_key_check reports {violations} dangling reference(s), \
                 e.g. {first:?}"
            ),
        });
    }
    tx.execute(
        "INSERT INTO _migrations (version, label) VALUES (?1, ?2)",
        rusqlite::params![i64::from(step.version), step.label],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn apply_all(conn: &mut Connection) -> Result<()> {
    crate::db::ensure_migrations_table(conn)?;
    // The secrets master key is anchored next to the database file and must
    // exist before the migration steps run: m049 seals the legacy plaintext
    // webhook_secret into the encrypted envelope. In-memory databases (tests,
    // tooling) have no file to anchor a key next to; m049 then skips with a
    // warning and tests install a key explicitly where needed.
    if let Some(db_path) = crate::db::db_file_path(conn) {
        let key_path = db_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."))
            .join("secrets.key");
        crate::settings::init_master_key(&key_path)?;
    }
    crate::migrations::run_all(conn)?;
    apply_boot_steps(conn, BOOT_STEPS)
}
