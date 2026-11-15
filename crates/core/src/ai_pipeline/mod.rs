use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ai_evidence::{AiSourceRef, build as build_evidence, sources_for};
use crate::ai_lm_studio::{ChatOpts, OpenAiCompatibleClient};
use crate::ai_prompts::{
    CUSTOMER_DRAFT_SYSTEM, ClusterConversation, DRAFT_VERIFICATION_SYSTEM, DraftVerification,
    INTERACTION_OBSERVATION_SYSTEM, INTERACTION_RECOMMENDATION_SYSTEM, ISSUE_CLUSTER_SYSTEM,
    InteractionChangeInput, MEMORY_EXTRACTION_SYSTEM, PROMPT_VERSIONS_CUSTOMER_DRAFT,
    PROMPT_VERSIONS_DRAFT_VERIFICATION, PROMPT_VERSIONS_INTERACTION_OBSERVATION,
    PROMPT_VERSIONS_INTERACTION_RECOMMENDATION, PROMPT_VERSIONS_ISSUE_CLUSTER,
    PROMPT_VERSIONS_MEMORY_EXTRACTION, PROMPT_VERSIONS_TICKET_ANALYSIS, TICKET_ANALYSIS_SYSTEM,
    TicketAnalysis, build_customer_draft_user, build_draft_verification_user,
    build_interaction_observation_user, build_interaction_recommendation_user,
    build_issue_cluster_user, build_memory_extraction_user, build_ticket_analysis_user,
};
use crate::ai_provider::ChatMessage;
use crate::business_hours::now_iso;
use crate::crypto::hex;
use crate::error::Result;
use crate::search::apply_fts_migration;

mod interaction;
pub(crate) use interaction::{analyze_interaction, build_interaction_strategy_block};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct LmStudioError {
    pub message: String,
    pub retryable: bool,
}

impl LmStudioError {
    pub fn new(message: impl Into<String>, retryable: bool) -> Self {
        Self {
            message: message.into(),
            retryable,
        }
    }
}

pub(crate) fn lm_error(e: &crate::error::Error) -> Option<&LmStudioError> {
    if let crate::error::Error::Other(b) = e {
        b.downcast_ref::<LmStudioError>()
    } else {
        None
    }
}

const AI_DISABLED_MESSAGE: &str =
    "AI is disabled in Settings. Enable LM Studio in Settings > LM Studio to use AI features.";

pub enum AiBackend {
    LmStudio {
        client: OpenAiCompatibleClient,
        model: Option<String>,
        endpoint_id: Option<String>,
    },
    Disabled,
}

pub(crate) fn backend_from_settings(conn: &Connection) -> AiBackend {
    let enabled = crate::settings::get_bool(conn, "ai_enabled", true).unwrap_or(true);
    if !enabled {
        return AiBackend::Disabled;
    }
    let status = crate::ai_center::get_ai_status(conn).unwrap_or_default();
    let base_url = crate::settings::get_string(conn, "lmstudio_base_url")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .or_else(|| status.base_url.clone())
        .unwrap_or_else(|| crate::ai_lm_studio::LM_STUDIO_BASE_URL.to_string());
    let timeout_ms = u64::try_from(
        crate::settings::get_i64(conn, "lmstudio_timeout_ms", 120_000).unwrap_or(120_000),
    )
    .unwrap_or(crate::ai_lm_studio::LM_STUDIO_DEFAULT_TIMEOUT_MS);
    let model = crate::settings::get_string(conn, "lmstudio_chat_model")
        .ok()
        .flatten()
        .map(|s| if s.is_empty() { None } else { Some(s) })
        .unwrap_or(status.chat_model.clone());
    AiBackend::LmStudio {
        client: OpenAiCompatibleClient::new_with_timeout(base_url, timeout_ms),
        model,
        endpoint_id: None,
    }
}

impl AiBackend {
    pub fn kind(&self) -> &'static str {
        match self {
            AiBackend::LmStudio { .. } => "lmstudio",
            AiBackend::Disabled => "disabled",
        }
    }

    #[must_use]
    pub fn endpoint_id(&self) -> Option<&str> {
        match self {
            AiBackend::LmStudio { endpoint_id, .. } => endpoint_id.as_deref(),
            AiBackend::Disabled => None,
        }
    }

    pub(crate) async fn chat_json(
        &self,
        conn: &Connection,
        system: &str,
        user: &str,
        redact: bool,
        max_tokens: Option<u32>,
    ) -> std::result::Result<ChatJsonResult, LmStudioError> {
        let safe_user = if redact {
            let enabled =
                crate::settings::get_bool(conn, "redaction_enabled", true).unwrap_or(true);
            crate::security::redact_text(user, enabled)
                .map_err(|e| LmStudioError::new(e.to_string(), false))?
                .0
        } else {
            user.to_string()
        };
        let (client, model) = match self {
            AiBackend::LmStudio { client, model, .. } => (client, model.as_deref()),
            AiBackend::Disabled => {
                return Err(LmStudioError::new(AI_DISABLED_MESSAGE, false));
            }
        };
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: system.into(),
            },
            ChatMessage {
                role: "user".into(),
                content: safe_user,
            },
        ];
        let res = client
            .chat_with_opts(
                model,
                &messages,
                ChatOpts {
                    temperature: Some(0.15),
                    max_tokens: Some(max_tokens.unwrap_or(2048)),
                    json_mode: true,
                },
            )
            .await
            .map_err(|e| LmStudioError::new(e.to_string(), true))?;
        let raw = res.content;
        Ok(ChatJsonResult {
            json: raw.as_deref().and_then(extract_json),
            raw,
            latency_ms: res.latency_ms,
            model: res.model,
            usage: res.usage,
        })
    }

    pub(crate) async fn chat_qa(
        &self,
        messages: Vec<crate::ai_provider::ChatMessage>,
        temperature: f64,
        max_tokens: u32,
        json_mode: bool,
    ) -> std::result::Result<crate::ai_lm_studio::ChatOptsResult, LmStudioError> {
        let (client, model) = match self {
            AiBackend::LmStudio { client, model, .. } => (client, model.as_deref()),
            AiBackend::Disabled => {
                return Err(LmStudioError::new(AI_DISABLED_MESSAGE, false));
            }
        };
        client
            .chat_with_opts(
                model,
                &messages,
                ChatOpts {
                    temperature: Some(temperature),
                    max_tokens: Some(max_tokens),
                    json_mode,
                },
            )
            .await
            .map_err(|e| LmStudioError::new(e.to_string(), true))
    }
}

#[derive(Debug)]
pub(crate) struct ChatJsonResult {
    pub json: Option<serde_json::Value>,
    pub raw: Option<String>,
    pub latency_ms: u64,
    pub model: String,
    pub usage: Option<crate::ai_provider::TokenUsage>,
}

pub(crate) fn extract_json(text: &str) -> Option<serde_json::Value> {
    let cleaned = text.replace("```json", "```").replace("```", "");
    let cleaned = cleaned.trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(cleaned) {
        return Some(v);
    }
    let start = cleaned.find('{')?;
    let end = cleaned.rfind('}')?;
    if end > start {
        serde_json::from_str(&cleaned[start..=end]).ok()
    } else {
        None
    }
}

