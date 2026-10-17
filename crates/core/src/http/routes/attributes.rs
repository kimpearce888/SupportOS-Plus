use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use std::collections::HashMap;

use super::super::server::AppState;
use crate::ai_attributes::{self, MatchOp};
use crate::catalog::{AI_ATTRIBUTE_SCHEMA_VERSION, AiAttributeKey, AttributeValueType};

fn clamp_list_param(raw: Option<&String>, fallback: i64, min: i64, max: i64) -> i64 {
    let n = match raw {
        Some(s) if !s.is_empty() => crate::conversation_ops::js_number(s),
        _ => Some(fallback as f64),
    };
    match n {
        Some(v) if v.is_finite() => (v.trunc() as i64).clamp(min, max),
        _ => fallback,
    }
}

fn parse_positive_int(raw: &str) -> Option<i64> {
    let n = crate::conversation_ops::js_number(raw)?;
    if n.fract() != 0.0 || n <= 0.0 || n > i64::MAX as f64 {
        return None;
    }
    Some(n as i64)
}

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

fn not_found_404() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "statusCode": 404,
            "error": "NotFound",
            "message": "Conversation not found.",
        })),
    )
        .into_response()
}

pub async fn catalog(State(state): State<AppState>) -> Response {
    let _ = state;
    let catalog: Vec<Value> = AiAttributeKey::ALL
        .iter()
        .map(|k| {
            let mut entry = json!({
                "key": k.as_str(),
                "label": k.label(),
                "value_type": k.value_type().as_str(),
                "description": k.description(),
            });
            if k.value_type() == AttributeValueType::Enum {
                entry["values"] = json!(k.values());
            }
            entry
        })
        .collect();
    (
        StatusCode::OK,
        Json(json!({
            "catalog": catalog,
            "schema_version": AI_ATTRIBUTE_SCHEMA_VERSION,
            "note": "A missing attribute reads as unknown - it is never fabricated. \
                     Attributes are local-only and never written to Help Scout."
        })),
    )
        .into_response()
}

pub async fn conversation_attributes(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let Some(id) = parse_positive_int(&id) else {
        return validation_422("Conversation id must be a positive integer.");
    };
    let conn = state.conn_lock();
    match ai_attributes::snapshot(&conn, id) {
        Ok(Some(snapshot)) => (StatusCode::OK, Json(snapshot)).into_response(),
        Ok(None) => not_found_404(),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "statusCode": 503,
                "error": "ServiceUnavailable",
                "message": e.to_string(),
            })),
        )
            .into_response(),
    }
}

pub async fn conversation_attribute_history(
    State(state): State<AppState>,
    Path((id, attribute)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let Some(id) = parse_positive_int(&id) else {
        return validation_422("Conversation id must be a positive integer.");
    };
    let Some(key) = AiAttributeKey::parse(&attribute) else {
        return validation_422(&format!(
            "Unknown attribute '{attribute}'. Use /api/attributes/catalog for the closed list."
        ));
    };
    let limit = clamp_list_param(params.get("limit"), 50, 1, 200);
    let conn = state.conn_lock();
    match ai_attributes::history(&conn, id, key, limit) {
        Ok(history) => (StatusCode::OK, Json(json!({ "history": history }))).into_response(),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "statusCode": 503,
                "error": "ServiceUnavailable",
                "message": e.to_string(),
            })),
        )
            .into_response(),
    }
}

pub async fn conversations(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let attribute = params.get("attribute").cloned().unwrap_or_default();
    let Some(key) = AiAttributeKey::parse(&attribute) else {
        return (
            StatusCode::OK,
            Json(json!({
                "attribute": attribute,
                "op": null,
                "value": null,
                "conversations": [],
                "note": format!(
                    "Unknown attribute '{attribute}'. Use /api/attributes/catalog."
                ),
            })),
        )
            .into_response();
    };
    let op_raw = params.get("op").cloned().unwrap_or_else(|| "equals".into());
    let op = MatchOp::parse(&op_raw, key.value_type());
    let value: String = params
        .get("value")
        .cloned()
        .unwrap_or_default()
        .chars()
        .take(120)
        .collect();
    let limit = clamp_list_param(params.get("limit"), 25, 1, 200);
    let conn = state.conn_lock();
    let conversations =
        ai_attributes::conversations_matching(&conn, key, op, &value, limit).unwrap_or_default();
    (
        StatusCode::OK,
        Json(json!({
            "attribute": attribute,
            "op": op.as_str(),
            "value": value,
            "conversations": conversations,
        })),
    )
        .into_response()
}

