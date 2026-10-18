use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde_json::{Value, json};

use super::super::server::AppState;
use axum::response::IntoResponse;

pub const APP_VERSION: &str = "2.2.1";

fn js_iso_now() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

fn zod_type_name(v: &Value) -> &'static str {
    match v {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        Value::Null => "null",
    }
}

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute_batch("SELECT 1").is_ok()
    };
    let status = if db_ok { "ok" } else { "error" };
    let code = if db_ok {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    };
    (
        code,
        Json(json!({
            "status": status,
            "database": db_ok,
            "version": APP_VERSION,
            "time": js_iso_now(),
        })),
    )
}

pub async fn health_detailed(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let ui_format = params.get("format").map(String::as_str) == Some("ui");
    let (db_ok, lmstudio_base, lmstudio_embedding) = {
        let conn = state.conn_lock();
        let db_ok = conn.execute_batch("SELECT 1").is_ok();

        let lmstudio_base = crate::settings::get_string(&conn, "lmstudio_base_url")
            .ok()
            .flatten()
            .unwrap_or_else(|| "http://127.0.0.1:1234".to_string());
        let lmstudio_embedding: Option<String> =
            crate::settings::get_string(&conn, "ai_embedding_model")
                .ok()
                .flatten();
        (db_ok, lmstudio_base, lmstudio_embedding)
    };
    let helpscout_connected = if state.demo_mode {
        true
    } else {
        let cached: Option<String> = {
            let conn = state.conn_lock();
            conn.query_row(
                "SELECT value FROM application_settings WHERE key='hs_last_ping'",
                [],
                |r| r.get(0),
            )
            .ok()
        };
        let parsed = cached
            .as_deref()
            .and_then(|v| serde_json::from_str::<Value>(v).ok());
        let fresh = parsed.as_ref().is_some_and(|p| {
            p.get("at").and_then(Value::as_i64).is_some_and(|at| {
                let now_ms = chrono::Utc::now().timestamp_millis();
                now_ms.saturating_sub(at) < 60_000
            })
        });
        if fresh {
            parsed
                .as_ref()
                .and_then(|p| p.get("connected").and_then(Value::as_bool))
                .unwrap_or(false)
        } else {
            let provider = state.sync.as_ref().map(|s| s.provider().clone());
            let connected = match provider {
                Some(p) => p.ping().await.is_ok(),
                None => false,
            };
            let entry = json!({
                "connected": connected,
                "error": if connected { Value::Null } else { json!("Help Scout is unreachable") },
                "at": chrono::Utc::now().timestamp_millis(),
            });
            let conn = state.conn_lock();
            let _ = conn.execute(
                "INSERT OR REPLACE INTO application_settings (key, value, updated_at)
                 VALUES ('hs_last_ping', ?1, datetime('now'))",
                rusqlite::params![entry.to_string()],
            );
            connected
        }
    };
    let lm_probe =
        crate::ai_lm_studio::OpenAiCompatibleClient::new_with_timeout(&lmstudio_base, 5_000)
            .list_models()
            .await;
    let (lm_connected, lm_models, lm_error) = match lm_probe {
        Ok(models) => (
            true,
            models.into_iter().map(|m| m.id).collect::<Vec<String>>(),
            Value::Null,
        ),
        Err(_) => (
            false,
            Vec::new(),
            json!(format!(
                "LM Studio is not reachable at {lmstudio_base}. Start LM Studio, load a model, and enable the local server (Developer tab > Start Server)."
            )),
        ),
    };
    let last_inference: Option<String> = {
        let conn = state.conn_lock();
        conn.query_row("SELECT MAX(created_at) FROM ai_runs", [], |r| {
            r.get::<_, Option<String>>(0)
        })
        .ok()
        .flatten()
    };

    let qdrant_indexed = {
        let conn = state.conn_lock();
        let conv_indexed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM conversation_threads WHERE embedding_state = 'indexed'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let chunks_indexed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_chunks WHERE embedding_state = 'indexed'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let chunks_pending: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_chunks WHERE embedding_state IN ('not_indexed','queued')",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let chunks_failed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge_chunks WHERE embedding_state = 'failed'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        json!({
            "conversations_indexed": conv_indexed,
            "chunks_indexed": chunks_indexed,
            "chunks_pending": chunks_pending,
            "chunks_failed": chunks_failed,
        })
    };
    let qdrant = state.qdrant.health();

    let (
        sync_state,
        last_success,
        queued_jobs,
        failed_jobs,
        db_path,
        size_bytes,
        migrations_applied,
    ) = {
        let conn = state.conn_lock();
        (
            crate::settings::get_string(&conn, "sync_state")
                .ok()
                .flatten()
                .and_then(|v| serde_json::from_str::<Value>(&v).ok())
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| "NEW".to_string()),
            conn.query_row(
                "SELECT MAX(finished_at) FROM sync_runs WHERE status = 'success'",
                [],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten(),
            conn.query_row(
                "SELECT COUNT(*) FROM jobs WHERE status = 'queued'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0),
            conn.query_row(
                "SELECT COUNT(*) FROM jobs WHERE status = 'failed'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0),
            state
                .data_dir
                .join("supportos-plusplus.db")
                .display()
                .to_string(),
            std::fs::metadata(state.data_dir.join("supportos-plusplus.db"))
                .map(|m| m.len())
                .unwrap_or(0),
            conn.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _migrations",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0),
        )
    };

    let status = if db_ok {
        if helpscout_connected || state.demo_mode {
            "ok"
        } else {
            "degraded"
        }
    } else {
        "error"
    };
    let code = if status == "error" && !ui_format {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    } else {
        axum::http::StatusCode::OK
    };

    (
        code,
        Json(json!({
            "status": status,
            "version": APP_VERSION,
            "time": js_iso_now(),
            "database": {
                "ok": db_ok,
                "path": db_path,
                "size_bytes": size_bytes,
                "migrations_applied": migrations_applied,
                "wal": true,
            },
            "helpscout": {
                "connected": helpscout_connected,
                "demo_mode": state.demo_mode,
                "error": if helpscout_connected { Value::Null } else { json!("Not checked") },
                "oauth": {
                    "configured": !state.demo_mode,
                    "authenticated": state.demo_mode,
                    "demoMode": state.demo_mode,
                    "expiresAt": Value::Null,
                },
            },
            "lmstudio": {
                "connected": lm_connected,
                "base_url": lmstudio_base,
                "models": lm_models,
                "embedding_model": lmstudio_embedding,
                "last_inference": last_inference,
                "error": lm_error,
            },
            "qdrant": {
                "connected": qdrant.connected,
                "url": qdrant.url,
                "collections": qdrant.collections,
                "indexed": qdrant_indexed,
                "error": qdrant.error,
            },
            "sync": {
                "state": sync_state,
                "last_success": last_success,
                "queued_jobs": queued_jobs,
                "failed_jobs": failed_jobs,
            },
            "workers": {
                "running": state
                    .workers
                    .as_ref()
                    .is_some_and(|w| w.is_running()),
                "queue_depth": queued_jobs,
            },
        })),
    )
}

pub async fn db_stats(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let db_path = state.data_dir.join("supportos-plusplus.db");
    let size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
    let mut tables: Vec<Value> = Vec::new();
    if let Ok(mut stmt) =
        conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
    {
        let names: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .ok()
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default();
        for name in names {
            let rows: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM \"{name}\""), [], |r| {
                    r.get(0)
                })
                .unwrap_or(0);
            tables.push(json!({ "table": name, "rows": rows }));
        }
    }
    let migrations: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _migrations",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    (
        StatusCode::OK,
        Json(json!({
            "path": db_path.display().to_string(),
            "size_bytes": size,
            "migrations": migrations,
            "tables": tables,
        })),
    )
}

