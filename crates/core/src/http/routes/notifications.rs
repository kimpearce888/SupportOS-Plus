use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::{Value, json};
use std::collections::HashMap;

use super::super::server::AppState;
use spp_catalog::NotificationType;

fn clamp_list_param(value: Option<&String>, fallback: i64, min: i64, max: i64) -> i64 {
    let n = value
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(fallback);
    n.clamp(min, max)
}

pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let me = crate::notifications::me_user_local_id(&conn).ok().flatten();
    let notif_type = params
        .get("type")
        .filter(|t| {
            NotificationType::ALL
                .iter()
                .any(|n| n.as_str() == t.as_str())
        })
        .map(String::as_str);
    let unread_only = matches!(
        params.get("unreadOnly").map(String::as_str),
        Some("true") | Some("1")
    );
    let opts = crate::notifications::ListOptions {
        me_user_local_id: me,
        unread_only,
        notif_type,
        limit: clamp_list_param(params.get("limit"), 50, 1, 200),
        offset: clamp_list_param(params.get("offset"), 0, 0, 100_000),
    };
    match crate::notifications::list(&conn, &opts) {
        Ok(result) => match serde_json::to_value(&result) {
            Ok(v) => (StatusCode::OK, Json(v)),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"message": e.to_string()})),
            ),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        ),
    }
}

pub async fn unread_count(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let me = crate::notifications::me_user_local_id(&conn).ok().flatten();
    let unread = crate::notifications::unread_count(&conn, me).unwrap_or(0);
    (StatusCode::OK, Json(json!({"unread": unread})))
}

pub async fn mark_read(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> impl IntoResponse {
    let Ok(id) = id.parse::<i64>() else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "id must be a positive integer."
            })),
        );
    };
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
    let read = match body {
        None => true,
        Some(Json(body)) => match body.get("read") {
            None | Some(Value::Null) => true,
            Some(Value::Bool(b)) => *b,
            Some(_) => {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422,
                        "error": "ValidationError",
                        "message": "Body must be { read: boolean }."
                    })),
                );
            }
        },
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let me = crate::notifications::me_user_local_id(&conn).ok().flatten();
    match crate::notifications::mark_read(&conn, id, me, read) {
        Ok(true) => {
            let unread = crate::notifications::unread_count(&conn, me).unwrap_or(0);
            (StatusCode::OK, Json(json!({"unread": unread})))
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Notification not found (or not visible to you)."
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        ),
    }
}

pub async fn mark_all_read(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let me = crate::notifications::me_user_local_id(&conn).ok().flatten();
    match crate::notifications::mark_all_read(&conn, me) {
        Ok(marked) => {
            let unread = crate::notifications::unread_count(&conn, me).unwrap_or(0);
            (
                StatusCode::OK,
                Json(json!({"marked": marked, "unread": unread})),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        ),
    }
}

fn build_prefs(conn: &rusqlite::Connection) -> Vec<Value> {
    NotificationType::ALL
        .iter()
        .map(|t| {
            let enabled =
                crate::notifications::pref_for(conn, *t).unwrap_or_else(|_| t.default_enabled());
            json!({
                "type": t.as_str(),
                "enabled": enabled,
                "default_enabled": t.default_enabled(),
            })
        })
        .collect()
}

pub async fn list_prefs(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    (StatusCode::OK, Json(json!({"prefs": build_prefs(&conn)})))
}

pub async fn set_pref(
    State(state): State<AppState>,
    Path(notif_type): Path<String>,
    body: Option<Json<Value>>,
) -> impl IntoResponse {
    let valid = NotificationType::ALL
        .iter()
        .any(|t| t.as_str() == notif_type);
    if !valid {
        let names: Vec<&str> = NotificationType::ALL.iter().map(|t| t.as_str()).collect();
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": format!("type must be one of: {}.", names.join(", ")),
            })),
        );
    }

    let Some(Json(body)) = body else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Body must be { enabled: boolean }.",
            })),
        );
    };
    let Some(enabled) = body.get("enabled").and_then(|v| v.as_bool()) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Body must be { enabled: boolean }.",
            })),
        );
    };

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let key = format!("notifications.{notif_type}.enabled");
    if let Err(e) = crate::settings::set_bool(&conn, &key, enabled) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        );
    }
    (StatusCode::OK, Json(json!({"prefs": build_prefs(&conn)})))
}

pub async fn mentions(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let me = crate::notifications::me_user_local_id(&conn).ok().flatten();
    let Some(me) = me else {
        return (
            StatusCode::OK,
            Json(json!({"me": null, "notifications": [], "side_thread_mentions": []})),
        );
    };
    let notifications = crate::notifications::mentions_for_me(&conn, me, 100)
        .unwrap_or_default()
        .iter()
        .filter_map(|n| serde_json::to_value(n).ok())
        .collect::<Vec<_>>();
    let side_thread_mentions = crate::side_threads::mentions_for_user(&conn, me)
        .unwrap_or_default()
        .iter()
        .map(|m| {
            json!({
                "message_id": m.message_id,
                "thread_id": m.thread_id,
                "thread_title": m.thread_title,
                "conversation_id": m.conversation_id,
                "conversation_number": m.conversation_number,
                "author": m.author,
                "body": m.body,
                "created_at": m.created_at,
            })
        })
        .collect::<Vec<_>>();
    (
        StatusCode::OK,
        Json(json!({
            "me": me,
            "notifications": notifications,
            "side_thread_mentions": side_thread_mentions,
        })),
    )
}

pub async fn sweep(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::notification_sweep::sweep(&conn, Some(&state.bus)) {
        Ok(outcome) => (StatusCode::OK, Json(json!({"created": outcome.created}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        ),
    }
}