pub async fn report(State(state): State<AppState>) -> Response {
    let conn = state.conn_lock();
    let distributions = ai_attributes::distributions(&conn).unwrap_or_default();
    (
        StatusCode::OK,
        Json(json!({ "distributions": distributions })),
    )
        .into_response()
}

pub async fn values(
    State(state): State<AppState>,
    Path(attribute): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let Some(key) = AiAttributeKey::parse(&attribute) else {
        return validation_422(&format!("Unknown attribute '{attribute}'."));
    };
    let limit = clamp_list_param(params.get("limit"), 50, 1, 200);
    let conn = state.conn_lock();
    let values = ai_attributes::distinct_values(&conn, key, limit).unwrap_or_default();
    (
        StatusCode::OK,
        Json(json!({ "attribute": attribute, "values": values })),
    )
        .into_response()
}

pub async fn recompute(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let Some(id) = parse_positive_int(&id) else {
        return validation_422("Conversation id must be a positive integer.");
    };
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let force = match body.get("force") {
        None => false,
        Some(v) if v.is_boolean() => v.as_bool().unwrap_or(false),
        Some(v) => {
            return crate::conversation_ops::zod_422(
                "force",
                &format!("Expected boolean, received {}", zod_received(v)),
            );
        }
    };

    let (model, use_ai, base_url, timeout_ms) = {
        let conn = state.conn_lock();
        let status = crate::ai_center::get_ai_status(&conn).unwrap_or_default();
        let model = status.chat_model.clone().unwrap_or_default();
        let use_ai = crate::ai_center::ai_features_enabled(&status);
        let base_url = crate::settings::get_string(&conn, "lmstudio_base_url")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty())
            .or_else(|| status.base_url.clone())
            .unwrap_or_else(|| crate::ai_lm_studio::LM_STUDIO_BASE_URL.to_string());
        let timeout_ms = u64::try_from(
            crate::settings::get_i64(&conn, "lmstudio_timeout_ms", 120_000).unwrap_or(120_000),
        )
        .unwrap_or(crate::ai_lm_studio::LM_STUDIO_DEFAULT_TIMEOUT_MS);
        (model, use_ai, base_url, timeout_ms)
    };
    let provider: Box<dyn crate::ai_provider::LocalAiProvider> = if use_ai {
        Box::new(
            crate::ai_lm_studio::LmStudioProvider::with_base_url_and_timeout(base_url, timeout_ms),
        )
    } else {
        Box::new(crate::ai_provider::NoopAiProvider)
    };

    let prepared = {
        let conn = state.conn_lock();
        match ai_attributes::prepare_computation(&conn, id, &model, force) {
            Ok(Some(prepared)) => prepared,
            Ok(None) => return not_found_404(),
            Err(e) => return service_unavailable(e.to_string()),
        }
    };

    let fresh = match (&prepared.prompt, prepared.cached.is_none()) {
        (Some(prompt), true) => {
            ai_attributes::run_extraction(provider.as_ref(), &model, prompt).await
        }
        _ => None,
    };

    let result = {
        let mut conn = state.conn_lock();
        let snapshot = match ai_attributes::finish_computation(&mut conn, prepared, fresh, &model) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => return not_found_404(),
            Err(e) => return service_unavailable(e.to_string()),
        };
        let _ = crate::audit::audit(
            &conn,
            &crate::audit::AuditEntry {
                actor: "user",
                action: "attributes_recomputed".into(),
                conversation_id: Some(id),
                before_state: None,
                after_state: None,
                remote_operation: None,
                remote_result: None,
                ai_involvement: true,
                job_id: None,
                correlation_id: None,
            },
        );
        snapshot
    };

    (
        StatusCode::OK,
        Json(json!({ "ok": true, "snapshot": result })),
    )
        .into_response()
}

fn service_unavailable(message: String) -> Response {
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

fn zod_received(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