pub(crate) fn ensure_pipeline_schema(conn: &Connection) -> Result<()> {
    apply_fts_migration(conn)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ai_sources (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id      INTEGER NOT NULL REFERENCES ai_runs(id) ON DELETE CASCADE,
            source_type TEXT NOT NULL,
            source_id   INTEGER NOT NULL,
            title       TEXT,
            relevance   REAL,
            visibility  TEXT,
            timestamp   TEXT
        );
        CREATE TABLE IF NOT EXISTS ai_extracted_facts (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER REFERENCES conversations(id) ON DELETE CASCADE,
            run_id          INTEGER REFERENCES ai_runs(id) ON DELETE CASCADE,
            key             TEXT NOT NULL,
            value           TEXT,
            confidence      TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_ai_facts_conversation ON ai_extracted_facts(conversation_id);
        CREATE TABLE IF NOT EXISTS ai_drafts (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            run_id          INTEGER REFERENCES ai_runs(id),
            content         TEXT NOT NULL,
            mode            TEXT DEFAULT 'standard',
            model           TEXT,
            prompt_version  TEXT,
            state           TEXT DEFAULT 'generated',
            verification    TEXT,
            sources         TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            provenance      TEXT NOT NULL DEFAULT 'ai_generated'
        );
        CREATE INDEX IF NOT EXISTS idx_ai_drafts_conversation ON ai_drafts(conversation_id);
        CREATE TABLE IF NOT EXISTS ai_verifications (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            draft_id          INTEGER NOT NULL REFERENCES ai_drafts(id) ON DELETE CASCADE,
            run_id            INTEGER REFERENCES ai_runs(id),
            verified          INTEGER NOT NULL,
            unsupported_claims TEXT,
            missing_questions TEXT,
            internal_leakage  TEXT,
            conflicts         TEXT,
            warnings          TEXT,
            created_at        TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS ai_feedback (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            draft_id         INTEGER NOT NULL REFERENCES ai_drafts(id) ON DELETE CASCADE,
            original_content TEXT,
            final_content    TEXT,
            edit_distance    INTEGER,
            was_sent         INTEGER DEFAULT 0,
            sent_at          TEXT,
            rating_after     TEXT,
            created_at       TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS customer_memory (
            id                      INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_id             INTEGER NOT NULL,
            memory_key              TEXT NOT NULL,
            memory_value            TEXT NOT NULL,
            evidence_excerpt        TEXT NOT NULL,
            source_conversation_id  INTEGER,
            created_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            origin                  TEXT DEFAULT 'conversation',
            first_seen_at           TEXT,
            last_seen_at            TEXT,
            confidence              TEXT DEFAULT 'unknown',
            provenance              TEXT DEFAULT 'ai_generated',
            kind                    TEXT NOT NULL DEFAULT 'fact',
            source                  TEXT NOT NULL DEFAULT 'ai'
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_customer_memory_upsert
            ON customer_memory(customer_id, memory_key);
        CREATE INDEX IF NOT EXISTS idx_customer_memory_customer
            ON customer_memory(customer_id, memory_key);
        CREATE INDEX IF NOT EXISTS idx_ai_runs_conversation ON ai_runs(conversation_id);
        CREATE INDEX IF NOT EXISTS idx_ai_runs_type ON ai_runs(type, status);
        CREATE INDEX IF NOT EXISTS idx_ai_runs_cache ON ai_runs(type, input_hash, prompt_version);
        CREATE TABLE IF NOT EXISTS golden_test_set (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL UNIQUE,
            category    TEXT NOT NULL,
            subject     TEXT NOT NULL,
            body        TEXT NOT NULL,
            active      INTEGER NOT NULL DEFAULT 1,
            created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE TABLE IF NOT EXISTS golden_test_runs (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            finished_at TEXT,
            backend     TEXT NOT NULL DEFAULT '',
            passed      INTEGER NOT NULL DEFAULT 0,
            failed      INTEGER NOT NULL DEFAULT 0,
            results     TEXT
        );
        ",
    )?;
    conn.execute_batch(
        "INSERT OR IGNORE INTO golden_test_set (name, category, subject, body) VALUES
          ('simple question', 'simple', 'What time do you close?', 'Hi, what are your support hours?'),
          ('multi-question ticket', 'multi', 'Two things: export + timezone', 'How do I export data? Also how do I change the timezone for scheduled reports?'),
          ('ambiguous ticket', 'ambiguous', 'It does not work', 'The thing keeps failing sometimes. Not sure what is wrong.'),
          ('known issue', 'known_issue', 'Meeting reminders one hour late', 'Since the DST change our reminders are all one hour late.'),
          ('customer history', 'history', 'Follow-up on the export issue', 'The export you helped me with last month broke again.'),
          ('timezone issue', 'timezone', 'Santiago timezone wrong', 'Scheduled report sends at 3 AM instead of 8 AM Chile time.'),
          ('integration issue', 'integration', 'Slack integration broken', 'The Slack integration stopped posting updates to our channel.'),
          ('billing question', 'billing', 'Card declined', 'My payment failed but the card works everywhere else.'),
          ('internal escalation', 'escalation', 'URGENT outage for key account', 'Our production access is down, we need this escalated now.');
        ",
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn start_run(
    conn: &Connection,
    kind: &str,
    conversation_id: Option<i64>,
    model: Option<&str>,
    prompt_version: &str,
    input_hash: Option<&str>,
    input_refs: &serde_json::Value,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO ai_runs (type, conversation_id, status, model, prompt_version, input_hash,
                              input_refs, created_at, started_at, response_json)
         VALUES (?1, ?2, 'running', ?3, ?4, ?5, ?6, datetime('now'), datetime('now'), '')",
        params![
            kind,
            conversation_id,
            model.unwrap_or(""),
            prompt_version,
            input_hash.unwrap_or(""),
            serde_json::to_string(input_refs).unwrap_or_else(|_| "[]".into())
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub(crate) fn complete_run(
    conn: &Connection,
    id: i64,
    output: &serde_json::Value,
    latency_ms: u64,
) -> Result<()> {
    complete_run_with_usage(conn, id, output, latency_ms, None, None)
}

pub(crate) fn complete_run_with_usage(
    conn: &Connection,
    id: i64,
    output: &serde_json::Value,
    latency_ms: u64,
    usage: Option<&crate::ai_provider::TokenUsage>,
    endpoint_id: Option<&str>,
) -> Result<()> {
    let usage_json = match usage {
        Some(u) => serde_json::to_string(u)?,
        None => "null".to_string(),
    };
    conn.execute(
        "UPDATE ai_runs SET status='completed', response_json=?2, latency_ms=?3,
                            completed_at=datetime('now'),
                            token_usage=?4, endpoint_id=?5
          WHERE id=?1",
        params![
            id,
            serde_json::to_string(output)?,
            latency_ms as i64,
            usage_json,
            endpoint_id
        ],
    )?;
    Ok(())
}

pub(crate) fn fail_run(conn: &Connection, id: i64, error: &str) -> Result<()> {
    let capped: String = error.chars().take(2000).collect();
    conn.execute(
        "UPDATE ai_runs SET status='failed', error=?2, completed_at=datetime('now') WHERE id=?1",
        params![id, capped],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct CachedRun {
    pub output: String,
}

pub(crate) fn find_cached_run(
    conn: &Connection,
    kind: &str,
    input_hash: &str,
    prompt_version: &str,
) -> Result<Option<CachedRun>> {
    let mut stmt = conn.prepare(
        "SELECT response_json FROM ai_runs
          WHERE type=?1 AND input_hash=?2 AND prompt_version=?3 AND status='completed'
          ORDER BY id DESC LIMIT 1",
    )?;
    let run = stmt
        .query_row(params![kind, input_hash, prompt_version], |r| {
            Ok(CachedRun { output: r.get(0)? })
        })
        .ok();
    Ok(run)
}

pub(crate) fn input_hash(conversation_id: i64, content_signature: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{conversation_id}:{content_signature}"));
    hex(&hasher.finalize())
}

pub(crate) fn get_analysis_signature(conn: &Connection, conversation_id: i64) -> String {
    let (n, max_id): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(MAX(t.id),0) FROM conversation_threads t
              WHERE t.conversation_id = ?1 AND t.deleted_at IS NULL",
            params![conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));
    let mut hasher = Sha256::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT COALESCE(raw_json_hash, '') FROM conversation_threads
          WHERE conversation_id = ?1 AND deleted_at IS NULL ORDER BY id",
    ) {
        let rows = stmt.query_map(params![conversation_id], |r| r.get::<_, String>(0));
        if let Ok(rows) = rows {
            for hash in rows.flatten() {
                hasher.update(hash.as_bytes());
                hasher.update(b"|");
            }
        }
    }
    format!("{n}:{max_id}:{}", hex(&hasher.finalize()))
}

#[derive(Debug, Clone)]
pub(crate) struct LatestAnalysis {
    pub analysis: TicketAnalysis,
}

pub(crate) fn get_latest_analysis(
    conn: &Connection,
    conversation_id: i64,
) -> Result<Option<LatestAnalysis>> {
    let output: Option<String> = conn
        .query_row(
            "SELECT response_json FROM ai_runs
              WHERE conversation_id=?1 AND type='ticket_analysis' AND status='completed'
              ORDER BY id DESC LIMIT 1",
            params![conversation_id],
            |r| r.get::<_, String>(0),
        )
        .ok();
    let Some(output) = output else {
        return Ok(None);
    };
    let analysis: TicketAnalysis = serde_json::from_str(&output).unwrap_or_default();
    Ok(Some(LatestAnalysis { analysis }))
}

pub(crate) fn save_analysis(
    conn: &Connection,
    run_id: i64,
    conversation_id: i64,
    analysis: &TicketAnalysis,
    sources: &[AiSourceRef],
) -> Result<()> {
    let entries: [(&str, Option<&String>); 8] = [
        ("intent", analysis.intent.as_ref()),
        ("primary_question", analysis.primary_question.as_ref()),
        ("customer_goal", analysis.customer_goal.as_ref()),
        ("product", analysis.product.as_ref()),
        ("feature", analysis.feature.as_ref()),
        ("problem_type", analysis.problem_type.as_ref()),
        ("requested_action", analysis.requested_action.as_ref()),
        ("summary", analysis.summary.as_ref()),
    ];
    for (k, v) in entries {
        if let Some(v) = v {
            conn.execute(
                "INSERT INTO ai_extracted_facts (conversation_id, run_id, key, value, confidence)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![conversation_id, run_id, k, v, analysis.confidence],
            )?;
        }
    }
    for s in sources {
        conn.execute(
            "INSERT INTO ai_sources (run_id, source_type, source_id, title, relevance, visibility, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                run_id,
                s.source_type,
                s.source_id,
                s.title,
                s.relevance,
                s.visibility,
                s.timestamp
            ],
        )?;
    }
    conn.execute(
        "INSERT INTO fts_ai_analyses (summary, primary_question, intent, conversation_id, run_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            analysis.summary.as_deref().unwrap_or(""),
            analysis.primary_question.as_deref().unwrap_or(""),
            analysis.intent.as_deref().unwrap_or(""),
            conversation_id,
            run_id
        ],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AiDraftRecord {
    pub id: i64,
    pub conversation_id: i64,
    pub content: String,
    pub mode: String,
    pub model: Option<String>,
    pub prompt_version: Option<String>,
    pub created_at: String,
    pub verification: Option<DraftVerification>,
    #[serde(default)]
    pub sources: Vec<AiSourceRef>,
    pub state: String,
}

pub(crate) struct CreateDraftOpts<'a> {
    pub run_id: Option<i64>,
    pub mode: &'a str,
    pub model: Option<&'a str>,
    pub prompt_version: &'a str,
    pub verification: Option<&'a DraftVerification>,
    pub sources: &'a [AiSourceRef],
}

pub(crate) fn create_draft(
    conn: &Connection,
    conversation_id: i64,
    content: &str,
    opts: &CreateDraftOpts<'_>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO ai_drafts (conversation_id, run_id, content, mode, model, prompt_version,
                                state, verification, sources, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'generated', ?7, ?8, datetime('now'))",
        params![
            conversation_id,
            opts.run_id,
            content,
            opts.mode,
            opts.model,
            opts.prompt_version,
            opts.verification
                .map(|v| serde_json::to_string(v).unwrap_or_default()),
            serde_json::to_string(opts.sources).unwrap_or_else(|_| "[]".into())
        ],
    )?;
    let id = conn.last_insert_rowid();
    if let Some(v) = opts.verification {
        conn.execute(
            "INSERT INTO ai_verifications (draft_id, run_id, verified, unsupported_claims,
                                            missing_questions, internal_leakage, conflicts, warnings)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                opts.run_id,
                v.verified as i64,
                serde_json::to_string(&v.unsupported_claims).unwrap_or_else(|_| "[]".into()),
                serde_json::to_string(&v.missing_questions).unwrap_or_else(|_| "[]".into()),
                serde_json::to_string(&v.internal_leakage).unwrap_or_else(|_| "[]".into()),
                serde_json::to_string(&v.conflicts).unwrap_or_else(|_| "[]".into()),
                serde_json::to_string(&v.warnings).unwrap_or_else(|_| "[]".into())
            ],
        )?;
    }
    Ok(id)
}

pub(crate) fn get_draft(conn: &Connection, id: i64) -> Result<Option<AiDraftRecord>> {
    let row = conn
        .query_row(
            "SELECT id, conversation_id, content, mode, model, prompt_version, created_at,
                    verification, sources, state
               FROM ai_drafts WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, Option<String>>(9)?,
                ))
            },
        )
        .ok();
    let Some((
        id,
        conversation_id,
        content,
        mode,
        model,
        prompt_version,
        created_at,
        verification,
        sources,
        state,
    )) = row
    else {
        return Ok(None);
    };
    Ok(Some(AiDraftRecord {
        id,
        conversation_id,
        content,
        mode: mode.unwrap_or_else(|| "standard".into()),
        model,
        prompt_version,
        created_at,
        verification: verification
            .as_deref()
            .and_then(|v| serde_json::from_str(v).ok()),
        sources: sources
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default(),
        state: state.unwrap_or_else(|| "generated".into()),
    }))
}

