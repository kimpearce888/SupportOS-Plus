use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use super::super::server::AppState;
use crate::ai_pipeline::{self, AiBackend, LmStudioError};

fn validation_422(message: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "statusCode": 422,
            "error": "ValidationError",
            "message": message,
        })),
    )
        .into_response()
}

fn service_503(message: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "statusCode": 503,
            "error": "ServiceUnavailable",
            "message": message,
        })),
    )
        .into_response()
}

fn not_found_404(message: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "statusCode": 404,
            "error": "NotFound",
            "message": message,
        })),
    )
        .into_response()
}

fn ensure_pipeline_or_log(conn: &rusqlite::Connection) {
    if let Err(e) = ai_pipeline::ensure_pipeline_schema(conn) {
        tracing::warn!(error = %e, "ensure_pipeline_schema failed; AI routes will serve empty data");
    }
}

pub async fn status(State(state): State<AppState>) -> Json<Value> {
    let (base_url, chat_model, embedding_model, timeout_ms, concurrency, provider_kind) = {
        let conn = state.conn_lock();
        let ai_enabled = crate::settings::get_bool(&conn, "ai_enabled", true).unwrap_or(true);
        let provider_kind = if ai_enabled { "lmstudio" } else { "disabled" }.to_string();
        let base_url = crate::settings::get_string(&conn, "lmstudio_base_url")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                crate::ai_center::get_ai_status(&conn)
                    .ok()
                    .and_then(|s| s.base_url)
            })
            .unwrap_or_else(|| "http://127.0.0.1:1234".into());
        let chat_model = crate::settings::get_string(&conn, "lmstudio_chat_model")
            .ok()
            .flatten()
            .map(|s| if s.is_empty() { None } else { Some(s) })
            .unwrap_or_else(|| {
                crate::ai_center::get_ai_status(&conn)
                    .ok()
                    .and_then(|s| s.chat_model)
            });
        let embedding_model = crate::settings::get_string(&conn, "lmstudio_embedding_model")
            .ok()
            .flatten()
            .map(|s| if s.is_empty() { None } else { Some(s) })
            .unwrap_or_else(|| {
                crate::ai_center::get_ai_status(&conn)
                    .ok()
                    .and_then(|s| s.embedding_model)
            });
        let timeout_ms =
            crate::settings::get_i64(&conn, "lmstudio_timeout_ms", 120_000).unwrap_or(120_000);
        let concurrency = crate::settings::get_i64(&conn, "lmstudio_concurrency", 2).unwrap_or(2);
        (
            base_url,
            chat_model,
            embedding_model,
            timeout_ms,
            concurrency,
            provider_kind,
        )
    };
    let client = crate::ai_lm_studio::OpenAiCompatibleClient::new_with_timeout(
        &base_url,
        u64::try_from(timeout_ms).unwrap_or(crate::ai_lm_studio::LM_STUDIO_DEFAULT_TIMEOUT_MS),
    );
    let lmstudio = match client.list_models().await {
        Ok(models) => json!({
            "connected": true,
            "models": models.iter().map(|m| json!(m.id)).collect::<Vec<_>>(),
            "error": Value::Null,
        }),
        Err(e) => json!({
            "connected": false,
            "models": [],
            "error": format!(
                "LM Studio is not reachable at {base_url}. Start LM Studio, load a model, and enable the local server (Developer tab > Start Server). Details: {e}"
            ),
        }),
    };
    let conn = state.conn_lock();
    let last_inference: Value = conn
        .query_row(
            "SELECT created_at, latency_ms FROM ai_runs
              WHERE status = 'completed' AND latency_ms IS NOT NULL
              ORDER BY id DESC LIMIT 1",
            [],
            |r| {
                Ok(json!({
                    "at": r.get::<_, String>(0)?,
                    "latencyMs": r.get::<_, i64>(1)?,
                }))
            },
        )
        .unwrap_or(Value::Null);
    let (queued, running, failed): (i64, i64, i64) = conn
        .query_row(
            "SELECT SUM(CASE WHEN status = 'queued' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status = 'running' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END)
             FROM jobs",
            [],
            |r| {
                Ok((
                    r.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                ))
            },
        )
        .unwrap_or((0, 0, 0));
    let (conversations_indexed, chunks_indexed, chunks_pending, chunks_failed): (
        i64,
        i64,
        i64,
        i64,
    ) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM conversation_threads
                  WHERE embedding_state = 'indexed'),
                (SELECT COUNT(*) FROM knowledge_chunks
                  WHERE embedding_state = 'indexed'),
                (SELECT COUNT(*) FROM knowledge_chunks
                  WHERE embedding_state IN ('not_indexed', 'queued')),
                (SELECT COUNT(*) FROM knowledge_chunks
                  WHERE embedding_state = 'failed')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap_or((0, 0, 0, 0));
    Json(json!({
        "ai_enabled": provider_kind == "lmstudio",
        "settings": {
            "base_url": base_url,
            "chat_model": chat_model,
            "embedding_model": embedding_model,
            "timeout_ms": timeout_ms,
            "concurrency": concurrency,
        },
        "lmstudio": lmstudio,
        "last_inference": last_inference,
        "provider": provider_kind,
        "queued_ai_jobs": queued + running,
        "failed_ai_jobs": failed,
        "index": {
            "conversations_indexed": conversations_indexed,
            "chunks_indexed": chunks_indexed,
            "chunks_pending": chunks_pending,
            "chunks_failed": chunks_failed,
        },
    }))
}

