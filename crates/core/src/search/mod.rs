use rusqlite::{Connection, OptionalExtension, ToSql, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::migrations::add_column_if_missing;

pub const MAX_SEARCH_QUERY_LENGTH: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub scope: String,
    pub id: i64,
    pub title: String,
    pub subtitle: String,
    pub snippet: String,
    #[serde(serialize_with = "serialize_score")]
    pub score: f64,
    pub href: String,
    pub why: Vec<String>,
}

fn serialize_score<S>(value: &f64, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 9.0e15 {
        serializer.serialize_i64(*value as i64)
    } else {
        serializer.serialize_f64(*value)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    pub status: Option<String>,
    pub mailbox_id: Option<i64>,
    pub tag: Option<String>,
    pub since_days: Option<i64>,
    pub assignee_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub hits: Vec<SearchHit>,
    pub total: usize,
    pub used_semantic: bool,
    pub semantic_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRawHit {
    pub document_id: i64,
    pub title: String,
    pub snippet: String,
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub resource_type: String,
    pub remote_id: i64,
    pub title: String,
    pub snippet: String,
}

pub fn apply_fts_migration(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS fts_conversations USING fts5(
            subject, preview, customer, tags, numbers,
            conversation_id UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_threads USING fts5(
            body,
            thread_id UNINDEXED, conversation_id UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_knowledge USING fts5(
            title, content,
            chunk_id UNINDEXED, document_id UNINDEXED, visibility UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_known_issues USING fts5(
            title, symptoms, workaround, customer_safe_explanation,
            known_issue_id UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_saved_replies USING fts5(
            name, preview, text,
            saved_reply_id UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_ai_analyses USING fts5(
            summary, primary_question, intent,
            conversation_id UNINDEXED, run_id UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS docs_fts USING fts5(
            name, text,
            article_id UNINDEXED
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_custom_objects USING fts5(
            title,
            search_text,
            object_id UNINDEXED
        );

        -- Clean-audit item 155: the legacy pre-parity tables
        -- (conversations_fts / customers_fts, written by the dead
        -- index_conversation / index_customer writers and never queried
        -- by any search lane) are no longer created.",
    )?;

    if table_exists(conn, "conversations")? {
        add_column_if_missing(conn, "conversations", "deleted_at", "TEXT")?;
        add_column_if_missing(
            conn,
            "conversations",
            "merged_into_conversation_id",
            "INTEGER",
        )?;
    }
    if table_exists(conn, "customers")? {
        add_column_if_missing(conn, "customers", "deleted_at", "TEXT")?;
        add_column_if_missing(conn, "customers", "organization_id", "INTEGER")?;
    }
    if table_exists(conn, "conversation_threads")? {
        add_column_if_missing(conn, "conversation_threads", "deleted_at", "TEXT")?;
        add_column_if_missing(
            conn,
            "conversation_threads",
            "fts_indexed",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
    }
    if table_exists(conn, "known_issues")? {
        add_column_if_missing(conn, "known_issues", "title", "TEXT")?;
        add_column_if_missing(conn, "known_issues", "symptoms", "TEXT")?;
        add_column_if_missing(conn, "known_issues", "workaround", "TEXT")?;
        add_column_if_missing(conn, "known_issues", "customer_safe_explanation", "TEXT")?;
    }

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS knowledge_sources (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            kind        TEXT DEFAULT 'local_file',
            visibility  TEXT NOT NULL DEFAULT 'internal_only',
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS knowledge_documents (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id       INTEGER NOT NULL REFERENCES knowledge_sources (id) ON DELETE CASCADE,
            title           TEXT NOT NULL,
            visibility      TEXT NOT NULL DEFAULT 'internal_only',
            version         INTEGER DEFAULT 1,
            checksum        TEXT,
            content         TEXT,
            format          TEXT DEFAULT 'markdown',
            last_reviewed_at TEXT,
            last_verified_at TEXT,
            last_indexed_at TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
            provenance      TEXT DEFAULT 'human_local'
        );
        CREATE INDEX IF NOT EXISTS idx_knowledge_documents_source
            ON knowledge_documents (source_id);

        CREATE TABLE IF NOT EXISTS knowledge_chunks (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            document_id     INTEGER NOT NULL REFERENCES knowledge_documents (id) ON DELETE CASCADE,
            chunk_index     INTEGER NOT NULL,
            content         TEXT NOT NULL,
            fts_indexed     INTEGER DEFAULT 0,
            embedding_state TEXT DEFAULT 'not_indexed',
            embedding_model TEXT,
            embedding       BLOB,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (document_id, chunk_index)
        );
        CREATE INDEX IF NOT EXISTS idx_knowledge_chunks_doc ON knowledge_chunks (document_id);

        CREATE TABLE IF NOT EXISTS customer_emails (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id   INTEGER UNIQUE,
            customer_id INTEGER NOT NULL REFERENCES customers (id) ON DELETE CASCADE,
            value       TEXT NOT NULL,
            type        TEXT,
            UNIQUE (customer_id, value)
        );
        CREATE INDEX IF NOT EXISTS idx_customer_emails_value ON customer_emails (value);

        CREATE TABLE IF NOT EXISTS ai_runs (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            input_hash      TEXT NOT NULL,
            prompt_version  TEXT NOT NULL,
            model           TEXT NOT NULL,
            response_json   TEXT NOT NULL,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_ai_runs_lookup
            ON ai_runs (input_hash, prompt_version, model);

        CREATE TABLE IF NOT EXISTS ai_extracted_facts (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER REFERENCES conversations (id) ON DELETE CASCADE,
            run_id          INTEGER REFERENCES ai_runs (id) ON DELETE CASCADE,
            key             TEXT NOT NULL,
            value           TEXT,
            confidence      TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_ai_facts_conversation
            ON ai_extracted_facts (conversation_id);

        CREATE TABLE IF NOT EXISTS organizations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id INTEGER UNIQUE NOT NULL,
            name TEXT NOT NULL,
            domains TEXT,
            raw_json TEXT,
            raw_json_hash TEXT,
            remote_created_at TEXT,
            remote_updated_at TEXT,
            last_seen_at TEXT,
            last_synced_at TEXT,
            local_created_at TEXT NOT NULL DEFAULT (datetime('now')),
            local_updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            deleted_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_organizations_name ON organizations(name);

        CREATE TABLE IF NOT EXISTS conversation_tags (
            conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            tag_id         INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (conversation_id, tag_id)
        );
        CREATE INDEX IF NOT EXISTS idx_conversation_tags_tag
            ON conversation_tags(tag_id);",
    )?;
    Ok(())
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?1",
            params![name],
            |_| Ok(()),
        )
        .optional()?;
    Ok(exists.is_some())
}

#[must_use]
pub fn fts_query(q: &str) -> String {
    let tokens = tokenize(q, 1, 8);
    if tokens.is_empty() {
        return "\"\"".to_string();
    }
    tokens
        .iter()
        .map(|t| format!("\"{t}\"*"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[must_use]
pub fn fts_query_or(q: &str, max_tokens: usize) -> String {
    let tokens = tokenize(q, 3, max_tokens);
    if tokens.is_empty() {
        return "\"\"".to_string();
    }
    tokens
        .iter()
        .map(|t| format!("\"{t}\"*"))
        .collect::<Vec<_>>()
        .join(" OR ")
}

pub fn tokenize(q: &str, min_chars: usize, max_tokens: usize) -> Vec<String> {
    let cleaned: String = q
        .chars()
        .map(|c| {
            if matches!(c, '"' | '*' | '(' | ')') {
                ' '
            } else {
                c
            }
        })
        .collect();
    cleaned
        .split_whitespace()
        .filter(|t| t.chars().count() >= min_chars)
        .take(max_tokens)
        .map(str::to_string)
        .collect()
}

#[must_use]
pub fn escape_like(q: &str) -> String {
    let mut out = String::with_capacity(q.len());
    for c in q.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

mod lanes;

pub use lanes::{
    search_ai_analyses, search_conversations, search_customers, search_knowledge,
    search_knowledge_raw, search_known_issues, search_saved_replies,
};

pub fn search(
    conn: &Connection,
    query: &str,
    scope: &str,
    filters: &SearchFilters,
) -> Result<SearchResponse> {
    apply_fts_migration(conn)?;
    let scope = if scope.is_empty() { "all" } else { scope };
    let mut hits = Vec::new();
    if scope == "all" || scope == "tickets" {
        hits.extend(search_conversations(conn, query, filters, 40)?);
    }
    if scope == "all" || scope == "customers" {
        hits.extend(search_customers(conn, query, 15)?);
    }
    if scope == "all" || scope == "knowledge" {
        hits.extend(search_knowledge(conn, query, None, 10)?);
    }
    if scope == "all" || scope == "issues" {
        hits.extend(search_known_issues(conn, query, 10)?);
    }
    if scope == "all" || scope == "saved_replies" {
        hits.extend(search_saved_replies(conn, query, 10)?);
    }
    if scope == "all" || scope == "ai" {
        hits.extend(search_ai_analyses(conn, query, 10)?);
    }
    let total = hits.len();
    Ok(SearchResponse {
        query: query.to_string(),
        hits,
        total,
        used_semantic: false,
        semantic_available: false,
        mode_note: None,
    })
}

pub fn search_with_semantics(
    conn: &Connection,
    index: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
    query: &str,
    scope: &str,
    filters: &SearchFilters,
) -> Result<SearchResponse> {
    let mut response = search(conn, query, scope, filters)?;
    let scope = if scope.is_empty() { "all" } else { scope };

    let index = match index {
        Some(ix) => ix,
        None => {
            response.semantic_available = false;
            response.mode_note =
                Some("FTS only — the vector index is off (Settings > Vector index).".into());
            return Ok(response);
        }
    };
    if !crate::ai_hub::ai_feature_enabled(conn, "retrieval") {
        response.semantic_available = false;
        response.mode_note =
            Some("FTS only — semantic retrieval is disabled (AI Center > Features).".into());
        return Ok(response);
    }
    let embed_settings = crate::embeddings::lm_studio_embedding_settings(conn)?;
    if embed_settings.embedding_model.is_none() {
        response.semantic_available = false;
        response.mode_note = Some(
            "FTS only — no embedding model routed (AI Center > Model-role routing, Embed role)."
                .into(),
        );
        return Ok(response);
    }
    response.semantic_available = true;

    let query_vec = match crate::embeddings::embed_query_text_blocking(conn, query) {
        Ok(Some(v)) => v,
        Ok(None) => {
            response.mode_note = Some(
                "FTS only — the embedding endpoint could not be reached for this query.".into(),
            );
            return Ok(response);
        }
        Err(e) => {
            response.mode_note = Some(format!(
                "FTS only — the embedding endpoint could not be reached for this query: {e}"
            ));
            return Ok(response);
        }
    };
    let dense = index.search(&query_vec, 60);

    let want_tickets = scope == "all" || scope == "tickets";
    let want_knowledge = scope == "all" || scope == "knowledge";
    let mut semantic_hits: Vec<SearchHit> = Vec::new();
    for hit in dense {
        let payload = match hit.payload.as_object() {
            Some(p) => p,
            None => continue,
        };
        let entity_type = payload.get("entity_type").and_then(|v| v.as_str());
        let entity_id = payload.get("entity_id").and_then(|v| v.as_i64());
        let Some(entity_id) = entity_id else { continue };
        let text: String = payload
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .chars()
            .take(240)
            .collect();
        match entity_type {
            Some("conversation_chunk") if want_tickets => {
                let Ok((number, subject, status)) = conn.query_row(
                    "SELECT number, subject, status FROM conversations
                          WHERE id = ?1 AND deleted_at IS NULL",
                    params![entity_id],
                    |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, Option<String>>(1)?,
                            r.get::<_, Option<String>>(2)?,
                        ))
                    },
                ) else {
                    continue;
                };
                semantic_hits.push(SearchHit {
                    scope: "tickets".into(),
                    id: entity_id,
                    title: format!(
                        "#{number} {}",
                        subject.unwrap_or_else(|| "(no subject)".into())
                    ),
                    subtitle: status.unwrap_or_default(),
                    snippet: text,
                    score: (f64::from(hit.score) * 100.0).round() / 100.0,
                    href: format!("/inbox/conversation/{entity_id}"),
                    why: vec!["semantic match".into()],
                });
            }
            Some("knowledge_chunk") if want_knowledge => {
                let Ok((document_id, title)) = conn.query_row(
                    "SELECT d.id, d.title FROM knowledge_chunks c
                          JOIN knowledge_documents d ON d.id = c.document_id
                          WHERE c.id = ?1",
                    params![
                        payload
                            .get("chunk_id")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0)
                    ],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
                ) else {
                    continue;
                };
                semantic_hits.push(SearchHit {
                    scope: "knowledge".into(),
                    id: document_id,
                    title,
                    subtitle: "Knowledge base".into(),
                    snippet: text,
                    score: (f64::from(hit.score) * 100.0).round() / 100.0,
                    href: format!("/knowledge?doc={document_id}"),
                    why: vec!["semantic match".into()],
                });
            }
            Some("docs_chunk") if want_knowledge => {
                let Ok((article_id, name)) = conn.query_row(
                    "SELECT a.id, a.name FROM docs_articles a WHERE a.id = ?1",
                    params![entity_id],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
                ) else {
                    continue;
                };
                semantic_hits.push(SearchHit {
                    scope: "knowledge".into(),
                    id: article_id,
                    title: name,
                    subtitle: "Help center article".into(),
                    snippet: text,
                    score: (f64::from(hit.score) * 100.0).round() / 100.0,
                    href: format!("/knowledge/help-center/{article_id}"),
                    why: vec!["semantic match".into()],
                });
            }
            _ => {}
        }
    }
    if semantic_hits.is_empty() {
        response.mode_note =
            Some("FTS only — the vector index has no indexed content for this query yet.".into());
        return Ok(response);
    }

    let k = crate::rrf::RRF_K;
    let mut fused: Vec<SearchHit> = Vec::new();
    for lane in ["tickets", "knowledge"] {
        let fts_lane: Vec<&SearchHit> = response.hits.iter().filter(|h| h.scope == lane).collect();
        let sem_lane: Vec<&SearchHit> = semantic_hits.iter().filter(|h| h.scope == lane).collect();
        if sem_lane.is_empty() {
            continue;
        }
        struct Fused {
            hit: SearchHit,
            rrf: f64,
        }
        let mut merged: Vec<Fused> = Vec::new();
        let push_or_merge = |hit: &SearchHit, rrf: f64, merged: &mut Vec<Fused>| {
            if let Some(existing) = merged
                .iter_mut()
                .find(|f| f.hit.scope == hit.scope && f.hit.id == hit.id)
            {
                existing.rrf += rrf;
                for reason in &hit.why {
                    if !existing.hit.why.contains(reason) {
                        existing.hit.why.push(reason.clone());
                    }
                }
            } else {
                merged.push(Fused {
                    hit: hit.clone(),
                    rrf,
                });
            }
        };
        for (rank, hit) in fts_lane.iter().enumerate() {
            push_or_merge(hit, 1.0 / (f64::from(k) + rank as f64 + 1.0), &mut merged);
        }
        for (rank, hit) in sem_lane.iter().enumerate() {
            push_or_merge(hit, 1.0 / (f64::from(k) + rank as f64 + 1.0), &mut merged);
        }
        merged.sort_by(|a, b| {
            b.rrf
                .partial_cmp(&a.rrf)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let lane_limit = if lane == "tickets" { 40 } else { 10 };
        for (i, f) in merged.into_iter().enumerate() {
            if i >= lane_limit {
                break;
            }
            let mut hit = f.hit;
            hit.score = (f.rrf * 10_000.0).round() / 10_000.0;
            fused.push(hit);
        }
        response.hits.retain(|h| h.scope != lane);
    }
    let fused_any = !fused.is_empty();
    if fused_any {
        rescore_fts_only_lanes_to_rrf_scale(&mut response.hits, k);
    }
    response.hits.extend(fused);
    response.hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    response.total = response.hits.len();
    if fused_any {
        response.used_semantic = true;
        response.mode_note = Some(format!(
            "BM25 + sqlite-vec fused (RRF k={k}, embeddings local)"
        ));
    }
    Ok(response)
}

fn rescore_fts_only_lanes_to_rrf_scale(hits: &mut [SearchHit], k: u32) {
    const FTS_ONLY_LANES: [&str; 4] = ["customers", "issues", "saved_replies", "ai"];
    let mut rank_by_lane: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for hit in hits.iter_mut() {
        if FTS_ONLY_LANES.contains(&hit.scope.as_str()) {
            let rank = rank_by_lane.entry(hit.scope.as_str()).or_insert(0);
            hit.score = 1.0 / (f64::from(k) + *rank as f64 + 1.0);
            *rank += 1;
        }
    }
}

pub fn universal_search(conn: &Connection, query: &str) -> Result<Vec<SearchResult>> {
    let query = cap_query(query);
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let response = search(conn, query, "all", &SearchFilters::default())?;
    Ok(response
        .hits
        .into_iter()
        .map(|h| SearchResult {
            resource_type: h.scope,
            remote_id: h.id,
            title: h.title,
            snippet: h.snippet,
        })
        .collect())
}

fn cap_query(query: &str) -> &str {
    if query.len() <= MAX_SEARCH_QUERY_LENGTH {
        return query;
    }
    let mut end = MAX_SEARCH_QUERY_LENGTH;
    while !query.is_char_boundary(end) {
        end -= 1;
    }
    &query[..end]
}

pub fn index_conversation_fts(conn: &Connection, local_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM fts_conversations WHERE conversation_id = ?1",
        params![local_id],
    )?;
    let row = conn
        .query_row(
            "SELECT c.id, c.number, c.subject, c.preview,
               TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, '')) AS customer,
               (SELECT GROUP_CONCAT(t.name) FROM conversation_tags ct
                  JOIN tags t ON t.id = ct.tag_id WHERE ct.conversation_id = c.id) AS tags
             FROM conversations c LEFT JOIN customers cu ON cu.id = c.customer_local_id
            WHERE c.id = ?1",
            params![local_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()?;
    if let Some((id, number, subject, preview, customer, tags)) = row {
        conn.execute(
            "INSERT INTO fts_conversations (subject, preview, customer, tags, numbers, conversation_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                subject.unwrap_or_default(),
                preview.unwrap_or_default(),
                customer.unwrap_or_default(),
                tags.unwrap_or_default(),
                number.to_string(),
                id
            ],
        )?;
        conn.execute(
            "DELETE FROM fts_threads WHERE conversation_id = ?1",
            params![local_id],
        )?;
        if table_exists(conn, "conversation_threads")? {
            conn.execute(
                "INSERT INTO fts_threads (body, thread_id, conversation_id)
                 SELECT t.body_text, t.id, t.conversation_id FROM conversation_threads t
                  WHERE t.conversation_id = ?1 AND t.body_text IS NOT NULL AND LENGTH(t.body_text) > 0
                    AND t.deleted_at IS NULL",
                params![local_id],
            )?;
        }
    }
    Ok(())
}

pub fn rebuild_indexes(conn: &Connection) -> Result<usize> {
    apply_fts_migration(conn)?;
    conn.execute_batch("BEGIN")?;
    let result = rebuild_indexes_tx(conn);
    match result {
        Ok(n) => {
            conn.execute_batch("COMMIT")?;
            Ok(n)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

fn rebuild_indexes_tx(conn: &Connection) -> Result<usize> {
    for table in [
        "fts_conversations",
        "fts_threads",
        "fts_knowledge",
        "fts_known_issues",
        "fts_saved_replies",
        "fts_ai_analyses",
        "docs_fts",
        "fts_custom_objects",
    ] {
        conn.execute(&format!("DELETE FROM {table}"), [])?;
    }

    let conv_ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM conversations WHERE deleted_at IS NULL")?;

        stmt.query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for id in &conv_ids {
        index_conversation_fts(conn, *id)?;
    }
    let conv_count = conv_ids.len();

    if table_exists(conn, "conversation_threads")? {
        conn.execute(
            "UPDATE conversation_threads SET fts_indexed = 1
              WHERE body_text IS NOT NULL AND LENGTH(body_text) > 0",
            [],
        )?;
    }

    conn.execute(
        "INSERT INTO fts_knowledge (title, content, chunk_id, document_id, visibility)
         SELECT d.title, c.content, c.id, c.document_id, d.visibility
           FROM knowledge_chunks c JOIN knowledge_documents d ON d.id = c.document_id",
        [],
    )?;
    conn.execute("UPDATE knowledge_chunks SET fts_indexed = 1", [])?;

    if table_exists(conn, "known_issues")? {
        conn.execute(
            "INSERT INTO fts_known_issues (title, symptoms, workaround, customer_safe_explanation, known_issue_id)
             SELECT COALESCE(title, ''), COALESCE(symptoms, ''), COALESCE(workaround, ''),
                    COALESCE(customer_safe_explanation, ''), id
               FROM known_issues",
            [],
        )?;
    }

    if table_exists(conn, "saved_replies")? {
        conn.execute(
            "INSERT INTO fts_saved_replies (saved_reply_id, name, preview, text)
             SELECT id, name, COALESCE(preview, ''), COALESCE(text, '')
               FROM saved_replies WHERE deleted_at IS NULL",
            [],
        )?;
    }

    conn.execute(
        "INSERT INTO fts_ai_analyses (summary, primary_question, intent, conversation_id, run_id)
         SELECT COALESCE(MAX(CASE WHEN key = 'summary' THEN value END), ''),
                COALESCE(MAX(CASE WHEN key = 'primary_question' THEN value END), ''),
                COALESCE(MAX(CASE WHEN key = 'intent' THEN value END), ''),
                conversation_id, run_id
           FROM ai_extracted_facts
          WHERE key IN ('summary', 'primary_question', 'intent') AND conversation_id IS NOT NULL
          GROUP BY conversation_id, run_id",
        [],
    )?;

    if table_exists(conn, "docs_articles")? {
        conn.execute(
            "INSERT INTO docs_fts (name, text, article_id)
             SELECT name, COALESCE(text, ''), id FROM docs_articles WHERE deleted_at IS NULL",
            [],
        )?;
    }

    if table_exists(conn, "custom_objects")? {
        rebuild_custom_objects_fts(conn)?;
    }

    Ok(conv_count)
}

fn rebuild_custom_objects_fts(conn: &Connection) -> Result<()> {
    let rows: Vec<(i64, String, Option<String>)> = {
        let mut stmt = conn.prepare(
            "SELECT o.id, o.title, o.data_json
               FROM custom_objects o
               JOIN custom_object_types t ON t.id = o.type_id
              WHERE o.deleted_at IS NULL",
        )?;

        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (id, title, data_json) in rows {
        let search_text = build_custom_object_search_text(&title, data_json.as_deref());
        conn.execute(
            "INSERT INTO fts_custom_objects (title, search_text, object_id) VALUES (?1, ?2, ?3)",
            params![title, search_text, id],
        )?;
    }
    Ok(())
}

fn build_custom_object_search_text(title: &str, data_json: Option<&str>) -> String {
    let mut parts = vec![title.to_string()];
    if let Some(json) = data_json
        && let Ok(serde_json::Value::Object(map)) = serde_json::from_str(json)
    {
        for value in map.values() {
            match value {
                serde_json::Value::String(s) => parts.push(s.clone()),
                serde_json::Value::Number(n) => parts.push(n.to_string()),
                serde_json::Value::Bool(b) => parts.push(b.to_string()),
                _ => {}
            }
        }
    }
    let joined = parts.join(" ");
    if joined.chars().count() > 5000 {
        joined.chars().take(5000).collect()
    } else {
        joined
    }
}

#[cfg(test)]
mod batch13_a4_search_tests {
    use super::*;
    use rusqlite::Connection;

    fn db_with_conversation() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (9601, 96, 'Export timeout errors', 'The export throws errors.', 'active')",
            [],
        )
        .expect("conversation");
        apply_fts_migration(&conn).expect("fts");
        rebuild_indexes(&conn).expect("index");
        conn
    }

    #[test]
    fn fused_search_degrades_honestly_without_index() {
        let conn = db_with_conversation();
        let response =
            search_with_semantics(&conn, None, "export", "all", &SearchFilters::default())
                .expect("search");
        assert!(!response.used_semantic, "no vector lane ran");
        assert!(
            response.mode_note.is_some(),
            "the fallback is explained, not silent"
        );
        assert!(
            !response.hits.is_empty(),
            "the FTS lane still finds the conversation"
        );
        assert!(
            response
                .hits
                .iter()
                .all(|h| h.why.contains(&"keyword match".to_string())),
            "lane provenance is reported"
        );
    }

    #[test]
    fn fused_search_with_disabled_index_stays_fts_only() {
        let conn = db_with_conversation();
        let dir = std::env::temp_dir().join(format!("spp-search-test-{}", std::process::id()));
        let index = crate::vectorstore_sqlitevec::VectorIndex::new(&dir, false);
        let response = search_with_semantics(
            &conn,
            Some(&index),
            "export",
            "all",
            &SearchFilters::default(),
        )
        .expect("search");
        assert!(!response.used_semantic, "a disabled index never fuses");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod batch25_search_tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        apply_fts_migration(&conn).expect("fts");
        conn
    }

    fn seed_knowledge(conn: &Connection, docs: &[(&str, &[&str])]) {
        conn.execute(
            "INSERT INTO knowledge_sources (name, kind) VALUES ('src', 'local_file')",
            [],
        )
        .expect("source");
        let source_id = conn.last_insert_rowid();
        for (title, chunks) in docs {
            conn.execute(
                "INSERT INTO knowledge_documents (source_id, title, visibility, content)
                 VALUES (?1, ?2, 'internal_only', ?3)",
                params![source_id, title, chunks.join("\n\n")],
            )
            .expect("document");
            let document_id = conn.last_insert_rowid();
            for (index, content) in chunks.iter().enumerate() {
                conn.execute(
                    "INSERT INTO knowledge_chunks (document_id, chunk_index, content)
                     VALUES (?1, ?2, ?3)",
                    params![document_id, index as i64, content],
                )
                .expect("chunk");
                let chunk_id = conn.last_insert_rowid();
                conn.execute(
                    "INSERT INTO fts_knowledge (title, content, chunk_id, document_id, visibility)
                     VALUES (?1, ?2, ?3, ?4, 'internal_only')",
                    params![title, content, chunk_id, document_id],
                )
                .expect("fts row");
            }
        }
    }

    #[test]
    fn audit58_knowledge_hits_dedupe_per_document() {
        let conn = db();
        seed_knowledge(
            &conn,
            &[
                (
                    "Invoice manual",
                    &[
                        "invoice one",
                        "invoice two",
                        "invoice three",
                        "invoice four",
                        "invoice five",
                        "invoice six",
                    ],
                ),
                ("Invoice FAQ", &["the invoice cycle"]),
                ("Invoice policy", &["pay the invoice in 30 days"]),
            ],
        );
        let hits =
            search_knowledge_raw(&conn, "invoice", None, 3, "and").expect("knowledge search");
        assert_eq!(
            hits.len(),
            3,
            "three DISTINCT documents fill the limit of three: {hits:?}"
        );
        let mut document_ids: Vec<i64> = hits.iter().map(|h| h.document_id).collect();
        document_ids.sort();
        document_ids.dedup();
        assert_eq!(
            document_ids.len(),
            3,
            "every hit is a different document: {hits:?}"
        );
        assert!(
            hits.iter().any(|h| h.title == "Invoice FAQ")
                && hits.iter().any(|h| h.title == "Invoice policy"),
            "single-chunk documents are not crowded out: {hits:?}"
        );
    }

    #[test]
    fn audit58_dedupe_respects_the_visibility_filter() {
        let conn = db();
        conn.execute(
            "INSERT INTO knowledge_sources (name, kind) VALUES ('src', 'local_file')",
            [],
        )
        .expect("source");
        let source_id = conn.last_insert_rowid();
        for (title, visibility) in [
            ("Safe doc", "customer_safe"),
            ("Internal doc", "internal_only"),
        ] {
            conn.execute(
                "INSERT INTO knowledge_documents (source_id, title, visibility, content)
                 VALUES (?1, ?2, ?3, 'billing content')",
                params![source_id, title, visibility],
            )
            .expect("document");
            let document_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO knowledge_chunks (document_id, chunk_index, content)
                 VALUES (?1, 0, 'billing content')",
                params![document_id],
            )
            .expect("chunk");
            let chunk_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO fts_knowledge (title, content, chunk_id, document_id, visibility)
                 VALUES (?1, 'billing content', ?2, ?3, ?4)",
                params![title, chunk_id, document_id, visibility],
            )
            .expect("fts row");
        }
        let hits = search_knowledge_raw(&conn, "billing", Some("customer_safe"), 5, "and")
            .expect("search");
        assert_eq!(hits.len(), 1, "only the customer-safe document matches");
        assert_eq!(hits[0].title, "Safe doc");
        assert_eq!(hits[0].visibility, "customer_safe");
    }

    fn hit(scope: &str, id: i64, score: f64) -> SearchHit {
        SearchHit {
            scope: scope.into(),
            id,
            title: format!("{scope} {id}"),
            subtitle: String::new(),
            snippet: String::new(),
            score,
            href: String::new(),
            why: vec!["keyword match".into()],
        }
    }

    #[test]
    fn audit59_fts_only_lanes_rescore_to_the_rrf_scale() {
        let k = crate::rrf::RRF_K;
        let mut hits = vec![
            hit("customers", 1, 1.0),
            hit("customers", 2, 1.0),
            hit("issues", 7, 1.0),
            hit("saved_replies", 9, 1.0),
        ];
        rescore_fts_only_lanes_to_rrf_scale(&mut hits, k);
        let expected = |rank: usize| 1.0 / (f64::from(k) + rank as f64 + 1.0);
        assert!(
            (hits[0].score - expected(0)).abs() < 1e-12,
            "customers rank 0: {}",
            hits[0].score
        );
        assert!(
            (hits[1].score - expected(1)).abs() < 1e-12,
            "customers rank 1: {}",
            hits[1].score
        );
        assert!(
            (hits[2].score - expected(0)).abs() < 1e-12,
            "issues rank 0: {}",
            hits[2].score
        );
        assert!(
            (hits[3].score - expected(0)).abs() < 1e-12,
            "saved_replies rank 0: {}",
            hits[3].score
        );
        let fused_both = 1.0 / (f64::from(k) + 1.0) + 1.0 / (f64::from(k) + 2.0);
        let mut all = hits.clone();
        all.push(hit("tickets", 42, fused_both));
        all.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        assert_eq!(
            all[0].id, 42,
            "the hit found by BOTH lanes ranks first on the shared scale"
        );
    }

    #[test]
    fn audit59_rescore_leaves_unknown_lanes_alone() {
        let k = crate::rrf::RRF_K;
        let mut hits = vec![hit("tickets", 5, 0.5), hit("customers", 1, 1.0)];
        rescore_fts_only_lanes_to_rrf_scale(&mut hits, k);
        assert_eq!(
            hits[0].score, 0.5,
            "the tickets lane (fused lanes' scope) is not re-scored by the helper"
        );
        let expected = 1.0 / (f64::from(k) + 1.0);
        assert!(
            (hits[1].score - expected).abs() < 1e-12,
            "the customers lane IS re-scored onto the RRF scale: {}",
            hits[1].score
        );
    }

    #[test]
    fn audit64_rebuild_inserts_each_thread_exactly_once() {
        let conn = db();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (7001, 70, 'Exports', 'preview', 'active')",
            [],
        )
        .expect("conversation");
        let conversation_id = conn.last_insert_rowid();
        for body in [
            "first thread body",
            "second thread body",
            "third thread body",
        ] {
            conn.execute(
                "INSERT INTO conversation_threads
                    (conversation_id, remote_id, type, body_text, created_at)
                 VALUES (?1, NULL, 'customer', ?2, '2026-10-01 10:00:00')",
                params![conversation_id, body],
            )
            .expect("thread");
        }
        rebuild_indexes(&conn).expect("rebuild");
        let fts_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM fts_threads", [], |r| r.get(0))
            .expect("count");
        assert_eq!(
            fts_rows, 3,
            "three live threads, three fts rows — pre-fix this was 6"
        );
        let distinct_thread_ids: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT thread_id) FROM fts_threads",
                [],
                |r| r.get(0),
            )
            .expect("distinct");
        assert_eq!(distinct_thread_ids, 3, "no thread id appears twice");
        let found = search_conversations(&conn, "second thread", &SearchFilters::default(), 10)
            .expect("search");
        assert!(
            found.iter().any(|h| h.id == conversation_id),
            "the rebuilt index still finds the thread text"
        );
    }
}
