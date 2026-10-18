use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde_json::{Value, json};

use super::super::server::AppState;

pub async fn handle(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let raw_body = String::from_utf8_lossy(&body).into_owned();

    // Secret resolution: env override first (CI / tests), then the encrypted
    // envelope. An empty result is handled inside process_webhook: outside
    // demo mode it is a hard reject (migration-map row 1).
    let secret = std::env::var("HELPSCOUT_WEBHOOK_SECRET")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            let conn = state.conn_lock();
            crate::settings::get_secret(&conn, "webhook_secret")
                .ok()
                .flatten()
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or_default()
        });

    let signature = headers
        .get("x-helpscout-signature")
        .and_then(|v| v.to_str().ok());

    let event_type = headers
        .get("x-helpscout-event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    let result = crate::webhook_handler::process_webhook(
        &state.conn_lock(),
        secret.as_bytes(),
        raw_body.as_bytes(),
        signature,
        event_type,
        state.demo_mode,
    );

    match result {
        crate::webhook_handler::WebhookProcessResult::Accepted { .. } => {
            (StatusCode::OK, Json(json!({"received": true})))
        }
        crate::webhook_handler::WebhookProcessResult::Duplicate { .. } => (
            StatusCode::OK,
            Json(json!({"received": true, "duplicate": true})),
        ),
        crate::webhook_handler::WebhookProcessResult::SignatureInvalid => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid webhook signature"})),
        ),
        crate::webhook_handler::WebhookProcessResult::SecretNotConfigured => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Webhook secret not configured",
                "message": "Unsigned webhook events are rejected because no webhook secret is \
                            configured. Set a secret in settings or via HELPSCOUT_WEBHOOK_SECRET, \
                            or enable demo mode."
            })),
        ),
        crate::webhook_handler::WebhookProcessResult::BadRequest => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "statusCode": 400,
                "error": "Bad Request",
                "message": "Request body is not valid JSON."
            })),
        ),
    }
}

#[allow(dead_code)]
fn _unused(v: Value) {}
