use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use super::super::server::AppState;
use axum::response::IntoResponse;

use crate::saved_views::{
    parse_create_view_request, parse_update_view_request, parse_view_definition, ViewCompileError,
    ViewEngine,
};

fn zod_envelope(issues: &[(String, String)]) -> (StatusCode, Json<Value>) {
    let Some((first_path, first_message)) = issues.first() else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Invalid request: request body failed validation.",
                "issues": [],
            })),
        );
    };
    let where_hint = if first_path.is_empty() {
        String::new()
    } else {
        format!(" ({first_path})")
    };
    let issue_list: Vec<Value> = issues
        .iter()
        .take(10)
        .map(|(path, message)| json!({ "path": path, "message": message }))
        .collect();
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "statusCode": 422,
            "error": "ValidationError",
            "message": format!("Invalid request{where_hint}: {first_message}"),
            "issues": issue_list,
        })),
    )
}

fn display_timezone(conn: &rusqlite::Connection) -> String {
    let stored = crate::settings::get_string(conn, "display_timezone")
        .ok()
        .flatten()
        .unwrap_or_default();
    crate::saved_views::resolve_timezone(None, Some(stored.as_str()))
}

fn sla_resolver(conn: &rusqlite::Connection) -> crate::saved_views::SlaResolver<'static> {
    let alerts = crate::sla::sla_alerts(conn).ok();
    Box::new(move |states: &[&str]| {
        alerts
            .as_ref()
            .map(|a| {
                a.alerts
                    .iter()
                    .filter(|alert| states.contains(&alert.state.as_str()))
                    .map(|alert| alert.conversation_id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    })
}

pub async fn list_views(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let views = crate::saved_views::list_views(&conn).unwrap_or_default();
    let items: Vec<Value> = views
        .iter()
        .filter_map(|v| serde_json::to_value(v).ok())
        .collect();
    Json(json!({ "views": items }))
}

pub async fn get_view(State(state): State<AppState>, Path(id): Path<i64>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::saved_views::get_view(&conn, id) {
        Ok(Some(view)) => (
            StatusCode::OK,
            Json(serde_json::to_value(&view).unwrap_or(json!({}))),
        ),
        _ => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Saved view not found.",
            })),
        ),
    }
}

pub async fn create_view(
    State(state): State<AppState>,
    body: Option<Json<Value>>,
) -> impl IntoResponse {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let req = match parse_create_view_request(&body) {
        Ok(req) => req,
        Err(issues) => {
            let zod_issues: Vec<(String, String)> = issues
                .iter()
                .map(|i| (i.path.clone(), i.message.clone()))
                .collect();
            return zod_envelope(&zod_issues);
        }
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let tz = display_timezone(&conn);
    if let Err(e) = ViewEngine::new(&tz).compile(&req.definition) {
        return compile_rejected(&e);
    }
    match crate::saved_views::create_view(
        &conn,
        &req.name,
        req.description.as_deref(),
        &req.definition,
        req.sort_order.unwrap_or(0),
        req.folder.as_deref(),
    ) {
        Ok(view) => {
            let _ = crate::jobs::audit(
                &conn,
                "user",
                "inbox_view_created",
                None,
                None,
                Some(
                    &serde_json::to_string(&json!({ "view_id": view.id, "name": view.name }))
                        .unwrap_or_default(),
                ),
                None,
                None,
                false,
            );
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "message": format!("View '{}' saved.", view.name),
                    "view": view,
                })),
            )
        }
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

pub async fn update_view(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    body: Option<Json<Value>>,
) -> impl IntoResponse {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let req = match parse_update_view_request(&body) {
        Ok(req) => req,
        Err(issues) => {
            let zod_issues: Vec<(String, String)> = issues
                .iter()
                .map(|i| (i.path.clone(), i.message.clone()))
                .collect();
            return zod_envelope(&zod_issues);
        }
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(definition) = req.definition.as_ref() {
        let tz = display_timezone(&conn);
        if let Err(e) = ViewEngine::new(&tz).compile(definition) {
            return compile_rejected(&e);
        }
    }
    match crate::saved_views::update_view(
        &conn,
        id,
        req.name.as_deref(),
        req.description.as_ref().map(|d| d.as_deref()),
        req.definition.as_ref(),
        req.sort_order,
        req.folder.as_ref().map(|f| f.as_deref()),
    ) {
        Ok(Some(view)) => {
            let _ = crate::jobs::audit(
                &conn,
                "user",
                "inbox_view_updated",
                None,
                None,
                Some(
                    &serde_json::to_string(
                        &json!({ "view_id": id, "name": view.name, "version": view.version }),
                    )
                    .unwrap_or_default(),
                ),
                None,
                None,
                false,
            );
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "message": "View updated.",
                    "view": view,
                })),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Saved view not found.",
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

pub async fn delete_view(State(state): State<AppState>, Path(id): Path<i64>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::saved_views::get_view(&conn, id) {
        Ok(Some(view)) => {
            let _ = crate::saved_views::delete_view(&conn, id);
            let _ = crate::jobs::audit(
                &conn,
                "user",
                "inbox_view_deleted",
                None,
                Some(
                    &serde_json::to_string(&json!({ "view_id": id, "name": view.name }))
                        .unwrap_or_default(),
                ),
                None,
                None,
                None,
                false,
            );
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "message": format!("View '{}' deleted.", view.name),
                })),
            )
        }
        _ => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Saved view not found.",
            })),
        ),
    }
}

fn compile_rejected(e: &ViewCompileError) -> (StatusCode, Json<Value>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "statusCode": 422,
            "error": "ValidationError",
            "message": format!("View definition cannot be evaluated: {}", e.message),
        })),
    )
}

pub async fn preview_view(
    State(state): State<AppState>,
    body: Option<Json<Value>>,
) -> impl IntoResponse {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let raw_tz = body.get("timezone").and_then(|v| v.as_str());
    let raw_definition = body.get("definition").cloned().unwrap_or(Value::Null);
    let definition = match parse_view_definition(&raw_definition) {
        Ok(def) => def,
        Err(issues) => {
            let zod_issues: Vec<(String, String)> = issues
                .iter()
                .map(|i| (i.path.clone(), i.message.clone()))
                .collect();
            return zod_envelope(&zod_issues);
        }
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let stored = crate::settings::get_string(&conn, "display_timezone")
        .ok()
        .flatten()
        .unwrap_or_default();
    let tz = crate::saved_views::resolve_timezone(raw_tz, Some(stored.as_str()));
    let engine = ViewEngine::new(&tz).with_sla_resolver(sla_resolver(&conn));
    let compiled = match engine.compile(&definition) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": e.message,
                })),
            );
        }
    };
    let sql = if compiled.where_sql == "1=1" {
        "SELECT COUNT(*) FROM conversations c WHERE c.deleted_at IS NULL AND c.merged_into_conversation_id IS NULL"
    } else {
        &format!(
            "SELECT COUNT(*) FROM conversations c WHERE c.deleted_at IS NULL AND c.merged_into_conversation_id IS NULL AND ({})",
            compiled.where_sql
        )
    };
    let count: i64 = match conn.query_row(
        sql,
        rusqlite::params_from_iter(compiled.params.iter()),
        |r| r.get(0),
    ) {
        Ok(n) => n,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "statusCode": 500,
                    "error": "InternalError",
                    "message": e.to_string(),
                })),
            );
        }
    };
    (
        StatusCode::OK,
        Json(json!({
            "matched": count,
            "notes": compiled.notes,
            "timezone": tz,
        })),
    )
}
