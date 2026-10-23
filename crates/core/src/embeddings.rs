use rusqlite::{Connection, params};

use crate::ai_provider::{EmbedResponse, LocalAiProvider};
use crate::error::{Error, Result};

pub const M008_SQL: &str = r#"
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
    CREATE INDEX IF NOT EXISTS idx_ai_runs_created_at
        ON ai_runs (created_at);

    UPDATE app_state SET schema_version = 8 WHERE id = 1;
"#;

pub fn apply_m008(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ai_runs (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            input_hash      TEXT NOT NULL,
            prompt_version  TEXT NOT NULL,
            model           TEXT NOT NULL,
            response_json   TEXT NOT NULL,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_ai_runs_lookup
            ON ai_runs (input_hash, prompt_version, model);
        CREATE INDEX IF NOT EXISTS idx_ai_runs_created_at
            ON ai_runs (created_at);",
    )?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 8 WHERE id = 1", []);
    Ok(())
}

pub const MAX_EMBED_RETRIES: u32 = 3;

#[must_use]
pub fn content_hash(text: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn lookup_cache(
    conn: &Connection,
    input_hash: &str,
    prompt_version: &str,
    model: &str,
) -> Result<Option<EmbedResponse>> {
    let json: Option<String> = conn
        .query_row(
            "SELECT response_json FROM ai_runs
             WHERE input_hash = ?1 AND prompt_version = ?2 AND model = ?3
             ORDER BY created_at DESC
             LIMIT 1",
            params![input_hash, prompt_version, model],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    match json {
        None => Ok(None),
        Some(j) => {
            let resp: EmbedResponse = serde_json::from_str(&j)
                .map_err(|e| Error::Config(format!("ai_runs cache deserialization failed: {e}")))?;
            Ok(Some(resp))
        }
    }
}

fn store_cache(
    conn: &Connection,
    input_hash: &str,
    prompt_version: &str,
    model: &str,
    response: &EmbedResponse,
) -> Result<()> {
    let json = serde_json::to_string(response)
        .map_err(|e| Error::Config(format!("ai_runs cache serialization failed: {e}")))?;
    conn.execute(
        "INSERT INTO ai_runs (input_hash, prompt_version, model, response_json)
         VALUES (?1, ?2, ?3, ?4)",
        params![input_hash, prompt_version, model, json],
    )?;
    Ok(())
}

pub async fn embed_with_cache(
    conn: &Connection,
    provider: &dyn LocalAiProvider,
    model: &str,
    text: &str,
    prompt_version: &str,
) -> Result<EmbedResponse> {
    let hash = content_hash(text);

    if let Some(cached) = lookup_cache(conn, &hash, prompt_version, model)? {
        return Ok(cached);
    }

    let mut last_error: Option<String> = None;
    for attempt in 0..MAX_EMBED_RETRIES {
        match provider.embed(model, text).await {
            Ok(response) => {
                store_cache(conn, &hash, prompt_version, model, &response)?;
                return Ok(response);
            }
            Err(e) => {
                last_error = Some(e.to_string());
                if attempt + 1 < MAX_EMBED_RETRIES {
                    let delay_ms = 100_u64 * (1 << attempt);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    Err(Error::Other(
        format!(
            "embedding failed after {MAX_EMBED_RETRIES} attempts (model={model}): {}",
            last_error.unwrap_or_else(|| "unknown error".into())
        )
        .into(),
    ))
}

pub fn count_cache_entries(conn: &Connection, model: &str) -> Result<u32> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ai_runs WHERE model = ?1",
        params![model],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(count).unwrap_or(0))
}

pub fn trim_cache_per_model(conn: &Connection, max_entries: u32) -> Result<u32> {
    let rows = conn.execute(
        "DELETE FROM ai_runs
         WHERE id NOT IN (
             SELECT id FROM ai_runs a
             WHERE (SELECT COUNT(*) FROM ai_runs b
                    WHERE b.model = a.model AND b.created_at >= a.created_at)
                   <= ?1
         )",
        params![i64::from(max_entries as i32)],
    )?;
    Ok(u32::try_from(rows).unwrap_or(0))
}

pub fn clear_cache(conn: &Connection) -> Result<u32> {
    let rows = conn.execute("DELETE FROM ai_runs", [])?;
    Ok(u32::try_from(rows).unwrap_or(0))
}

pub const EMBED_PASS_LIMIT: i64 = 60;

fn is_js_whitespace(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

pub fn utf16_slice(s: &str, start: usize, end: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    if start >= units.len() || end <= start {
        return String::new();
    }
    let end = end.min(units.len());
    String::from_utf16_lossy(&units[start..end])
}

pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

#[must_use]
pub fn chunk_text(text: &str, max_len: usize, overlap: usize) -> Vec<String> {
    let mut collapsed = String::with_capacity(text.len());
    let mut in_ws = false;
    for c in text.chars() {
        if is_js_whitespace(c) {
            if !in_ws {
                collapsed.push(' ');
                in_ws = true;
            }
        } else {
            collapsed.push(c);
            in_ws = false;
        }
    }
    let clean = collapsed.trim();
    let units: Vec<u16> = clean.encode_utf16().collect();
    if units.len() <= max_len {
        return if clean.is_empty() {
            Vec::new()
        } else {
            vec![clean.to_string()]
        };
    }

    let mut chunks: Vec<String> = Vec::new();
    let mut start: usize = 0;
    while start < units.len() {
        let mut end = (start + max_len).min(units.len());
        if end < units.len()
            && let Some(dot) = last_index_of_dot_space(&units, end)
            && dot as f64 > start as f64 + max_len as f64 * 0.5
        {
            end = dot + 1;
        }
        chunks.push(
            String::from_utf16_lossy(&units[start..end])
                .trim()
                .to_string(),
        );
        start = end.saturating_sub(overlap);
        if end >= units.len() {
            break;
        }
    }
    chunks
}

fn last_index_of_dot_space(units: &[u16], from: usize) -> Option<usize> {
    if units.len() < 2 {
        return None;
    }
    let max_start = from.min(units.len() - 2);
    let mut p = max_start as isize;
    while p >= 0 {
        let idx = p as usize;
        if units[idx] == u16::from(b'.') && units[idx + 1] == u16::from(b' ') {
            return Some(idx);
        }
        p -= 1;
    }
    None
}

/// Chunk a conversation for the A4 retrieval spine (idempotent: the
/// conversation's rows are rebuilt each call). Returns the number of chunks
/// written — callers use it to decide whether an embedding pass is worth
/// enqueuing. `thread` chunks carry the full transcript; a closed conversation
/// also gets a `resolution` chunk with the final published reply.
pub fn chunk_conversation(conn: &Connection, conversation_id: i64) -> Result<usize> {
    struct ConvRow {
        number: i64,
        subject: Option<String>,
        customer: Option<String>,
        tags: Option<String>,
        closed_at: Option<String>,
    }
    let conv: Option<ConvRow> = conn
        .query_row(
            "SELECT c.number, c.subject, c.closed_at,
                    TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, '')) AS customer,
                    (SELECT GROUP_CONCAT(t.name) FROM conversation_tags ct
                      JOIN tags t ON t.id = ct.tag_id WHERE ct.conversation_id = c.id) AS tags
             FROM conversations c LEFT JOIN customers cu ON cu.id = c.customer_local_id
             WHERE c.id = ?1",
            params![conversation_id],
            |r| {
                Ok(ConvRow {
                    number: r.get(0)?,
                    subject: r.get(1)?,
                    closed_at: r.get(2)?,
                    customer: r.get(3)?,
                    tags: r.get(4)?,
                })
            },
        )
        .ok();
    let Some(conv) = conv else {
        return Ok(0);
    };

    let mut stmt = conn.prepare(
        "SELECT body_text FROM conversation_threads
          WHERE conversation_id = ?1 AND body_text IS NOT NULL AND LENGTH(body_text) > 0
          ORDER BY created_at ASC, id ASC",
    )?;
    let bodies: Vec<String> = stmt
        .query_map(params![conversation_id], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let mut header_lines: Vec<String> = vec![format!(
        "#{} {}",
        conv.number,
        conv.subject.as_deref().unwrap_or("(no subject)")
    )];
    if let Some(customer) = conv.customer.as_deref().filter(|c| !c.is_empty()) {
        header_lines.push(format!("Customer: {customer}"));
    }
    if let Some(tags) = conv.tags.as_deref().filter(|t| !t.is_empty()) {
        header_lines.push(format!("Tags: {tags}"));
    }
    let header = header_lines.join("\n");

    let thread_text = bodies
        .iter()
        .map(|b| utf16_slice(b, 0, 4000))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");
    let full = format!("{header}\n\n{thread_text}");
    let mut chunks = if !thread_text.is_empty() {
        chunk_text(&full, 1200, 150)
    } else {
        vec![full]
    };

    // A4: resolved conversations carry a resolution-excerpt chunk — the last
    // published reply before closure, labeled so retrieval can prefer it for
    // "how was this solved" queries. Only one kind per row: `thread` or
    // `resolution` (m055).
    let resolution_chunk = conv
        .closed_at
        .as_deref()
        .filter(|c| !c.is_empty())
        .and_then(|_| resolution_excerpt(conn, conversation_id, &header));
    let has_resolution = resolution_chunk.is_some();
    if let Some(excerpt) = resolution_chunk {
        chunks.push(excerpt);
    }

    conn.execute_batch("BEGIN")?;
    let tx = (|| -> Result<()> {
        conn.execute(
            "DELETE FROM conversation_chunks WHERE conversation_id = ?1",
            params![conversation_id],
        )?;
        let mut ins = conn.prepare(
            "INSERT INTO conversation_chunks
                (conversation_id, chunk_index, content, chunk_version, chunk_kind)
             VALUES (?1, ?2, ?3, 1, ?4)",
        )?;
        let thread_count = chunks.len().saturating_sub(usize::from(has_resolution));
        for (i, c) in chunks.iter().enumerate() {
            let kind = if i < thread_count {
                "thread"
            } else {
                "resolution"
            };
            ins.execute(params![conversation_id, i as i64, c, kind])?;
        }
        Ok(())
    })();
    match tx {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    }
    Ok(chunks.len())
}

/// The A4 resolution excerpt: the last published reply of a closed
/// conversation, prefixed with a self-locating header so semantic hits on
/// "how was this resolved" land on it. `None` when the conversation has no
/// published reply text to excerpt (nothing fabricated — an unresolved or
/// empty thread simply gets no resolution chunk).
fn resolution_excerpt(conn: &Connection, conversation_id: i64, header: &str) -> Option<String> {
    let raw: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT body_text, body_html
               FROM conversation_threads
              WHERE conversation_id = ?1 AND type = 'reply' AND state = 'published'
                AND deleted_at IS NULL AND COALESCE(body_text, body_html, '') <> ''
              ORDER BY COALESCE(remote_created_at, created_at) DESC, id DESC
              LIMIT 1",
            params![conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let (body_text, body_html) = raw?;
    let body = body_text.filter(|b| !b.trim().is_empty()).or_else(|| {
        body_html
            .map(|h| crate::textutil::html_to_text(&h))
            .filter(|t| !t.trim().is_empty())
    })?;
    let excerpt = utf16_slice(body.trim(), 0, 1200);
    if excerpt.trim().is_empty() {
        return None;
    }
    Some(format!(
        "{header}\n\nRESOLUTION EXCERPT (final agent reply):\n{excerpt}"
    ))
}

// ---------------------------------------------------------------------------
// Migration m055: conversation_chunks.chunk_kind (A4 retrieval spine)
// ---------------------------------------------------------------------------

/// Migration m055 (boot step 46): `conversation_chunks` gains `chunk_kind`
/// ('thread' | 'resolution') so the A4 retrieval spine can prefer resolution
/// excerpts for "how was this solved" queries. Existing rows are all thread
/// content and keep the 'thread' default.
pub fn apply_m055_chunk_kind(conn: &Connection) -> Result<()> {
    if !crate::sync_schema::table_exists(conn, "conversation_chunks")? {
        return Ok(());
    }
    let has: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('conversation_chunks')
             WHERE name = 'chunk_kind'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
        > 0;
    if !has {
        conn.execute(
            "ALTER TABLE conversation_chunks
             ADD COLUMN chunk_kind TEXT NOT NULL DEFAULT 'thread'",
            [],
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConversationChunkStats {
    pub chunks: usize,
    pub indexed: usize,
    pub pending: usize,
    pub failed: usize,
}

pub fn conversation_chunk_stats(conn: &Connection) -> Result<ConversationChunkStats> {
    let row = conn.query_row(
        "SELECT COUNT(*) AS chunks,
           SUM(CASE WHEN embedding_state = 'indexed' THEN 1 ELSE 0 END) AS indexed,
           SUM(CASE WHEN embedding_state IN ('not_indexed', 'queued') THEN 1 ELSE 0 END) AS pending,
           SUM(CASE WHEN embedding_state = 'failed' THEN 1 ELSE 0 END) AS failed
         FROM conversation_chunks",
        [],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<i64>>(3)?,
            ))
        },
    );
    let (chunks, indexed, pending, failed) = match row {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => (0, None, None, None),
        Err(e) => return Err(e.into()),
    };
    Ok(ConversationChunkStats {
        chunks: usize::try_from(chunks).unwrap_or(0),
        indexed: indexed
            .map(|n| usize::try_from(n).unwrap_or(0))
            .unwrap_or(0),
        pending: pending
            .map(|n| usize::try_from(n).unwrap_or(0))
            .unwrap_or(0),
        failed: failed.map(|n| usize::try_from(n).unwrap_or(0)).unwrap_or(0),
    })
}

#[derive(Debug, Clone)]
pub struct ConversationChunkWithEmbedding {
    pub conversation_id: i64,
    pub content: String,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct DocChunkWithEmbedding {
    pub article_id: i64,
    pub content: String,
    pub embedding: Vec<f32>,
}

pub fn list_doc_chunks_with_embedding(conn: &Connection) -> Result<Vec<DocChunkWithEmbedding>> {
    let mut stmt = conn.prepare(
        "SELECT article_id, content, embedding FROM docs_chunks
          WHERE embedding IS NOT NULL AND embedding_state = 'indexed'
          LIMIT 2000",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<Vec<u8>>>(2)?.unwrap_or_default(),
            ))
        })?
        .filter_map(|r| r.ok())
        .map(|(article_id, content, blob)| DocChunkWithEmbedding {
            article_id,
            content,
            embedding: decode_f32_le(&blob),
        })
        .collect();
    Ok(rows)
}

pub fn docs_chunk_stats(conn: &Connection) -> Result<ConversationChunkStats> {
    let row = conn.query_row(
        "SELECT COUNT(*) AS chunks,
           SUM(CASE WHEN embedding_state = 'indexed' THEN 1 ELSE 0 END) AS indexed,
           SUM(CASE WHEN embedding_state IN ('not_indexed', 'queued') THEN 1 ELSE 0 END) AS pending,
           SUM(CASE WHEN embedding_state = 'failed' THEN 1 ELSE 0 END) AS failed
         FROM docs_chunks",
        [],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<i64>>(3)?,
            ))
        },
    );
    let (chunks, indexed, pending, failed) = match row {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => (0, None, None, None),
        Err(e) => return Err(e.into()),
    };
    Ok(ConversationChunkStats {
        chunks: usize::try_from(chunks).unwrap_or(0),
        indexed: indexed
            .map(|n| usize::try_from(n).unwrap_or(0))
            .unwrap_or(0),
        pending: pending
            .map(|n| usize::try_from(n).unwrap_or(0))
            .unwrap_or(0),
        failed: failed.map(|n| usize::try_from(n).unwrap_or(0)).unwrap_or(0),
    })
}