pub async fn analyze(
    State(state): State<AppState>,
    Path(conversation_id): Path<i64>,
    body: Option<Json<Value>>,
) -> Response {
    if conversation_id <= 0 {
        return validation_422("conversationId must be a positive integer.");
    }
    let force = body
        .and_then(|Json(b)| b.get("force").and_then(Value::as_bool))
        .unwrap_or(false);
    let result = state
        .run_ai(move |conn| {
            Box::pin(async move {
                ensure_pipeline_or_log(conn);
                let backend = ai_pipeline::backend_from_settings(conn);
                ai_pipeline::analyze_ticket(conn, &backend, conversation_id, force).await
            })
        })
        .await;
    match result {
        Ok(Ok(outcome)) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "analysis": outcome.analysis,
                "sources": outcome.sources,
                "cached": outcome.cached,
                "runId": outcome.run_id,
            })),
        )
            .into_response(),
        Ok(Err(e)) => service_503(&e.message),
        Err(join) => service_503(&join),
    }
}

pub async fn draft(
    State(state): State<AppState>,
    Path(conversation_id): Path<i64>,
    body: Option<Json<Value>>,
) -> Response {
    if conversation_id <= 0 {
        return validation_422("conversationId must be a positive integer.");
    }
    let body = body.map(|Json(b)| b).unwrap_or_else(|| json!({}));
    let mode = match body.get("mode") {
        None | Some(Value::Null) => "verified_answer",
        Some(Value::String(s)) if s == "verified_answer" || s == "standard" => s,
        Some(_) => return validation_422("mode must be 'verified_answer' or 'standard'."),
    };
    let force = match body.get("force") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return validation_422("force must be a boolean."),
    };
    let mode = mode.to_string();
    let result = state
        .run_ai(move |conn| {
            Box::pin(async move {
                ensure_pipeline_or_log(conn);
                let backend = ai_pipeline::backend_from_settings(conn);
                ai_pipeline::generate_draft(conn, &backend, conversation_id, &mode, force, None)
                    .await
            })
        })
        .await;
    match result {
        Ok(Ok((draft, verification))) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "draft": draft,
                "verification": verification,
            })),
        )
            .into_response(),
        Ok(Err(e)) => service_503(&e.message),
        Err(join) => service_503(&join),
    }
}

pub async fn rewrite(
    State(state): State<AppState>,
    Path(draft_id): Path<i64>,
    Json(body): Json<Value>,
) -> Response {
    let instruction = match body.get("instruction").and_then(Value::as_str) {
        Some(i) if ["shorten", "expand", "warmer", "more_direct"].contains(&i) => i.to_string(),
        _ => {
            return validation_422(
                "instruction must be one of 'shorten', 'expand', 'warmer', 'more_direct'.",
            )
        }
    };
    let result = state
        .run_ai(move |conn| {
            Box::pin(async move {
                ensure_pipeline_or_log(conn);
                let backend = ai_pipeline::backend_from_settings(conn);
                ai_pipeline::rewrite_draft(conn, &backend, draft_id, &instruction).await
            })
        })
        .await;
    match result {
        Ok(Ok(text)) => (StatusCode::OK, Json(json!({ "ok": true, "text": text }))).into_response(),
        Ok(Err(e)) => service_503(&e.message),
        Err(join) => service_503(&join),
    }
}

