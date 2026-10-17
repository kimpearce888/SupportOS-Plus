use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use rusqlite::Connection;
use serde_json::{Value, json};

use super::super::server::AppState;
use crate::embeddings;
use crate::hybrid_search::{FtsDocHit, SemanticDocHit, merge_doc_hits};

const FTS_ROW_LIMIT: i64 = 400;
const SEMANTIC_LIMIT: usize = 20;
const MATCHED_CHUNK_UNITS: usize = 220;

const MODE_NOTE_DEFAULT: &str = "Keyword search (FTS5).";
const MODE_NOTE_SEMANTIC_DISABLED: &str =
    "Keyword search (FTS) - semantic retrieval disabled for this query.";
const MODE_NOTE_QDRANT: &str =
    "Hybrid search: FTS5 + semantic vectors via Qdrant, fused with Reciprocal Rank Fusion.";
const MODE_NOTE_LOCAL_COSINE: &str = "Hybrid search: FTS5 + semantic vectors (local cosine scan - Qdrant not reachable), fused with Reciprocal Rank Fusion.";
const MODE_NOTE_NO_MODEL: &str = "Keyword search (FTS5). Semantic search needs an embedding model: Settings → LM Studio → embedding model, then the docs embedding job runs on the next sync.";
const MODE_NOTE_NO_CHUNKS: &str = "Keyword search (FTS5). An embedding model is configured but no docs chunks are embedded yet - run a sync so the embedding job can process the mirror.";
const MODE_NOTE_EMBED_FAILED: &str = "Keyword search (FTS5). Semantic retrieval failed this request (embedding provider unreachable) - retried automatically next search.";

type Reply = (StatusCode, Json<Value>);

const ARTICLE_SUMMARY_SELECT: &str = "SELECT a.id, a.remote_id,
       a.collection_local_id, c.name,
       a.category_local_id, cat.name,
       a.number, a.slug, a.name, a.status, a.preview, a.words, a.views,
       a.remote_created_at, a.remote_updated_at
  FROM docs_articles a
  LEFT JOIN docs_collections c ON c.id = a.collection_local_id
  LEFT JOIN docs_categories cat ON cat.id = a.category_local_id";

fn article_summary_json(r: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, Value)> {
    Ok((
        r.get::<_, i64>(0)?,
        json!({
            "id": r.get::<_, i64>(0)?,
            "remote_id": r.get::<_, i64>(1)?,
            "collection_id": r.get::<_, i64>(2)?,
            "collection_name": r.get::<_, Option<String>>(3)?,
            "category_id": r.get::<_, Option<i64>>(4)?,
            "category_name": r.get::<_, Option<String>>(5)?,
            "number": r.get::<_, Option<i64>>(6)?,
            "slug": r.get::<_, Option<String>>(7)?,
            "name": r.get::<_, String>(8)?,
            "status": r.get::<_, Option<String>>(9)?,
            "preview": r.get::<_, Option<String>>(10)?,
            "words": r.get::<_, Option<i64>>(11)?,
            "views": r.get::<_, Option<i64>>(12)?,
            "remote_created_at": r.get::<_, Option<String>>(13)?,
            "remote_updated_at": r.get::<_, Option<String>>(14)?,
        }),
    ))
}

fn validation_error(message: &str) -> Reply {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "statusCode": 422,
            "error": "ValidationError",
            "message": message,
        })),
    )
}

fn js_number(value: Option<&String>) -> Option<f64> {
    match value {
        None => None,
        Some(s) if s.is_empty() => None,
        Some(s) => Some(s.trim().parse::<f64>().unwrap_or(f64::NAN)),
    }
}

fn is_js_integer(n: f64) -> bool {
    n.is_finite() && n.fract() == 0.0
}