pub fn list_conversation_chunks_with_embedding(
    conn: &Connection,
) -> Result<Vec<ConversationChunkWithEmbedding>> {
    let mut stmt = conn.prepare(
        "SELECT conversation_id, content, embedding FROM conversation_chunks
          WHERE embedding IS NOT NULL AND embedding_state = 'indexed'
          LIMIT 5000",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<Vec<u8>>>(2)?.unwrap_or_default(),
            ))
        })?
        .filter_map(|r| r.ok())
        .map(
            |(conversation_id, content, blob)| ConversationChunkWithEmbedding {
                conversation_id,
                content,
                embedding: decode_f32_le(&blob),
            },
        )
        .collect();
    Ok(rows)
}

#[must_use]
pub fn decode_f32_le(bytes: &[u8]) -> Vec<f32> {
    let (words, _) = bytes.as_chunks::<4>();
    words.iter().map(|w| f32::from_le_bytes(*w)).collect()
}

#[must_use]
pub fn encode_f32_le(vector: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(vector.len() * 4);
    for f in vector {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

fn update_chunk_embedding(
    conn: &Connection,
    table: &str,
    chunk_id: i64,
    model: &str,
    embedding: &[f32],
) -> Result<()> {
    conn.execute(
        &format!("UPDATE {table} SET embedding = ?1, embedding_model = ?2, embedding_state = 'indexed' WHERE id = ?3"),
        params![encode_f32_le(embedding), model, chunk_id],
    )?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct LmStudioEmbeddingSettings {
    pub base_url: String,
    pub embedding_model: Option<String>,
    pub timeout_ms: u64,
    /// Envelope-stored BYOK key for cloud embedding endpoints (A4).
    pub api_key: Option<String>,
}

/// Resolve the embedding endpoint. A4 order of precedence: the hub **Embed
/// role** first (endpoint base URL + role model + envelope key), then the
/// legacy `lmstudio_*`/AI-status settings as the pre-hub fallback. Cloud
/// embeddings are an explicit opt-in (D-a): they only happen when the user
/// routed the Embed role to a cloud endpoint in the AI Center.
pub fn lm_studio_embedding_settings(conn: &Connection) -> Result<LmStudioEmbeddingSettings> {
    let timeout_ms = u64::try_from(
        crate::settings::get_i64(conn, "lmstudio_timeout_ms", 120_000).unwrap_or(120_000),
    )
    .unwrap_or(120_000);
    if let Some((endpoint, model, api_key)) =
        crate::ai_hub::resolve_role(conn, crate::ai_hub::AiRole::Embed)
    {
        let model = model.filter(|m: &String| !m.is_empty());
        return Ok(LmStudioEmbeddingSettings {
            base_url: normalize_lm_base_url(&endpoint.base_url),
            embedding_model: model,
            timeout_ms,
            api_key,
        });
    }
    let model = crate::settings::get_string(conn, "lmstudio_embedding_model")
        .ok()
        .flatten()
        .filter(|m| !m.is_empty())
        .or_else(|| {
            crate::ai_center::get_ai_status(conn)
                .ok()
                .and_then(|s| s.embedding_model)
                .filter(|m| !m.is_empty())
        });
    let base = crate::settings::get_string(conn, "lmstudio_base_url")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            crate::ai_center::get_ai_status(conn)
                .ok()
                .and_then(|s| s.base_url)
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "http://127.0.0.1:1234".to_string());
    Ok(LmStudioEmbeddingSettings {
        base_url: normalize_lm_base_url(&base),
        embedding_model: model,
        timeout_ms,
        api_key: None,
    })
}

#[must_use]
pub fn normalize_lm_base_url(url: &str) -> String {
    let s = url.strip_suffix('/').unwrap_or(url);
    let s = s.strip_suffix("/v1").unwrap_or(s);
    s.to_string()
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EmbeddingsResponseBody {
    pub data: Vec<EmbeddingsResponseItem>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EmbeddingsResponseItem {
    pub embedding: Vec<f32>,
}

pub async fn embed_texts(
    base_url: &str,
    model: &str,
    texts: &[String],
    timeout_ms: u64,
    api_key: Option<&str>,
) -> Result<Vec<Vec<f32>>> {
    let mut builder =
        reqwest::Client::builder().timeout(std::time::Duration::from_millis(timeout_ms.max(1)));
    if let Some(key) = api_key {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(v) = reqwest::header::HeaderValue::from_str(&format!("Bearer {key}")) {
            headers.insert(reqwest::header::AUTHORIZATION, v);
        }
        builder = builder.default_headers(headers);
    }
    let client = builder
        .build()
        .map_err(|e| Error::Config(format!("LM Studio embedding failed: {e}")))?;
    let res = client
        .post(format!("{}/v1/embeddings", base_url))
        .json(&serde_json::json!({ "input": texts, "model": model }))
        .send()
        .await
        .map_err(|e| {
            Error::Config(format!(
                "LM Studio embedding failed: {e}. Load an embedding model in LM Studio (e.g. nomic-embed) and retry."
            ))
        })?;
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        let truncated: String = text.chars().take(300).collect();
        return Err(Error::Config(format!(
            "LM Studio embedding request failed (HTTP {status}): {truncated}"
        )));
    }
    let body: EmbeddingsResponseBody = res
        .json()
        .await
        .map_err(|e| Error::Config(format!("LM Studio embedding failed: {e}")))?;
    Ok(body.data.into_iter().map(|d| d.embedding).collect())
}

fn embed_texts_blocking(
    base_url: String,
    model: String,
    texts: Vec<String>,
    timeout_ms: u64,
    api_key: Option<String>,
) -> Result<Vec<Vec<f32>>> {
    std::thread::Builder::new()
        .name("lmstudio-embed".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| Error::Config(format!("LM Studio embedding failed: {e}")))?;
            rt.block_on(embed_texts(
                &base_url,
                &model,
                &texts,
                timeout_ms,
                api_key.as_deref(),
            ))
        })
        .map_err(Error::Io)?
        .join()
        .map_err(|_| Error::Config("LM Studio embedding worker thread panicked".into()))?
}

/// Embed a single query text (A4 retrieval spine: fused search + evidence
/// semantic hits). `Ok(None)` is the honest no-endpoint answer — when no
/// embedding model is routed (Embed role unset and no legacy config) callers
/// stay BM25-only instead of inventing vectors.
pub fn embed_query_text_blocking(conn: &Connection, text: &str) -> Result<Option<Vec<f32>>> {
    let settings = lm_studio_embedding_settings(conn)?;
    let Some(model) = settings.embedding_model else {
        return Ok(None);
    };
    let input: String = utf16_slice(text.trim(), 0, 4000);
    if input.is_empty() {
        return Ok(None);
    }
    let vectors = embed_texts_blocking(
        settings.base_url.clone(),
        model,
        vec![input],
        settings.timeout_ms,
        settings.api_key.clone(),
    )?;
    Ok(vectors.into_iter().next().filter(|v| !v.is_empty()))
}

struct PendingChunk {
    id: i64,
    entity_id: i64,
    content: String,
    title: String,
    visibility: String,
    number: Option<i64>,
}

/// Payload `entity_type` value stored in the vector store for chunks of the
/// given chunk table name.
#[must_use]
pub fn entity_type_for(table: &str) -> &'static str {
    match table {
        "knowledge_chunks" => "knowledge_chunk",
        "docs_chunks" => "docs_chunk",
        _ => "conversation_chunk",
    }
}

