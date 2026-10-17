#[allow(warnings)]
pub mod ai;
#[allow(warnings)]
pub mod analytics;
#[allow(warnings)]
pub mod attributes;
#[allow(warnings)]
pub mod automation;
#[allow(warnings)]
pub mod coaching;
#[allow(warnings)]
pub mod collaboration;
#[allow(warnings)]
pub mod connectors;
#[allow(warnings)]
pub mod conversations;
#[allow(warnings)]
pub mod copilot;
#[allow(warnings)]
pub mod custom_objects;
#[allow(warnings)]
pub mod docs;
#[allow(warnings)]
pub mod events;
#[allow(warnings)]
pub mod graph;
#[allow(warnings)]
pub mod incidents;
#[allow(warnings)]
pub mod interactions;
#[allow(warnings)]
pub mod issues;
#[allow(warnings)]
pub mod knowledge;
#[allow(warnings)]
pub mod memory;
#[allow(warnings)]
pub mod notifications;
#[allow(warnings)]
pub mod oauth;
#[allow(warnings)]
pub mod operations;
#[allow(warnings)]
pub mod outreach;
#[allow(warnings)]
pub mod people;
#[allow(warnings)]
pub mod quality;
#[allow(warnings)]
pub mod search;
#[allow(warnings)]
pub mod settings;
#[allow(warnings)]
pub mod sync;
#[allow(warnings)]
pub mod system;
#[allow(warnings)]
pub mod translation;
#[allow(warnings)]
pub mod views;
#[allow(warnings)]
pub mod webhook;

use axum::body::Bytes;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

pub async fn not_found() -> Response {
    (
        axum::http::StatusCode::NOT_FOUND,
        [("Content-Type", "application/json")],
        r#"{"statusCode":404,"error":"NotFound","message":"Unknown API endpoint."}"#,
    )
        .into_response()
}

pub fn parse_json_body(bytes: &Bytes) -> Result<Value, Response> {
    if bytes.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice::<Value>(bytes).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "statusCode": 400,
                "error": "Bad Request",
                "message": format!("Request body is not valid JSON: {e}")
            })),
        )
            .into_response()
    })
}

#[must_use]
pub fn zod_type_name(v: &Value) -> &'static str {
    match v {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        Value::Null => "null",
    }
}

#[must_use]
pub fn zod_422(path: &str, message: &str) -> Response {
    crate::conversation_ops::zod_422(path, message)
}

#[must_use]
pub fn zod_422_multi(issues: &[(&str, &str)]) -> Response {
    crate::conversation_ops::zod_422_multi(issues)
}

#[must_use]
pub fn zod_enum_message(variants: &[&str], received: &str) -> String {
    crate::conversation_ops::zod_enum_message(variants, received)
}

pub fn zod_int_msg(body: &Value, field: &str) -> std::result::Result<i64, String> {
    crate::conversation_ops::zod_int_msg(body, field)
}
