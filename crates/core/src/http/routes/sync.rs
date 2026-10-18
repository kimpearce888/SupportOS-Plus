use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::super::server::AppState;
use crate::audit::AuditEntry;
use crate::helpscout::HelpScoutProvider;

fn fastify_error(code: StatusCode, error: &str, message: &str) -> Response {
    (
        code,
        [(header::CONTENT_TYPE, "application/json")],
        Json(json!({ "statusCode": code.as_u16(), "error": error, "message": message })),
    )
        .into_response()
}

pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();

    let sync_state = crate::sync_engine::get_state(&conn);

    let current_run: Option<Value> = conn
        .query_row(
            "SELECT id, kind, state, started_at, finished_at, resources_done, resources_total,
                    records_processed, errors, detail
               FROM sync_runs WHERE finished_at IS NULL ORDER BY id DESC LIMIT 1",
            [],
            row_to_json_run,
        )
        .ok();

    let checkpoints: Vec<Value> = conn
        .prepare(
            "SELECT resource, last_success_at, remote_cursor, page_state, records_processed,
                    records_failed, last_error, retry_count, status
               FROM sync_checkpoints ORDER BY resource",
        )
        .map(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "resource": r.get::<_, String>(0)?,
                    "last_success_at": r.get::<_, Option<String>>(1)?,
                    "remote_cursor": r.get::<_, Option<String>>(2)?,
                    "page_state": r.get::<_, Option<String>>(3)?,
                    "records_processed": r.get::<_, i64>(4)?,
                    "records_failed": r.get::<_, i64>(5)?,
                    "last_error": r.get::<_, Option<String>>(6)?,
                    "retry_count": r.get::<_, i64>(7)?,
                    "status": r.get::<_, String>(8)?,
                }))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
        .unwrap_or_default();

    let last_success = crate::sync_engine::last_successful_sync(&conn);

    let recent_runs: Vec<Value> = conn
        .prepare(
            "SELECT id, kind, state, started_at, finished_at, resources_done, resources_total,
                    records_processed, errors, detail
               FROM sync_runs ORDER BY id DESC LIMIT 10",
        )
        .map(|mut stmt| {
            stmt.query_map([], row_to_json_run)
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let (total, pending, processed, failed, duplicates): (i64, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*),
                    SUM(CASE WHEN processing_state = 'pending' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN processing_state = 'processed' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN processing_state = 'failed' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN processing_state = 'duplicate' THEN 1 ELSE 0 END)
               FROM webhook_events",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(4)?.unwrap_or(0),
                ))
            },
        )
        .unwrap_or((0, 0, 0, 0, 0));

    let webhook_recent: Vec<Value> = conn
        .prepare(
            "SELECT id, event_id, event_type, received_at, processing_state, attempts, processing_error
               FROM webhook_events ORDER BY id DESC LIMIT 20",
        )
        .map(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "event_id": r.get::<_, Option<String>>(1)?,
                    "event_type": r.get::<_, String>(2)?,
                    "received_at": r.get::<_, String>(3)?,
                    "processing_state": r.get::<_, String>(4)?,
                    "attempts": r.get::<_, i64>(5)?,
                    "processing_error": r.get::<_, Option<String>>(6)?,
                }))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
        .unwrap_or_default();

    let webhook_configured: Vec<Value> = conn
        .prepare("SELECT remote_id, url, events, status FROM webhook_configs ORDER BY id")
        .map(|mut stmt| {
            stmt.query_map([], |r| {
                let events: Option<String> = r.get(2)?;
                Ok(json!({
                    "remote_id": r.get::<_, Option<i64>>(0)?,
                    "url": r.get::<_, Option<String>>(1)?,
                    "events": events
                        .and_then(|e| serde_json::from_str::<Value>(&e).ok())
                        .unwrap_or(json!([])),
                    "status": r.get::<_, Option<String>>(3)?,
                }))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
        .unwrap_or_default();

    let secret_configured = crate::settings::has_secret(&conn, "webhook_secret").unwrap_or(false)
        || std::env::var("HELPSCOUT_WEBHOOK_SECRET")
            .map(|s| !s.is_empty())
            .unwrap_or(false);

    let running = state.sync.as_ref().is_some_and(|s| s.is_running());

    let (rate_limit, api_queue) = match &state.real {
        Some(real) => (real.limiter.snapshot(), real.queue.snapshot()),
        None => (Value::Null, Value::Null),
    };

    Json(json!({
        "state": sync_state,
        "running": running,
        "current_run": current_run,
        "checkpoints": checkpoints,
        "last_success": last_success,
        "recent_runs": recent_runs,
        "webhook": {
            "events": {
                "total": total,
                "pending": pending,
                "processed": processed,
                "failed": failed,
                "duplicates": duplicates,
            },
            "recent": webhook_recent,
            "configured": webhook_configured,
            "secret_configured": secret_configured,
        },
        "rate_limit": rate_limit,
        "api_queue": api_queue,
    }))
}

fn row_to_json_run(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let detail: Option<String> = r.get(9)?;
    Ok(json!({
        "id": r.get::<_, i64>(0)?,
        "kind": r.get::<_, String>(1)?,
        "state": r.get::<_, String>(2)?,
        "started_at": r.get::<_, String>(3)?,
        "finished_at": r.get::<_, Option<String>>(4)?,
        "resources_done": r.get::<_, i64>(5)?,
        "resources_total": r.get::<_, i64>(6)?,
        "records_processed": r.get::<_, i64>(7)?,
        "errors": r.get::<_, i64>(8)?,
        "detail": detail.and_then(|d| serde_json::from_str::<Value>(&d).ok()),
    }))
}

pub async fn initial(State(state): State<AppState>, body: Option<Json<Value>>) -> Response {
    let Some(sync) = state.sync.clone() else {
        return fastify_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "ServiceUnavailable",
            "The sync engine is not available in this build.",
        );
    };
    if sync.is_running() {
        return fastify_error(
            StatusCode::CONFLICT,
            "Conflict",
            "A sync is already running.",
        );
    }
    let wait = body
        .map(|Json(b)| b.get("wait").and_then(|w| w.as_bool()).unwrap_or(false))
        .unwrap_or(false);
    if wait {
        let results = sync.initial_sync().await;
        match results {
            Ok(results) => Json(json!({
                "ok": true,
                "message": "Initial sync completed.",
                "results": results,
            }))
            .into_response(),
            Err(e) => fastify_error(StatusCode::CONFLICT, "Conflict", &e.to_string()),
        }
    } else {
        let sync = sync.clone();
        let conn = state.conn.clone();
        tokio::spawn(async move {
            if let Err(e) = sync.initial_sync().await {
                if let Ok(conn) = conn.lock() {
                    let _ =
                        crate::jobs::log_error(&conn, "sync", &format!("initial sync failed: {e}"));
                }
            }
        });
        Json(json!({
            "ok": true,
            "message": "Initial sync started. Watch Sync Health for progress.",
        }))
        .into_response()
    }
}

pub async fn incremental(State(state): State<AppState>, body: Option<Json<Value>>) -> Response {
    let Some(sync) = state.sync.clone() else {
        return fastify_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "ServiceUnavailable",
            "The sync engine is not available in this build.",
        );
    };
    if sync.is_running() {
        return fastify_error(
            StatusCode::CONFLICT,
            "Conflict",
            "A sync is already running.",
        );
    }
    let wait = body
        .map(|Json(b)| b.get("wait").and_then(|w| w.as_bool()).unwrap_or(false))
        .unwrap_or(false);
    if wait {
        let results = sync.incremental_sync().await;
        match results {
            Ok(results) => Json(json!({
                "ok": true,
                "message": "Incremental sync completed.",
                "results": results,
            }))
            .into_response(),
            Err(e) => fastify_error(StatusCode::CONFLICT, "Conflict", &e.to_string()),
        }
    } else {
        let sync = sync.clone();
        let conn = state.conn.clone();
        tokio::spawn(async move {
            if let Err(e) = sync.incremental_sync().await {
                if let Ok(conn) = conn.lock() {
                    let _ = crate::jobs::log_error(
                        &conn,
                        "sync",
                        &format!("incremental sync failed: {e}"),
                    );
                }
            }
        });
        Json(json!({ "ok": true, "message": "Incremental sync started." })).into_response()
    }
}

pub async fn reconcile(State(state): State<AppState>, body: Option<Json<Value>>) -> Response {
    let Some(sync) = state.sync.clone() else {
        return fastify_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "ServiceUnavailable",
            "The sync engine is not available in this build.",
        );
    };
    if sync.is_running() {
        return fastify_error(
            StatusCode::CONFLICT,
            "Conflict",
            "A sync is already running.",
        );
    }
    let wait = body
        .map(|Json(b)| b.get("wait").and_then(|w| w.as_bool()).unwrap_or(false))
        .unwrap_or(false);
    if wait {
        let result = sync.reconcile().await;
        match result {
            Ok(result) => Json(json!({
                "ok": true,
                "message": "Reconciliation completed.",
                "result": result,
            }))
            .into_response(),
            Err(e) => fastify_error(StatusCode::CONFLICT, "Conflict", &e.to_string()),
        }
    } else {
        let sync = sync.clone();
        let conn = state.conn.clone();
        tokio::spawn(async move {
            if let Err(e) = sync.reconcile().await {
                if let Ok(conn) = conn.lock() {
                    let _ =
                        crate::jobs::log_error(&conn, "sync", &format!("reconcile failed: {e}"));
                }
            }
        });
        Json(json!({ "ok": true, "message": "Reconciliation started." })).into_response()
    }
}