fn list_pending_chunks(conn: &Connection, table: &str) -> Result<Vec<PendingChunk>> {
    let sql = match table {
        "knowledge_chunks" => format!(
            "SELECT c.id, c.document_id, c.content, d.title, d.visibility
             FROM knowledge_chunks c JOIN knowledge_documents d ON d.id = c.document_id
             WHERE c.embedding_state = 'not_indexed' OR c.embedding_state = 'failed'
             LIMIT {EMBED_PASS_LIMIT}"
        ),
        "docs_chunks" => format!(
            "SELECT c.id, c.article_id, c.content, a.name,
                    CASE WHEN a.status = 'published' THEN 'customer_safe' ELSE 'internal_only' END
             FROM docs_chunks c JOIN docs_articles a ON a.id = c.article_id
             WHERE (c.embedding_state = 'not_indexed' OR c.embedding_state = 'failed')
               AND c.embedding_attempts < 5
             LIMIT {EMBED_PASS_LIMIT}"
        ),
        _ => format!(
            "SELECT c.id, c.conversation_id, c.content,
                    COALESCE(substr(v.subject, 1, 300), '#' || v.number), v.number
             FROM conversation_chunks c JOIN conversations v ON v.id = c.conversation_id
             WHERE (c.embedding_state = 'not_indexed' OR c.embedding_state = 'failed')
               AND c.embedding_attempts < 5
             LIMIT {EMBED_PASS_LIMIT}"
        ),
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(PendingChunk {
                id: r.get(0)?,
                entity_id: r.get(1)?,
                content: r.get(2)?,
                title: r.get(3)?,
                visibility: r
                    .get::<_, Option<String>>(4)
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "internal_only".to_string()),
                number: r
                    .get::<_, Option<i64>>(4)
                    .ok()
                    .flatten()
                    .filter(|_| table == "conversation_chunks"),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    Ok(rows)
}

fn mark_chunk_failed(conn: &Connection, table: &str, chunk_id: i64) -> Result<()> {
    if table == "docs_chunks" || table == "conversation_chunks" {
        conn.execute(
            &format!(
                "UPDATE {table} SET embedding_state = 'failed',
                 embedding_attempts = embedding_attempts + 1 WHERE id = ?1"
            ),
            params![chunk_id],
        )?;
    } else {
        conn.execute(
            &format!("UPDATE {table} SET embedding_state = 'failed' WHERE id = ?1"),
            params![chunk_id],
        )?;
    }
    Ok(())
}

fn embed_pending_chunks(
    conn: &Connection,
    table: &str,
    qdrant: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
) -> Result<usize> {
    // Audit item 21: run the one-time point-id-schema migration before
    // the pending list is read, so freshly reset chunks are picked up in
    // this same pass.
    ensure_vector_id_schema_v2(conn, qdrant)?;
    let pending = list_pending_chunks(conn, table)?;
    if pending.is_empty() {
        return Ok(0);
    }
    let settings = lm_studio_embedding_settings(conn)?;
    let Some(model) = settings.embedding_model else {
        return Ok(0);
    };

    let qdrant_connected = qdrant.is_some_and(|q| q.health().connected);

    let texts: Vec<String> = pending
        .iter()
        .map(|c| utf16_slice(&c.content, 0, 4000))
        .collect();
    let vectors = match embed_texts_blocking(
        settings.base_url.clone(),
        model.clone(),
        texts,
        settings.timeout_ms,
        settings.api_key.clone(),
    ) {
        Ok(v) => v,
        Err(_) => {
            for chunk in &pending {
                mark_chunk_failed(conn, table, chunk.id)?;
            }
            return Ok(0);
        }
    };

    if table == "knowledge_chunks" {
        // Clean-audit item 46: the qdrant lane used to gate the whole
        // batch on the FIRST vector being non-empty, then upserted every
        // pending chunk — substituting an empty vector for any chunk the
        // model skipped — and stamped the entire batch 'indexed'. One
        // empty vector poisoned the batch: a zero-dim point landed in the
        // store and its chunk was marked done, never retried. The batch
        // is now partitioned per chunk: only chunks with a real vector
        // are upserted and marked 'indexed'; chunks the model left
        // empty are marked 'failed' and stay in the retry pipeline.
        if qdrant_connected {
            let mut points: Vec<crate::vectorstore_sqlitevec::VectorPoint> = Vec::new();
            let mut indexed_ids: Vec<i64> = Vec::new();
            let mut failed_ids: Vec<i64> = Vec::new();
            for (i, chunk) in pending.iter().enumerate() {
                match vectors.get(i) {
                    Some(v) if !v.is_empty() => {
                        points.push(vector_point(table, chunk, v, &model)?);
                        indexed_ids.push(chunk.id);
                    }
                    _ => failed_ids.push(chunk.id),
                }
            }
            if !points.is_empty() {
                let q = qdrant.expect("checked is_some above");
                let _ = q.ensure_collection(points[0].vector.len());
                let ok = q.upsert(&points);
                let state = if ok { "indexed" } else { "failed" };
                for id in &indexed_ids {
                    set_chunk_state(conn, table, *id, state, Some(&model))?;
                }
                let indexed = if ok { indexed_ids.len() } else { 0 };
                for id in &failed_ids {
                    set_chunk_state(conn, table, *id, "failed", None)?;
                }
                return Ok(indexed);
            }
            for id in &failed_ids {
                set_chunk_state(conn, table, *id, "failed", None)?;
            }
            return Ok(0);
        }
        let mut embedded = 0_usize;
        for (i, chunk) in pending.iter().enumerate() {
            match vectors.get(i) {
                Some(v) if !v.is_empty() => {
                    update_chunk_embedding(conn, table, chunk.id, &model, v)?;
                    embedded += 1;
                }
                _ => set_chunk_state(conn, table, chunk.id, "failed", None)?,
            }
        }
        return Ok(embedded);
    }

    let mut embedded = 0_usize;
    for (i, chunk) in pending.iter().enumerate() {
        match vectors.get(i) {
            Some(v) if !v.is_empty() => {
                update_chunk_embedding(conn, table, chunk.id, &model, v)?;
                embedded += 1;
            }
            _ => mark_chunk_failed(conn, table, chunk.id)?,
        }
    }
    if qdrant_connected {
        let points: Vec<crate::vectorstore_sqlitevec::VectorPoint> = pending
            .iter()
            .enumerate()
            .filter_map(|(i, chunk)| {
                let v = vectors.get(i)?;
                if v.is_empty() {
                    return None;
                }
                Some(vector_point(table, chunk, v, &model))
            })
            .collect::<Result<Vec<_>>>()?;
        if !points.is_empty() {
            let q = qdrant.expect("checked is_some above");
            let _ = q.ensure_collection(points[0].vector.len());
            let _ = q.upsert(&points);
        }
    }
    Ok(embedded)
}

/// Clean-audit item 21: the vector store's point-id space is namespaced
/// per chunk family (knowledge/docs/conversation) now. Databases whose
/// chunks were embedded before that carry colliding, silently overwritten
/// vectors — unrecoverable, since the overwritten family's embedding was
/// never re-read. One time only (marker setting `vector_id_schema`), the
/// shared collection is dropped and every chunk is reset to
/// `not_indexed` so the normal pending pipeline re-embeds the corpus
/// under collision-free ids. The migration runs only when the vector
/// store is actually live (enabled and openable); a disabled index
/// re-checks on the first pass after it is re-enabled, so pre-v2 vectors
/// can never survive into the namespaced era.
fn ensure_vector_id_schema_v2(
    conn: &Connection,
    qdrant: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
) -> Result<()> {
    const VERSION: i64 = 2;
    if crate::settings::get_i64(conn, "vector_id_schema", 0).unwrap_or(0) >= VERSION {
        return Ok(());
    }
    let migrated = qdrant.is_some_and(|q| q.drop_collection());
    if !migrated {
        return Ok(());
    }
    conn.execute(
        "UPDATE knowledge_chunks SET embedding_state = 'not_indexed'",
        [],
    )?;
    conn.execute(
        "UPDATE docs_chunks SET embedding_state = 'not_indexed', embedding_attempts = 0",
        [],
    )?;
    conn.execute(
        "UPDATE conversation_chunks
          SET embedding_state = 'not_indexed', embedding_attempts = 0",
        [],
    )?;
    crate::settings::set_i64(conn, "vector_id_schema", VERSION)?;
    // Restart the re-embed pipeline for every family: this pass covers
    // its own table, the queued jobs cover the other two (a no-op pass
    // when nothing is pending — the normal guard rails apply).
    for kind in [
        "embed_knowledge_chunks",
        "embed_docs_chunks",
        "embed_conversation_chunks",
    ] {
        let _ = crate::jobs::enqueue_on(conn, "embeddings", kind, "{}", 4, 2);
    }
    tracing::info!(
        operation = "vector_id_schema_v2",
        "vector collection dropped and all chunks reset — the corpus re-embeds \
         under namespaced point ids (audit item 21)"
    );
    Ok(())
}

fn vector_point(
    table: &str,
    chunk: &PendingChunk,
    vector: &[f32],
    model: &str,
) -> Result<crate::vectorstore_sqlitevec::VectorPoint> {
    let entity_type = entity_type_for(table);
    let mut payload = serde_json::json!({
        "entity_type": entity_type,
        "entity_id": chunk.entity_id,
        "chunk_id": chunk.id,
        "title": chunk.title,
        "text": utf16_slice(&chunk.content, 0, 2000),
        "visibility": chunk.visibility,
        "embedding_model": model,
        "index_version": 2
    });
    if table == "conversation_chunks" {
        payload
            .as_object_mut()
            .expect("payload is an object")
            .insert("number".to_string(), chunk.number.into());
    }
    Ok(crate::vectorstore_sqlitevec::VectorPoint {
        // Audit item 21: namespaced by family, so knowledge/docs/
        // conversation chunks can never land on the same point id in
        // the single shared collection.
        id: crate::vectorstore_sqlitevec::namespaced_chunk_point_id(entity_type, chunk.id)?,
        vector: vector.to_vec(),
        payload,
    })
}

fn set_chunk_state(
    conn: &Connection,
    table: &str,
    chunk_id: i64,
    state: &str,
    model: Option<&str>,
) -> Result<()> {
    conn.execute(
        &format!(
            "UPDATE {table} SET embedding_state = ?1,
             embedding_model = COALESCE(?2, embedding_model) WHERE id = ?3"
        ),
        params![state, model, chunk_id],
    )?;
    Ok(())
}

pub fn embed_pending_knowledge(
    conn: &Connection,
    qdrant: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
) -> Result<usize> {
    embed_pending_chunks(conn, "knowledge_chunks", qdrant)
}

pub fn embed_pending_docs(
    conn: &Connection,
    qdrant: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
) -> Result<usize> {
    embed_pending_chunks(conn, "docs_chunks", qdrant)
}

pub fn embed_pending_conversation_chunks(
    conn: &Connection,
    qdrant: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
) -> Result<usize> {
    embed_pending_chunks(conn, "conversation_chunks", qdrant)
}

#[cfg(test)]
mod batch20_audit21_tests {
    use super::*;
    use rusqlite::Connection;

    fn temp_data_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spp-embed-audit21-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    /// A pre-v2 database: chunks already 'indexed' (their vectors lived in
    /// the shared collection under colliding raw per-table ids) and a
    /// live vector collection holding a stale point.
    fn pre_v2_db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO knowledge_sources (name, kind) VALUES ('src', 'local_file')",
            [],
        )
        .expect("source");
        let source = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO knowledge_documents (source_id, title, content)
             VALUES (?1, 'Runbook', 'restart the widget')",
            params![source],
        )
        .expect("doc");
        let doc = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO knowledge_chunks (document_id, chunk_index, content, embedding_state)
             VALUES (?1, 0, 'restart the widget', 'indexed')",
            params![doc],
        )
        .expect("knowledge chunk");
        conn.execute(
            "INSERT INTO docs_chunks (article_id, chunk_index, content, embedding_state,
                                      embedding_attempts)
             VALUES (1, 0, 'billing article', 'indexed', 3)",
            [],
        )
        .expect("docs chunk");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (9301, 93, 'Broken export', 'preview', 'active')",
            [],
        )
        .expect("conversation");
        let conv = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversation_chunks (conversation_id, chunk_index, content,
                                              embedding_state, embedding_attempts)
             VALUES (?1, 0, 'the export fails', 'indexed', 2)",
            params![conv],
        )
        .expect("conversation chunk");
        conn
    }

    /// Audit item 21: the first embed pass on a pre-v2 database drops the
    /// colliding collection, resets every chunk family to `not_indexed`
    /// (attempt counters included), restarts the re-embed pipeline for all
    /// three families, and stamps the marker so it never runs twice.
    #[test]
    fn audit21_migration_drops_resets_and_stamps_once() {
        let conn = pre_v2_db();
        let dir = temp_data_dir("migrate");
        let index = crate::vectorstore_sqlitevec::VectorIndex::new(&dir, true);
        // Pre-v2 vectors exist in the shared collection.
        assert!(index.ensure_collection(2));
        assert!(index.upsert(&[crate::vectorstore_sqlitevec::VectorPoint {
            id: 1,
            vector: vec![1.0, 0.0],
            payload: serde_json::json!({"entity_type": "knowledge_chunk", "entity_id": 1}),
        }]));
        assert_eq!(index.try_point_count().expect("count"), Some(1));

        // No embedding model is routed, so the pass embeds nothing — but
        // the migration still runs first.
        embed_pending_knowledge(&conn, Some(&index)).expect("embed pass");

        assert_eq!(
            crate::settings::get_i64(&conn, "vector_id_schema", 0).unwrap_or(0),
            2,
            "the schema marker must be stamped"
        );
        assert_eq!(
            index.try_point_count().expect("count after"),
            None,
            "the pre-v2 collection must be gone"
        );
        for (table, has_attempts) in [
            ("knowledge_chunks", false),
            ("docs_chunks", true),
            ("conversation_chunks", true),
        ] {
            let state: String = conn
                .query_row(
                    &format!("SELECT embedding_state FROM {table} LIMIT 1"),
                    [],
                    |r| r.get(0),
                )
                .expect("state row");
            assert_eq!(state, "not_indexed", "{table} must be reset for re-embed");
            if has_attempts {
                let attempts: i64 = conn
                    .query_row(
                        &format!("SELECT embedding_attempts FROM {table} LIMIT 1"),
                        [],
                        |r| r.get(0),
                    )
                    .expect("attempts row");
                assert_eq!(attempts, 0, "{table} attempt counter must be reset");
            }
        }
        let queued: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM jobs
                  WHERE type IN ('embed_knowledge_chunks', 'embed_docs_chunks',
                                 'embed_conversation_chunks')
                    AND status = 'queued'",
                [],
                |r| r.get(0),
            )
            .expect("job count");
        assert_eq!(queued, 3, "the re-embed pipeline restarts for every family");

        // Second pass: the marker holds — no further reset or enqueue.
        conn.execute(
            "UPDATE knowledge_chunks SET embedding_state = 'indexed'",
            [],
        )
        .expect("pretend a re-embed happened");
        embed_pending_docs(&conn, Some(&index)).expect("second pass");
        let state: String = conn
            .query_row(
                "SELECT embedding_state FROM knowledge_chunks LIMIT 1",
                [],
                |r| r.get(0),
            )
            .expect("state row");
        assert_eq!(state, "indexed", "the migration must not run a second time");
    }

    /// Audit item 21: a disabled vector index never stamps the marker —
    /// so pre-v2 vectors cannot survive into the namespaced era by way of
    /// a disable/enable cycle; the migration re-checks once it matters.
    #[test]
    fn audit21_disabled_index_does_not_stamp_the_marker() {
        let conn = pre_v2_db();
        let dir = temp_data_dir("disabled");
        let index = crate::vectorstore_sqlitevec::VectorIndex::new(&dir, false);
        embed_pending_knowledge(&conn, Some(&index)).expect("embed pass");
        assert_eq!(
            crate::settings::get_i64(&conn, "vector_id_schema", 0).unwrap_or(0),
            0,
            "a disabled index must not consume the migration"
        );
        let state: String = conn
            .query_row(
                "SELECT embedding_state FROM knowledge_chunks LIMIT 1",
                [],
                |r| r.get(0),
            )
            .expect("state row");
        assert_eq!(
            state, "indexed",
            "chunk-table states are untouched while the index is off"
        );
    }
}

