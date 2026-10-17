use axum::Json;
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::super::server::AppState;
use crate::helpscout::HelpScoutProvider;

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const FAIL_STYLE: &str = "body{font-family:system-ui,sans-serif;max-width:36rem;margin:4rem auto;padding:0 1rem;color:#1f2933}h1{font-size:1.2rem}p{line-height:1.5;color:#52606d}";

fn html_response(title: &str, body_inner: &str) -> Response {
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title><style>{FAIL_STYLE}</style></head><body>{body_inner}</body></html>"
    );
    (StatusCode::OK, [(header::CONTENT_TYPE, "text/html")], html).into_response()
}

fn fail_page(message: &str) -> Response {
    html_response(
        "SupportOS - connection not completed",
        &format!(
            "<h1>Help Scout connection not completed</h1><p>{message}</p><p>Return to SupportOS Settings and try again, or use \"Connect with Client Credentials\".</p></body></html>"
        ),
    )
}

pub async fn authorize_url(State(state): State<AppState>) -> Response {
    let Some(real) = state.real.clone() else {
        return Json(json!({
            "demo_mode": true,
            "message": "Demo mode is active - OAuth is not needed.",
        }))
        .into_response();
    };
    if !real.credentials.is_configured() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "ok": false,
                "message": "HELPSCOUT_CLIENT_ID and HELPSCOUT_CLIENT_SECRET must be set in .env first.",
            })),
        )
            .into_response();
    }
    let mut state_bytes = [0u8; 16];
    rand::fill(&mut state_bytes[..]);
    let state_hex = crate::crypto::hex(&state_bytes);
    {
        let conn = state.conn_lock();
        let _ = conn.execute(
            "INSERT INTO application_settings (key, value, updated_at)
             VALUES ('oauth_state', ?1, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            rusqlite::params![serde_json::to_string(&state_hex).unwrap_or_default()],
        );
    }
    Json(json!({ "url": real.credentials.authorize_url(&state_hex) })).into_response()
}

pub async fn client_credentials(State(state): State<AppState>) -> Response {
    let Some(real) = state.real.clone() else {
        return Json(json!({ "demo_mode": true })).into_response();
    };
    if !real.credentials.is_configured() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "ok": false,
                "message": "HELPSCOUT_CLIENT_ID and HELPSCOUT_CLIENT_SECRET must be set in .env first.",
            })),
        )
            .into_response();
    }
    match real.client_credentials_login().await {
        Ok(_) => match real.get_me().await {
            Ok(me) => {
                let conn = state.conn_lock();
                let _ = conn.execute(
                    "INSERT INTO application_settings (key, value, updated_at)
                     VALUES ('me_remote_id', ?1, datetime('now'))
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                    rusqlite::params![serde_json::to_string(&me.remote_id).unwrap_or_default()],
                );
                let _ = crate::jobs::audit(
                    &conn,
                    "user",
                    "helpscout_connected",
                    None,
                    None,
                    None,
                    Some("POST /v2/oauth2/token"),
                    None,
                    false,
                );
                Json(json!({
                    "ok": true,
                    "message": format!("Connected to Help Scout as {} {}.", me.first_name.unwrap_or_default(), me.last_name.unwrap_or_default()),
                }))
                .into_response()
            }
            Err(e) => (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "ok": false, "message": e.to_string() })),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "ok": false, "message": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    if state.real.is_none() {
        return Json(json!({
            "configured": false,
            "authenticated": true,
            "demo_mode": true,
            "expires_at": null,
            "me": null,
        }));
    }
    let real = state.real.clone().unwrap();
    let mut v = real.oauth_status(state.demo_mode);
    if v["authenticated"].as_bool().unwrap_or(false) {
        if let Ok(me) = real.get_me().await {
            let conn = state.conn_lock();
            let _ = conn.execute(
                "INSERT INTO application_settings (key, value, updated_at)
                 VALUES ('me_remote_id', ?1, datetime('now'))
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                rusqlite::params![serde_json::to_string(&me.remote_id).unwrap_or_default()],
            );
            v["me"] = json!({
                "name": format!("{} {}", me.first_name.unwrap_or_default(), me.last_name.unwrap_or_default()).trim(),
                "email": me.email,
            });
        } else {
            v["me"] = Value::Null;
        }
    } else {
        v["me"] = Value::Null;
    }
    Json(v)
}

pub async fn disconnect(State(state): State<AppState>) -> impl IntoResponse {
    if let Some(real) = &state.real {
        let _ = real.revoke();
    }
    let conn = state.conn_lock();
    let _ = crate::jobs::audit(
        &conn,
        "user",
        "helpscout_disconnected",
        None,
        None,
        None,
        None,
        None,
        false,
    );
    Json(json!({ "ok": true, "message": "Disconnected. Local data is fully preserved." }))
}

pub async fn callback(
    State(state): State<AppState>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let Some(real) = state.real.clone() else {
        return fail_page("Demo mode is active - OAuth is not needed.");
    };
    if let Some(err) = q.get("error") {
        let detail = q
            .get("error_description")
            .map(|d| format!(" - {d}"))
            .unwrap_or_default();
        return fail_page(&format!(
            "Help Scout returned an error: {}{}",
            escape_html(err),
            escape_html(&detail)
        ));
    }
    let (Some(code), Some(state_param)) = (q.get("code"), q.get("state")) else {
        return fail_page("The callback is missing its authorization code or state parameter.");
    };
    let stored: Option<String> = {
        let conn = state.conn_lock();
        let row: Option<String> = conn
            .query_row(
                "SELECT value FROM application_settings WHERE key = 'oauth_state'",
                [],
                |r| r.get(0),
            )
            .ok();
        let _ = conn.execute(
            "DELETE FROM application_settings WHERE key = 'oauth_state'",
            [],
        );
        row
    };
    let stored = stored
        .as_deref()
        .and_then(|s| serde_json::from_str::<String>(s).ok());
    match stored {
        Some(s) if s == *state_param => {}
        _ => {
            return fail_page(
                "The state parameter did not match the authorization request (it may have expired or been reused). For safety the code was not exchanged.",
            );
        }
    }
    match real.exchange_code(code).await {
        Ok(_) => match real.get_me().await {
            Ok(me) => {
                let name = format!(
                    "{} {}",
                    me.first_name.unwrap_or_default(),
                    me.last_name.unwrap_or_default()
                )
                .trim()
                .to_string();
                let email_part = me
                    .email
                    .as_deref()
                    .map(|e| format!(" ({})", escape_html(e)))
                    .unwrap_or_default();
                {
                    let conn = state.conn_lock();
                    let _ = conn.execute(
                        "INSERT INTO application_settings (key, value, updated_at)
                         VALUES ('me_remote_id', ?1, datetime('now'))
                         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                        rusqlite::params![serde_json::to_string(&me.remote_id).unwrap_or_default()],
                    );
                    let _ = crate::jobs::audit(
                        &conn,
                        "user",
                        "helpscout_connected",
                        None,
                        None,
                        None,
                        Some("GET /oauth/callback (code exchange)"),
                        None,
                        false,
                    );
                }
                html_response(
                    "SupportOS - connected",
                    &format!(
                        "<h1>Connected to Help Scout</h1><p>Connected as {}{}.</p><p>You can close this tab and return to SupportOS. Reload the Settings page to see the connection status.</p>",
                        escape_html(&name),
                        email_part
                    ),
                )
            }
            Err(e) => fail_page(&escape_html(&e.to_string())),
        },
        Err(e) => fail_page(&format!(
            "Exchanging the authorization code failed: {}",
            escape_html(&e.to_string())
        )),
    }
}