pub async fn cancel(State(state): State<AppState>) -> impl IntoResponse {
    if let Some(sync) = &state.sync {
        sync.request_cancellation();
    }
    Json(json!({
        "ok": true,
        "message": "Cancellation requested - the current resource will finish, then the sync stops.",
    }))
}

const SUPPORTED_WEBHOOK_EVENTS: [&str; 12] = [
    "convo.created",
    "convo.updated",
    "convo.assigned",
    "convo.status",
    "convo.customer.reply.created",
    "convo.agent.reply.created",
    "convo.note.created",
    "satisfaction.ratings",
    "customer.created",
    "customer.updated",
    "conversation.merged",
    "team.updated",
];

pub async fn register_webhook(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let url = body.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let events: Vec<String> = body
        .get("events")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|e| e.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let url_ok = url.starts_with("https://") && url.len() > "https://".len();
    let events_ok = !events.is_empty()
        && events
            .iter()
            .all(|e| SUPPORTED_WEBHOOK_EVENTS.contains(&e.as_str()));
    if !url_ok || !events_ok {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "ok": false,
                "message": "Invalid webhook registration: a reachable https URL and at least one supported event are required.",
            })),
        )
            .into_response();
    }
    let provider_kind = state.provider_kind.clone();
    if provider_kind != "real" {
        return Json(json!({
            "ok": false,
            "message": "Webhook registration targets a real Help Scout account - demo mode cannot register webhooks. Use POST /api/demo/simulate-webhook to exercise the pipeline locally.",
        }))
        .into_response();
    }
    let secret = {
        let conn = state.conn_lock();
        crate::settings::get_secret(&conn, "webhook_secret")
            .ok()
            .flatten()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default()
    };
    let secret = if secret.is_empty() {
        std::env::var("HELPSCOUT_WEBHOOK_SECRET").unwrap_or_default()
    } else {
        secret
    };
    if secret.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "ok": false,
                "message": "HELPSCOUT_WEBHOOK_SECRET must be set in .env first: Help Scout signs events with it and SupportOS verifies that signature.",
            })),
        )
            .into_response();
    }
    let Some(real) = state.real.clone() else {
        return Json(json!({
            "ok": false,
            "message": "Webhook registration requires the real Help Scout provider.",
        }))
        .into_response();
    };
    match real
        .create_webhook(url, &events, &secret, "SupportOS")
        .await
    {
        Ok(remote_id) => {
            let conn = state.conn_lock();
            let _ = conn.execute(
                "INSERT INTO webhook_configs (remote_id, url, events, status, last_synced_at)
                 VALUES (?1, ?2, ?3, 'enabled', datetime('now'))
                 ON CONFLICT(remote_id) DO UPDATE SET url = excluded.url,
                   events = excluded.events, status = 'enabled', last_synced_at = datetime('now')",
                rusqlite::params![
                    remote_id,
                    url,
                    serde_json::to_string(&events).unwrap_or_default()
                ],
            );
            let _ = crate::jobs::audit(
                &conn,
                "user",
                "webhook_registered",
                None,
                None,
                Some(
                    &serde_json::to_string(&json!({"url": url, "events": events}))
                        .unwrap_or_default(),
                ),
                Some("POST /v2/webhooks"),
                None,
                false,
            );
            Json(json!({
                "ok": true,
                "message": format!("Webhook #{} registered for {} event type(s). Help Scout will push changes to that URL; a localhost app needs a relay (see docs/API-INTEGRATION.md).", remote_id, events.len()),
                "remoteId": remote_id,
                "events": events,
            }))
            .into_response()
        }
        Err(e) => Json(json!({
            "ok": false,
            "message": format!("Help Scout rejected the webhook registration: {e}"),
        }))
        .into_response(),
    }
}