#[cfg(test)]
mod batch13_a4_tests {
    use super::*;
    use rusqlite::Connection;

    fn db_with_conversation(status: &str, closed_at: Option<&str>) -> (Connection, i64) {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, closed_at)
             VALUES (9201, 92, 'Export failing', 'preview', ?1, ?2)",
            params![status, closed_at],
        )
        .expect("conversation");
        let conv = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversation_threads
                (conversation_id, type, state, body_text, remote_created_at, created_at)
             VALUES (?1, 'customer', 'published', 'The export throws an error every time.',
                     '2026-10-20T10:00:00Z', datetime('now'))",
            params![conv],
        )
        .expect("customer thread");
        conn.execute(
            "INSERT INTO conversation_threads
                (conversation_id, type, state, body_text, remote_created_at, created_at)
             VALUES (?1, 'reply', 'published', 'We fixed the timeout by raising the limit.',
                     '2026-10-20T11:00:00Z', datetime('now'))",
            params![conv],
        )
        .expect("reply thread");
        (conn, conv)
    }

    fn chunk_kinds(conn: &Connection, conv: i64) -> Vec<(String, String)> {
        let mut stmt = conn
            .prepare(
                "SELECT chunk_kind, content FROM conversation_chunks
                  WHERE conversation_id = ?1 ORDER BY chunk_index",
            )
            .expect("chunks");
        stmt.query_map(params![conv], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("rows")
            .filter_map(|r| r.ok())
            .collect()
    }

    /// A4: a closed conversation carries exactly one `resolution` chunk whose
    /// content is the final published reply, plus the `thread` chunks.
    #[test]
    fn closed_conversation_gains_resolution_chunk() {
        let (conn, conv) = db_with_conversation("closed", Some("2026-10-20T12:00:00Z"));
        let written = chunk_conversation(&conn, conv).expect("chunk");
        assert!(
            written >= 2,
            "thread + resolution chunks expected, got {written}"
        );
        let kinds = chunk_kinds(&conn, conv);
        let resolutions: Vec<&(String, String)> =
            kinds.iter().filter(|(k, _)| k == "resolution").collect();
        assert_eq!(resolutions.len(), 1, "exactly one resolution chunk");
        let (_, content) = resolutions[0];
        assert!(
            content.contains("RESOLUTION EXCERPT"),
            "the resolution chunk self-locates: {content}"
        );
        assert!(
            content.contains("raising the limit"),
            "the excerpt is the final reply, not the customer thread"
        );
        assert!(
            kinds.iter().any(|(k, _)| k == "thread"),
            "thread chunks still present"
        );
    }

    /// A4: an open conversation gets no resolution chunk — nothing is
    /// fabricated for unresolved threads.
    #[test]
    fn open_conversation_has_no_resolution_chunk() {
        let (conn, conv) = db_with_conversation("active", None);
        chunk_conversation(&conn, conv).expect("chunk");
        let kinds = chunk_kinds(&conn, conv);
        assert!(
            kinds.iter().all(|(k, _)| k == "thread"),
            "only thread chunks for open conversations"
        );
    }

    /// A4 (m055): chunking is idempotent — re-running replaces the rows
    /// instead of duplicating them.
    #[test]
    fn chunking_is_idempotent() {
        let (conn, conv) = db_with_conversation("closed", Some("2026-10-20T12:00:00Z"));
        chunk_conversation(&conn, conv).expect("first");
        chunk_conversation(&conn, conv).expect("second");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM conversation_chunks WHERE conversation_id = ?1",
                params![conv],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let first = chunk_conversation(&conn, conv).expect("third");
        assert_eq!(first as i64, count, "no duplication across re-chunks");
    }
}

