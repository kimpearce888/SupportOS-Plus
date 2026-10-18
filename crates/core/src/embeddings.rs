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

pub fn chunk_conversation(conn: &Connection, conversation_id: i64) -> Result<()> {
    struct ConvRow {
        number: i64,
        subject: Option<String>,
        customer: Option<String>,
        tags: Option<String>,
    }
    let conv: Option<ConvRow> = conn
        .query_row(
            "SELECT c.number, c.subject,
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
                    customer: r.get(2)?,
                    tags: r.get(3)?,
                })
            },
        )
        .ok();
    let Some(conv) = conv else {
        return Ok(());
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
    let chunks = if !thread_text.is_empty() {
        chunk_text(&full, 1200, 150)
    } else {
        vec![full]
    };

    conn.execute_batch("BEGIN")?;
    let tx = (|| -> Result<()> {
        conn.execute(
            "DELETE FROM conversation_chunks WHERE conversation_id = ?1",
            params![conversation_id],
        )?;
        let mut ins = conn.prepare(
            "INSERT INTO conversation_chunks (conversation_id, chunk_index, content, chunk_version)
             VALUES (?1, ?2, ?3, 1)",
        )?;
        for (i, c) in chunks.iter().enumerate() {
            ins.execute(params![conversation_id, i as i64, c])?;
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
}

pub fn lm_studio_embedding_settings(conn: &Connection) -> Result<LmStudioEmbeddingSettings> {
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
    let timeout_ms = u64::try_from(
        crate::settings::get_i64(conn, "lmstudio_timeout_ms", 120_000).unwrap_or(120_000),
    )
    .unwrap_or(120_000);
    Ok(LmStudioEmbeddingSettings {
        base_url: normalize_lm_base_url(&base),
        embedding_model: model,
        timeout_ms,
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
) -> Result<Vec<Vec<f32>>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(timeout_ms.max(1)))
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
) -> Result<Vec<Vec<f32>>> {
    std::thread::Builder::new()
        .name("lmstudio-embed".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| Error::Config(format!("LM Studio embedding failed: {e}")))?;
            rt.block_on(embed_texts(&base_url, &model, &texts, timeout_ms))
        })
        .map_err(Error::Io)?
        .join()
        .map_err(|_| Error::Config("LM Studio embedding worker thread panicked".into()))?
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
    qdrant: Option<&crate::vectorstore_qdrant::EmbeddedQdrant>,
) -> Result<usize> {
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
        if qdrant_connected && vectors.first().is_some_and(|v| !v.is_empty()) {
            let dim = vectors[0].len();
            let points: Vec<crate::vectorstore_qdrant::VectorPoint> = pending
                .iter()
                .enumerate()
                .map(|(i, chunk)| {
                    let empty: Vec<f32> = Vec::new();
                    let vector = vectors.get(i).unwrap_or(&empty);
                    vector_point(table, chunk, vector, &model)
                })
                .collect();
            let q = qdrant.expect("checked is_some above");
            let _ = q.ensure_collection(dim);
            let ok = q.upsert(&points);
            let state = if ok { "indexed" } else { "failed" };
            for chunk in &pending {
                set_chunk_state(conn, table, chunk.id, state, Some(&model))?;
            }
            return Ok(if ok { pending.len() } else { 0 });
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
        let points: Vec<crate::vectorstore_qdrant::VectorPoint> = pending
            .iter()
            .enumerate()
            .filter_map(|(i, chunk)| {
                let v = vectors.get(i)?;
                if v.is_empty() {
                    return None;
                }
                Some(vector_point(table, chunk, v, &model))
            })
            .collect();
        if !points.is_empty() {
            let q = qdrant.expect("checked is_some above");
            let _ = q.ensure_collection(points[0].vector.len());
            let _ = q.upsert(&points);
        }
    }
    Ok(embedded)
}

fn vector_point(
    table: &str,
    chunk: &PendingChunk,
    vector: &[f32],
    model: &str,
) -> crate::vectorstore_qdrant::VectorPoint {
    let mut payload = serde_json::json!({
        "entity_type": entity_type_for(table),
        "entity_id": chunk.entity_id,
        "chunk_id": chunk.id,
        "title": chunk.title,
        "text": utf16_slice(&chunk.content, 0, 2000),
        "visibility": chunk.visibility,
        "embedding_model": model,
        "index_version": 1
    });
    if table == "conversation_chunks" {
        payload
            .as_object_mut()
            .expect("payload is an object")
            .insert("number".to_string(), chunk.number.into());
    }
    crate::vectorstore_qdrant::VectorPoint {
        id: chunk.id,
        vector: vector.to_vec(),
        payload,
    }
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
    qdrant: Option<&crate::vectorstore_qdrant::EmbeddedQdrant>,
) -> Result<usize> {
    embed_pending_chunks(conn, "knowledge_chunks", qdrant)
}

pub fn embed_pending_docs(
    conn: &Connection,
    qdrant: Option<&crate::vectorstore_qdrant::EmbeddedQdrant>,
) -> Result<usize> {
    embed_pending_chunks(conn, "docs_chunks", qdrant)
}

pub fn embed_pending_conversation_chunks(
    conn: &Connection,
    qdrant: Option<&crate::vectorstore_qdrant::EmbeddedQdrant>,
) -> Result<usize> {
    embed_pending_chunks(conn, "conversation_chunks", qdrant)
}
