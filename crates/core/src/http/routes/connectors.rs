use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::super::server::AppState;
use crate::connectors;

fn bad_request(issues: &[String]) -> Response {
    let joined: String = issues.join("; ").chars().take(300).collect();
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "statusCode": 400,
            "error": "BadRequest",
            "message": joined,
        })),
    )
        .into_response()
}

fn validation_error(message: &str) -> Response {
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

fn unprocessable(message: String) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({ "ok": false, "message": message })),
    )
        .into_response()
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "statusCode": 404,
            "error": "NotFound",
            "message": "Connector not found.",
        })),
    )
        .into_response()
}

fn path_id(raw: &str) -> Option<i64> {
    crate::conversation_ops::js_number(raw)
        .filter(|v| v.fract() == 0.0 && *v > 0.0 && *v <= i64::MAX as f64)
        .map(|v| v as i64)
}

fn clamp_list_param(raw: Option<&String>, fallback: i64, min: i64, max: i64) -> i64 {
    match raw {
        Some(v) if !v.is_empty() => match crate::conversation_ops::js_number(v) {
            Some(n) if n.is_finite() => (n.trunc() as i64).clamp(min, max),
            _ => fallback,
        },
        _ => fallback,
    }
}

fn non_empty(field: &Value, label: &str, max: usize, issues: &mut Vec<String>) -> Option<String> {
    let Value::String(s) = field else {
        issues.push(format!("{label} is required."));
        return None;
    };
    let trimmed = s.trim();
    if trimmed.is_empty() {
        issues.push(format!("{label} must not be empty."));
    } else if trimmed.chars().count() > max {
        issues.push(format!("{label} must contain at most {max} characters."));
    }
    if trimmed.is_empty() || trimmed.chars().count() > max {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn is_jail_file_name(s: &str) -> bool {
    !s.is_empty() && !s.contains('/') && !s.contains('\\')
}

fn key_column(config: &Map<String, Value>, issues: &mut Vec<String>) -> Value {
    match config.get("keyColumn") {
        None | Some(Value::Null) => Value::Null,
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.chars().count() > 60 {
                issues.push("keyColumn must contain at most 60 characters.".into());
            }
            Value::String(trimmed.to_string())
        }
        Some(_) => {
            issues.push("keyColumn must be a string or null.".into());
            Value::Null
        }
    }
}

fn validate_config_schema(body: &Value, issues: &mut Vec<String>) -> Option<Value> {
    let Value::Object(config) = body else {
        issues.push("config is required.".into());
        return None;
    };
    let kind = match config.get("kind") {
        Some(Value::String(k)) => k.as_str(),
        _ => {
            issues.push("config.kind must be one of 'local_json', 'csv', 'sqlite', 'http'.".into());
            return None;
        }
    };
    match kind {
        "local_json" | "csv" | "sqlite" => {
            let mut out = Map::new();
            out.insert("kind".into(), json!(kind));
            let file = match config.get("file") {
                Some(f @ Value::String(_)) => match non_empty(f, "config.file", 200, issues) {
                    Some(file) if !is_jail_file_name(&file) => {
                        issues.push(
                            "File name inside the connectors directory (no absolute paths)".into(),
                        );
                        None
                    }
                    Some(file) => Some(file),
                    None => None,
                },
                _ => {
                    issues.push("config.file is required.".into());
                    None
                }
            };
            file.inspect(|f| {
                out.insert("file".into(), json!(f));
            });
            if kind == "sqlite" {
                let table = match config.get("table") {
                    Some(t @ Value::String(_)) => non_empty(t, "config.table", 60, issues),
                    _ => {
                        issues.push("config.table is required.".into());
                        None
                    }
                };
                match table {
                    Some(t) if !is_table_identifier(&t) => {
                        issues.push("Table name identifier".into());
                    }
                    Some(t) => {
                        out.insert("table".into(), json!(t));
                    }
                    None => {}
                }
            }
            let kc = key_column(config, issues);
            out.insert("keyColumn".into(), kc);
            if issues.is_empty() {
                Some(Value::Object(out))
            } else {
                None
            }
        }
        "http" => {
            let mut out = Map::new();
            out.insert("kind".into(), json!("http"));
            let url = match config.get("url") {
                Some(Value::String(u)) => {
                    let trimmed = u.trim();
                    let len = trimmed.chars().count();
                    if len < 8 {
                        issues.push("config.url must contain at least 8 characters.".into());
                        None
                    } else if len > 600 {
                        issues.push("config.url must contain at most 600 characters.".into());
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                }
                _ => {
                    issues.push("config.url is required.".into());
                    None
                }
            };
            url.inspect(|u| {
                out.insert("url".into(), json!(u));
            });
            let kc = key_column(config, issues);
            out.insert("keyColumn".into(), kc);
            if issues.is_empty() {
                Some(Value::Object(out))
            } else {
                None
            }
        }
        other => {
            issues.push(format!(
                "config.kind must be one of 'local_json', 'csv', 'sqlite', 'http' (got '{other}')."
            ));
            None
        }
    }
}

fn is_table_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn validate_auth_schema(body: &Value, issues: &mut Vec<String>) -> Option<Value> {
    let Value::Object(auth) = body else {
        issues.push("auth must be an object.".into());
        return None;
    };
    let mode = match auth.get("mode") {
        Some(Value::String(m)) => m.as_str(),
        _ => {
            issues.push("auth.mode must be one of 'none', 'header', 'bearer'.".into());
            return None;
        }
    };
    match mode {
        "none" => Some(json!({ "mode": "none" })),
        "header" => {
            let header_name = match auth.get("headerName") {
                Some(h @ Value::String(_)) => match non_empty(h, "auth.headerName", 60, issues) {
                    Some(name) if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') => {
                        issues.push("Header names are token identifiers".into());
                        None
                    }
                    Some(name) => Some(name),
                    None => None,
                },
                _ => {
                    issues.push("auth.headerName is required.".into());
                    None
                }
            };
            let header_value = match auth.get("headerValue") {
                Some(v @ Value::String(_)) => match non_empty(v, "auth.headerValue", 500, issues) {
                    Some(value) => Some(value),
                    None => None,
                },
                _ => {
                    issues.push("auth.headerValue is required.".into());
                    None
                }
            };
            match (header_name, header_value) {
                (Some(n), Some(v)) if issues.is_empty() => {
                    Some(json!({ "mode": "header", "headerName": n, "headerValue": v }))
                }
                _ => None,
            }
        }
        "bearer" => {
            let token = match auth.get("token") {
                Some(t @ Value::String(_)) => match non_empty(t, "auth.token", 500, issues) {
                    Some(token) => Some(token),
                    None => None,
                },
                _ => {
                    issues.push("auth.token is required.".into());
                    None
                }
            };
            match token {
                Some(t) if issues.is_empty() => Some(json!({ "mode": "bearer", "token": t })),
                _ => None,
            }
        }
        other => {
            issues.push(format!(
                "auth.mode must be one of 'none', 'header', 'bearer' (got '{other}')."
            ));
            None
        }
    }
}

pub async fn list(State(state): State<AppState>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let items: Vec<Value> = connectors::list(&conn)
        .unwrap_or_default()
        .iter()
        .map(|c| connectors::redacted(&conn, c))
        .collect();
    Json(json!({ "connectors": items })).into_response()
}

pub async fn create(State(state): State<AppState>, body: Option<Json<Value>>) -> Response {
    let Some(Json(body)) = body else {
        return bad_request(&["body is required.".to_string()]);
    };
    let mut issues: Vec<String> = Vec::new();

    let name = match body.get("name") {
        Some(n) => non_empty(n, "name", 80, &mut issues),
        None => {
            issues.push("name is required.".into());
            None
        }
    };
    let config = match body.get("config") {
        Some(c) => validate_config_schema(c, &mut issues),
        None => {
            issues.push("config is required.".into());
            None
        }
    };
    let auth = match body.get("auth") {
        None => json!({ "mode": "none" }),
        Some(a) => validate_auth_schema(a, &mut issues).unwrap_or(json!({ "mode": "none" })),
    };
    let refresh_method = match body.get("refreshMethod") {
        None => "manual".to_string(),
        Some(Value::String(m)) if m == "manual" || m == "interval" => m.clone(),
        Some(_) => {
            issues.push("refreshMethod must be 'manual' or 'interval'.".into());
            "manual".to_string()
        }
    };
    let refresh_seconds = match body.get("refreshSeconds") {
        None => 3600,
        Some(Value::Number(n)) if n.is_u64() => {
            let v = n.as_u64().unwrap_or(3600);
            if (60..=86400).contains(&v) {
                v as i64
            } else {
                issues.push("refreshSeconds must be between 60 and 86400.".into());
                3600
            }
        }
        Some(_) => {
            issues.push("refreshSeconds must be an integer.".into());
            3600
        }
    };
    let allowed_ai = match body.get("allowedAi") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            issues.push("allowedAi must be a boolean.".into());
            false
        }
    };
    if !issues.is_empty() {
        return bad_request(&issues);
    }
    let (name, config) = (name.unwrap_or_default(), config.unwrap_or(Value::Null));

    let kind = config.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let file = config.get("file").and_then(|v| v.as_str());
    let url = config.get("url").and_then(|v| v.as_str());
    if let Some(problem) = connectors::validate_config(&state.data_dir, kind, file, url).await {
        return validation_error(&problem);
    }

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match connectors::create(
        &conn,
        &name,
        kind,
        &config,
        &auth,
        &refresh_method,
        refresh_seconds,
        allowed_ai,
    ) {
        Ok(connector) => {
            let after = json!({
                "id": connector.get("id"),
                "name": connector.get("name"),
                "kind": connector.get("kind"),
            });
            let _ = crate::jobs::audit(
                &conn,
                "user",
                "connector_created",
                None,
                None,
                Some(&after.to_string()),
                None,
                None,
                false,
            );
            let redacted = connectors::redacted(&conn, &connector);
            Json(json!({ "ok": true, "connector": redacted })).into_response()
        }
        Err(e) => unprocessable(e.to_string()),
    }
}

pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Some(id) = path_id(&id) else {
        return not_found();
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match connectors::get(&conn, id).ok().flatten() {
        Some(c) => Json(json!({ "connector": connectors::redacted(&conn, &c) })).into_response(),
        None => not_found(),
    }
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let Some(id) = path_id(&id) else {
        return not_found();
    };
    let Some(Json(body)) = body else {
        return bad_request(&["body is required.".to_string()]);
    };
    let mut issues: Vec<String> = Vec::new();
    let mut changes = Map::new();
    let mut fields: Vec<&str> = Vec::new();

    if let Some(n) = body.get("name") {
        if let Some(name) = non_empty(n, "name", 80, &mut issues) {
            changes.insert("name".into(), json!(name));
            fields.push("name");
        }
    }
    if let Some(c) = body.get("config") {
        if let Some(config) = validate_config_schema(c, &mut issues) {
            changes.insert("config".into(), config);
            fields.push("config");
        }
    }
    if let Some(a) = body.get("auth") {
        if let Some(auth) = validate_auth_schema(a, &mut issues) {
            changes.insert("auth".into(), auth);
            fields.push("auth");
        }
    }
    if let Some(rm) = body.get("refreshMethod") {
        match rm.as_str() {
            Some(m) if m == "manual" || m == "interval" => {
                changes.insert("refreshMethod".into(), json!(m));
                fields.push("refreshMethod");
            }
            _ => issues.push("refreshMethod must be 'manual' or 'interval'.".into()),
        }
    }
    if let Some(rs) = body.get("refreshSeconds") {
        match rs.as_u64() {
            Some(v) if (60..=86400).contains(&v) => {
                changes.insert("refreshSeconds".into(), json!(v));
                fields.push("refreshSeconds");
            }
            _ => issues.push("refreshSeconds must be an integer between 60 and 86400.".into()),
        }
    }
    if let Some(ai) = body.get("allowedAi") {
        match ai.as_bool() {
            Some(b) => {
                changes.insert("allowedAi".into(), json!(b));
                fields.push("allowedAi");
            }
            None => issues.push("allowedAi must be a boolean.".into()),
        }
    }
    if let Some(en) = body.get("enabled") {
        match en.as_bool() {
            Some(b) => {
                changes.insert("enabled".into(), json!(b));
                fields.push("enabled");
            }
            None => issues.push("enabled must be a boolean.".into()),
        }
    }
    if !issues.is_empty() {
        return bad_request(&issues);
    }
    let changes = Value::Object(changes);

    if let Some(config) = changes.get("config") {
        let kind = config.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let file = config.get("file").and_then(|v| v.as_str());
        let url = config.get("url").and_then(|v| v.as_str());
        if let Some(problem) = connectors::validate_config(&state.data_dir, kind, file, url).await {
            return validation_error(&problem);
        }
    }

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match connectors::patch(&conn, id, &changes) {
        Ok(Some(connector)) => {
            let after = json!({ "id": id, "fields": fields });
            let _ = crate::jobs::audit(
                &conn,
                "user",
                "connector_updated",
                None,
                None,
                Some(&after.to_string()),
                None,
                None,
                false,
            );
            let redacted = connectors::redacted(&conn, &connector);
            Json(json!({ "ok": true, "connector": redacted })).into_response()
        }
        Ok(None) => not_found(),
        Err(e) => unprocessable(e.to_string()),
    }
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Some(id) = path_id(&id) else {
        return not_found();
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match connectors::delete(&conn, id) {
        Ok(true) => {
            let before = json!({ "id": id });
            let _ = crate::jobs::audit(
                &conn,
                "user",
                "connector_deleted",
                None,
                Some(&before.to_string()),
                None,
                None,
                None,
                false,
            );
            Json(json!({
                "ok": true,
                "message": "Connector and its cached rows deleted."
            }))
            .into_response()
        }
        Ok(false) => not_found(),
        Err(e) => unprocessable(e.to_string()),
    }
}

pub async fn refresh(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Some(id) = path_id(&id) else {
        return not_found();
    };
    {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        if connectors::get(&conn, id).ok().flatten().is_none() {
            return not_found();
        }
    }
    let result = match connectors::refresh(&state.conn, id, &state.data_dir).await {
        Ok(result) => result,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "statusCode": 500,
                    "error": "InternalServerError",
                    "message": e.to_string()
                })),
            )
                .into_response()
        }
    };
    let result_json = json!({
        "ok": result.ok,
        "rows": result.rows,
        "pruned": result.pruned,
        "schema": result.schema.clone().unwrap_or(Value::Null),
        "error": result.error.clone().map(Value::String).unwrap_or(Value::Null),
    });
    {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        let after = json!({
            "id": id,
            "ok": result.ok,
            "rows": result.rows,
            "pruned": result.pruned,
        });
        let _ = crate::jobs::audit(
            &conn,
            "user",
            "connector_refreshed",
            None,
            None,
            Some(&after.to_string()),
            None,
            None,
            false,
        );
    }
    if !result.ok {
        let message = format!(
            "Refresh failed: {}",
            result.error.as_deref().unwrap_or("unknown error")
        );
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "ok": false, "message": message, "result": result_json })),
        )
            .into_response();
    }
    Json(json!({ "ok": true, "result": result_json })).into_response()
}