pub fn set_draft_state(conn: &Connection, id: i64, state: &str) -> Result<()> {
    conn.execute(
        "UPDATE ai_drafts SET state = ?1 WHERE id = ?2",
        params![state, id],
    )?;
    Ok(())
}

pub fn record_feedback(
    conn: &Connection,
    draft_id: i64,
    original: &str,
    final_text: &str,
    was_sent: bool,
) -> Result<()> {
    let edit_distance = levenshtein(original, final_text) as i64;
    conn.execute(
        "INSERT INTO ai_feedback (draft_id, original_content, final_content, edit_distance,
                                  was_sent, sent_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            draft_id,
            original,
            final_text,
            edit_distance,
            was_sent as i64,
            if was_sent { Some(now_iso()) } else { None }
        ],
    )?;
    Ok(())
}

fn levenshtein(a: &str, b: &str) -> usize {
    if a == b {
        return 0;
    }
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    for i in 1..=m {
        let mut cur = vec![i];
        for j in 1..=n {
            cur.push(
                (prev[j] + 1)
                    .min(cur[j - 1] + 1)
                    .min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1])),
            );
        }
        prev = cur;
    }
    prev[n]
}

pub(crate) struct UpsertMemoryOpts<'a> {
    pub source: &'a str,
    pub origin: &'a str,
    pub conversation_id: Option<i64>,
    pub confidence: &'a str,
}

