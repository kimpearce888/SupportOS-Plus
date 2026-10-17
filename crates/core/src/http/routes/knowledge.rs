use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use super::super::server::AppState;
use crate::knowledge_store;

fn not_found(message: &str) -> Response {
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

fn bad_request(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "statusCode": 400,
            "error": "BadRequest",
            "message": message,
        })),
    )
        .into_response()
}

pub async fn list_sources(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    Json(json!({"sources": knowledge_store::list_sources(&conn)}))
}

pub async fn list_documents(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let source_id = params.get("sourceId").and_then(|s| s.parse::<i64>().ok());
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    Json(json!({"documents": knowledge_store::list_documents(&conn, source_id)}))
}

pub async fn get_document(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let Some(document) = knowledge_store::get_document(&conn, id) else {
        return not_found("Document not found.");
    };
    let related_ticket_estimate = knowledge_store::related_ticket_estimate(&conn, id);
    let related_known_issues = knowledge_store::search_known_issues_by_title(
        &conn,
        document.get("title").and_then(|v| v.as_str()).unwrap_or(""),
    );
    Json(json!({
        "document": document,
        "related_ticket_estimate": related_ticket_estimate,
        "related_known_issues": related_known_issues,
    }))
    .into_response()
}

pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let query = params.get("q").cloned().unwrap_or_default();
    let visibility = params
        .get("visibility")
        .map(String::as_str)
        .filter(|v| *v == "customer_safe");
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let results = knowledge_store::search_knowledge(&conn, &query, visibility);
    Json(json!({"results": results, "query": query}))
}

pub async fn freshness(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    Json(knowledge_store::freshness_report(&conn))
}

pub async fn import(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let source_name = body
        .get("sourceName")
        .and_then(|v| v.as_str())
        .unwrap_or("Manual import")
        .to_string();
    let visibility = match body.get("visibility") {
        None | Some(Value::Null) => "internal_only".to_string(),
        Some(Value::String(s)) if s == "customer_safe" || s == "internal_only" => s.clone(),
        Some(_) => return bad_request("visibility must be customer_safe or internal_only."),
    };
    let Some(documents) = body.get("documents").and_then(|v| v.as_array()) else {
        return bad_request("At least one document is required.");
    };
    if documents.is_empty() {
        return bad_request("At least one document is required.");
    }
    for doc in documents {
        let content_empty = doc
            .get("content")
            .and_then(|v| v.as_str())
            .map(str::is_empty)
            .unwrap_or(true);
        if content_empty {
            return bad_request("Document content must not be empty.");
        }
        if let Some(format) = doc.get("format") {
            if !matches!(
                format.as_str(),
                Some("markdown") | Some("txt") | Some("html")
            ) {
                return bad_request("format must be markdown, txt or html.");
            }
        }
    }

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match knowledge_store::import_manual(&conn, &source_name, documents, &visibility) {
        Ok(results) => {
            let _ =
                crate::jobs::enqueue_on(&conn, "embeddings", "embed_knowledge_chunks", "{}", 4, 2);
            let _ = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("knowledge_imported").with_after_state(json!({
                    "count": results.len(),
                    "visibility": visibility,
                })),
            );
            Json(json!({
                "ok": true,
                "imported": results.len(),
                "documents": results,
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "message": format!("Import failed: {e}")})),
        )
            .into_response(),
    }
}

pub async fn import_file(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let Some(path) = body
        .get("path")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|p| !p.is_empty())
    else {
        return bad_request("A file path is required.");
    };
    if path.len() > 1024 {
        return bad_request("A file path is required.");
    }
    let source_name = body
        .get("sourceName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let visibility = match body.get("visibility") {
        None | Some(Value::Null) => "internal_only".to_string(),
        Some(Value::String(s)) if s == "customer_safe" || s == "internal_only" => s.clone(),
        Some(_) => return bad_request("visibility must be customer_safe or internal_only."),
    };

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match knowledge_store::import_file(
        &conn,
        &state.data_dir,
        path,
        source_name.as_deref(),
        &visibility,
    ) {
        Ok(results) => {
            let _ =
                crate::jobs::enqueue_on(&conn, "embeddings", "embed_knowledge_chunks", "{}", 4, 2);
            Json(json!({
                "ok": true,
                "imported": results.len(),
                "documents": results,
            }))
            .into_response()
        }
        Err(crate::error::Error::Validation(message)) => {
            if message.starts_with("For safety") || message.starts_with("File not found") {
                bad_request(&message)
            } else {
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({"ok": false, "message": message})),
                )
                    .into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "message": format!("Import failed: {e}")})),
        )
            .into_response(),
    }
}

pub async fn importable(State(state): State<AppState>) -> Json<Value> {
    Json(knowledge_store::importable_files(&state.data_dir))
}

pub async fn review_document(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match knowledge_store::mark_reviewed(&conn, id) {
        Ok(true) => {
            let _ = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("knowledge_reviewed")
                    .with_after_state(json!({ "id": id })),
            );
            Json(json!({
                "ok": true,
                "message": "Reviewed (timestamp recorded; nothing is published automatically)."
            }))
            .into_response()
        }
        Ok(false) => not_found("Document not found."),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "message": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn verify_document(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match knowledge_store::mark_verified(&conn, id) {
        Ok(true) => {
            let _ = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("knowledge_verified")
                    .with_after_state(json!({ "id": id })),
            );
            Json(json!({
                "ok": true,
                "message": "Verified (timestamp recorded; nothing is published automatically)."
            }))
            .into_response()
        }
        Ok(false) => not_found("Document not found."),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "message": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn reindex(State(state): State<AppState>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::search::rebuild_indexes(&conn) {
        Ok(_) => {
            let _ =
                crate::jobs::enqueue_on(&conn, "embeddings", "embed_knowledge_chunks", "{}", 4, 2);
            Json(json!({"ok": true, "message": "Reindexing queued."})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "message": format!("Reindex failed: {e}")})),
        )
            .into_response(),
    }
}

pub async fn delete_document(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match knowledge_store::delete_document(&conn, id) {
        Ok(_) => {
            let _ = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("knowledge_deleted")
                    .with_after_state(json!({ "id": id })),
            );
            Json(json!({"ok": true, "message": "Document deleted."})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "message": e.to_string()})),
        )
            .into_response(),
    }
}