fn fts_doc_ranks(conn: &Connection, q: &str) -> (Vec<FtsDocHit>, HashMap<i64, String>) {
    let tokens = crate::search::tokenize(q, 2, 8);
    if tokens.is_empty() {
        return (Vec::new(), HashMap::new());
    }
    let pattern = tokens
        .iter()
        .map(|t| format!("\"{t}\"*"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut stmt = match conn.prepare(
        "SELECT article_id, snippet(docs_fts, 1, '[', ']', '…', 14)
           FROM docs_fts WHERE docs_fts MATCH ?1 ORDER BY rank LIMIT ?2",
    ) {
        Ok(s) => s,
        Err(_) => return (Vec::new(), HashMap::new()),
    };
    let rows = stmt
        .query_map(rusqlite::params![pattern, FTS_ROW_LIMIT], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut hits: Vec<FtsDocHit> = Vec::new();
    let mut snippets: HashMap<i64, String> = HashMap::new();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for (article_id, snippet) in rows {
        if seen.insert(article_id) {
            hits.push(FtsDocHit {
                article_id,
                rank: hits.len(),
            });
            if let Some(s) = snippet {
                snippets.insert(article_id, s);
            }
        }
    }
    (hits, snippets)
}

pub async fn collections(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn_lock();
    let collections: Vec<Value> = conn
        .prepare(
            "SELECT id, remote_id, name, slug, description, visibility,
                    article_count, last_synced_at
               FROM docs_collections ORDER BY name",
        )
        .map(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "remote_id": r.get::<_, i64>(1)?,
                    "name": r.get::<_, String>(2)?,
                    "slug": r.get::<_, Option<String>>(3)?,
                    "description": r.get::<_, Option<String>>(4)?,
                    "visibility": r.get::<_, Option<String>>(5)?,
                    "article_count": r.get::<_, Option<i64>>(6)?,
                    "last_synced_at": r.get::<_, Option<String>>(7)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
        .unwrap_or_default();
    let docs_sync_available = state.provider_kind == "fake"
        || state
            .real
            .as_ref()
            .is_some_and(|r| !r.credentials.docs_api_key.is_empty());
    Json(json!({
        "collections": collections,
        "docs_sync_available": docs_sync_available,
    }))
}

pub async fn stats(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn_lock();
    let (collections, articles, published, drafts, internal, total_views, last_synced_at) = conn
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM docs_collections),
               (SELECT COUNT(*) FROM docs_articles),
               (SELECT COUNT(*) FROM docs_articles WHERE status = 'published'),
               (SELECT COUNT(*) FROM docs_articles WHERE status = 'draft'),
               (SELECT COUNT(*) FROM docs_articles WHERE status = 'internal'),
               (SELECT COALESCE(SUM(views), 0) FROM docs_articles),
               (SELECT MAX(last_synced_at) FROM docs_articles)",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .unwrap_or((0, 0, 0, 0, 0, 0, None));
    let (chat_sessions, email_conversations): (i64, i64) = conn
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM conversations
                 WHERE deleted_at IS NULL AND type = 'chat'),
               (SELECT COUNT(*) FROM conversations
                 WHERE deleted_at IS NULL AND type = 'email')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));
    let embedding = embeddings::docs_chunk_stats(&conn).unwrap_or_default();
    Json(json!({
        "collections": collections,
        "articles": articles,
        "published": published,
        "drafts": drafts,
        "internal": internal,
        "total_views": total_views,
        "last_synced_at": last_synced_at,
        "chat_sessions": chat_sessions,
        "email_conversations": email_conversations,
        "docs_chunks": embedding.chunks,
        "docs_chunks_indexed": embedding.indexed,
        "docs_chunks_pending": embedding.pending,
        "docs_chunks_failed": embedding.failed,
    }))
}

pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Reply {
    let limit = match js_number(params.get("limit")) {
        None => 25_i64,
        Some(n) if is_js_integer(n) && (1.0..=100.0).contains(&n) => n as i64,
        Some(_) => {
            return validation_error("limit must be 1-100.");
        }
    };
    let query = params
        .get("q")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if query.is_empty() {
        return validation_error("q is required.");
    }
    let semantic_requested = match params.get("semantic").map(|s| s.as_str()) {
        None | Some("") | Some("1") | Some("true") => true,
        Some(_) => false,
    };

    let (fts, snippets, settings, qdrant_enabled, stats) = {
        let conn = state.conn_lock();
        let (fts, snippets) = fts_doc_ranks(&conn, &query);
        let settings = embeddings::lm_studio_embedding_settings(&conn).unwrap_or(
            embeddings::LmStudioEmbeddingSettings {
                base_url: String::new(),
                embedding_model: None,
                timeout_ms: 0,
            },
        );
        let qdrant_enabled =
            crate::settings::get_bool(&conn, "qdrant_enabled", true).unwrap_or(true);
        let stats = embeddings::docs_chunk_stats(&conn).unwrap_or_default();
        (fts, snippets, settings, qdrant_enabled, stats)
    };

    let mut semantic: Vec<SemanticDocHit> = Vec::new();
    let mut matched_chunks: HashMap<i64, String> = HashMap::new();
    let mut used_semantic = false;
    let mut mode_note = MODE_NOTE_DEFAULT.to_string();

    if !semantic_requested {
        mode_note = MODE_NOTE_SEMANTIC_DISABLED.to_string();
    } else if settings.embedding_model.is_none() {
        mode_note = MODE_NOTE_NO_MODEL.to_string();
    } else if stats.indexed == 0 {
        mode_note = MODE_NOTE_NO_CHUNKS.to_string();
    } else if let Some(model) = settings.embedding_model.clone() {
        let embedded = embeddings::embed_texts(
            &settings.base_url,
            &model,
            &[query.clone()],
            settings.timeout_ms,
        )
        .await;
        match embedded {
            Ok(vectors) if vectors.first().is_some_and(|v| !v.is_empty()) => {
                let vector = vectors.into_iter().next().unwrap_or_default();
                if qdrant_enabled {
                    for h in state.qdrant.search(&vector, SEMANTIC_LIMIT) {
                        if h.payload.get("entity_type").and_then(|v| v.as_str())
                            != Some("docs_chunk")
                        {
                            continue;
                        }
                        let Some(article_id) = h.payload.get("entity_id").and_then(|v| v.as_i64())
                        else {
                            continue;
                        };
                        semantic.push(SemanticDocHit {
                            article_id,
                            score: h.score,
                        });
                    }
                }
                let served_by_qdrant = !semantic.is_empty();
                if semantic.is_empty() {
                    let conn = state.conn_lock();
                    if let Ok(chunks) = embeddings::list_doc_chunks_with_embedding(&conn) {
                        let mut best: Vec<(i64, f32)> = Vec::new();
                        let mut index: HashMap<i64, usize> = HashMap::new();
                        for c in &chunks {
                            let sim =
                                crate::hybrid_search::cosine_similarity(&vector, &c.embedding);
                            match index.get(&c.article_id) {
                                Some(&i) => {
                                    if sim > best[i].1 {
                                        best[i].1 = sim;
                                        matched_chunks.insert(
                                            c.article_id,
                                            embeddings::utf16_slice(
                                                &c.content,
                                                0,
                                                MATCHED_CHUNK_UNITS,
                                            ),
                                        );
                                    }
                                }
                                None => {
                                    index.insert(c.article_id, best.len());
                                    best.push((c.article_id, sim));
                                    matched_chunks.insert(
                                        c.article_id,
                                        embeddings::utf16_slice(&c.content, 0, MATCHED_CHUNK_UNITS),
                                    );
                                }
                            }
                        }
                        best.sort_by(|a, b| {
                            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                        });
                        semantic = best
                            .into_iter()
                            .take(SEMANTIC_LIMIT)
                            .map(|(article_id, score)| SemanticDocHit { article_id, score })
                            .collect();
                    }
                }
                if served_by_qdrant {
                    mode_note = MODE_NOTE_QDRANT.to_string();
                } else {
                    mode_note = MODE_NOTE_LOCAL_COSINE.to_string();
                }
                used_semantic = !semantic.is_empty();
            }
            _ => {
                mode_note = MODE_NOTE_EMBED_FAILED.to_string();
            }
        }
    }

    let merged = merge_doc_hits(&fts, &semantic, usize::try_from(limit).unwrap_or(25));

    let mut hits: Vec<Value> = Vec::new();
    {
        let conn = state.conn_lock();
        let by_id: HashMap<i64, Value> = if merged.is_empty() {
            HashMap::new()
        } else {
            let list = merged
                .iter()
                .map(|m| m.article_id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            conn.prepare(&format!("{ARTICLE_SUMMARY_SELECT} WHERE a.id IN ({list})"))
                .and_then(|mut stmt| {
                    stmt.query_map([], |r| article_summary_json(r))
                        .map(|rows| rows.filter_map(|r| r.ok()).collect())
                })
                .unwrap_or_default()
        };
        for m in &merged {
            let Some(article) = by_id.get(&m.article_id) else {
                continue;
            };
            let snippet = snippets.get(&m.article_id).cloned().or_else(|| {
                article
                    .get("preview")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            });
            hits.push(json!({
                "article": article,
                "score": m.score,
                "why": m.why,
                "snippet": snippet,
                "matched_chunk": matched_chunks.get(&m.article_id).cloned(),
            }));
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "query": query,
            "hits": hits,
            "total": hits.len(),
            "used_semantic": used_semantic,
            "semantic_available": settings.embedding_model.is_some() && stats.indexed > 0,
            "mode_note": mode_note,
        })),
    )
}

