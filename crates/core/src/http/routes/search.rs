use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use rusqlite::Connection;
use serde_json::{Value, json};

use super::super::server::AppState;
use crate::embeddings::{self, utf16_len};
use crate::hybrid_search::{FtsDocHit, SemanticDocHit, cosine_similarity, merge_doc_hits};
use crate::search::{SearchFilters, SearchHit, SearchResponse};

const MAX_QUERY_UNITS: usize = 500;

const SEMANTIC_TICKET_LIMIT: usize = 24;

const MERGED_TICKET_LIMIT: usize = 40;

const MODE_NOTE_QDRANT: &str =
    "Hybrid ticket search: FTS5 + semantic vectors via Qdrant, fused with Reciprocal Rank Fusion.";
const MODE_NOTE_LOCAL_QDRANT_DOWN: &str = "Hybrid ticket search: FTS5 + semantic vectors (local cosine scan - Qdrant not reachable), fused with Reciprocal Rank Fusion.";
const MODE_NOTE_LOCAL: &str =
    "Hybrid ticket search: FTS5 + local semantic vectors, fused with Reciprocal Rank Fusion.";
const MODE_NOTE_NO_CHUNKS: &str = "Keyword search (FTS5). An embedding model is configured but no ticket chunks are embedded yet - run a sync so the embedding job can process the mirror.";
const MODE_NOTE_NO_MODEL: &str = "Keyword search (FTS5). Semantic ticket search needs an embedding model: Settings → LM Studio → embedding model, then the ticket embedding job runs on the next sync.";

pub async fn search(State(state): State<AppState>, body: Bytes) -> impl IntoResponse {
    let body: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "statusCode": 400,
                    "error": "Bad Request",
                    "message": "Request body is not valid JSON."
                })),
            );
        }
    };

    let issues = validate_search_request(&body);
    if !issues.is_empty() {
        let first = &issues[0];
        let where_hint = if first.0.is_empty() {
            String::new()
        } else {
            format!(" ({})", first.0)
        };
        let issue_list: Vec<Value> = issues
            .iter()
            .take(10)
            .map(|(path, message)| json!({ "path": path, "message": message }))
            .collect();
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": format!("Invalid request{where_hint}: {}", first.1),
                "issues": issue_list
            })),
        );
    }

    let query = body
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let scope = body
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or("all")
        .to_string();
    let filters = parse_filters(&body);

    let (mut fts, settings, qdrant_enabled, stats) = {
        let conn = state.conn_lock();
        let fts = match crate::search::search(&conn, &query, &scope, &filters) {
            Ok(r) => r,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"message": e.to_string()})),
                );
            }
        };
        let settings = match embeddings::lm_studio_embedding_settings(&conn) {
            Ok(s) => s,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"message": e.to_string()})),
                );
            }
        };
        let qdrant_enabled =
            crate::settings::get_bool(&conn, "qdrant_enabled", true).unwrap_or(true);
        let stats = embeddings::conversation_chunk_stats(&conn);
        (fts, settings, qdrant_enabled, stats)
    };

    if !query.trim().is_empty() && (scope == "all" || scope == "tickets") {
        let stats = match stats {
            Ok(s) => s,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"message": e.to_string()})),
                );
            }
        };
        if settings.embedding_model.is_some() && stats.indexed > 0 {
            let ticket_query_vector: Option<Vec<f32>> = match &settings.embedding_model {
                Some(model) => {
                    match embeddings::embed_texts(
                        &settings.base_url,
                        model,
                        &[query.clone()],
                        settings.timeout_ms,
                    )
                    .await
                    {
                        Ok(vectors) => vectors.into_iter().next().filter(|v| !v.is_empty()),
                        Err(_) => None,
                    }
                }
                None => None,
            };

            if let Some(vector) = ticket_query_vector.filter(|v| !v.is_empty()) {
                let mut semantic: Vec<SemanticTicketHit> = Vec::new();
                let mut served_by_qdrant = false;
                if qdrant_enabled {
                    let hits = state.qdrant.search(&vector, SEMANTIC_TICKET_LIMIT);
                    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
                    for h in &hits {
                        if h.payload.get("entity_type").and_then(|v| v.as_str())
                            != Some("conversation_chunk")
                        {
                            continue;
                        }
                        let Some(conv_id) = h.payload.get("entity_id").and_then(|v| v.as_i64())
                        else {
                            continue;
                        };
                        if seen.insert(conv_id) {
                            let snippet = h
                                .payload
                                .get("text")
                                .and_then(|v| v.as_str())
                                .map(|t| embeddings::utf16_slice(t, 0, 200));
                            semantic.push(SemanticTicketHit {
                                conversation_id: conv_id,
                                rank: semantic.len(),
                                snippet,
                            });
                        }
                    }
                    served_by_qdrant = !semantic.is_empty();
                }
                if semantic.is_empty() {
                    let conn = state.conn_lock();
                    match semantic_ticket_retrieval(&conn, &vector) {
                        Ok(local) => semantic = local,
                        Err(_) => {
                            fts.semantic_available = false;
                            fts.total = fts.hits.len();
                            return (
                                StatusCode::OK,
                                Json(serde_json::to_value(&fts).unwrap_or(Value::Null)),
                            );
                        }
                    }
                }
                let mode_note = if served_by_qdrant {
                    MODE_NOTE_QDRANT
                } else if qdrant_enabled {
                    MODE_NOTE_LOCAL_QDRANT_DOWN
                } else {
                    MODE_NOTE_LOCAL
                };
                let conn = state.conn_lock();
                rebuild_ticket_hits(&conn, &mut fts, &semantic);
                fts.used_semantic = true;
                fts.semantic_available = true;
                fts.mode_note = Some(mode_note.to_string());

                if settings.embedding_model.is_some() && qdrant_enabled {
                    let hits = state.qdrant.search(&vector, 8);
                    for h in &hits {
                        if h.payload.get("entity_type").and_then(|v| v.as_str())
                            != Some("knowledge_chunk")
                        {
                            continue;
                        }
                        let (Some(entity_id), Some(title), Some(text)) = (
                            h.payload.get("entity_id").and_then(|v| v.as_i64()),
                            h.payload.get("title").and_then(|v| v.as_str()),
                            h.payload.get("text").and_then(|v| v.as_str()),
                        ) else {
                            continue;
                        };
                        let already = fts
                            .hits
                            .iter()
                            .any(|x| x.scope == "knowledge" && x.id == entity_id);
                        if already {
                            continue;
                        }
                        let visibility = if h.payload.get("visibility").and_then(|v| v.as_str())
                            == Some("customer_safe")
                        {
                            "customer-safe"
                        } else {
                            "internal"
                        };
                        fts.hits.push(crate::search::SearchHit {
                            scope: "knowledge".to_string(),
                            id: entity_id,
                            title: title.to_string(),
                            subtitle: format!("Knowledge · {visibility}"),
                            snippet: embeddings::utf16_slice(text, 0, 200),
                            score: h.score as f64,
                            href: format!("/knowledge/{entity_id}"),
                            why: vec!["semantic match".to_string()],
                        });
                    }
                    fts.total = fts.hits.len();
                }
            }
        } else if settings.embedding_model.is_some() && stats.indexed == 0 {
            fts.mode_note = Some(MODE_NOTE_NO_CHUNKS.to_string());
        } else if settings.embedding_model.is_none() {
            fts.mode_note = Some(MODE_NOTE_NO_MODEL.to_string());
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::to_value(&fts).unwrap_or(Value::Null)),
    )
}

