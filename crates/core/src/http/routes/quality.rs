use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

use super::super::server::AppState;

fn positive_int_422(field: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "statusCode": 422,
            "error": "ValidationError",
            "message": format!("{field} must be a positive integer.")
        })),
    )
        .into_response()
}

fn path_positive_int(raw: &str, field: &str) -> Result<i64, Response> {
    match crate::conversation_ops::js_number(raw) {
        Some(v) if v.fract() == 0.0 && v > 0.0 && v <= i64::MAX as f64 => Ok(v as i64),
        _ => Err(positive_int_422(field)),
    }
}

fn not_found(message: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "statusCode": 404,
            "error": "NotFound",
            "message": message
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
            "message": message
        })),
    )
        .into_response()
}

pub(crate) fn clamp_days_param(raw: Option<&String>, fallback: i64, min: i64, max: i64) -> i64 {
    let Some(s) = raw.filter(|s| !s.is_empty()) else {
        return fallback;
    };
    match crate::conversation_ops::js_number(s) {
        Some(n) if n.is_finite() => (n.trunc() as i64).clamp(min, max),
        _ => fallback,
    }
}

pub async fn list_gaps(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    Json(crate::quality::gap_report(&conn))
}

pub async fn rebuild_gaps(State(state): State<AppState>, body: Option<Json<Value>>) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let days: i64 = match body.get("days") {
        None | Some(Value::Null) => 90,
        Some(v) => {
            let Some(n) = v.as_i64() else {
                return crate::conversation_ops::zod_422(
                    "days",
                    "Expected number, received non-number",
                );
            };
            if !(1..=3650).contains(&n) {
                return crate::conversation_ops::zod_422(
                    "days",
                    "Number must be greater than or equal to 1 and less than or equal to 3650",
                );
            }
            n
        }
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let (candidates, new) = crate::quality::rebuild_gaps(&conn, days);
    Json(json!({ "candidates": candidates, "new": new })).into_response()
}

