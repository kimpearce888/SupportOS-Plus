use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use serde_json::{Value, json};

use super::super::server::AppState;
use axum::response::IntoResponse;

pub async fn center(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let mailbox_ids: Option<Vec<i64>> = params
        .get("mailboxes")
        .filter(|raw| !raw.is_empty() && *raw != "all")
        .map(|raw| {
            raw.split(',')
                .filter_map(|v| v.trim().parse::<i64>().ok())
                .filter(|v| *v > 0)
                .take(50)
                .collect::<Vec<_>>()
        });
    match crate::operations::snapshot(&conn, mailbox_ids.as_deref()) {
        Ok(snap) => (
            StatusCode::OK,
            Json(serde_json::to_value(&snap).unwrap_or_else(|_| json!({}))),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "InternalError",
                "message": e.to_string(),
            })),
        ),
    }
}

pub async fn workload(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::workload::snapshot(&conn) {
        Ok(snap) => (
            StatusCode::OK,
            Json(serde_json::to_value(&snap).unwrap_or_else(|_| json!({}))),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "InternalError",
                "message": e.to_string(),
            })),
        ),
    }
}

pub async fn set_capacity(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    match crate::workload::validate_capacity_model(&body) {
        Err(issues) => {
            let message = issues
                .first()
                .cloned()
                .unwrap_or_else(|| "Invalid capacity model.".to_string());
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": message,
                    "detail": issues,
                })),
            )
        }
        Ok(model) => {
            let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
            match crate::workload::set_capacity_model(&conn, &model) {
                Ok(()) => {
                    let stored = crate::workload::get_capacity_model(&conn);
                    (
                        StatusCode::OK,
                        Json(json!({
                            "saved": true,
                            "capacity_model": serde_json::to_value(&stored)
                                .unwrap_or_else(|_| json!({})),
                        })),
                    )
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "statusCode": 500,
                        "error": "InternalError",
                        "message": e,
                    })),
                ),
            }
        }
    }
}

pub async fn set_waiting_threshold(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let Some(minutes) = body.get("minutes").and_then(|v| v.as_i64()) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "minutes must be an integer.",
            })),
        );
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::operations::set_waiting_threshold_minutes(&conn, minutes) {
        Ok(()) => (StatusCode::OK, Json(json!({ "ok": true }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "InternalError",
                "message": e.to_string(),
            })),
        ),
    }
}

pub async fn suggested_assignees(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let raw = params.get("limit");
    let limit = match raw.filter(|s| !s.is_empty()) {
        Some(s) => match crate::conversation_ops::js_number(s) {
            Some(n) if n.is_finite() => (n.trunc() as i64).clamp(1, 50) as u32,
            _ => 10,
        },
        None => 10,
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::workload::suggested_assignees(&conn, limit) {
        Ok(suggestions) => (
            StatusCode::OK,
            Json(json!({
                "suggestions": serde_json::to_value(&suggestions)
                    .unwrap_or_else(|_| json!([])),
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "InternalError",
                "message": e.to_string(),
            })),
        ),
    }
}