pub async fn verify(State(state): State<AppState>, Path(draft_id): Path<i64>) -> Response {
    let prepared = {
        let conn = state.conn_lock();
        ensure_pipeline_or_log(&conn);
        ai_pipeline::get_draft(&conn, draft_id)
            .ok()
            .flatten()
            .map(|d| (d.conversation_id, d.content))
    };
    let Some((conversation_id, content)) = prepared else {
        return not_found_404("Draft not found.");
    };
    let result = state
        .run_ai(move |conn| {
            Box::pin(async move {
                let backend = ai_pipeline::backend_from_settings(conn);
                let analysis = ai_pipeline::get_latest_analysis(conn, conversation_id)
                    .ok()
                    .flatten()
                    .map(|a| a.analysis);
                ai_pipeline::verify_draft_internal(
                    conn,
                    &backend,
                    conversation_id,
                    &content,
                    analysis.as_ref(),
                )
                .await
            })
        })
        .await;
    match result {
        Ok(Ok(verification)) => {
            let conn = state.conn_lock();
            let _ = conn.execute(
                "UPDATE ai_drafts SET verification = ?1 WHERE id = ?2",
                rusqlite::params![
                    serde_json::to_string(&verification).unwrap_or_default(),
                    draft_id
                ],
            );
            (
                StatusCode::OK,
                Json(json!({ "ok": true, "verification": verification })),
            )
                .into_response()
        }
        Ok(Err(e)) => service_503(&e.message),
        Err(join) => service_503(&join),
    }
}

pub async fn feedback(
    State(state): State<AppState>,
    Path(draft_id): Path<i64>,
    Json(body): Json<Value>,
) -> Response {
    let action = match body.get("action").and_then(Value::as_str) {
        Some(a) if ["accept", "reject", "edit"].contains(&a) => a.to_string(),
        _ => return validation_422("action must be 'accept', 'reject' or 'edit'."),
    };
    let final_text = body
        .get("finalText")
        .and_then(Value::as_str)
        .map(str::to_string);
    if action == "edit" && body.get("finalText").is_some() && final_text.is_none() {
        return validation_422("finalText must be a string.");
    }
    let conn = state.conn_lock();
    ensure_pipeline_or_log(&conn);
    let Some(draft) = ai_pipeline::get_draft(&conn, draft_id).ok().flatten() else {
        return Json(json!({ "ok": false, "message": "Draft not found." })).into_response();
    };
    match action.as_str() {
        "accept" => {
            let _ = ai_pipeline::set_draft_state(&conn, draft_id, "accepted");
        }
        "reject" => {
            let _ = ai_pipeline::set_draft_state(&conn, draft_id, "rejected");
        }
        "edit" => {
            let _ = ai_pipeline::set_draft_state(&conn, draft_id, "edited");
            if let Some(final_text) = final_text.as_deref() {
                let _ = ai_pipeline::record_feedback(
                    &conn,
                    draft_id,
                    &draft.content,
                    final_text,
                    false,
                );
            }
        }
        _ => unreachable!(),
    }
    let _ = crate::audit::audit(
        &conn,
        &crate::audit::AuditEntry {
            actor: "user",
            action: format!("ai_draft_{action}"),
            conversation_id: Some(draft.conversation_id),
            before_state: None,
            after_state: None,
            remote_operation: None,
            remote_result: None,
            ai_involvement: true,
            job_id: None,
            correlation_id: None,
        },
    );
    let message = format!(
        "Draft marked as {}.",
        if action == "edit" {
            "edited".to_string()
        } else {
            format!("{action}ed")
        }
    );
    Json(json!({ "ok": true, "message": message })).into_response()
}

pub async fn similar(State(state): State<AppState>, Path(conversation_id): Path<i64>) -> Response {
    if conversation_id <= 0 {
        return validation_422("conversationId must be a positive integer.");
    }
    let conn = state.conn_lock();
    ensure_pipeline_or_log(&conn);
    let similar: Vec<Value> = crate::ai_evidence::find_similar(&conn, conversation_id, 5, &[])
        .unwrap_or_default()
        .iter()
        .map(|s| {
            json!({
                "conversation_id": s.conversation_id,
                "number": s.number,
                "subject": s.subject,
                "resolution": s.resolution,
                "date": s.date,
                "status": s.status,
                "score": s.score,
                "why": s.why,
            })
        })
        .collect();
    Json(json!({ "similar": similar })).into_response()
}