pub async fn capabilities() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "matrix": [],
            "summary": {
                "implemented": 0,
                "total": 0,
                "tested": 0,
            }
        })),
    )
}

pub async fn table_stats(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let tables: Vec<Value> =
        match conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name") {
            Ok(mut stmt) => match stmt.query_map([], |r| r.get::<_, String>(0)) {
                Ok(rows) => rows
                    .filter_map(|r| r.ok())
                    .map(|name| {
                        let count: i64 = conn
                            .query_row(&format!("SELECT COUNT(*) FROM \"{name}\""), [], |r| {
                                r.get(0)
                            })
                            .unwrap_or(0);
                        json!({"name": name, "rows": count})
                    })
                    .collect(),
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        };

    (StatusCode::OK, Json(json!({"tables": tables})))
}

pub async fn onboarding(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let first_run = crate::settings::first_run_done(&conn).unwrap_or(false);
    let conv_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
        .unwrap_or(0);
    let hs_configured: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM oauth_tokens WHERE access_token IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let sync_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sync_runs", [], |r| r.get(0))
        .unwrap_or(0);
    let webhook_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM webhook_events", [], |r| r.get(0))
        .unwrap_or(0);
    let sync_state = if webhook_count > 0 {
        "receiving"
    } else if sync_count > 0 {
        "registered"
    } else {
        "new"
    };
    (
        StatusCode::OK,
        Json(json!({
            "step": if first_run { "complete" } else { "welcome" },
            "completed": first_run,
            "demo_mode": state.demo_mode,
            "conversations": conv_count,
            "hs_configured": hs_configured > 0,
            "hs_authenticated": hs_configured > 0,
            "sync_state": sync_state,
        })),
    )
}