pub async fn articles(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Reply {
    let page_n = js_number(params.get("page")).unwrap_or(1.0);
    let page_size_n = js_number(params.get("pageSize")).unwrap_or(25.0);
    if !is_js_integer(page_n)
        || page_n < 1.0
        || !is_js_integer(page_size_n)
        || !(1.0..=100.0).contains(&page_size_n)
    {
        return validation_error("page must be >= 1 and pageSize must be 1-100.");
    }
    let collection_id = match params.get("collectionId") {
        None => None,
        Some(s) if s.is_empty() => None,
        Some(_) => {
            let n = js_number(params.get("collectionId")).unwrap_or(f64::NAN);
            if is_js_integer(n) && n > 0.0 {
                Some(n as i64)
            } else {
                return validation_error("collectionId must be a positive integer.");
            }
        }
    };
    let status = params.get("status").filter(|s| !s.is_empty());
    if let Some(status) = status {
        if !matches!(&**status, "published" | "draft" | "internal") {
            return validation_error("status must be one of 'published', 'draft', 'internal'.");
        }
    }

    let conn = state.conn_lock();

    let mut where_parts: Vec<String> = Vec::new();
    let mut binds: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(collection) = collection_id {
        where_parts.push(format!("a.collection_local_id = ?{}", binds.len() + 1));
        binds.push(Box::new(collection));
    }
    if let Some(status) = status {
        where_parts.push(format!("a.status = ?{}", binds.len() + 1));
        binds.push(Box::new(status.to_string()));
    }
    if let Some(q) = params.get("q").filter(|s| !s.is_empty()) {
        let tokens = crate::search::tokenize(q, 2, 6);
        if !tokens.is_empty() {
            let pattern = tokens
                .iter()
                .map(|t| format!("\"{t}\"*"))
                .collect::<Vec<_>>()
                .join(" ");
            let ids: Vec<i64> = conn
                .prepare("SELECT article_id FROM docs_fts WHERE docs_fts MATCH ?1")
                .and_then(|mut stmt| {
                    stmt.query_map(rusqlite::params![pattern], |r| r.get::<_, i64>(0))
                        .map(|rows| rows.filter_map(|r| r.ok()).collect())
                })
                .unwrap_or_default();
            if ids.is_empty() {
                where_parts.push("0".to_string());
            } else {
                let list = ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                where_parts.push(format!("a.id IN ({list})"));
            }
        }
    }
    let where_sql = if where_parts.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_parts.join(" AND "))
    };

    let page = page_n as i64;
    let page_size = page_size_n as i64;
    let offset = (page - 1) * page_size;

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = binds.iter().map(|v| v.as_ref()).collect();
    let total: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM docs_articles a {where_sql}"),
            params_ref.as_slice(),
            |r| r.get(0),
        )
        .unwrap_or(0);

    let mut all_binds = binds;
    all_binds.push(Box::new(page_size));
    all_binds.push(Box::new(offset));
    let all_refs: Vec<&dyn rusqlite::types::ToSql> = all_binds.iter().map(|v| v.as_ref()).collect();
    let article_rows: Vec<Value> = conn
        .prepare(&format!(
            "{ARTICLE_SUMMARY_SELECT} {where_sql}
             ORDER BY COALESCE(a.remote_updated_at, a.remote_created_at) DESC, a.id DESC
             LIMIT ?{} OFFSET ?{}",
            all_binds.len() - 1,
            all_binds.len(),
        ))
        .and_then(|mut stmt| {
            stmt.query_map(all_refs.as_slice(), |r| {
                article_summary_json(r).map(|(_, v)| v)
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    (
        StatusCode::OK,
        Json(json!({
            "articles": article_rows,
            "total": total,
            "page": page,
            "page_size": page_size,
        })),
    )
}

pub async fn article(State(state): State<AppState>, Path(id): Path<i64>) -> Reply {
    if id <= 0 {
        return validation_error("Article id must be a positive integer.");
    }
    let conn = state.conn_lock();
    let row: Option<Value> = conn
        .query_row(
            &format!(
                "SELECT a.id, a.remote_id,
                        a.collection_local_id, c.name,
                        a.category_local_id, cat.name,
                        a.number, a.slug, a.name, a.status, a.preview, a.words, a.views,
                        a.remote_created_at, a.remote_updated_at, a.text
                   FROM docs_articles a
                   LEFT JOIN docs_collections c ON c.id = a.collection_local_id
                   LEFT JOIN docs_categories cat ON cat.id = a.category_local_id
                  WHERE a.id = ?1"
            ),
            rusqlite::params![id],
            |r| {
                let (_, mut summary) = article_summary_json(r)?;
                summary["text"] = json!(r.get::<_, Option<String>>(15)?);
                Ok(summary)
            },
        )
        .ok();
    match row {
        Some(article) => (
            StatusCode::OK,
            Json(json!({
                "article": article,
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Article not found in the local docs mirror. Run a sync with a Docs API key configured."
            })),
        ),
    }
}
