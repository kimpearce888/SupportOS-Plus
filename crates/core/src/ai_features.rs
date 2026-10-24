use rusqlite::Connection;

use crate::error::Result;

pub const M011_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS verified_drafts (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        conversation_id INTEGER NOT NULL,
        draft_text      TEXT NOT NULL,
        status          TEXT NOT NULL DEFAULT 'pending',
        created_by_ai_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        approved_by_user_id INTEGER,
        approved_at     TEXT,
        sent_at         TEXT
    );
    CREATE INDEX IF NOT EXISTS idx_verified_drafts_conv
        ON verified_drafts (conversation_id, status);
"#;

pub const M012_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS customer_memory (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        customer_id     INTEGER NOT NULL,
        memory_key      TEXT NOT NULL,
        memory_value    TEXT NOT NULL,
        evidence_excerpt TEXT NOT NULL,
        source_conversation_id INTEGER,
        created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_customer_memory_customer
        ON customer_memory (customer_id, memory_key);
"#;

pub const M013_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS post_resolution_qa (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        conversation_id INTEGER NOT NULL,
        qa_score        REAL,
        qa_notes        TEXT,
        checked_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_post_resolution_qa_conv
        ON post_resolution_qa (conversation_id);
"#;

pub fn apply_m011_to_m013(conn: &Connection) -> Result<()> {
    conn.execute_batch(M011_SQL)?;
    conn.execute_batch(M012_SQL)?;
    conn.execute_batch(M013_SQL)?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 13 WHERE id = 1", []);
    Ok(())
}

// 1.4.0 R7 (D14): the legacy verified-draft approval workflow (DraftStatus,
// VerifiedDraft, create/approve/send/reject/list_drafts) was removed — it
// had zero production callers and is superseded by the outbound dispatcher
// plus the draft verification gate (`ai_pipeline::verify_draft_internal`,
// `ai_drafts` + `ai_verifications` tables). The `verified_drafts` table
// remains as inert schema (created by m011).

// Clean-audit item 149: the dead `ai_features` runtime surface below the
// migrations is gone — zero callers anywhere for the coaching-suggestion
// parser, the customer-memory helpers (the live memory path is
// `ai_tools.rs`, the copilot tool the feature actually serves), the
// `translation-v1` prompt pair (mismatched duplicate of
// `ai_prompts::PROMPT_VERSIONS_TRANSLATION` = "translation_v1", the one
// `translation.rs` really uses), the post-resolution QA trio (the live
// QA implementation is `quality.rs`), and the duplicate suggestions
// parser.
