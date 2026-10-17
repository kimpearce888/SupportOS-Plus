use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::{Value, json};

use super::super::server::AppState;

pub async fn list_side_threads(
    State(state): State<AppState>,
    Path(conversation_id): Path<i64>,
) -> impl IntoResponse {
    if conversation_id <= 0 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "id must be a positive integer."
            })),
        );
    }
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let exists = conn
        .query_row(
            "SELECT 1 FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![conversation_id],
            |_| Ok(()),
        )
        .is_ok();
    if !exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Conversation not found."
            })),
        );
    }
    match crate::side_threads::list_side_thread_summaries(&conn, conversation_id) {
        Ok(items) => (StatusCode::OK, Json(json!({ "side_threads": items }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "Internal Server Error",
                "message": e.to_string()
            })),
        ),
    }
}

pub async fn create_side_thread(
    State(state): State<AppState>,
    Path(conversation_id): Path<i64>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    if conversation_id <= 0 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "id must be a positive integer."
            })),
        );
    }
    let input = match crate::side_threads::parse_create_side_thread_input(&body) {
        Ok(input) => input,
        Err(issues) => {
            let message = issues
                .iter()
                .map(|(path, msg)| format!("{path}: {msg}"))
                .collect::<Vec<_>>()
                .join("; ");
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": message
                })),
            );
        }
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let exists = conn
        .query_row(
            "SELECT 1 FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![conversation_id],
            |_| Ok(()),
        )
        .is_ok();
    if !exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Conversation not found."
            })),
        );
    }
    let issues = crate::side_threads::check_create_references(&conn, &input);
    if !issues.is_empty() {
        let message = issues
            .iter()
            .map(|(path, msg)| format!("{path}: {msg}"))
            .collect::<Vec<_>>()
            .join("; ");
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": message
            })),
        );
    }
    match crate::side_threads::create_side_thread_full(
        &conn,
        Some(&state.bus),
        conversation_id,
        &input,
    ) {
        Ok(id) => {
            let detail = crate::side_threads::get_side_thread_detail(&conn, id)
                .ok()
                .flatten();
            (
                StatusCode::OK,
                Json(json!({"ok": true, "id": id, "side_thread": detail})),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "Internal Server Error",
                "message": e.to_string()
            })),
        ),
    }
}

pub async fn get_side_thread(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if id <= 0 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "id must be a positive integer."
            })),
        );
    }
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::side_threads::get_side_thread_detail(&conn, id) {
        Ok(Some(detail)) => (StatusCode::OK, Json(json!({ "side_thread": detail }))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Side thread not found."
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "Internal Server Error",
                "message": e.to_string()
            })),
        ),
    }
}

pub async fn add_message(
    State(state): State<AppState>,
    Path(thread_id): Path<i64>,
    body: Option<Json<Value>>,
) -> Json<Value> {
    let Some(Json(body)) = body else {
        return Json(json!({"ok": false, "error": "Body must be { body: string }."}));
    };
    let text = body.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let author = body.get("authorUserId").and_then(|v| v.as_i64());
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::side_threads::add_side_thread_message(
        &conn,
        Some(&state.bus),
        thread_id,
        text,
        author,
    ) {
        Ok(id) => Json(json!({"ok": true, "id": id})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn resolve(State(state): State<AppState>, Path(id): Path<i64>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::side_threads::resolve_side_thread(&conn, id) {
        Ok(_) => Json(json!({"ok": true})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn reopen(State(state): State<AppState>, Path(id): Path<i64>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::side_threads::reopen_side_thread(&conn, id) {
        Ok(_) => Json(json!({"ok": true})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn add_participants(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    body: Option<Json<Value>>,
) -> impl IntoResponse {
    if id <= 0 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "id must be a positive integer."
            })),
        );
    }
    let Json(body) = body.unwrap_or(Json(Value::Null));
    let user_ids: Vec<i64> = ["userIds", "user_ids", "participant_user_ids"]
        .iter()
        .find_map(|k| {
            body.get(*k)
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_i64()).collect::<Vec<i64>>())
        })
        .unwrap_or_default();
    if user_ids.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "userIds: Required and must be a non-empty array of user ids."
            })),
        );
    }
    let added_by = ["addedByUserId", "added_by_user_local_id"]
        .iter()
        .find_map(|k| body.get(*k).and_then(|v| v.as_i64()));
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let thread_exists = conn
        .query_row(
            "SELECT 1 FROM side_threads WHERE id = ?1",
            rusqlite::params![id],
            |_| Ok(()),
        )
        .is_ok();
    if !thread_exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Side thread not found."
            })),
        );
    }
    match crate::side_threads::add_participants_checked(&conn, id, &user_ids, added_by) {
        Ok(inserted) => (StatusCode::OK, Json(json!({"ok": true, "added": inserted}))),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": e.to_string()
            })),
        ),
    }
}

pub async fn mention_directory(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let people: Vec<Value> = conn
        .prepare("SELECT id, first_name, last_name FROM users ORDER BY first_name")
        .ok()
        .map(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "name": format!("{} {}", r.get::<_, String>(1)?, r.get::<_, String>(2)?),
                }))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
        .unwrap_or_default();
    Json(json!({"directory": people}))
}