pub async fn get_memory(State(state): State<AppState>, Path(customer_id): Path<i64>) -> Response {
    if customer_id <= 0 {
        return validation_422("customerId must be a positive integer.");
    }
    let conn = state.conn_lock();
    ensure_pipeline_or_log(&conn);
    let memories: Vec<Value> = conn
        .prepare(
            "SELECT id, customer_id, memory_key, memory_value, source, confidence, origin, created_at
               FROM customer_memory WHERE customer_id = ?1
              ORDER BY COALESCE(last_seen_at, created_at) DESC",
        )
        .ok()
        .map(|mut stmt| {
            stmt.query_map(rusqlite::params![customer_id], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "customer_id": r.get::<_, i64>(1)?,
                    "key": r.get::<_, String>(2)?,
                    "value": r.get::<_, Option<String>>(3)?,
                    "source": r.get::<_, String>(4)?,
                    "confidence": r.get::<_, Option<String>>(5)?,
                    "origin": r.get::<_, Option<String>>(6)?,
                    "created_at": r.get::<_, String>(7)?,
                }))
            })
            .ok()
            .map(|rows| {
                rows.filter_map(|r| r.ok())
                    .filter(|m| {
                        !super::memory::is_quarantined(
                            m["key"].as_str().unwrap_or(""),
                            m["value"].as_str(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
        })
        .unwrap_or_default();
    Json(json!({ "memories": memories, "customerId": customer_id })).into_response()
}

pub async fn set_memory(
    State(state): State<AppState>,
    Path(customer_id): Path<i64>,
    Json(body): Json<Value>,
) -> Response {
    if customer_id <= 0 {
        return validation_422("customerId must be a positive integer.");
    }
    let key = body.get("key").and_then(Value::as_str).unwrap_or_default();
    let value = body
        .get("value")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if key.is_empty() || key.chars().count() > 200 {
        return validation_422("key must be a non-empty string of at most 200 characters.");
    }
    if value.chars().count() > 5000 {
        return validation_422("value must be at most 5000 characters.");
    }
    let conn = state.conn_lock();
    ensure_pipeline_or_log(&conn);
    let customer_exists = conn
        .query_row(
            "SELECT 1 FROM customers WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![customer_id],
            |_| Ok(()),
        )
        .is_ok();
    if !customer_exists {
        return not_found_404("Customer not found.");
    }
    if super::memory::is_quarantined(key, Some(value)) {
        return validation_422(
            "SupportOS policy: psychological/personality judgments are never stored as customer memory. Rephrase as an observable fact.",
        );
    }
    let _ = ai_pipeline::upsert_memory(
        &conn,
        customer_id,
        key,
        value,
        &ai_pipeline::UpsertMemoryOpts {
            source: "human",
            origin: "manual",
            conversation_id: None,
            confidence: "high",
        },
    );
    let _ = crate::audit::audit(
        &conn,
        &crate::audit::AuditEntry {
            actor: "user",
            action: "memory_added".into(),
            conversation_id: None,
            before_state: None,
            after_state: None,
            remote_operation: None,
            remote_result: None,
            ai_involvement: false,
            job_id: None,
            correlation_id: None,
        },
    );
    Json(json!({ "ok": true, "message": "Memory saved (human-entered)." })).into_response()
}

pub async fn cluster_issues(State(state): State<AppState>, body: Option<Json<Value>>) -> Response {
    let days = body
        .map(|Json(b)| b.get("days").and_then(Value::as_i64))
        .unwrap_or(None);
    let days = match days {
        None => 60,
        Some(d) if (1..=3650).contains(&d) => d,
        Some(_) => return validation_422("days must be an integer between 1 and 3650."),
    };
    let result = state
        .run_ai(move |conn| {
            Box::pin(async move {
                ensure_pipeline_or_log(conn);
                let backend = ai_pipeline::backend_from_settings(conn);
                ai_pipeline::cluster_issues(conn, &backend, days).await
            })
        })
        .await;
    match result {
        Ok(Ok(clusters)) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "clusters": clusters,
            })),
        )
            .into_response(),
        Ok(Err(e)) => service_503(&e.message),
        Err(join) => service_503(&join),
    }
}