pub(crate) fn upsert_memory(
    conn: &Connection,
    customer_id: i64,
    key: &str,
    value: &str,
    opts: &UpsertMemoryOpts<'_>,
) -> Result<()> {
    let existing: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, source FROM customer_memory WHERE customer_id = ?1 AND memory_key = ?2",
            params![customer_id, key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let provenance = if opts.source == "human" {
        "human_local"
    } else {
        "ai_generated"
    };
    match existing {
        None => {
            conn.execute(
                "INSERT INTO customer_memory (customer_id, memory_key, memory_value,
                        evidence_excerpt, source_conversation_id, origin, first_seen_at,
                        last_seen_at, confidence, provenance, source)
                 VALUES (?1, ?2, ?3, '', ?4, ?5, datetime('now'), datetime('now'), ?6, ?7, ?8)",
                params![
                    customer_id,
                    key,
                    value,
                    opts.conversation_id,
                    opts.origin,
                    opts.confidence,
                    provenance,
                    opts.source
                ],
            )?;
        }
        Some((id, source)) => {
            if source == "human" && opts.source != "human" {
                return Ok(());
            }
            conn.execute(
                "UPDATE customer_memory SET memory_value=?2, last_seen_at=datetime('now'),
                        confidence=?3, origin=?4,
                        source_conversation_id=COALESCE(?5, source_conversation_id),
                        source=?6, provenance=?7
                  WHERE id=?1",
                params![
                    id,
                    value,
                    opts.confidence,
                    opts.origin,
                    opts.conversation_id,
                    opts.source,
                    provenance
                ],
            )?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct AnalyzeOutcome {
    pub analysis: TicketAnalysis,
}

pub(crate) async fn analyze_ticket(
    conn: &Connection,
    backend: &AiBackend,
    index: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
    conversation_local_id: i64,
    force: bool,
) -> std::result::Result<AnalyzeOutcome, LmStudioError> {
    let signature = get_analysis_signature(conn, conversation_local_id);
    let hash = input_hash(conversation_local_id, &signature);
    if !force
        && let Ok(Some(cached)) = find_cached_run(
            conn,
            "ticket_analysis",
            &hash,
            PROMPT_VERSIONS_TICKET_ANALYSIS,
        )
        && let Some(cached_json) = serde_json::from_str::<serde_json::Value>(&cached.output).ok()
        && let Some(analysis) = parse_ticket_analysis(Some(&cached_json))
        && analysis_has_core_content(&analysis)
    {
        return Ok(AnalyzeOutcome { analysis });
    }
    let ctx = build_evidence(conn, index, conversation_local_id, true)
        .map_err(|e| LmStudioError::new(e.to_string(), true))?
        .ok_or_else(|| LmStudioError::new("Conversation not found", false))?;
    let model = match backend {
        AiBackend::LmStudio { model, .. } => model.clone(),
        AiBackend::Disabled => None,
    };
    let run_id = start_run(
        conn,
        "ticket_analysis",
        Some(conversation_local_id),
        model.as_deref(),
        PROMPT_VERSIONS_TICKET_ANALYSIS,
        Some(&hash),
        &serde_json::json!([conversation_local_id]),
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let res = match backend
        .chat_json(
            conn,
            TICKET_ANALYSIS_SYSTEM,
            &build_ticket_analysis_user(&ctx),
            true,
            None,
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let _ = fail_run(
                conn,
                run_id,
                &format!("LM Studio chat failed: {}", e.message),
            );
            return Err(e);
        }
    };
    let analysis = match parse_ticket_analysis(res.json.as_ref()) {
        Some(a) => a,
        None => {
            let _ = fail_run(
                conn,
                run_id,
                "AI analysis returned an unparseable structure",
            );
            return Err(LmStudioError::new(
                "The local model did not return a valid analysis JSON. Try a stronger model or retry.",
                false,
            ));
        }
    };
    if !analysis_has_core_content(&analysis) {
        let _ = fail_run(
            conn,
            run_id,
            "AI analysis returned no usable core field (summary/intent/customer_goal all empty)",
        );
        return Err(LmStudioError::new(
            "The local model returned an empty analysis (no summary, intent, or customer goal). \
             Try a stronger model or retry.",
            false,
        ));
    }
    complete_run_with_usage(
        conn,
        run_id,
        &serde_json::to_value(&analysis).unwrap_or_default(),
        res.latency_ms,
        res.usage.as_ref(),
        backend.endpoint_id(),
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let sources = sources_for(conn, &ctx);
    save_analysis(conn, run_id, conversation_local_id, &analysis, &sources)
        .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    Ok(AnalyzeOutcome { analysis })
}

fn analysis_has_core_content(a: &TicketAnalysis) -> bool {
    [
        a.summary.as_deref(),
        a.intent.as_deref(),
        a.customer_goal.as_deref(),
    ]
    .into_iter()
    .any(|v| v.is_some_and(|s| !s.trim().is_empty()))
}

pub(crate) fn parse_ticket_analysis(json: Option<&serde_json::Value>) -> Option<TicketAnalysis> {
    let obj = json?.as_object()?;
    let get_str = |k: &str| -> Option<String> {
        obj.get(k)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
    };
    let get_str_list = |k: &str| -> Vec<String> {
        obj.get(k)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    };
    let enum_or_none = |k: &str, allowed: &[&str]| -> Option<String> {
        get_str(k).filter(|s| allowed.contains(&s.as_str()))
    };
    let urgency = enum_or_none("urgency", &["low", "normal", "high", "critical"]);
    let sentiment = enum_or_none(
        "sentiment",
        &["positive", "neutral", "negative", "frustrated"],
    );
    let evidence_quality = enum_or_none(
        "evidence_quality",
        &["strong", "some", "limited", "insufficient"],
    );
    let evidence_to_confidence = |q: Option<&str>| -> Option<String> {
        q.map(|q| {
            match q {
                "strong" => "high",
                "some" => "medium",
                "limited" => "low",
                _ => "unknown",
            }
            .to_string()
        })
    };
    Some(TicketAnalysis {
        intent: get_str("intent"),
        primary_question: get_str("primary_question"),
        secondary_questions: get_str_list("secondary_questions"),
        customer_goal: get_str("customer_goal"),
        product: get_str("product"),
        feature: get_str("feature"),
        problem_type: get_str("problem_type"),
        requested_action: get_str("requested_action"),
        urgency,
        sentiment,
        known_issue_candidate: get_str("known_issue_candidate"),
        issue_cluster_candidate: get_str("issue_cluster_candidate"),
        missing_information: get_str_list("missing_information"),
        summary: get_str("summary"),
        confidence: evidence_to_confidence(evidence_quality.as_deref()).or(Some("unknown".into())),
    })
}

pub(crate) async fn generate_draft(
    conn: &Connection,
    backend: &AiBackend,
    index: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
    conversation_local_id: i64,
    mode: &str,
    force: bool,
    analysis_override: Option<&TicketAnalysis>,
) -> std::result::Result<(AiDraftRecord, Option<DraftVerification>), LmStudioError> {
    let analysis = match analysis_override {
        Some(a) => a.clone(),
        None => {
            analyze_ticket(conn, backend, index, conversation_local_id, force)
                .await?
                .analysis
        }
    };
    let mut ctx = build_evidence(conn, index, conversation_local_id, false)
        .map_err(|e| LmStudioError::new(e.to_string(), true))?
        .ok_or_else(|| LmStudioError::new("Conversation not found", false))?;
    ctx.interaction_strategy = build_interaction_strategy_block(conn, conversation_local_id);
    let model = match backend {
        AiBackend::LmStudio { model, .. } => model.clone(),
        AiBackend::Disabled => None,
    };
    let run_id = start_run(
        conn,
        "customer_draft",
        Some(conversation_local_id),
        model.as_deref(),
        PROMPT_VERSIONS_CUSTOMER_DRAFT,
        None,
        &serde_json::json!([]),
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let res = backend
        .chat_json(
            conn,
            CUSTOMER_DRAFT_SYSTEM,
            &build_customer_draft_user(&ctx, mode, Some(&analysis)),
            true,
            Some(1600),
        )
        .await;
    let res = match res {
        Ok(r) => r,
        Err(e) => {
            let _ = fail_run(conn, run_id, &e.message);
            return Err(e);
        }
    };
    let json_draft = res
        .json
        .as_ref()
        .and_then(|j| {
            j.get("draft")
                .and_then(|d| d.as_str())
                .or_else(|| j.as_str())
        })
        .map(|s| s.to_string());
    let draft_text = match json_draft {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ if res.json.is_none() => match res.raw.as_deref().map(str::trim) {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => {
                let _ = fail_run(conn, run_id, "AI draft generation returned empty content");
                return Err(LmStudioError::new(
                    "The local model returned an empty draft. Retry or use a different model.",
                    false,
                ));
            }
        },
        _ => {
            let _ = fail_run(
                conn,
                run_id,
                "AI draft JSON had no draft text (envelope without a usable draft field)",
            );
            return Err(LmStudioError::new(
                "The local model returned JSON without a draft field. Retry or use a different model.",
                false,
            ));
        }
    };
    let used_evidence: Vec<String> = res
        .json
        .as_ref()
        .and_then(|j| j.get("used_evidence"))
        .and_then(|u| u.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    complete_run_with_usage(
        conn,
        run_id,
        &serde_json::json!({ "draft": draft_text, "usedEvidence": used_evidence }),
        res.latency_ms,
        res.usage.as_ref(),
        backend.endpoint_id(),
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let verification = match verify_draft_internal(
        conn,
        backend,
        index,
        conversation_local_id,
        &draft_text,
        Some(&analysis),
    )
    .await
    {
        Ok(v) => Some(v),
        Err(_) => Some(DraftVerification {
            verified: false,
            unsupported_claims: vec![],
            missing_questions: vec![],
            internal_leakage: vec![],
            conflicts: vec![],
            warnings: vec!["Verification pass failed - treat this draft as unverified".to_string()],
        }),
    };
    let sources = sources_for(conn, &ctx);
    let draft_id = create_draft(
        conn,
        conversation_local_id,
        &draft_text,
        &CreateDraftOpts {
            run_id: Some(run_id),
            mode,
            model: model.as_deref(),
            prompt_version: PROMPT_VERSIONS_CUSTOMER_DRAFT,
            verification: verification.as_ref(),
            sources: &sources,
        },
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let draft = get_draft(conn, draft_id)
        .map_err(|e| LmStudioError::new(e.to_string(), true))?
        .expect("draft just created");
    Ok((draft, verification))
}

pub(crate) async fn verify_draft_internal(
    conn: &Connection,
    backend: &AiBackend,
    index: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
    conversation_local_id: i64,
    draft_text: &str,
    analysis: Option<&TicketAnalysis>,
) -> std::result::Result<DraftVerification, LmStudioError> {
    let ctx = build_evidence(conn, index, conversation_local_id, true)
        .map_err(|e| LmStudioError::new(e.to_string(), true))?
        .ok_or_else(|| LmStudioError::new("Conversation not found", false))?;
    let run_id = start_run(
        conn,
        "draft_verification",
        Some(conversation_local_id),
        None,
        PROMPT_VERSIONS_DRAFT_VERIFICATION,
        None,
        &serde_json::json!([]),
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let mut questions: Vec<String> = Vec::new();
    if let Some(a) = analysis {
        if let Some(ref q) = a.primary_question {
            questions.push(q.clone());
        }
        questions.extend(a.secondary_questions.iter().cloned());
    }
    let res = backend
        .chat_json(
            conn,
            DRAFT_VERIFICATION_SYSTEM,
            &build_draft_verification_user(&ctx, draft_text, &questions),
            true,
            None,
        )
        .await;
    let res = match res {
        Ok(r) => r,
        Err(e) => {
            let _ = fail_run(conn, run_id, &e.message);
            return Err(e);
        }
    };
    let verification = parse_draft_verification(res.json.as_ref());
    complete_run_with_usage(
        conn,
        run_id,
        &serde_json::to_value(&verification).unwrap_or_default(),
        res.latency_ms,
        res.usage.as_ref(),
        backend.endpoint_id(),
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    Ok(verification)
}

fn parse_draft_verification(json: Option<&serde_json::Value>) -> DraftVerification {
    let fallback = || DraftVerification {
        verified: false,
        warnings: vec!["Verification output could not be parsed - treat as unverified".to_string()],
        ..Default::default()
    };
    let Some(obj) = json.and_then(|j| j.as_object()) else {
        return fallback();
    };
    let verified = obj.get("verified").and_then(|v| v.as_bool());
    let Some(verified) = verified else {
        return fallback();
    };
    let list = |k: &str| -> Vec<String> {
        obj.get(k)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    };
    DraftVerification {
        verified,
        unsupported_claims: list("unsupported_claims"),
        missing_questions: list("missing_questions"),
        internal_leakage: list("internal_leakage"),
        conflicts: list("conflicts"),
        warnings: list("warnings"),
    }
}

#[must_use]
pub(crate) fn rewrite_instruction(instruction: &str) -> &'static str {
    match instruction {
        "shorten" => {
            "Rewrite the reply to be roughly half as long while keeping every essential fact."
        }
        "expand" => {
            "Rewrite the reply with a little more context and a warmer structure, without adding any new facts."
        }
        "warmer" => "Rewrite the reply with a warmer, friendlier tone. Keep facts identical.",
        "more_direct" => "Rewrite the reply to be more direct and concise. Keep facts identical.",
        _ => "Rewrite the reply to be more direct and concise. Keep facts identical.",
    }
}

fn redact_for_model(conn: &Connection, text: &str) -> std::result::Result<String, LmStudioError> {
    let enabled = crate::settings::get_bool(conn, "redaction_enabled", true).unwrap_or(true);
    let (safe, findings) = crate::security::redact_text(text, enabled)
        .map_err(|e| LmStudioError::new(e.to_string(), false))?;
    if !findings.is_empty() {
        tracing::info!(
            operation = "draft_rewrite",
            patterns = ?findings.iter().filter_map(|f| f.get("pattern")).collect::<Vec<_>>(),
            "redaction boundary applied to the rewrite prompt"
        );
    }
    Ok(safe)
}

pub(crate) async fn rewrite_composer_text(
    conn: &Connection,
    backend: &AiBackend,
    conversation_id: Option<i64>,
    text: &str,
    instruction: &str,
) -> std::result::Result<String, LmStudioError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(LmStudioError::new(
            "Nothing to rewrite: the draft is empty.",
            false,
        ));
    }
    let (client, model) = match backend {
        AiBackend::LmStudio { client, model, .. } => (client, model.as_deref()),
        AiBackend::Disabled => return Err(LmStudioError::new(AI_DISABLED_MESSAGE, false)),
    };
    let safe_text = redact_for_model(conn, text)?;
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: "You rewrite customer support replies. Preserve all facts exactly; never add new facts. Output only the rewritten reply text.".into(),
        },
        ChatMessage {
            role: "user".into(),
            content: format!(
                "{}\n\nReply to rewrite:\n\"\"\"\n{}\n\"\"\"",
                rewrite_instruction(instruction),
                safe_text
            ),
        },
    ];
    let run_id = start_run(
        conn,
        "draft_rewrite",
        conversation_id,
        model,
        crate::ai_prompts::PROMPT_VERSIONS_DRAFT_REWRITE,
        None,
        &serde_json::json!([conversation_id, instruction]),
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let res = client
        .chat_with_opts(
            model,
            &messages,
            ChatOpts {
                temperature: Some(0.3),
                max_tokens: Some(1600),
                json_mode: false,
            },
        )
        .await;
    match res {
        Ok(res) => {
            let rewritten = res.content.unwrap_or_default().trim().to_string();
            if rewritten.is_empty() {
                let _ = fail_run(conn, run_id, "The model returned an empty rewrite.");
                return Err(LmStudioError::new(
                    "The local model returned an empty rewrite.",
                    false,
                ));
            }
            complete_run_with_usage(
                conn,
                run_id,
                &serde_json::json!({ "instruction": instruction, "output": rewritten, "output_chars": rewritten.chars().count() }),
                res.latency_ms,
                res.usage.as_ref(),
                backend.endpoint_id(),
            )
            .map_err(|e| LmStudioError::new(e.to_string(), true))?;
            Ok(rewritten)
        }
        Err(e) => {
            let _ = fail_run(conn, run_id, &e.to_string());
            Err(LmStudioError::new(e.to_string(), true))
        }
    }
}

pub(crate) async fn extract_memories(
    conn: &Connection,
    backend: &AiBackend,
    conversation_local_id: i64,
) -> usize {
    let conv = conn
        .query_row(
            "SELECT c.customer_local_id,
                    (SELECT TRIM(COALESCE(cu.first_name,'') || ' ' || COALESCE(cu.last_name,''))
                       FROM customers cu WHERE cu.id = c.customer_local_id)
               FROM conversations c WHERE c.id = ?1",
            params![conversation_local_id],
            |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .ok();
    let Some((Some(customer_id), customer_name)) = conv else {
        return 0;
    };
    let threads: Vec<(String, String)> = match crate::db::query_rows(
        conn,
        "SELECT type, from_name, body_html, body_text FROM conversation_threads
          WHERE conversation_id = ?1 AND deleted_at IS NULL ORDER BY COALESCE(remote_created_at, created_at) ASC",
        params![conversation_local_id],
        |r| {
            let kind: Option<String> = r.get(0)?;
            let from_name: Option<String> = r.get(1)?;
            let body_html: Option<String> = r.get(2)?;
            let body: Option<String> = r.get(3)?;
            let author = from_name.or(kind).unwrap_or_else(|| "unknown".into());
            let text: String =
                crate::textutil::html_to_text(&body_html.or(body).unwrap_or_default())
                    .chars()
                    .take(1200)
                    .collect();
            Ok((author, text))
        },
    ) {
        Ok(threads) => threads,
        Err(_) => return 0,
    };
    let run_id = match start_run(
        conn,
        "memory_extraction",
        Some(conversation_local_id),
        None,
        PROMPT_VERSIONS_MEMORY_EXTRACTION,
        None,
        &serde_json::json!([]),
    ) {
        Ok(id) => id,
        Err(_) => return 0,
    };
    let res = backend
        .chat_json(
            conn,
            MEMORY_EXTRACTION_SYSTEM,
            &build_memory_extraction_user(customer_name.as_deref().unwrap_or("Customer"), &threads),
            true,
            None,
        )
        .await;
    let res = match res {
        Ok(r) => r,
        Err(e) => {
            let _ = fail_run(conn, run_id, &e.message);
            return 0;
        }
    };
    let memories: Vec<(String, String, String)> = res
        .json
        .as_ref()
        .and_then(|j| j.get("memories"))
        .and_then(|m| m.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|m| {
                    let key = m.get("key")?.as_str()?.to_string();
                    let value = m.get("value")?.as_str()?.to_string();
                    let confidence = m
                        .get("confidence")
                        .and_then(|c| c.as_str())
                        .filter(|c| ["high", "medium", "low"].contains(c))
                        .unwrap_or("low")
                        .to_string();
                    Some((key, value, confidence))
                })
                .take(10)
                .collect()
        })
        .unwrap_or_default();
    let _ = complete_run_with_usage(
        conn,
        run_id,
        &serde_json::json!({ "memories": memories }),
        res.latency_ms,
        res.usage.as_ref(),
        backend.endpoint_id(),
    );
    for (key, value, confidence) in &memories {
        let _ = upsert_memory(
            conn,
            customer_id,
            key,
            value,
            &UpsertMemoryOpts {
                source: "ai",
                origin: "conversation",
                conversation_id: Some(conversation_local_id),
                confidence,
            },
        );
    }
    memories.len()
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClusterOut {
    pub title: String,
    pub summary: String,
    pub category: Option<String>,
    pub product: Option<String>,
    pub feature: Option<String>,
    pub conversation_ids: Vec<i64>,
}

const CLUSTER_PROMPT_CONVERSATION_CAP: usize = 250;

pub(crate) async fn cluster_issues(
    conn: &Connection,
    backend: &AiBackend,
    days: i64,
) -> std::result::Result<Vec<ClusterOut>, LmStudioError> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.number, c.subject, c.preview,
                    (SELECT GROUP_CONCAT(t.name) FROM conversation_tags ct
                       JOIN tags t ON t.id = ct.tag_id WHERE ct.conversation_id = c.id) AS tags
               FROM conversations c
              WHERE c.deleted_at IS NULL
                AND julianday(COALESCE(c.remote_created_at, c.created_at)) >= julianday('now', '-' || ?1 || ' days')
              ORDER BY COALESCE(c.remote_created_at, c.created_at) DESC, c.id DESC
              LIMIT ?2",
        )
        .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let mut conversations: Vec<(i64, ClusterConversation)> = stmt
        .query_map(params![days, CLUSTER_PROMPT_CONVERSATION_CAP as i64], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                ClusterConversation {
                    number: r.get(1)?,
                    subject: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    preview: r
                        .get::<_, Option<String>>(3)?
                        .unwrap_or_default()
                        .chars()
                        .take(200)
                        .collect(),
                    tags: r
                        .get::<_, Option<String>>(4)?
                        .unwrap_or_default()
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect(),
                },
            ))
        })
        .map_err(|e| LmStudioError::new(e.to_string(), true))?
        .filter_map(|r| r.ok())
        .collect();
    conversations.sort_by_key(|(_, c)| c.number);
    if conversations.len() < 3 {
        return Ok(Vec::new());
    }
    let total_in_window: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations c
              WHERE c.deleted_at IS NULL
                AND julianday(COALESCE(c.remote_created_at, c.created_at))
                    >= julianday('now', '-' || ?1 || ' days')",
            params![days],
            |r| r.get(0),
        )
        .unwrap_or(conversations.len() as i64);
    let run_id = start_run(
        conn,
        "issue_cluster",
        None,
        None,
        PROMPT_VERSIONS_ISSUE_CLUSTER,
        None,
        &serde_json::json!([]),
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let res = backend
        .chat_json(
            conn,
            ISSUE_CLUSTER_SYSTEM,
            &build_issue_cluster_user(
                &conversations
                    .iter()
                    .map(|(_, c)| c.clone())
                    .collect::<Vec<_>>(),
                total_in_window,
            ),
            true,
            Some(3000),
        )
        .await;
    let res = match res {
        Ok(r) => r,
        Err(e) => {
            let _ = fail_run(conn, run_id, &e.message);
            return Err(e);
        }
    };
    let raw_clusters: Vec<serde_json::Value> = res
        .json
        .as_ref()
        .and_then(|j| j.get("clusters"))
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    let _ = complete_run_with_usage(
        conn,
        run_id,
        &serde_json::json!({ "clusters": raw_clusters }),
        res.latency_ms,
        res.usage.as_ref(),
        backend.endpoint_id(),
    );
    let mut out: Vec<ClusterOut> = Vec::new();
    for c in raw_clusters {
        let numbers: Vec<i64> = c
            .get("conversation_numbers")
            .and_then(|n| n.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
            .unwrap_or_default();
        if numbers.len() < 2 {
            continue;
        }
        let ids: Vec<i64> = conversations
            .iter()
            .filter(|(_, x)| numbers.contains(&x.number))
            .map(|(id, _)| *id)
            .collect();
        let cluster = ClusterOut {
            title: c
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string(),
            summary: c
                .get("summary")
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string(),
            category: c
                .get("category")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string()),
            product: c
                .get("product")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string()),
            feature: c
                .get("feature")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string()),
            conversation_ids: ids,
        };
        let _ = upsert_cluster(
            conn,
            &ClusterUpsert {
                title: cluster.title.clone(),
                summary: cluster.summary.clone(),
                category: cluster.category.clone(),
                product: cluster.product.clone(),
                feature: cluster.feature.clone(),
                known_issue_id: None,
                ai_generated: true,
                conversation_ids: cluster.conversation_ids.clone(),
            },
        );
        out.push(cluster);
    }
    let _ = compute_trends(conn);
    Ok(out)
}