pub async fn unregister_webhook(
    State(state): State<AppState>,
    Path(remote_id): Path<String>,
) -> Response {
    let Ok(remote_id) = remote_id.parse::<i64>() else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "ok": false, "message": "Webhook id must be a positive integer." })),
        )
            .into_response();
    };
    if remote_id <= 0 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "ok": false, "message": "Webhook id must be a positive integer." })),
        )
            .into_response();
    }
    if state.provider_kind != "real" {
        return Json(json!({ "ok": false, "message": "Not available in demo mode." }))
            .into_response();
    }
    let Some(real) = state.real.clone() else {
        return Json(json!({ "ok": false, "message": "Not available in demo mode." }))
            .into_response();
    };
    match real.delete_webhook(remote_id).await {
        Ok(deleted) => {
            let conn = state.conn_lock();
            let _ = conn.execute(
                "DELETE FROM webhook_configs WHERE remote_id = ?1",
                rusqlite::params![remote_id],
            );
            let _ = crate::jobs::audit(
                &conn,
                "user",
                "webhook_deleted",
                None,
                None,
                None,
                Some(&format!("DELETE /v2/webhooks/{remote_id}")),
                None,
                false,
            );
            Json(json!({
                "ok": deleted,
                "message": if deleted {
                    format!("Webhook #{remote_id} deleted.")
                } else {
                    format!("Webhook #{remote_id} not found remotely.")
                },
            }))
            .into_response()
        }
        Err(e) => Json(json!({
            "ok": false,
            "message": format!("Help Scout rejected the webhook deletion: {e}"),
        }))
        .into_response(),
    }
}