pub async fn rows(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let Some(id) = path_id(&id) else {
        return not_found();
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let Some(_connector) = connectors::get(&conn, id).ok().flatten() else {
        return not_found();
    };
    let q: Option<String> = query
        .get("q")
        .map(|q| q.chars().take(120).collect::<String>())
        .filter(|q| !q.is_empty());
    let page_size = clamp_list_param(query.get("pageSize"), 50, 1, 200);
    let page = clamp_list_param(query.get("page"), 1, 1, 100_000);
    let offset = (page - 1) * page_size;
    match connectors::list_rows(&conn, id, q.as_deref(), page_size, offset) {
        Ok((rows, total)) => Json(json!({ "rows": rows, "total": total })).into_response(),
        Err(e) => unprocessable(e.to_string()),
    }
}

pub async fn test(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Some(id) = path_id(&id) else {
        return not_found();
    };
    let (kind, file, url) = {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        let Some(connector) = connectors::get(&conn, id).ok().flatten() else {
            return not_found();
        };
        let config = connector.get("config").cloned().unwrap_or(Value::Null);
        (
            config
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            config
                .get("file")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            config
                .get("url")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        )
    };
    if let Some(problem) =
        connectors::validate_config(&state.data_dir, &kind, file.as_deref(), url.as_deref()).await
    {
        return Json(json!({ "ok": false, "message": problem })).into_response();
    }
    Json(json!({
        "ok": true,
        "message": "Configuration is valid and the source host resolves. Full reachability is verified on the next refresh."
    }))
    .into_response()
}