pub async fn onboarding_step(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let step = body
        .get("step")
        .and_then(|v| v.as_str())
        .unwrap_or("welcome");
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let _ = crate::settings::set_string(&conn, "onboarding_step", step);
    (StatusCode::OK, Json(json!({"ok": true})))
}

pub async fn onboarding_complete(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let _ = crate::settings::mark_first_run_done(&conn);
    (
        StatusCode::OK,
        Json(json!({"ok": true, "message": "Onboarding complete."})),
    )
}

pub async fn demo_enable(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let _ = crate::settings::set_bool(&conn, "demo_mode", true);
    let _ = crate::settings::mark_first_run_done(&conn);
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "message": "Demo mode active with a simulated Help Scout account. Run the initial sync from Sync Health to populate the demo database."
        })),
    )
}

pub async fn demo_simulate_incoming(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    if !state.demo_mode {
        return (
            StatusCode::OK,
            Json(json!({"ok": false, "message": "Not in demo mode."})),
        );
    }
    let subject = match body.get("subject") {
        None | Some(Value::Null) => "New question about exports".to_string(),
        Some(Value::String(s)) => {
            if s.is_empty() || s.chars().count() > 500 {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422, "error": "ValidationError",
                        "message": "subject must be a string of 1-500 characters."
                    })),
                );
            }
            s.clone()
        }
        Some(v) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422, "error": "ValidationError",
                    "message": format!("Expected string, received {}", zod_type_name(v))
                })),
            );
        }
    };
    let text = match body.get("body") {
        None | Some(Value::Null) => {
            "Hello, can scheduled exports include the raw JSON fields in addition to CSV?"
                .to_string()
        }
        Some(Value::String(s)) if s.chars().count() <= 50_000 => s.clone(),
        Some(_) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422, "error": "ValidationError",
                    "message": "body must be a string of at most 50000 characters."
                })),
            );
        }
    };
    let positive_int = |v: Option<&Value>| -> Result<Option<i64>, (StatusCode, Value)> {
        match v {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(n)) => {
                let i = n.as_i64().filter(|i| *i > 0);
                if i.is_some() {
                    Ok(i)
                } else {
                    Err((
                        StatusCode::UNPROCESSABLE_ENTITY,
                        json!({
                            "statusCode": 422, "error": "ValidationError",
                            "message": "Expected positive integer, received number"
                        }),
                    ))
                }
            }
            Some(other) => Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                json!({
                    "statusCode": 422, "error": "ValidationError",
                    "message": format!("Expected number, received {}", zod_type_name(other))
                }),
            )),
        }
    };
    let mailbox_id = match positive_int(body.get("mailboxId")) {
        Ok(v) => v.unwrap_or(201),
        Err(r) => return (r.0, Json(r.1)),
    };
    let customer_id = match positive_int(body.get("customerRemoteId")) {
        Ok(v) => v.unwrap_or(3003),
        Err(r) => return (r.0, Json(r.1)),
    };

    let provider = match state.sync.as_ref() {
        Some(sync) => sync.provider().clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"ok": false, "message": "No provider available."})),
            );
        }
    };
    let created = match provider
        .create_conversation(crate::helpscout::CreateConversationInput {
            mailbox_id,
            customer_id,
            subject: subject.clone(),
            body: text.clone(),
            tags: Vec::new(),
            status: None,
        })
        .await
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    json!({"ok": false, "message": format!("Failed to create conversation: {e}")}),
                ),
            );
        }
    };
    {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        let _ = crate::jobs::enqueue_on(
            &conn,
            "sync",
            "sync_conversation",
            &serde_json::json!({ "remoteId": created.conversation_id }).to_string(),
            2,
            2,
        );
    }
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "message": format!(
                "Simulated incoming conversation #{}. It will appear after the next sync tick (a few seconds).",
                created.number
            )
        })),
    )
}