pub async fn queue(
    State(state): State<AppState>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let limit = match q.get("limit").map(|s| s.trim()) {
        Some(s) if !s.is_empty() => match s.parse::<f64>() {
            Ok(n) if n.is_finite() => (n.trunc() as i64).clamp(1, 500),
            _ => 100,
        },
        _ => 100,
    };
    let conn = state.conn_lock();
    let jobs = crate::jobs::list_jobs(
        &conn,
        q.get("status").map(|s| s.as_str()),
        q.get("queue").map(|s| s.as_str()),
        limit,
    )
    .unwrap_or_default();
    let (queued, running, failed, completed) =
        crate::jobs::queue_stats(&conn).unwrap_or((0, 0, 0, 0));
    let outbound =
        crate::jobs::list_outbound_jobs(&conn, q.get("outbound_status").map(|s| s.as_str()), 50)
            .unwrap_or_default();
    Json(json!({
        "jobs": jobs,
        "stats": {
            "queued": queued,
            "running": running,
            "failed": failed,
            "completed": completed,
        },
        "outbound": outbound,
    }))
}

pub async fn retry_job(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return fastify_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ValidationError",
            "A positive numeric job id is required.",
        );
    };
    if id <= 0 {
        return fastify_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ValidationError",
            "A positive numeric job id is required.",
        );
    }
    let conn = state.conn_lock();
    let Some(job) = crate::jobs::get_job(&conn, id).unwrap_or(None) else {
        return fastify_error(StatusCode::NOT_FOUND, "NotFound", "Job not found.");
    };
    let patch = if job.status == "awaiting_approval" {
        Some(r#"{"approved":true}"#)
    } else {
        None
    };
    match crate::jobs::retry_job(&conn, id, patch) {
        Ok(Some(true)) => Json(json!({
            "ok": true,
            "message": if job.status == "awaiting_approval" {
                "Approved - the action runs on the next worker tick."
            } else {
                "Job requeued."
            },
        }))
        .into_response(),
        Ok(Some(false)) => fastify_error(
            StatusCode::CONFLICT,
            "Conflict",
            "Job is not in a retryable state.",
        )
        .into_response(),
        _ => fastify_error(StatusCode::NOT_FOUND, "NotFound", "Job not found.").into_response(),
    }
}

