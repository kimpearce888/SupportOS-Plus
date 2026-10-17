use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::super::server::AppState;

fn card_labels() -> Value {
    json!({
        "featureTitle": "Client Interaction Profile",
        "note": "Observable support-communication behavior only — never a psychological assessment."
    })
}

pub async fn get(
    State(state): State<AppState>,
    Path(conversation_id): Path<i64>,
) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![conversation_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if exists == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Conversation not found."
            })),
        );
    }
    match crate::interaction_engine::build_card(&conn, conversation_id, None) {
        Ok(Some(card)) => {
            let card = serde_json::to_value(&card).unwrap_or_else(|_| json!({}));
            (
                StatusCode::OK,
                Json(json!({ "card": card, "labels": card_labels() })),
            )
        }
        _ => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Conversation not found."
            })),
        ),
    }
}

pub async fn refresh(State(state): State<AppState>, Path(conversation_id): Path<i64>) -> Response {
    let exists: i64 = {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.query_row(
            "SELECT COUNT(*) FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![conversation_id],
            |r| r.get(0),
        )
        .unwrap_or(0)
    };
    if exists == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Conversation not found."
            })),
        )
            .into_response();
    }
    let result = state
        .run_ai(move |conn| {
            Box::pin(async move {
                crate::ai_pipeline::ensure_pipeline_schema(conn).ok();
                crate::interaction_engine::ensure_schema(conn).ok();
                let backend = crate::ai_pipeline::backend_from_settings(conn);
                let analysis =
                    crate::ai_pipeline::analyze_interaction(conn, &backend, conversation_id).await;
                let card = crate::interaction_engine::build_card(conn, conversation_id, None);
                (analysis, card)
            })
        })
        .await;
    match result {
        Ok((Ok(analysis), Ok(Some(card)))) => {
            let card = serde_json::to_value(&card).unwrap_or_else(|_| json!({}));
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "ai_enriched": analysis.ai_enriched,
                    "error": analysis.error,
                    "card": card,
                })),
            )
                .into_response()
        }
        Ok((_, _)) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Conversation not found."
            })),
        )
            .into_response(),
        Err(join) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "ok": false, "error": join })),
        )
            .into_response(),
    }
}

pub async fn evidence(
    State(state): State<AppState>,
    Path(conversation_id): Path<i64>,
) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![conversation_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if exists == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Conversation not found."
            })),
        );
    }
    let observations: Vec<Value> =
        match crate::interaction_engine::conversation_customer(&conn, conversation_id) {
            None => Vec::new(),
            Some(customer_id) => crate::interaction_engine::get_observations_for_customer(
                &conn,
                customer_id,
            )
            .unwrap_or_default()
            .into_iter()
            .filter(|o| o.conversation_id == Some(conversation_id) || o.conversation_id.is_none())
            .map(|o| {
                json!({
                    "dimension": o.dimension,
                    "value": o.value,
                    "confidence": o.confidence,
                    "evidence_excerpt": o.evidence_excerpt,
                    "conversation_local_id": o.conversation_id,
                    "thread_local_id": o.thread_local_id,
                    "observed_at": o.observed_at,
                    "provenance": if o.source == "ai" { "ai_generated" } else { "heuristic" },
                })
            })
            .collect(),
        };
    (
        StatusCode::OK,
        Json(json!({ "observations": observations })),
    )
}

pub async fn profile(
    State(state): State<AppState>,
    Path(customer_id): Path<i64>,
) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM customers WHERE id = ?1",
            rusqlite::params![customer_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if exists == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Customer not found or no interaction data."
            })),
        );
    }
    match crate::interaction_engine::build_profile(&conn, customer_id) {
        Ok(Some(profile)) => {
            let profile = serde_json::to_value(&profile).unwrap_or_else(|_| json!({}));
            (StatusCode::OK, Json(json!({ "profile": profile })))
        }
        _ => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Customer not found or no interaction data."
            })),
        ),
    }
}

const RESPONSE_PREFERENCE_VALUES: [&str; 6] = [
    "concise",
    "detailed",
    "step_by_step",
    "technical",
    "conversational",
    "outcome_focused",
];

fn customer_exists_by_local_id(
    conn: &std::sync::MutexGuard<'_, rusqlite::Connection>,
    id: i64,
) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM customers WHERE id = ?1",
        rusqlite::params![id],
        |r| r.get::<_, i64>(0),
    )
    .unwrap_or(0)
        > 0
}

pub async fn set_override(
    State(state): State<AppState>,
    Path(customer_id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let Ok(customer_id) = customer_id.parse::<i64>() else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Customer not found."
            })),
        )
            .into_response();
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    if !customer_exists_by_local_id(&conn, customer_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Customer not found."
            })),
        )
            .into_response();
    }
    let body = body.map(|b| b.0);
    let valid = match body.as_ref() {
        Some(b) => {
            let field_ok = b.get("field").and_then(Value::as_str) == Some("response_preference");
            let value_ok = b
                .get("value")
                .and_then(Value::as_str)
                .is_some_and(|v| RESPONSE_PREFERENCE_VALUES.contains(&v));
            let reason_ok = match b.get("reason") {
                None | Some(Value::Null) => true,
                Some(Value::String(r)) => r.chars().count() <= 500,
                Some(_) => false,
            };
            field_ok && value_ok && reason_ok
        }
        None => false,
    };
    if !valid {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Invalid override payload: field must be response_preference and value must be a known preference (concise, detailed, step_by_step, technical, conversational, outcome_focused)."
            })),
        )
            .into_response();
    }
    let b = body.unwrap_or_default();
    let value = b.get("value").and_then(Value::as_str).unwrap_or_default();
    let reason = b.get("reason").and_then(Value::as_str);
    let effective = crate::interaction_engine::get_preferences(&conn, customer_id)
        .unwrap_or_default()
        .iter()
        .find(|p| p.origin == "human_entered")
        .map(|p| p.preference.clone());
    let res = crate::interaction_engine::set_human_override(
        &conn,
        customer_id,
        value,
        effective.as_deref(),
        reason,
    );
    let _ = crate::audit::audit(
        &conn,
        &crate::audit::AuditEntry::user("interaction_override_set:response_preference")
            .with_after_state(json!({ "value": value })),
    );
    match res {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "message": "Human preference saved. It takes precedence over AI-inferred preferences."
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "InternalError",
                "message": e.to_string()
            })),
        )
            .into_response(),
    }
}

pub async fn clear_override(
    State(state): State<AppState>,
    Path((customer_id, field)): Path<(String, String)>,
) -> Response {
    let Ok(customer_id) = customer_id.parse::<i64>() else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Customer not found."
            })),
        )
            .into_response();
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    if !customer_exists_by_local_id(&conn, customer_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Customer not found."
            })),
        )
            .into_response();
    }
    if field != "response_preference" {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Only response_preference overrides exist."
            })),
        )
            .into_response();
    }
    let res = crate::interaction_engine::clear_human_override(&conn, customer_id);
    let _ = crate::audit::audit(
        &conn,
        &crate::audit::AuditEntry::user("interaction_override_cleared:response_preference"),
    );
    match res {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "message": "Override removed. AI-inferred preferences (if any) apply again."
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "InternalError",
                "message": e.to_string()
            })),
        )
            .into_response(),
    }
}