pub async fn demo_simulate_rating(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    if !state.demo_mode {
        return (
            StatusCode::OK,
            Json(json!({"ok": false, "message": "Not in demo mode."})),
        );
    }
    let conversation_remote_id = match body.get("conversationRemoteId") {
        Some(Value::Number(n)) => match n.as_i64() {
            Some(id) if id > 0 => Some(id),
            _ => None,
        },
        _ => None,
    };
    let Some(conversation_remote_id) = conversation_remote_id else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422, "error": "ValidationError",
                "message": "conversationRemoteId (remote Help Scout id) is required."
            })),
        );
    };
    const RATINGS: [&str; 3] = ["great", "okay", "not-good"];
    let rating = match body.get("rating") {
        None | Some(Value::Null) => "great".to_string(),
        Some(Value::String(s)) => {
            if !RATINGS.contains(&s.as_str()) {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422, "error": "ValidationError",
                        "message": "rating must be 'great', 'okay' or 'not-good'."
                    })),
                );
            }
            s.clone()
        }
        Some(v) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422, "error": "ValidationError",
                    "message": format!("Expected 'great' | 'okay' | 'not-good', received {}", zod_type_name(v))
                })),
            );
        }
    };
    let comments = body
        .get("comments")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let provider = match state.sync.as_ref() {
        Some(sync) => sync.provider().clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"ok": false, "message": "No provider available."})),
            );
        }
    };
    let Some(hs) = provider.submit_rating(conversation_remote_id, &rating, comments.as_deref())
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404, "error": "NotFound",
                "message": "Conversation not found in the simulated account."
            })),
        );
    };
    let (conv_local, conv_number, customer_local) = {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        match crate::sync_engine::persist_rating(&conn, &hs) {
            Ok((inserted, conv_local, conv_number, customer_local)) => {
                debug_assert!(inserted, "a fresh world rating is always a new row");
                (conv_local, conv_number, customer_local)
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"ok": false, "message": format!("Failed to persist rating: {e}")})),
                );
            }
        }
    };
    crate::http::event_bus::notify_rating_received(
        &state.bus,
        hs.rating.as_deref().or(Some("great")),
        conv_local,
        conv_number,
        customer_local,
        hs.customer_name.as_deref(),
        hs.comment.as_deref(),
    );
    crate::http::event_bus::notify_ratings_refreshed(&state.bus, 1, 1);
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "message": format!(
                "Simulated a {} rating on conversation #{}. Connected dashboards update instantly via /api/events.",
                hs.rating.as_deref().unwrap_or("(no)"),
                conv_number.unwrap_or(conversation_remote_id)
            )
        })),
    )
}