#[derive(Debug, Clone)]
pub(crate) struct ClusterUpsert {
    pub title: String,
    pub summary: String,
    pub category: Option<String>,
    pub product: Option<String>,
    pub feature: Option<String>,
    pub known_issue_id: Option<i64>,
    pub ai_generated: bool,
    pub conversation_ids: Vec<i64>,
}

pub(crate) fn upsert_cluster(conn: &Connection, c: &ClusterUpsert) -> Result<i64> {
    let known_issue_id = c.known_issue_id.filter(|id| {
        conn.query_row(
            "SELECT COUNT(*) FROM known_issues WHERE id = ?1",
            [id],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false)
    });
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM issue_clusters WHERE title = ?1",
            params![c.title],
            |r| r.get(0),
        )
        .ok();
    let cluster_id = match existing {
        Some(id) => {
            conn.execute(
                "UPDATE issue_clusters SET summary=?2, category=?3, product=?4, feature=?5,
                        known_issue_id=?6, updated_at=datetime('now') WHERE id=?1",
                params![
                    id,
                    c.summary,
                    c.category,
                    c.product,
                    c.feature,
                    known_issue_id
                ],
            )?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO issue_clusters (name, title, summary, category, product, feature, known_issue_id, ai_generated)
                 VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    c.title,
                    c.summary,
                    c.category,
                    c.product,
                    c.feature,
                    known_issue_id,
                    i64::from(c.ai_generated)
                ],
            )?;
            conn.last_insert_rowid()
        }
    };
    for id in &c.conversation_ids {
        conn.execute(
            "INSERT OR IGNORE INTO issue_cluster_conversations (cluster_id, conversation_id)
             VALUES (?1, ?2)",
            params![cluster_id, id],
        )?;
    }
    conn.execute(
        "UPDATE issue_clusters SET
            conversation_count = (SELECT COUNT(*) FROM issue_cluster_conversations WHERE cluster_id = ?1),
            customer_count = (SELECT COUNT(DISTINCT c.customer_local_id) FROM issue_cluster_conversations m
                               JOIN conversations c ON c.id = m.conversation_id
                              WHERE m.cluster_id = ?1 AND c.customer_local_id IS NOT NULL),
            first_seen_at = COALESCE((SELECT MIN(COALESCE(c.remote_created_at, c.created_at)) FROM issue_cluster_conversations m
                               JOIN conversations c ON c.id = m.conversation_id WHERE m.cluster_id = ?1), first_seen_at),
            last_seen_at = COALESCE((SELECT MAX(COALESCE(c.remote_created_at, c.created_at)) FROM issue_cluster_conversations m
                              JOIN conversations c ON c.id = m.conversation_id WHERE m.cluster_id = ?1), last_seen_at)
          WHERE id = ?1",
        params![cluster_id],
    )?;
    Ok(cluster_id)
}