pub async fn cancel_job(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return fastify_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ValidationError",
            "A positive numeric job id is required.",
        );
    };
    if id <= 0 {
        return fastify_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ValidationError",
            "A positive numeric job id is required.",
        );
    }
    let conn = state.conn_lock();
    match crate::jobs::cancel_job(&conn, id) {
        Ok(true) => Json(json!({ "ok": true, "message": "Job cancelled." })).into_response(),
        _ => fastify_error(
            StatusCode::NOT_FOUND,
            "NotFound",
            "Job not found (or already finished).",
        ),
    }
}

pub async fn clear_completed(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let n = crate::jobs::clear_completed(&conn).unwrap_or(0);
    Json(json!({ "ok": true, "message": format!("{n} completed jobs older than 24h removed.") }))
}

pub async fn rebuild_search_index(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let _ = crate::jobs::enqueue_on(&conn, "maintenance", "rebuild_search_index", "{}", 4, 1);
    Json(json!({ "ok": true, "message": "Search index rebuild queued." }))
}

pub async fn rebuild_embeddings(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let _ = crate::jobs::enqueue_on(&conn, "maintenance", "rebuild_embeddings", "{}", 4, 1);
    Json(json!({
        "ok": true,
        "message": "Embedding rebuild queued (requires LM Studio embedding model).",
    }))
}

fn bundles_dir(state: &AppState) -> std::path::PathBuf {
    state.data_dir.join("bundles")
}

pub async fn encrypted_list(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let bundles = crate::encrypted_sync::list_bundles(&bundles_dir(&state));
    let log = crate::encrypted_sync::sync_log(&conn);
    (
        StatusCode::OK,
        Json(json!({
            "bundles": bundles,
            "log": log,
            "bundle_dir": bundles_dir(&state).to_string_lossy(),
            "design": "File-based end-to-end encrypted bundles. No relay server: SupportOS never sees your data in transit - move the .sosync file yourself (cloud drive, USB, company share). Only the passphrase holder can decrypt it."
        })),
    )
}