pub async fn demo_simulate_webhook(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    if !state.demo_mode {
        return (
            StatusCode::OK,
            Json(json!({"ok": false, "message": "Not in demo mode."})),
        );
    }

    const EVENTS: [&str; 4] = [
        "convo.created",
        "convo.customer.reply.created",
        "convo.agent.reply.created",
        "convo.note.created",
    ];
    let event = match body.get("event") {
        Some(Value::String(s)) if EVENTS.contains(&s.as_str()) => s.clone(),
        _ => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "ok": false,
                    "message": "Invalid simulation request: event must be one of the supported convo.* types."
                })),
            );
        }
    };
    let requested_remote = match body.get("conversationRemoteId") {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => n.as_i64(),
        Some(_) => None,
    };
    let reply_text = match body.get("replyText") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => {
            if s.chars().count() > 4_000 {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "ok": false,
                        "message": "Invalid simulation request: replyText must be at most 4000 characters."
                    })),
                );
            }
            Some(s.clone())
        }
        Some(_) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "ok": false,
                    "message": "Invalid simulation request: replyText must be a string."
                })),
            );
        }
    };

    let provider = match state.sync.as_ref() {
        Some(sync) => sync.provider().clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"ok": false, "message": "No provider available."})),
            );
        }
    };

    let remote_id: i64;
    if event == "convo.created" {
        let conv = match provider
            .create_conversation(crate::helpscout::CreateConversationInput {
                mailbox_id: 201,
                customer_id: 3004,
                subject: "Webhook push: SSO callback rejected".into(),
                body: reply_text.clone().unwrap_or_else(|| {
                    "Our identity provider logs show a rejected SAML assertion right after the 2.5 rollout. SSO logins fail for about half our users. Is this a known issue?".into()
                }),
                tags: vec!["sso".into()],
                status: None,
            })
            .await
        {
            Ok(c) => c,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"ok": false, "message": format!("Failed to create conversation: {e}")})),
                )
            }
        };
        remote_id = conv.conversation_id;
    } else {
        let fallback: Option<i64> = {
            let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
            conn.query_row(
                "SELECT remote_id FROM conversations
                  WHERE deleted_at IS NULL AND status = 'active'
                  ORDER BY id LIMIT 1",
                [],
                |r| r.get(0),
            )
            .ok()
        };
        let Some(id) = requested_remote.or(fallback) else {
            return (
                StatusCode::NOT_FOUND,
                Json(
                    json!({"ok": false, "message": "No conversation available to push an event for."}),
                ),
            );
        };
        remote_id = id;
        if event == "convo.customer.reply.created" || event == "convo.agent.reply.created" {
            provider.customer_replies(
                remote_id,
                &reply_text.clone().unwrap_or_else(|| {
                    "Any update on this? Our team is blocked until SSO works again.".into()
                }),
            );
        }
    }

    let envelope = serde_json::json!({
        "conversationId": remote_id,
        "objectID": remote_id,
        "id": remote_id,
        "nonce": chrono::Utc::now().timestamp_millis(),
    });
    let envelope_bytes = envelope.to_string().into_bytes();

    let secret = {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        crate::settings::get_secret(&conn, "webhook_secret")
            .ok()
            .flatten()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "demo_webhook_secret".into())
    };
    let signature = crate::webhook::compute_signature(secret.as_bytes(), &envelope_bytes);

    let result = crate::webhook_handler::process_webhook(
        &state.conn.lock().unwrap_or_else(|p| p.into_inner()),
        secret.as_bytes(),
        &envelope_bytes,
        Some(&signature),
        &event,
        // Demo simulator route: the secret is always set (demo default).
        true,
    );

    let event_id = format!("demo_wh_{}", chrono::Utc::now().timestamp_millis());
    match result {
        crate::webhook_handler::WebhookProcessResult::Accepted { .. } => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "message": format!(
                    "{event} pushed through the real webhook pipeline. The sync job runs on the next worker tick (~2s); connected clients get an SSE conversation-updated event."
                ),
                "remoteId": remote_id,
                "event_id": event_id,
            })),
        ),
        crate::webhook_handler::WebhookProcessResult::Duplicate { .. } => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "message": format!("{event} was a duplicate (already processed)."),
                "remoteId": remote_id,
                "event_id": event_id,
                "duplicate": true,
            })),
        ),
        crate::webhook_handler::WebhookProcessResult::SignatureInvalid => (
            StatusCode::OK,
            Json(json!({
                "ok": false,
                "message": "Signature verification failed for simulated webhook event.",
                "event_id": event_id,
            })),
        ),
        crate::webhook_handler::WebhookProcessResult::BadRequest => (
            StatusCode::OK,
            Json(json!({
                "ok": false,
                "message": "Simulated webhook payload was rejected.",
                "event_id": event_id,
            })),
        ),
        crate::webhook_handler::WebhookProcessResult::SecretNotConfigured => (
            StatusCode::OK,
            Json(json!({
                "ok": false,
                "message": "No webhook secret is configured; the simulation was not delivered.",
                "event_id": event_id,
            })),
        ),
    }
}