fn compute_trends(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "UPDATE issue_clusters SET trend = CASE
            WHEN julianday(first_seen_at) >= julianday('now', '-14 days') THEN 'new'
            ELSE 'stable'
        END;",
    )?;
    let mut stmt = conn.prepare(
        "SELECT ic.id,
            (SELECT COUNT(*) FROM issue_cluster_conversations m JOIN conversations c ON c.id = m.conversation_id
              WHERE m.cluster_id = ic.id AND julianday(COALESCE(c.remote_created_at, c.created_at)) >= julianday('now', '-14 days')) AS recent,
            (SELECT COUNT(*) FROM issue_cluster_conversations m JOIN conversations c ON c.id = m.conversation_id
              WHERE m.cluster_id = ic.id AND julianday(COALESCE(c.remote_created_at, c.created_at)) >= julianday('now', '-28 days')
                AND julianday(COALESCE(c.remote_created_at, c.created_at)) < julianday('now', '-14 days')) AS previous
           FROM issue_clusters ic",
    )?;
    let rows: Vec<(i64, f64, f64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .filter_map(|r| r.ok())
        .collect();
    for (id, recent, previous) in rows {
        let trend = if recent > previous * 1.3 && recent >= 3.0 {
            "rising"
        } else if previous > 0.0 && recent < previous * 0.7 {
            "falling"
        } else if previous == 0.0 && recent > 0.0 {
            "new"
        } else {
            "stable"
        };
        conn.execute(
            "UPDATE issue_clusters SET trend = ?1 WHERE id = ?2",
            params![trend, id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
fn digest_row(
    conn: &rusqlite::Connection,
    id: i64,
) -> (String, Option<String>, String, serde_json::Value) {
    conn.query_row(
        "SELECT status, narrative, trigger_kind, facts FROM ai_digests WHERE id = ?1",
        [id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
                serde_json::from_str::<serde_json::Value>(&r.get::<_, String>(3)?)
                    .expect("facts json"),
            ))
        },
    )
    .expect("digest row")
}

#[cfg(test)]
mod batch13_a3_a6_tests;
#[cfg(test)]
mod batch14_k2_k4_tests;
#[cfg(test)]
mod batch23_audit_tests;
#[cfg(test)]
mod batch28_ai_tests;
#[cfg(test)]
mod batch29_audit97_tests;

pub(crate) async fn draft_article(
    conn: &Connection,
    backend: &AiBackend,
    candidate_id: i64,
) -> std::result::Result<i64, LmStudioError> {
    let Some(candidate) = crate::quality::gap_by_id(conn, candidate_id) else {
        return Err(LmStudioError::new(
            format!("Gap candidate {candidate_id} does not exist."),
            false,
        ));
    };
    if crate::knowledge_store::article_draft_exists_for_candidate(conn, candidate_id) {
        return Err(LmStudioError::new(
            "An article draft for this gap is already in the review queue.",
            false,
        ));
    }
    let question = candidate["query_text"]
        .as_str()
        .or_else(|| candidate["question"].as_str())
        .unwrap_or_default()
        .to_string();
    let evidence_ids: Vec<i64> = candidate["evidence_conversation_ids"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_i64()).collect::<Vec<i64>>())
        .unwrap_or_default();
    let resolutions = crate::quality::best_resolutions(conn, &evidence_ids, 6);
    if resolutions.is_empty() {
        return Err(LmStudioError::new(
            "No closed conversations with a published reply among the evidence - \
             there is nothing grounded to draft an article from. Close the \
             conversations or wait for real resolutions.",
            false,
        ));
    }
    if matches!(backend, AiBackend::Disabled) {
        return Err(LmStudioError::new(AI_DISABLED_MESSAGE, false));
    }
    let run_id = start_run(
        conn,
        "article_draft",
        None,
        None,
        crate::ai_prompts::PROMPT_VERSIONS_ARTICLE_DRAFT,
        None,
        &serde_json::json!([candidate_id]),
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let res = backend
        .chat_json(
            conn,
            crate::ai_prompts::ARTICLE_DRAFT_SYSTEM,
            &crate::ai_prompts::build_article_draft_user(&question, &resolutions),
            true,
            Some(3072),
        )
        .await;
    let res = match res {
        Ok(r) => r,
        Err(e) => {
            let _ = fail_run(conn, run_id, &e.message);
            return Err(e);
        }
    };
    let json = res.json.clone().unwrap_or_else(|| serde_json::json!({}));
    let title = json["title"].as_str().unwrap_or_default().trim();
    let body = json["body_markdown"].as_str().unwrap_or_default().trim();
    if title.is_empty() || body.is_empty() {
        let _ = fail_run(
            conn,
            run_id,
            "model returned no article (empty title or body)",
        );
        return Err(LmStudioError::new(
            "The model did not return a usable article (empty title or body).",
            false,
        ));
    }
    let citations = serde_json::to_value(&resolutions).unwrap_or_else(|_| serde_json::json!([]));
    let draft_id = crate::knowledge_store::insert_article_draft(
        conn,
        Some(candidate_id),
        title,
        body,
        &citations,
        Some(run_id),
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let _ = complete_run_with_usage(
        conn,
        run_id,
        &serde_json::json!({ "draft_id": draft_id, "title": title }),
        res.latency_ms,
        res.usage.as_ref(),
        backend.endpoint_id(),
    );
    Ok(draft_id)
}

pub(crate) async fn weekly_digest(
    conn: &Connection,
    backend: &AiBackend,
    trigger_kind: &str,
    days: i64,
) -> std::result::Result<i64, LmStudioError> {
    let (facts, start, end) = crate::digest::collect_facts(conn, days);
    let (narrative, run_id, status) = match backend {
        AiBackend::Disabled => (None, None, "facts_only"),
        AiBackend::LmStudio { .. } => {
            let rid = start_run(
                conn,
                "weekly_digest",
                None,
                None,
                crate::ai_prompts::PROMPT_VERSIONS_WEEKLY_DIGEST,
                None,
                &serde_json::json!([trigger_kind, days]),
            )
            .map_err(|e| LmStudioError::new(e.to_string(), true))?;
            match backend
                .chat_json(
                    conn,
                    crate::ai_prompts::WEEKLY_DIGEST_SYSTEM,
                    &crate::ai_prompts::build_weekly_digest_user(&facts),
                    false,
                    Some(2048),
                )
                .await
            {
                Ok(res) => {
                    let narrative = res
                        .json
                        .as_ref()
                        .and_then(|j| j.get("narrative"))
                        .and_then(|n| n.as_str())
                        .map(str::to_string)
                        .or_else(|| {
                            res.raw
                                .as_deref()
                                .map(|r| r.trim().to_string())
                                .filter(|r| !r.is_empty())
                        });
                    match narrative {
                        Some(n) if !n.trim().is_empty() => {
                            let _ = complete_run_with_usage(
                                conn,
                                rid,
                                &serde_json::json!({ "narrative": n }),
                                res.latency_ms,
                                res.usage.as_ref(),
                                backend.endpoint_id(),
                            );
                            (Some(n), Some(rid), "complete")
                        }
                        _ => {
                            let _ = fail_run(
                                conn,
                                rid,
                                "the model returned no usable narrative for the digest",
                            );
                            (None, Some(rid), "failed")
                        }
                    }
                }
                Err(e) => {
                    let _ = fail_run(conn, rid, &e.message);
                    (None, Some(rid), "failed")
                }
            }
        }
    };
    crate::digest::store_digest(
        conn,
        &crate::digest::NewDraft {
            period_start: &start,
            period_end: &end,
            trigger_kind,
            facts: &facts,
            narrative: narrative.as_deref(),
            ai_run_id: run_id,
            status,
        },
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))
}