pub async fn encrypted_export(
    State(state): State<AppState>,
    body: Option<Json<Value>>,
) -> impl IntoResponse {
    let passphrase = body
        .and_then(|Json(v)| {
            v.get("passphrase")
                .and_then(|p| p.as_str())
                .map(String::from)
        })
        .unwrap_or_default();
    let conn = state.conn_lock();
    let result = crate::encrypted_sync::export_bundle(&conn, &bundles_dir(&state), &passphrase);
    if result.ok {
        let _ = crate::audit::audit(
            &conn,
            &crate::audit::AuditEntry::user("encrypted_sync_export").with_after_state(json!({
                "path": result.path.clone().unwrap_or_default(),
                "size": result.size_bytes.unwrap_or(0),
            })),
        );
    }
    (
        StatusCode::OK,
        Json(serde_json::to_value(result).unwrap_or(Value::Null)),
    )
}

pub async fn encrypted_verify(
    State(state): State<AppState>,
    body: Option<Json<Value>>,
) -> impl IntoResponse {
    let Json(v) = body.unwrap_or(Json(Value::Null));
    let path = v.get("path").and_then(|p| p.as_str()).unwrap_or_default();
    if path.is_empty() {
        return (
            StatusCode::OK,
            Json(json!({ "ok": false, "message": "A bundle path is required." })),
        );
    }
    let passphrase = v
        .get("passphrase")
        .and_then(|p| p.as_str())
        .unwrap_or_default();
    let conn = state.conn_lock();
    let result = crate::encrypted_sync::verify_bundle(
        &conn,
        &bundles_dir(&state),
        std::path::Path::new(path),
        passphrase,
    );
    (StatusCode::OK, Json(result))
}

pub async fn encrypted_import(
    State(state): State<AppState>,
    body: Option<Json<Value>>,
) -> impl IntoResponse {
    let Json(v) = body.unwrap_or(Json(Value::Null));
    let path = v.get("path").and_then(|p| p.as_str()).unwrap_or_default();
    if path.is_empty() {
        return (
            StatusCode::OK,
            Json(json!({ "ok": false, "message": "A bundle path is required." })),
        );
    }
    let passphrase = v
        .get("passphrase")
        .and_then(|p| p.as_str())
        .unwrap_or_default();
    let conn = state.conn_lock();
    let db_path = state.data_dir.join("supportos-plusplus.db");
    let result = crate::encrypted_sync::import_bundle(
        &conn,
        &db_path,
        &bundles_dir(&state),
        std::path::Path::new(path),
        passphrase,
    );
    if result.ok {
        let _ = crate::audit::audit(
            &conn,
            &crate::audit::AuditEntry::user("encrypted_sync_import")
                .with_after_state(json!({ "path": path })),
        );
    }
    (
        StatusCode::OK,
        Json(serde_json::to_value(result).unwrap_or(Value::Null)),
    )
}

pub async fn encrypted_upload(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    if body.len() < 32 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({ "ok": false, "message": "Upload a .sosync bundle as the raw request body (application/octet-stream)." }),
            ),
        );
    }
    if &body[0..6] != b"SOSYNC" {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({ "ok": false, "message": "This is not a SupportOS encrypted sync bundle (.sosync files start with SOSYNC)." }),
            ),
        );
    }
    let _ = headers;
    let dir = state.data_dir.join("bundles");
    if std::fs::create_dir_all(&dir).is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "message": "Could not create the bundles directory." })),
        );
    }
    let name = format!("uploaded-{}.sosync", chrono::Utc::now().timestamp_millis());
    let target = dir.join(&name);
    if std::fs::write(&target, &body).is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "message": "Could not write the uploaded bundle." })),
        );
    }
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "path": target.to_string_lossy(),
            "message": "Bundle uploaded. Now import it with your passphrase.",
        })),
    )
}