pub async fn jobs(
    State(state): State<AppState>,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
) -> Json<Value> {
    let limit = query
        .as_deref()
        .and_then(|q| q.split('&').find_map(|p| p.strip_prefix("limit=")))
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(100)
        .clamp(1, 1000);
    let conn = state.conn_lock();
    let jobs: Vec<Value> = conn
        .prepare(
            "SELECT id, type, status, conversation_id, model, prompt_version, error,
                    created_at, started_at, completed_at, latency_ms
               FROM ai_runs ORDER BY id DESC LIMIT ?1",
        )
        .ok()
        .map(|mut stmt| {
            stmt.query_map(rusqlite::params![limit], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "type": r.get::<_, String>(1)?,
                    "status": r.get::<_, String>(2)?,
                    "conversation_id": r.get::<_, Option<i64>>(3)?,
                    "model": r.get::<_, Option<String>>(4)?,
                    "prompt_version": r.get::<_, Option<String>>(5)?,
                    "error": r.get::<_, Option<String>>(6)?,
                    "created_at": r.get::<_, String>(7)?,
                    "started_at": r.get::<_, Option<String>>(8)?,
                    "completed_at": r.get::<_, Option<String>>(9)?,
                    "latency_ms": r.get::<_, Option<i64>>(10)?,
                }))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
        .unwrap_or_default();
    Json(json!({ "jobs": jobs }))
}

pub async fn ai_analytics(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn_lock();
    let (tickets_analyzed, analysis_completed, analysis_failed): (i64, i64, i64) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(DISTINCT conversation_id) FROM ai_runs
                  WHERE type = 'ticket_analysis' AND status = 'completed'
                    AND conversation_id IS NOT NULL),
                (SELECT COUNT(*) FROM ai_runs
                  WHERE type = 'ticket_analysis' AND status = 'completed'),
                (SELECT COUNT(*) FROM ai_runs
                  WHERE type = 'ticket_analysis' AND status = 'failed')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, 0, 0));
    let (draft_count, drafts_accepted, drafts_rejected, drafts_edited): (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM ai_drafts),
                (SELECT COUNT(*) FROM ai_drafts WHERE state = 'accepted'),
                (SELECT COUNT(*) FROM ai_drafts WHERE state = 'rejected'),
                (SELECT COUNT(DISTINCT f.draft_id) FROM ai_feedback f
                   JOIN ai_drafts d ON d.id = f.draft_id
                  WHERE f.edit_distance > 20)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap_or((0, 0, 0, 0));
    let (verification_warnings, unsupported_claims): (i64, i64) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM ai_verifications
                  WHERE verified = 1 AND warnings != '[]'),
                (SELECT COUNT(*) FROM ai_verifications
                  WHERE unsupported_claims != '[]')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));
    let analysis_success_rate = if analysis_completed + analysis_failed > 0 {
        ((analysis_completed as f64) / ((analysis_completed + analysis_failed) as f64) * 1000.0)
            .round()
            / 10.0
    } else {
        0.0
    };
    let feedback_total = drafts_accepted + drafts_rejected + drafts_edited;
    let draft_edit_rate = if feedback_total > 0 {
        ((drafts_edited as f64) / (feedback_total as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };
    let unsupported_claim_rate = if draft_count > 0 {
        ((unsupported_claims as f64) / (draft_count as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };
    let patterns: Vec<Value> = conn
        .prepare(
            "SELECT json_extract(verification, '$.warnings[0]') AS pattern, COUNT(*) AS count
               FROM ai_drafts
              WHERE verification IS NOT NULL
                AND json_extract(verification, '$.warnings[0]') IS NOT NULL
              GROUP BY pattern ORDER BY count DESC LIMIT 5",
        )
        .ok()
        .map(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "pattern": r.get::<_, String>(0)?,
                    "count": r.get::<_, i64>(1)?,
                }))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
        .unwrap_or_default();
    Json(json!({
        "tickets_analyzed": tickets_analyzed,
        "analysis_success_rate": analysis_success_rate,
        "draft_count": draft_count,
        "draft_accepted": drafts_accepted,
        "draft_rejected": drafts_rejected,
        "draft_edit_rate": draft_edit_rate,
        "verification_warnings": verification_warnings,
        "unsupported_claim_rate": unsupported_claim_rate,
        "common_failure_patterns": patterns,
        "source": "local",
    }))
}

pub async fn evaluation(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn_lock();
    let mode = crate::settings::get_bool(&conn, "ai_evaluation_mode", false).unwrap_or(false);
    ensure_pipeline_or_log(&conn);
    let tests: Vec<Value> = conn
        .prepare(
            "SELECT name, category, subject, body FROM golden_test_set
              WHERE active = 1 ORDER BY id",
        )
        .map(|mut stmt| {
            let rows: Vec<std::result::Result<Value, _>> = stmt
                .query_map([], |r| {
                    Ok(json!({
                        "name": r.get::<_, String>(0)?,
                        "category": r.get::<_, String>(1)?,
                        "payload": {
                            "subject": r.get::<_, String>(2)?,
                            "body": r.get::<_, String>(3)?,
                        },
                    }))
                })
                .map(|rows| rows.collect())
                .unwrap_or_default();
            rows.into_iter().filter_map(|r| r.ok()).collect()
        })
        .unwrap_or_default();
    let last_run: Option<Value> = conn
        .query_row(
            "SELECT id, started_at, finished_at, backend, passed, failed, results
               FROM golden_test_runs ORDER BY id DESC LIMIT 1",
            [],
            |r| {
                let results: Option<String> = r.get(6)?;
                let results: Value = results
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_else(|| json!([]));
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "started_at": r.get::<_, Option<String>>(1)?,
                    "finished_at": r.get::<_, Option<String>>(2)?,
                    "backend": r.get::<_, String>(3)?,
                    "passed": r.get::<_, i64>(4)?,
                    "failed": r.get::<_, i64>(5)?,
                    "results": results,
                }))
            },
        )
        .ok();
    Json(json!({
        "tests": tests,
        "evaluation_mode": mode,
        "last_run": last_run,
    }))
}