pub async fn draft_candidate(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Ok(id) = path_positive_int(&id, "Candidate id") else {
        return positive_int_422("Candidate id");
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::quality::draft_gap(&conn, id) {
        Some(draft) => (StatusCode::OK, Json(draft)).into_response(),
        None => not_found("Knowledge candidate not found."),
    }
}

pub async fn decide_candidate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let Ok(id) = path_positive_int(&id, "Candidate id") else {
        return positive_int_422("Candidate id");
    };
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let decision = match body.get("decision") {
        Some(Value::String(s)) if s == "approved" || s == "rejected" => s.clone(),
        Some(Value::String(s)) => {
            return crate::conversation_ops::zod_422(
                "decision",
                &crate::conversation_ops::zod_enum_message(&["approved", "rejected"], s),
            );
        }
        Some(_) => {
            return crate::conversation_ops::zod_422(
                "decision",
                "Expected string, received non-string",
            );
        }
        None => return crate::conversation_ops::zod_422("decision", "Required"),
    };
    let note: Option<String> = match body.get("note") {
        None | Some(Value::Null) => None,
        Some(Value::String(n)) => {
            if n.chars().count() > 500 {
                return crate::conversation_ops::zod_422(
                    "note",
                    "String must contain at most 500 character(s)",
                );
            }
            Some(n.clone())
        }
        Some(_) => {
            return crate::conversation_ops::zod_422(
                "note",
                "Expected string, received non-string",
            );
        }
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::quality::decide_gap(&conn, id, &decision, note.as_deref(), None) {
        Some(candidate) => (
            StatusCode::OK,
            Json(json!({ "ok": true, "candidate": candidate })),
        )
            .into_response(),
        None => (
            StatusCode::CONFLICT,
            Json(json!({
                "statusCode": 409,
                "error": "Conflict",
                "message": "Candidate not found or already decided. Rebuild does not reset human decisions."
            })),
        )
            .into_response(),
    }
}

pub async fn qa_overview(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    Json(crate::quality::qa_overview(&conn))
}

pub async fn qa_rebuild(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let conversations = crate::quality::rebuild_qa(&conn);
    Json(json!({ "conversations": conversations }))
}

fn conversation_exists(conn: &rusqlite::Connection, id: i64) -> bool {
    conn.query_row(
        "SELECT 1 FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
        rusqlite::params![id],
        |_| Ok(()),
    )
    .is_ok()
}

pub async fn qa_conversation(
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
) -> Response {
    let Ok(id) = path_positive_int(&conversation_id, "Conversation id") else {
        return positive_int_422("Conversation id");
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    if !conversation_exists(&conn, id) {
        return not_found("Conversation not found.");
    }
    let ai_available = crate::settings::get_bool(&conn, "ai_enabled", true).unwrap_or(true);
    let qa = crate::quality::get_qa(&conn, id, ai_available);
    let friction = crate::quality::analyze_friction(&conn, id);
    Json(json!({ "qa": qa, "friction": friction })).into_response()
}

pub async fn qa_analyze(
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let Ok(id) = path_positive_int(&conversation_id, "Conversation id") else {
        return positive_int_422("Conversation id");
    };
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let include_ai = match body.get("includeAi") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            return crate::conversation_ops::zod_422(
                "includeAi",
                "Expected boolean, received non-boolean",
            );
        }
    };
    {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        if !conversation_exists(&conn, id) {
            return not_found("Conversation not found.");
        }
        if include_ai {
            let ai_enabled = crate::settings::get_bool(&conn, "ai_enabled", true).unwrap_or(true);
            if !ai_enabled {
                return service_503(
                    "AI is disabled in Settings. The deterministic QA layer works without it; enable LM Studio for the optional AI layer.",
                );
            }
        }
    }
    if include_ai {
        let result = state
            .run_ai(move |conn| {
                Box::pin(async move {
                    crate::quality::ensure_quality_tables(conn).ok();
                    let backend = crate::ai_pipeline::backend_from_settings(conn);
                    crate::quality::compute_qa_ai_layer(conn, &backend, id).await
                })
            })
            .await;
        match result {
            Ok(outcome) => {
                if let Some(error) = outcome.error {
                    return service_503(&format!(
                        "{error} The deterministic QA layer was computed and stored."
                    ));
                }
                (
                    StatusCode::OK,
                    Json(json!({
                        "ok": true,
                        "qa": outcome.qa,
                        "ai": outcome.ai,
                        "error": Value::Null
                    })),
                )
                    .into_response()
            }
            Err(join) => service_503(&join),
        }
    } else {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        match crate::quality::compute_qa_deterministic(&conn, id) {
            None => not_found("Conversation not found."),
            Some(_) => {
                let ai_available =
                    crate::settings::get_bool(&conn, "ai_enabled", true).unwrap_or(true);
                let qa = crate::quality::get_qa(&conn, id, ai_available);
                (
                    StatusCode::OK,
                    Json(json!({
                        "ok": true,
                        "qa": qa,
                        "ai": Value::Null,
                        "error": Value::Null
                    })),
                )
                    .into_response()
            }
        }
    }
}

pub async fn friction_overview(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    let days = clamp_days_param(params.get("days"), 30, 1, 3650);
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    Json(crate::quality::friction_overview(&conn, days))
}

pub async fn friction_rebuild(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    crate::quality::ensure_quality_tables(&conn).ok();
    let (conversations, findings) = crate::quality::rebuild_friction(&conn);
    Json(json!({ "conversations": conversations, "findings": findings }))
}

pub async fn friction_conversation(
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
) -> Response {
    let Ok(id) = path_positive_int(&conversation_id, "Conversation id") else {
        return positive_int_422("Conversation id");
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    if !conversation_exists(&conn, id) {
        return not_found("Conversation not found.");
    }
    let findings = crate::quality::analyze_friction(&conn, id);
    Json(json!({ "findings": findings })).into_response()
}