#[cfg(test)]
mod batch24_audit46_tests {
    use super::*;
    use rusqlite::Connection;

    fn temp_data_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spp-embed-audit46-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    /// Two pending knowledge chunks. The embedding endpoint is a mock that
    /// answers `power-cycle` texts with an empty vector and everything else
    /// with a healthy 2-dim vector — the batch is answered per input, so
    /// the test does not depend on row order.
    fn db_with_two_pending_chunks() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO knowledge_sources (name, kind) VALUES ('src', 'local_file')",
            [],
        )
        .expect("source");
        let source = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO knowledge_documents (source_id, title, content)
             VALUES (?1, 'Runbook', 'restart the widget')",
            params![source],
        )
        .expect("doc");
        let doc = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO knowledge_chunks (document_id, chunk_index, content, embedding_state)
             VALUES (?1, 0, 'restart the widget', 'not_indexed')",
            params![doc],
        )
        .expect("chunk 1");
        conn.execute(
            "INSERT INTO knowledge_chunks (document_id, chunk_index, content, embedding_state)
             VALUES (?1, 1, 'then power-cycle it', 'not_indexed')",
            params![doc],
        )
        .expect("chunk 2");
        conn
    }

    fn mixed_batch_handler() -> std::sync::Arc<
        dyn Fn(&crate::mockserver::MockRequest) -> crate::mockserver::MockResponse + Send + Sync,
    > {
        std::sync::Arc::new(|req: &crate::mockserver::MockRequest| {
            let body: serde_json::Value =
                serde_json::from_str(&req.body).unwrap_or(serde_json::Value::Null);
            let inputs = body
                .get("input")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let data: Vec<serde_json::Value> = inputs
                .iter()
                .map(|t| {
                    let text = t.as_str().unwrap_or_default();
                    let embedding: Vec<f32> = if text.contains("power-cycle") {
                        Vec::new()
                    } else {
                        vec![0.25, 0.5]
                    };
                    serde_json::json!({ "embedding": embedding })
                })
                .collect();
            crate::mockserver::MockResponse::json(200, serde_json::json!({ "data": data }))
        })
    }

    /// Clean-audit item 46: one empty vector no longer poisons the
    /// knowledge batch. The chunk the model skipped is marked 'failed'
    /// (staying in the retry pipeline) instead of riding the batch into
    /// the store as an empty point stamped 'indexed'; the healthy chunks
    /// are upserted and marked 'indexed'.
    #[test]
    fn audit46_empty_vector_does_not_poison_the_batch() {
        let conn = db_with_two_pending_chunks();
        let dir = temp_data_dir("mixed");
        let index = crate::vectorstore_sqlitevec::VectorIndex::new(&dir, true);
        let mock = crate::mockserver::MockServer::start(mixed_batch_handler()).expect("mock");
        crate::settings::set_string(&conn, "lmstudio_base_url", &mock.url()).expect("base url");
        crate::settings::set_string(&conn, "lmstudio_embedding_model", "test-embed")
            .expect("model");

        let indexed = embed_pending_knowledge(&conn, Some(&index)).expect("embed pass");
        assert_eq!(indexed, 1, "only the healthy chunk counts as indexed");

        let mut stmt = conn
            .prepare("SELECT content, embedding_state FROM knowledge_chunks ORDER BY id")
            .expect("states");
        let states: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("rows")
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        assert_eq!(
            states,
            vec![
                ("restart the widget".to_string(), "indexed".to_string()),
                ("then power-cycle it".to_string(), "failed".to_string()),
            ],
            "the skipped chunk must be failed, not indexed"
        );
        assert_eq!(
            index.try_point_count().expect("count"),
            Some(1),
            "exactly one point in the store — no empty-vector point"
        );
    }

    /// Clean-audit item 46, all-empty corner: when the model returns
    /// nothing usable for any chunk, nothing is upserted and every chunk
    /// is marked 'failed' for retry.
    #[test]
    fn audit46_all_empty_batch_upserts_nothing() {
        let conn = db_with_two_pending_chunks();
        let dir = temp_data_dir("allempty");
        let index = crate::vectorstore_sqlitevec::VectorIndex::new(&dir, true);
        let mock = crate::mockserver::MockServer::start(std::sync::Arc::new(
            |_req: &crate::mockserver::MockRequest| {
                crate::mockserver::MockResponse::json(
                    200,
                    serde_json::json!({ "data": [
                        { "embedding": [] },
                        { "embedding": [] }
                    ]}),
                )
            },
        ))
        .expect("mock");
        crate::settings::set_string(&conn, "lmstudio_base_url", &mock.url()).expect("base url");
        crate::settings::set_string(&conn, "lmstudio_embedding_model", "test-embed")
            .expect("model");

        let indexed = embed_pending_knowledge(&conn, Some(&index)).expect("embed pass");
        assert_eq!(indexed, 0, "nothing usable came back");
        let failed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_chunks WHERE embedding_state = 'failed'",
                [],
                |r| r.get(0),
            )
            .expect("failed count");
        assert_eq!(failed, 2, "both chunks stay in the retry pipeline");
        // No point was upserted, so no collection is created either.
        assert!(
            matches!(index.try_point_count().expect("count"), None | Some(0)),
            "the store stays empty"
        );
    }
}