pub async fn evaluation_run(State(state): State<AppState>) -> Response {
    let result = state
        .run_ai(move |conn| {
            Box::pin(async move {
                ensure_pipeline_or_log(conn);
                let backend = ai_pipeline::backend_from_settings(conn);
                let backend_label = match &backend {
                    ai_pipeline::AiBackend::LmStudio { .. } => "lmstudio",
                    ai_pipeline::AiBackend::Disabled => "disabled",
                };
                let tests: Vec<(String, String, String, String)> = conn
                    .prepare(
                        "SELECT name, category, subject, body FROM golden_test_set
                          WHERE active = 1 ORDER BY id",
                    )
                    .map(|mut stmt| {
                        let rows: Vec<std::result::Result<(String, String, String, String), _>> =
                            stmt.query_map([], |r| {
                                Ok((
                                    r.get::<_, String>(0)?,
                                    r.get::<_, String>(1)?,
                                    r.get::<_, String>(2)?,
                                    r.get::<_, String>(3)?,
                                ))
                            })
                            .map(|rows| rows.collect())
                            .unwrap_or_default();
                        rows.into_iter().filter_map(|r| r.ok()).collect()
                    })
                    .unwrap_or_default();
                let run_id: i64 = conn
                    .query_row(
                        "INSERT INTO golden_test_runs (backend) VALUES (?1) RETURNING id",
                        rusqlite::params![backend_label],
                        |r| r.get(0),
                    )
                    .unwrap_or_default();
                let mut passed: i64 = 0;
                let mut failed: i64 = 0;
                let mut results: Vec<Value> = Vec::new();
                for (name, category, subject, body) in tests {
                    let outcome = run_one_golden_test(conn, &backend, &subject, &body).await;
                    let ok = outcome
                        .get("passed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if ok {
                        passed += 1;
                    } else {
                        failed += 1;
                    }
                    results.push(json!({
                        "name": name,
                        "category": category,
                        "passed": ok,
                        "stages": outcome.get("stages").cloned().unwrap_or_else(|| json!({})),
                        "error": outcome.get("error").cloned().unwrap_or_else(|| json!(null)),
                    }));
                }
                let _ = conn.execute(
                    "UPDATE golden_test_runs
                        SET finished_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                            passed = ?1, failed = ?2, results = ?3
                      WHERE id = ?4",
                    rusqlite::params![
                        passed,
                        failed,
                        serde_json::to_string(&results).unwrap_or_else(|_| "[]".into()),
                        run_id
                    ],
                );
                run_id
            })
        })
        .await;
    match result {
        Ok(run_id) => {
            let conn = state.conn_lock();
            let run: Option<Value> = conn
                .query_row(
                    "SELECT id, started_at, finished_at, backend, passed, failed, results
                       FROM golden_test_runs WHERE id = ?1",
                    rusqlite::params![run_id],
                    |r| {
                        let results: Option<String> = r.get(6)?;
                        let results: Value = results
                            .and_then(|s| serde_json::from_str(&s).ok())
                            .unwrap_or_else(|| json!([]));
                        Ok(json!({
                            "id": r.get::<_, i64>(0)?,
                            "started_at": r.get::<_, Option<String>>(1)?,
                            "finished_at": r.get::<_, Option<String>>(2)?,
                            "backend": r.get::<_, String>(3)?,
                            "passed": r.get::<_, i64>(4)?,
                            "failed": r.get::<_, i64>(5)?,
                            "results": results,
                        }))
                    },
                )
                .ok();
            (StatusCode::OK, Json(json!({ "ok": true, "run": run }))).into_response()
        }
        Err(join) => service_503(&join),
    }
}

