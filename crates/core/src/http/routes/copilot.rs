use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use super::super::server::AppState;
use crate::ai_pipeline;

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

pub async fn list_sessions(
    State(state): State<AppState>,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
) -> Json<Value> {
    let limit = query
        .as_deref()
        .and_then(|q| q.split('&').find_map(|p| p.strip_prefix("limit=")))
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50)
        .clamp(1, 200);
    let conn = state.conn_lock();
    crate::copilot::ensure_copilot_schema(&conn).ok();
    Json(json!({ "sessions": crate::copilot::list_sessions(&conn, limit) }))
}

pub async fn get_session(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return validation_422("Session id must be a positive integer.");
    };
    if id <= 0 {
        return validation_422("Session id must be a positive integer.");
    }
    let conn = state.conn_lock();
    crate::copilot::ensure_copilot_schema(&conn).ok();
    let Some(session) = crate::copilot::list_sessions(&conn, 200)
        .into_iter()
        .find(|s| s["id"] == json!(id))
    else {
        return not_found_404("Copilot session not found.");
    };
    Json(json!({
        "session": session,
        "messages": crate::copilot::list_messages(&conn, id),
    }))
    .into_response()
}

pub async fn chat(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let question = match body.get("question").and_then(Value::as_str) {
        Some(q) if !q.is_empty() && q.chars().count() <= 4000 => q.to_string(),
        _ => {
            return validation_422(
                "question must be a non-empty string of at most 4000 characters.",
            )
        }
    };
    let conversation_id = body.get("conversationId").and_then(Value::as_i64);
    let session_id = body.get("sessionId").and_then(Value::as_i64);
    {
        let conn = state.conn_lock();
        crate::copilot::ensure_copilot_schema(&conn).ok();
        if !crate::settings::get_bool(&conn, "ai_enabled", true).unwrap_or(true) {
            return service_503(
                "AI is disabled in Settings. Enable LM Studio to use the Local Copilot.",
            );
        }
        if let Some(cid) = conversation_id {
            let exists = conn
                .query_row(
                    "SELECT 1 FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
                    rusqlite::params![cid],
                    |_| Ok(()),
                )
                .is_ok();
            if !exists {
                return not_found_404("Conversation not found.");
            }
        }
        if let Some(sid) = session_id {
            let known = crate::copilot::list_sessions(&conn, 200)
                .into_iter()
                .any(|s| s["id"] == json!(sid));
            if !known {
                return not_found_404("Copilot session not found.");
            }
        }
    }
    let result = state
        .run_ai(move |conn| {
            Box::pin(async move {
                let backend = ai_pipeline::backend_from_settings(conn);
                crate::copilot::chat(conn, &backend, &question, conversation_id, session_id).await
            })
        })
        .await;
    match result {
        Ok(Ok(mut payload)) => {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("ok".into(), json!(true));
            }
            (StatusCode::OK, Json(payload)).into_response()
        }
        Ok(Err(e)) => service_503(&e.message),
        Err(join) => service_503(&join),
    }
}

pub async fn delete_session(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return validation_422("Session id must be a positive integer.");
    };
    if id <= 0 {
        return validation_422("Session id must be a positive integer.");
    }
    let conn = state.conn_lock();
    crate::copilot::ensure_copilot_schema(&conn).ok();
    if crate::copilot::delete_session(&conn, id) {
        Json(json!({ "ok": true, "message": "Copilot session deleted." })).into_response()
    } else {
        not_found_404("Copilot session not found.")
    }
}

pub async fn starter_questions(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return validation_422("Conversation id must be a positive integer.");
    };
    if id <= 0 {
        return validation_422("Conversation id must be a positive integer.");
    }
    let conn = state.conn_lock();
    let questions = crate::copilot::starter_questions(&conn, id);
    if questions.is_empty() {
        return not_found_404("Conversation not found.");
    }
    Json(json!({
        "questions": questions
            .iter()
            .map(|(q, why)| json!({ "question": q, "why": why }))
            .collect::<Vec<_>>()
    }))
    .into_response()
}

pub async fn tools(State(_state): State<AppState>) -> Json<Value> {
    let tools: Vec<Value> = crate::ai_tools::definitions()
        .iter()
        .map(|t| json!({ "name": t.name, "description": t.description }))
        .collect();
    Json(json!({
        "tools": tools,
        "note": "The Copilot can only call these allowlisted read-only tools. No SQL is ever exposed to the model."
    }))
}
