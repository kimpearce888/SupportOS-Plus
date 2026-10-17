use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftStatus {
    Pending,
    Approved,
    Sent,
    Rejected,
}

impl DraftStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Sent => "sent",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifiedDraft {
    pub id: Option<i64>,
    pub conversation_id: i64,
    pub draft_text: String,
    pub status: String,
    pub created_by_ai_at: String,
    pub approved_by_user_id: Option<i64>,
    pub approved_at: Option<String>,
    pub sent_at: Option<String>,
}

pub fn create_draft(conn: &Connection, conversation_id: i64, draft_text: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO verified_drafts (conversation_id, draft_text, status)
         VALUES (?1, ?2, 'pending')",
        params![conversation_id, draft_text],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn approve_draft(conn: &Connection, draft_id: i64, user_id: i64) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE verified_drafts
         SET status = 'approved', approved_by_user_id = ?1,
             approved_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?2 AND status = 'pending'",
        params![user_id, draft_id],
    )?;
    Ok(rows > 0)
}

pub fn send_draft(conn: &Connection, draft_id: i64) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE verified_drafts
         SET status = 'sent', sent_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?1 AND status = 'approved'",
        params![draft_id],
    )?;
    Ok(rows > 0)
}

pub fn reject_draft(conn: &Connection, draft_id: i64) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE verified_drafts SET status = 'rejected' WHERE id = ?1 AND status = 'pending'",
        params![draft_id],
    )?;
    Ok(rows > 0)
}

pub fn list_drafts(conn: &Connection, conversation_id: i64) -> Result<Vec<VerifiedDraft>> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, draft_text, status, created_by_ai_at,
                approved_by_user_id, approved_at, sent_at
         FROM verified_drafts WHERE conversation_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt
        .query_map(params![conversation_id], |r| {
            Ok(VerifiedDraft {
                id: r.get(0)?,
                conversation_id: r.get(1)?,
                draft_text: r.get(2)?,
                status: r.get(3)?,
                created_by_ai_at: r.get(4)?,
                approved_by_user_id: r.get(5)?,
                approved_at: r.get(6)?,
                sent_at: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoachingSuggestion {
    pub suggestion: String,
    pub rationale: String,
}

pub const MAX_SUGGESTIONS: usize = 5;

#[must_use]
pub fn parse_coaching_suggestions(response: &str) -> Vec<CoachingSuggestion> {
    #[derive(serde::Deserialize)]
    struct CoachingResponse {
        suggestions: Vec<CoachingSuggestionRaw>,
    }
    #[derive(serde::Deserialize)]
    struct CoachingSuggestionRaw {
        suggestion: String,
        rationale: String,
    }
    let parsed: std::result::Result<CoachingResponse, _> = serde_json::from_str(response);
    match parsed {
        Ok(resp) => resp
            .suggestions
            .into_iter()
            .take(MAX_SUGGESTIONS)
            .map(|s| CoachingSuggestion {
                suggestion: s.suggestion,
                rationale: s.rationale,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomerMemory {
    pub id: Option<i64>,
    pub customer_id: i64,
    pub memory_key: String,
    pub memory_value: String,
    pub evidence_excerpt: String,
    pub source_conversation_id: Option<i64>,
    pub created_at: String,
}

pub fn set_memory(
    conn: &Connection,
    customer_id: i64,
    key: &str,
    value: &str,
    evidence: &str,
    source_conv: Option<i64>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO customer_memory (customer_id, memory_key, memory_value, evidence_excerpt, source_conversation_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![customer_id, key, value, evidence, source_conv],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_memory(conn: &Connection, customer_id: i64) -> Result<Vec<CustomerMemory>> {
    let mut stmt = conn.prepare(
        "SELECT id, customer_id, memory_key, memory_value, evidence_excerpt, source_conversation_id, created_at
         FROM customer_memory WHERE customer_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt
        .query_map(params![customer_id], |r| {
            Ok(CustomerMemory {
                id: r.get(0)?,
                customer_id: r.get(1)?,
                memory_key: r.get(2)?,
                memory_value: r.get(3)?,
                evidence_excerpt: r.get(4)?,
                source_conversation_id: r.get(5)?,
                created_at: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn count_memory_entries(conn: &Connection, customer_id: i64, key: &str) -> Result<u32> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM customer_memory WHERE customer_id = ?1 AND memory_key = ?2",
        params![customer_id, key],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(count).unwrap_or(0))
}

pub const MIN_OBSERVATIONS_FOR_PREFERENCE: u32 = 3;

pub fn is_confirmed_pattern(conn: &Connection, customer_id: i64, key: &str) -> Result<bool> {
    Ok(count_memory_entries(conn, customer_id, key)? >= MIN_OBSERVATIONS_FOR_PREFERENCE)
}

pub const TRANSLATION_PROMPT_VERSION: &str = "translation-v1";

#[must_use]
pub fn build_translation_prompt(text: &str, target_lang: &str) -> String {
    format!(
        "Translate the following text to {target_lang}. Provide ONLY the translation, no explanation.\n\n{text}"
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QaResult {
    pub conversation_id: i64,
    pub qa_score: Option<f64>,
    pub qa_notes: String,
    pub from_cache: bool,
}

pub fn store_qa(
    conn: &Connection,
    conversation_id: i64,
    score: Option<f64>,
    notes: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO post_resolution_qa (conversation_id, qa_score, qa_notes)
         VALUES (?1, ?2, ?3)",
        params![conversation_id, score, notes],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_qa(conn: &Connection, conversation_id: i64) -> Result<Option<QaResult>> {
    let row: Option<(Option<f64>, String)> = conn
        .query_row(
            "SELECT qa_score, qa_notes FROM post_resolution_qa
             WHERE conversation_id = ?1 ORDER BY checked_at DESC LIMIT 1",
            params![conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    match row {
        None => Ok(None),
        Some((score, notes)) => Ok(Some(QaResult {
            conversation_id,
            qa_score: score,
            qa_notes: notes,
            from_cache: false,
        })),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Suggestion {
    pub suggestion: String,
    pub category: String,
}

#[must_use]
pub fn parse_suggestions(response: &str) -> Vec<Suggestion> {
    #[derive(serde::Deserialize)]
    struct SuggestionsResponse {
        suggestions: Vec<SuggestionRaw>,
    }
    #[derive(serde::Deserialize)]
    struct SuggestionRaw {
        suggestion: String,
        category: String,
    }
    let parsed: std::result::Result<SuggestionsResponse, _> = serde_json::from_str(response);
    match parsed {
        Ok(resp) => resp
            .suggestions
            .into_iter()
            .take(MAX_SUGGESTIONS)
            .map(|s| Suggestion {
                suggestion: s.suggestion,
                category: s.category,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}