async fn run_one_golden_test(
    conn: &rusqlite::Connection,
    backend: &ai_pipeline::AiBackend,
    subject: &str,
    body: &str,
) -> Value {
    let now = chrono::Utc::now().to_rfc3339();
    let insert = (|| -> std::result::Result<i64, rusqlite::Error> {
        let remote_id = -(conn.query_row(
            "SELECT COALESCE(MAX(id), 0) + 1000000 FROM conversations",
            [],
            |r| r.get::<_, i64>(0),
        )?);
        conn.execute(
            "INSERT INTO conversations
                (remote_id, number, subject, status, mailbox_local_id, customer_local_id,
                 priority, created_at, updated_at)
             VALUES (?1, ?1, ?2, 'active',
                     (SELECT id FROM mailboxes WHERE id = 1),
                     (SELECT id FROM customers WHERE id = 3001),
                     'normal', ?3, ?3)",
            rusqlite::params![remote_id, subject, now],
        )?;
        let conv_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversation_threads
                (conversation_id, type, state, body_text, from_type, created_at)
             VALUES (?1, 'customer_message', 'published', ?2, 'customer', ?3)",
            rusqlite::params![conv_id, body, now],
        )?;
        Ok(conv_id)
    })();
    let Ok(conv_id) = insert else {
        return json!({
            "passed": false,
            "stages": {},
            "error": "Could not create the synthetic evaluation conversation.",
        });
    };

    let analysis = ai_pipeline::analyze_ticket(conn, backend, conv_id, true).await;
    let mut stages = serde_json::json!({});
    match analysis {
        Ok(outcome) => {
            stages["analysis"] = json!({ "ok": true, "intent": outcome.analysis.intent.clone() });
        }
        Err(e) => {
            cleanup_golden_conversation(conn, conv_id);
            return json!({
                "passed": false,
                "stages": stages,
                "error": format!("analysis: {}", e.message),
            });
        }
    }

    match ai_pipeline::generate_draft(conn, backend, conv_id, "verified_answer", true, None).await {
        Ok((_draft, verification)) => {
            stages["draft"] = json!({ "ok": true });
            stages["verification"] = json!({
                "ok": verification.as_ref().is_some_and(|v| v.verified),
            });
            let verified = verification.as_ref().is_some_and(|v| v.verified);
            cleanup_golden_conversation(conn, conv_id);
            json!({ "passed": verified, "stages": stages, "error": if verified { Value::Null } else { json!("verification: the draft did not pass the verification pass") } })
        }
        Err(e) => {
            cleanup_golden_conversation(conn, conv_id);
            json!({
                "passed": false,
                "stages": stages,
                "error": format!("draft: {}", e.message),
            })
        }
    }
}

fn cleanup_golden_conversation(conn: &rusqlite::Connection, conv_id: i64) {
    let _ = conn.execute(
        "DELETE FROM conversations WHERE id = ?1",
        rusqlite::params![conv_id],
    );
}