pub async fn audit_log(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let conn = state.conn_lock();
    let conv_id = params
        .get("conversationId")
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<i64>().ok());

    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(200)
        .clamp(1, 1000);
    match crate::audit::list_audit(&conn, conv_id, limit) {
        Ok(entries) => (StatusCode::OK, Json(json!({ "entries": entries }))),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                json!({ "statusCode": 500, "error": "InternalServerError", "message": "Failed to read the audit log." }),
            ),
        ),
    }
}

pub async fn errors(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    match crate::audit::list_recent_errors(&conn, 50) {
        Ok(list) => (StatusCode::OK, Json(json!({ "errors": list }))),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                json!({ "statusCode": 500, "error": "InternalServerError", "message": "Failed to read application errors." }),
            ),
        ),
    }
}

fn backups_dir(state: &AppState) -> std::path::PathBuf {
    state.data_dir.join("backups")
}

pub async fn backups_list(State(state): State<AppState>) -> impl IntoResponse {
    let list = crate::backup_service::list_backups(&backups_dir(&state));
    (
        StatusCode::OK,
        Json(json!({ "backups": list, "exports": [] })),
    )
}

pub async fn backups_create(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let result = crate::backup_service::backup(&conn, &backups_dir(&state));
    if result.ok {
        let path = result.path.clone().unwrap_or_default();
        let _ = crate::audit::audit(
            &conn,
            &crate::audit::AuditEntry::user("backup_created")
                .with_after_state(json!({ "path": path })),
        );
    }
    (
        StatusCode::OK,
        Json(serde_json::to_value(result).unwrap_or(Value::Null)),
    )
}

pub async fn backups_export_json(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let result = crate::backup_service::export_json(&conn, &backups_dir(&state));
    (StatusCode::OK, Json(result))
}

pub async fn backups_export_csv(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let result = crate::backup_service::export_conversations_csv(&conn, &backups_dir(&state));
    (StatusCode::OK, Json(result))
}

pub async fn attachment_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let attachment_id = match id.parse::<i64>() {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "statusCode": 400, "error": "BadRequest",
                    "message": "Invalid attachment id."
                })),
            )
                .into_response();
        }
    };
    let att = {
        let conn = state.conn_lock();
        crate::conversation_ops::attachment_by_id(&conn, attachment_id)
    };
    let Some(att) = att else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404, "error": "NotFound",
                "message": "Attachment not downloaded yet. Use the download action first."
            })),
        )
            .into_response();
    };
    let Some(local_path) = att.local_path else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404, "error": "NotFound",
                "message": "Attachment not downloaded yet. Use the download action first."
            })),
        )
            .into_response();
    };
    let bytes = match std::fs::read(&local_path) {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "statusCode": 404, "error": "NotFound",
                    "message": "Attachment file is no longer readable on disk."
                })),
            )
                .into_response();
        }
    };
    let root = state.data_dir.join("attachments");
    let resolved = std::path::Path::new(&local_path)
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from(&local_path));
    let root = root
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from(&root));
    if !resolved.starts_with(&root) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "statusCode": 400, "error": "BadRequest",
                "message": "Invalid attachment path."
            })),
        )
            .into_response();
    }
    let mime = att
        .mime_type
        .unwrap_or_else(|| "application/octet-stream".into());
    let inline = regex_lite_check_image(&mime);
    let safe_filename = att
        .filename
        .unwrap_or_else(|| "attachment".into())
        .replace(['"', '\\', '\r', '\n'], "_");
    let mut headers = axum::http::HeaderMap::new();
    let content_type = if inline {
        mime
    } else {
        "application/octet-stream".into()
    };
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_str(&content_type).unwrap_or(
            axum::http::HeaderValue::from_static("application/octet-stream"),
        ),
    );
    if let Ok(v) =
        axum::http::HeaderValue::from_str(&format!("attachment; filename=\"{safe_filename}\""))
    {
        headers.insert(axum::http::header::CONTENT_DISPOSITION, v);
    }
    headers.insert(
        "x-content-type-options",
        axum::http::HeaderValue::from_static("nosniff"),
    );
    (StatusCode::OK, headers, bytes).into_response()
}

fn regex_lite_check_image(mime: &str) -> bool {
    let m = mime.to_ascii_lowercase();
    let Some(rest) = m.strip_prefix("image/") else {
        return false;
    };
    matches!(
        rest,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg+xml"
    )
}
