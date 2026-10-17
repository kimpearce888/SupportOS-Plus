use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use super::super::server::AppState;
use crate::translation;

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

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "statusCode": 404,
            "error": "NotFound",
            "message": "Conversation not found."
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

fn bad_gateway(message: String) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        Json(json!({
            "statusCode": 502,
            "error": "BadGateway",
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

pub async fn meta(State(state): State<AppState>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let languages: Vec<Value> = translation::SUPPORTED_LANGUAGES
        .iter()
        .map(|(code, name)| json!({ "code": code, "name": name }))
        .collect();
    Json(json!({
        "languages": languages,
        "agent_language": translation::agent_language(&conn),
        "note": "Detection is deterministic (script ranges + function words). Translation runs on the locally configured model only; nothing is ever sent automatically."
    }))
    .into_response()
}

pub async fn detect(State(_state): State<AppState>, body: Option<Json<Value>>) -> Response {
    let Some(Json(body)) = body else {
        return validation_422("body is required.");
    };
    let Some(texts) = body.get("texts").and_then(|v| v.as_array()) else {
        return validation_422("texts must be an array of strings.");
    };
    if texts.is_empty() || texts.len() > 50 {
        return validation_422("texts must contain between 1 and 50 strings.");
    }
    let mut parsed: Vec<String> = Vec::with_capacity(texts.len());
    for t in texts {
        let Some(s) = t.as_str() else {
            return validation_422("texts must be an array of strings.");
        };
        if s.chars().count() > 8000 {
            return validation_422("Each text must contain at most 8000 characters.");
        }
        parsed.push(s.to_string());
    }
    Json(json!({ "detections": translation::detect(&parsed) })).into_response()
}

pub async fn conversation(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let id = match path_positive_int(&id) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match translation::conversation_languages(&conn, id) {
        Ok(Some(summary)) => Json(summary).into_response(),
        Ok(None) => not_found(),
        Err(e) => bad_gateway(e.to_string()),
    }
}

pub async fn translate(State(state): State<AppState>, body: Option<Json<Value>>) -> Response {
    let Some(Json(body)) = body else {
        return validation_422("body is required.");
    };
    let text = match body.get("text") {
        Some(Value::String(s)) if !s.is_empty() && s.chars().count() <= 8000 => s.clone(),
        _ => return validation_422("text must be a string of 1 to 8000 characters."),
    };
    let from = match body.get("from") {
        None => None,
        Some(Value::Null) => None,
        Some(Value::String(s)) if s.chars().count() <= 8 => Some(s.clone()),
        Some(_) => {
            return validation_422("from must be a language code (at most 8 characters) or null.")
        }
    };
    let to = match body.get("to") {
        Some(Value::String(s)) if (2..=8).contains(&s.chars().count()) => s.clone(),
        _ => return validation_422("to must be a language code (2 to 8 characters)."),
    };
    let purpose = match body.get("purpose") {
        None => None,
        Some(Value::Null) => None,
        Some(Value::String(s))
            if s == "customer_inbound" || s == "agent_draft" || s == "general" =>
        {
            Some(s.clone())
        }
        Some(_) => {
            return validation_422(
                "purpose must be one of 'customer_inbound', 'agent_draft', 'general'.",
            )
        }
    };
    if !translation::SUPPORTED_LANGUAGES
        .iter()
        .any(|(code, _)| *code == to)
    {
        return validation_422(format!(
            "Unsupported target language. Supported: {}.",
            translation::SUPPORTED_LANGUAGES
                .iter()
                .map(|(c, _)| *c)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        let ai_enabled = crate::settings::get_bool(&conn, "ai_enabled", true).unwrap_or(true);
        if !ai_enabled {
            return service_503(
                "AI is disabled in Settings. Translation uses the local LM Studio model only - there is no cloud fallback, so translation is unavailable.",
            );
        }
    }
    let outcome = state
        .run_ai(move |conn| {
            Box::pin(async move {
                crate::translation::ensure_translation_schema(conn).ok();
                let backend = crate::ai_pipeline::backend_from_settings(conn);
                let chat = |messages: Vec<crate::ai_provider::ChatMessage>,
                            temperature: f64,
                            max_tokens: u32| {
                    let backend = &backend;
                    Box::pin(async move {
                        backend
                            .chat_qa(messages, temperature, max_tokens, false)
                            .await
                            .map(|res| (res.content, res.model))
                            .map_err(|e| e.message)
                    })
                        as std::pin::Pin<
                            Box<
                                dyn std::future::Future<
                                        Output = std::result::Result<
                                            (Option<String>, String),
                                            String,
                                        >,
                                    > + '_,
                            >,
                        >
                };
                translation::translate(conn, &chat, &text, from.as_deref(), &to, purpose.as_deref())
                    .await
            })
        })
        .await;
    match outcome {
        Ok(Ok(result)) => {
            let mut out = json!({ "ok": true });
            if let (Value::Object(base), Value::Object(extra)) = (out.clone(), result) {
                let mut merged = base;
                for (k, v) in extra {
                    merged.insert(k, v);
                }
                out = Value::Object(merged);
            }
            Json(out).into_response()
        }
        Ok(Err(crate::translation::TranslateError::Client(message))) => validation_422(message),
        Ok(Err(crate::translation::TranslateError::Service(message))) => service_503(&message),
        Ok(Err(crate::translation::TranslateError::BadGateway(message))) => bad_gateway(message),
        Err(join) => bad_gateway(join),
    }
}
