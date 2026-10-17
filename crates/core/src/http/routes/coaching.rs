use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::super::server::AppState;
use crate::coaching;

fn validation_422(message: impl Into<String>) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "statusCode": 422,
            "error": "ValidationError",
            "message": message.into(),
        })),
    )
        .into_response()
}

fn not_found_review() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "statusCode": 404,
            "error": "NotFound",
            "message": "No coaching review stored for this conversation yet."
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

fn path_positive_int(raw: &str) -> Result<i64, Response> {
    match crate::conversation_ops::js_number(raw) {
        Some(v) if v.fract() == 0.0 && v > 0.0 && v <= i64::MAX as f64 => Ok(v as i64),
        _ => Err(validation_422(
            "Conversation id must be a positive integer.",
        )),
    }
}

pub async fn meta(State(_state): State<AppState>) -> Response {
    Json(json!({
        "checks": coaching::COACHING_CHECK_KINDS,
        "advisory_only": true,
        "note": "Coaching reviews drafts on request and never blocks, delays or modifies the send. Deterministic checks always run; the AI layer needs LM Studio."
    }))
    .into_response()
}

pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let id = match path_positive_int(&id) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let _ = coaching::ensure_coaching_schema(&conn);
    match coaching::get(&conn, id) {
        Ok(Some(review)) => Json(review).into_response(),
        Ok(None) => not_found_review(),
        Err(e) => service_503(&e.to_string()),
    }
}

pub async fn review(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let id = match path_positive_int(&id) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let Some(Json(body)) = body else {
        return validation_422("body is required.");
    };
    let draft = match body.get("draft") {
        Some(Value::String(s))
            if !s.is_empty() && s.chars().count() <= coaching::COACHING_MAX_DRAFT_CHARS =>
        {
            s.clone()
        }
        _ => {
            return validation_422(format!(
                "draft must be a string of 1 to {} characters.",
                coaching::COACHING_MAX_DRAFT_CHARS
            ));
        }
    };
    let include_ai = match body.get("includeAi") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return validation_422("includeAi must be a boolean."),
    };
    if draft.len() > coaching::COACHING_MAX_DRAFT_BYTES {
        return validation_422(format!(
            "Draft exceeds {} bytes.",
            coaching::COACHING_MAX_DRAFT_BYTES
        ));
    }
    if include_ai {
        let ai_enabled = {
            let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
            crate::settings::get_bool(&conn, "ai_enabled", true).unwrap_or(true)
        };
        if !ai_enabled {
            return service_503(
                "AI is disabled in Settings. The deterministic coaching checks work without it; enable LM Studio for the optional AI layer.",
            );
        }
    }
    let outcome = state
        .run_ai(move |conn| {
            Box::pin(async move {
                let _ = coaching::ensure_coaching_schema(conn);
                let backend = crate::ai_pipeline::backend_from_settings(conn);
                let chat = |messages: Vec<crate::ai_provider::ChatMessage>,
                            temperature: f64,
                            max_tokens: u32,
                            json_mode: bool| {
                    let backend = &backend;
                    Box::pin(async move {
                        backend
                            .chat_qa(messages, temperature, max_tokens, json_mode)
                            .await
                            .map(|res| (res.content, res.model, res.latency_ms))
                            .map_err(|e| e.message)
                    })
                        as std::pin::Pin<
                            Box<
                                dyn std::future::Future<
                                        Output = std::result::Result<
                                            (Option<String>, String, u64),
                                            String,
                                        >,
                                    > + '_,
                            >,
                        >
                };
                coaching::review_draft(conn, Some(&chat), id, &draft, include_ai).await
            })
        })
        .await;
    match outcome {
        Ok(Ok(Ok(review))) => {
            let ai_error = review
                .get("ai")
                .and_then(|a| a.get("error"))
                .and_then(|e| e.as_str())
                .map(str::to_string);
            let mut response = Json(review).into_response();
            if ai_error.is_some() {
                response.headers_mut().insert(
                    "x-ai-layer",
                    axum::http::HeaderValue::from_static("unavailable"),
                );
            }
            response
        }
        Ok(Ok(Err("not_found"))) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Conversation not found."
            })),
        )
            .into_response(),
        Ok(Ok(Err("empty_draft"))) => validation_422("Draft is empty."),
        Ok(Ok(Err(_))) => service_503("coaching failure"),
        Ok(Err(e)) => service_503(&format!(
            "{e} The deterministic coaching checks were computed and stored."
        )),
        Err(join) => service_503(&join),
    }
}
