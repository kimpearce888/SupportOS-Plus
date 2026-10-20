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