fn is_valid_scope(scope: &str) -> bool {
    matches!(
        scope,
        "all" | "tickets" | "customers" | "knowledge" | "issues" | "saved_replies" | "ai"
    )
}

fn zod_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn zod_received(v: &Value) -> String {
    match v {
        Value::String(s) => format!("'{s}'"),
        Value::Null => "null".into(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(_) => "array".into(),
        Value::Object(_) => "object".into(),
    }
}

fn validate_search_request(body: &Value) -> Vec<(String, String)> {
    let mut issues: Vec<(String, String)> = Vec::new();
    let Some(obj) = body.as_object() else {
        issues.push((
            String::new(),
            format!("Expected object, received {}", zod_type_name(body)),
        ));
        return issues;
    };

    if let Some(q) = obj.get("query") {
        if !q.is_string() {
            issues.push((
                "query".into(),
                format!("Expected string, received {}", zod_type_name(q)),
            ));
        } else if utf16_len(q.as_str().unwrap_or("")) > MAX_QUERY_UNITS {
            issues.push((
                "query".into(),
                "String must contain at most 500 character(s)".into(),
            ));
        }
    }

    if let Some(s) = obj.get("scope") {
        if !s.as_str().is_some_and(is_valid_scope) {
            issues.push((
                "scope".into(),
                format!(
                    "Invalid enum value. Expected 'all' | 'tickets' | 'customers' | 'knowledge' | 'issues' | 'saved_replies' | 'ai', received {}",
                    zod_received(s)
                ),
            ));
        }
    }

    if let Some(f) = obj.get("filters") {
        if !f.is_object() {
            issues.push((
                "filters".into(),
                format!("Expected object, received {}", zod_type_name(f)),
            ));
        } else {
            let f = f.as_object().expect("checked above");
            for (key, expected) in [
                ("status", "string"),
                ("mailbox_id", "number"),
                ("tag", "string"),
                ("since_days", "number"),
                ("assignee_id", "number"),
            ] {
                if let Some(v) = f.get(key) {
                    let matches = if expected == "string" {
                        v.is_string()
                    } else {
                        v.is_number()
                    };
                    if !matches {
                        issues.push((
                            format!("filters.{key}"),
                            format!("Expected {expected}, received {}", zod_type_name(v)),
                        ));
                    }
                }
            }
        }
    }

    issues
}

fn parse_filters(body: &Value) -> SearchFilters {
    let empty = serde_json::Map::new();
    let f = body
        .get("filters")
        .and_then(|v| v.as_object())
        .unwrap_or(&empty);
    let get_str = |k: &str| f.get(k).and_then(|v| v.as_str()).map(String::from);
    let get_num = |k: &str| f.get(k).and_then(|v| v.as_i64());
    SearchFilters {
        status: get_str("status"),
        mailbox_id: get_num("mailbox_id"),
        tag: get_str("tag"),
        since_days: get_num("since_days"),
        assignee_id: get_num("assignee_id"),
    }
}

struct SemanticTicketHit {
    conversation_id: i64,
    rank: usize,
    snippet: Option<String>,
}

fn semantic_ticket_retrieval(
    conn: &Connection,
    query_vector: &[f32],
) -> crate::error::Result<Vec<SemanticTicketHit>> {
    let chunks = embeddings::list_conversation_chunks_with_embedding(conn)?;
    let mut best: Vec<(i64, f32, String)> = Vec::new();
    for chunk in chunks {
        let sim = cosine_similarity(query_vector, &chunk.embedding);
        match best
            .iter_mut()
            .find(|(cid, _, _)| *cid == chunk.conversation_id)
        {
            Some(entry) => {
                if sim > entry.1 {
                    entry.1 = sim;
                    entry.2 = chunk.content;
                }
            }
            None => best.push((chunk.conversation_id, sim, chunk.content)),
        }
    }
    best.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    best.truncate(SEMANTIC_TICKET_LIMIT);

    Ok(best
        .into_iter()
        .enumerate()
        .map(|(rank, (conversation_id, _, content))| SemanticTicketHit {
            conversation_id,
            rank,
            snippet: Some(embeddings::utf16_slice(&content, 0, 200)),
        })
        .collect())
}

fn rebuild_ticket_hits(
    conn: &Connection,
    fts: &mut SearchResponse,
    semantic: &[SemanticTicketHit],
) {
    let fts_tickets: Vec<SearchHit> = fts
        .hits
        .iter()
        .filter(|h| h.scope == "tickets")
        .cloned()
        .collect();
    let fts_ranks: Vec<FtsDocHit> = fts_tickets
        .iter()
        .enumerate()
        .map(|(i, h)| FtsDocHit {
            article_id: h.id,
            rank: i,
        })
        .collect();
    let semantic_ranks: Vec<SemanticDocHit> = semantic
        .iter()
        .map(|s| SemanticDocHit {
            article_id: s.conversation_id,
            score: 1.0 / (1.0 + s.rank as f32),
        })
        .collect();
    let merged = merge_doc_hits(&fts_ranks, &semantic_ranks, MERGED_TICKET_LIMIT);

    let fts_by_id: std::collections::HashMap<i64, &SearchHit> =
        fts_tickets.iter().map(|h| (h.id, h)).collect();

    let mut rebuilt: Vec<SearchHit> = Vec::new();
    for m in &merged {
        if let Some(existing) = fts_by_id.get(&m.article_id) {
            let mut hit = (*existing).clone();
            hit.score = m.score;
            if m.why.iter().any(|w| *w == "semantic") {
                hit.why.push("semantic match".into());
            }
            rebuilt.push(hit);
            continue;
        }
        let conv: Option<(i64, i64, Option<String>)> = conn
            .query_row(
                "SELECT id, number, subject FROM conversations WHERE id = ?1",
                rusqlite::params![m.article_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok();
        let Some((id, number, subject)) = conv else {
            continue;
        };
        let snippet = semantic
            .iter()
            .find(|s| s.conversation_id == m.article_id)
            .and_then(|s| s.snippet.clone())
            .unwrap_or_default();
        rebuilt.push(SearchHit {
            scope: "tickets".into(),
            id,
            title: format!(
                "#{} {}",
                number,
                subject.unwrap_or_else(|| "(no subject)".into())
            ),
            subtitle: "semantic match".into(),
            snippet,
            score: m.score,
            href: format!("/inbox/conversation/{id}"),
            why: vec!["semantic match".into()],
        });
    }

    let mut hits: Vec<SearchHit> = rebuilt;
    hits.extend(fts.hits.drain(..).filter(|h| h.scope != "tickets"));
    fts.hits = hits;
    fts.total = fts.hits.len();
}
