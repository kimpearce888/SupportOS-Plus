use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::super::server::AppState;
use crate::custom_objects::{self, FieldInput, LinkInput, LINK_TARGET_KINDS};

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

fn conflict_409(message: &str) -> Response {
    (
        StatusCode::CONFLICT,
        Json(json!({
            "statusCode": 409,
            "error": "Conflict",
            "message": message,
        })),
    )
        .into_response()
}

fn internal_500(message: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "statusCode": 500,
            "error": "InternalError",
            "message": message,
        })),
    )
        .into_response()
}

fn clamp_list_param(raw: Option<&String>, fallback: i64, min: i64, max: i64) -> i64 {
    let Some(s) = raw.filter(|s| !s.is_empty()) else {
        return fallback;
    };
    match crate::conversation_ops::js_number(s) {
        Some(n) if n.is_finite() => (n.trunc() as i64).clamp(min, max),
        _ => fallback,
    }
}

fn non_empty(trimmed: &str, max: usize) -> Option<String> {
    let t = trimmed.trim();
    if t.is_empty() || t.chars().count() > max {
        None
    } else {
        Some(t.to_string())
    }
}

fn parse_fields(body: &Value, key: &str, issues: &mut Vec<String>) -> Option<Vec<FieldInput>> {
    let raw = body.get(key)?;
    let Some(arr) = raw.as_array() else {
        issues.push(format!("{key} must be an array of field definitions."));
        return None;
    };
    if arr.is_empty() || arr.len() > 40 {
        issues.push(format!("{key} must contain between 1 and 40 fields."));
        return None;
    }
    let mut fields = Vec::with_capacity(arr.len());
    let mut keys = std::collections::HashSet::new();
    for item in arr {
        let key_str = item
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let label_raw = item.get("label").and_then(|v| v.as_str()).unwrap_or("");
        let Some(label) = non_empty(label_raw, 80) else {
            issues.push("Field label must be a non-empty string of at most 80 characters.".into());
            continue;
        };
        let Some(field_type) = item.get("fieldType").and_then(|v| v.as_str()) else {
            issues.push(
                "fieldType must be one of text, long_text, number, date, boolean, select.".into(),
            );
            continue;
        };
        if !custom_objects::FIELD_TYPES.contains(&field_type) {
            issues.push(format!(
                "fieldType must be one of text, long_text, number, date, boolean, select."
            ));
            continue;
        }
        let required = item
            .get("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let options = match item.get("options") {
            None | Some(Value::Null) => None,
            Some(Value::Array(arr)) => {
                if arr.len() > 50 {
                    issues.push("options must contain at most 50 entries.".into());
                    continue;
                }
                let mut opts = Vec::with_capacity(arr.len());
                for o in arr {
                    let Some(s) = o.as_str().and_then(|s| non_empty(s, 80)) else {
                        issues.push(
                            "options entries must be non-empty strings of at most 80 characters."
                                .into(),
                        );
                        continue;
                    };
                    opts.push(s);
                }
                Some(opts)
            }
            Some(_) => {
                issues.push("options must be an array of strings or null.".into());
                continue;
            }
        };
        if field_type == "select" && options.as_ref().is_none_or(Vec::is_empty) {
            issues.push("Select fields need at least one option".into());
            continue;
        }
        if !keys.insert(key_str.clone()) {
            issues.push("Field keys must be unique within a type".into());
            continue;
        }
        fields.push(FieldInput {
            key: key_str,
            label,
            field_type: field_type.to_string(),
            required,
            options,
        });
    }
    if issues.is_empty() {
        Some(fields)
    } else {
        None
    }
}

fn parse_description(body: &Value, issues: &mut Vec<String>) -> Option<Option<String>> {
    match body.get("description") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(s)) if s.chars().count() <= 500 => Some(Some(s.clone())),
        Some(_) => {
            issues.push("description must be a string of at most 500 characters or null.".into());
            None
        }
    }
}

fn parse_links(body: &Value, issues: &mut Vec<String>) -> Option<Option<Vec<LinkInput>>> {
    let raw = body.get("links")?;
    let Some(arr) = raw.as_array() else {
        issues.push("links must be an array.".into());
        return None;
    };
    if arr.len() > 100 {
        issues.push("links must contain at most 100 entries.".into());
        return None;
    }
    let mut links = Vec::with_capacity(arr.len());
    for item in arr {
        let Some(kind) = item.get("targetKind").and_then(|v| v.as_str()) else {
            issues.push("targetKind must be one of customer, organization, conversation, known_issue, incident, campaign.".into());
            continue;
        };
        if !LINK_TARGET_KINDS.contains(&kind) {
            issues.push("targetKind must be one of customer, organization, conversation, known_issue, incident, campaign.".into());
            continue;
        }
        let Some(id) = item.get("targetLocalId").and_then(|v| v.as_i64()) else {
            issues.push("targetLocalId must be a positive integer.".into());
            continue;
        };
        if id <= 0 {
            issues.push("targetLocalId must be a positive integer.".into());
            continue;
        }
        let note = match item.get("note") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if s.chars().count() <= 500 => Some(s.clone()),
            Some(_) => {
                issues.push("note must be a string of at most 500 characters or null.".into());
                continue;
            }
        };
        links.push(LinkInput {
            target_kind: kind.to_string(),
            target_local_id: id,
            note,
        });
    }
    if issues.is_empty() {
        Some(Some(links))
    } else {
        None
    }
}

pub async fn list_types(State(state): State<AppState>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match custom_objects::list_types(&conn) {
        Ok(types) => Json(json!({
            "types": types.iter().map(|t| t.to_json()).collect::<Vec<_>>(),
        }))
        .into_response(),
        Err(e) => internal_500(&e.to_string()),
    }
}

pub async fn create_type(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let mut issues = Vec::new();
    let Some(name) = body
        .get("name")
        .and_then(|v| v.as_str())
        .and_then(|s| non_empty(s, 80))
    else {
        issues.push("name must be a non-empty string of at most 80 characters.".into());
        return bad_request(&issues);
    };
    let description = parse_description(&body, &mut issues);
    let fields = if body.get("fields").is_none() {
        issues.push("fields must contain between 1 and 40 fields.".into());
        None
    } else {
        parse_fields(&body, "fields", &mut issues)
    };
    if !issues.is_empty() {
        return bad_request(&issues);
    }
    let Some(fields) = fields else {
        return bad_request(&["fields must contain between 1 and 40 fields.".to_string()]);
    };
    let description = description.flatten();
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match custom_objects::create_type(&conn, &name, description.as_deref(), &fields) {
        Ok((id, slug)) => {
            let _ = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("custom_object_type_created")
                    .with_after_state(json!({ "id": id, "slug": slug })),
            );
            match custom_objects::get_type(&conn, id) {
                Ok(Some(t)) => Json(json!({"ok": true, "type": t.to_json()})).into_response(),
                _ => internal_500("type vanished after create"),
            }
        }
        Err(crate::error::Error::Validation(m)) => validation_422(&m),
        Err(e) => internal_500(&e.to_string()),
    }
}

pub async fn get_type(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match custom_objects::get_type(&conn, id) {
        Ok(Some(t)) => Json(json!({"type": t.to_json()})).into_response(),
        Ok(None) => not_found("Type not found."),
        Err(e) => internal_500(&e.to_string()),
    }
}

pub async fn update_type(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Response {
    let mut issues = Vec::new();
    let name = match body.get("name") {
        None => None,
        Some(v) => match v.as_str().and_then(|s| non_empty(s, 80)) {
            Some(n) => Some(n),
            None => {
                issues.push("name must be a non-empty string of at most 80 characters.".into());
                None
            }
        },
    };
    let description = parse_description(&body, &mut issues);
    let fields = parse_fields(&body, "fields", &mut issues);
    if !issues.is_empty() {
        return bad_request(&issues);
    }
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match custom_objects::patch_type(
        &conn,
        id,
        name.as_deref(),
        description.as_ref().map(|d| d.as_deref()),
        fields.as_deref(),
    ) {
        Ok(()) => {
            let _ = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("custom_object_type_updated")
                    .with_after_state(json!({ "id": id })),
            );
            match custom_objects::get_type(&conn, id) {
                Ok(Some(t)) => Json(json!({"ok": true, "type": t.to_json()})).into_response(),
                Ok(None) => not_found("Type not found."),
                Err(e) => internal_500(&e.to_string()),
            }
        }
        Err(crate::error::Error::Validation(m)) => validation_422(&m),
        Err(e) => internal_500(&e.to_string()),
    }
}

pub async fn delete_type(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match custom_objects::delete_type(&conn, id) {
        Ok(true) => {
            let _ = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("custom_object_type_deleted")
                    .with_before_state(json!({ "id": id })),
            );
            Json(json!({"ok": true, "message": "Type deleted."})).into_response()
        }
        Ok(false) => not_found("Type not found."),
        Err(crate::error::Error::Validation(m)) => conflict_409(&m),
        Err(e) => internal_500(&e.to_string()),
    }
}

pub async fn list_objects(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let type_id = params.get("typeId").and_then(|t| t.parse::<i64>().ok());
    let query: Option<String> = params
        .get("q")
        .map(|q| q.chars().take(120).collect::<String>());
    let page_size = clamp_list_param(params.get("pageSize"), 50, 1, 200);
    let page = clamp_list_param(params.get("page"), 1, 1, 100_000);
    let offset = ((page - 1) * page_size) as u32;
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match custom_objects::list_objects(&conn, type_id, query.as_deref(), page_size as u32, offset) {
        Ok((objects, total)) => Json(json!({"objects": objects, "total": total})).into_response(),
        Err(e) => internal_500(&e.to_string()),
    }
}

pub async fn create_object(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let mut issues = Vec::new();
    let Some(type_id) = body.get("typeId").and_then(|v| v.as_i64()) else {
        issues.push("typeId must be a positive integer.".into());
        return bad_request(&issues);
    };
    if type_id <= 0 {
        issues.push("typeId must be a positive integer.".into());
        return bad_request(&issues);
    }
    let Some(title) = body
        .get("title")
        .and_then(|v| v.as_str())
        .and_then(|s| non_empty(s, 200))
    else {
        issues.push("title must be a non-empty string of at most 200 characters.".into());
        return bad_request(&issues);
    };
    let properties = match body.get("properties") {
        None => Map::new(),
        Some(Value::Object(m)) => m.clone(),
        Some(_) => {
            issues.push("properties must be an object.".into());
            return bad_request(&issues);
        }
    };
    let links = parse_links(&body, &mut issues)
        .unwrap_or_default()
        .unwrap_or_default();
    if !issues.is_empty() {
        return bad_request(&issues);
    }
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match custom_objects::create_object(&conn, type_id, &title, &properties, &links) {
        Ok(id) => {
            let _ = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("custom_object_created").with_after_state(json!({
                    "id": id, "type_id": type_id, "title": title
                })),
            );
            match custom_objects::get_object(&conn, id) {
                Ok(Some(object)) => Json(json!({"ok": true, "object": object})).into_response(),
                _ => internal_500("object vanished after create"),
            }
        }
        Err(crate::error::Error::Validation(m)) => validation_422(&m),
        Err(e) => internal_500(&e.to_string()),
    }
}

pub async fn get_object(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match custom_objects::get_object(&conn, id) {
        Ok(Some(object)) => Json(json!({"object": object})).into_response(),
        Ok(None) => not_found("Object not found."),
        Err(e) => internal_500(&e.to_string()),
    }
}

pub async fn update_object(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Response {
    let mut issues = Vec::new();
    let title = match body.get("title") {
        None => None,
        Some(v) => match v.as_str().and_then(|s| non_empty(s, 200)) {
            Some(t) => Some(t),
            None => {
                issues.push("title must be a non-empty string of at most 200 characters.".into());
                None
            }
        },
    };
    let properties = match body.get("properties") {
        None => None,
        Some(Value::Object(m)) => Some(m.clone()),
        Some(_) => {
            issues.push("properties must be an object.".into());
            None
        }
    };
    let links = parse_links(&body, &mut issues).flatten();
    if !issues.is_empty() {
        return bad_request(&issues);
    }
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match custom_objects::patch_object(
        &conn,
        id,
        title.as_deref(),
        properties.as_ref(),
        links.as_deref(),
    ) {
        Ok(Some(object)) => {
            let _ = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("custom_object_updated")
                    .with_after_state(json!({ "id": id })),
            );
            Json(json!({"ok": true, "object": object})).into_response()
        }
        Ok(None) => not_found("Object not found."),
        Err(crate::error::Error::Validation(m)) => validation_422(&m),
        Err(e) => internal_500(&e.to_string()),
    }
}

pub async fn delete_object(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match custom_objects::delete_object(&conn, id) {
        Ok(true) => {
            let _ = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("custom_object_deleted")
                    .with_before_state(json!({ "id": id })),
            );
            Json(json!({"ok": true, "message": "Object deleted."})).into_response()
        }
        Ok(false) => not_found("Object not found."),
        Err(e) => internal_500(&e.to_string()),
    }
}

pub async fn create_link(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Response {
    let links = match body.get("links").and_then(|v| v.as_array()) {
        Some(arr) if arr.len() == 1 => arr.clone(),
        _ => {
            return bad_request(&[
                "Body must be { links: [{ targetKind, targetLocalId, note? }] }.".to_string(),
            ])
        }
    };
    let mut issues = Vec::new();
    let parsed = parse_links(&json!({ "links": links }), &mut issues)
        .flatten()
        .unwrap_or_default();
    if !issues.is_empty() {
        return bad_request(&issues);
    }
    let Some(link) = parsed.first() else {
        return bad_request(&[
            "Body must be { links: [{ targetKind, targetLocalId, note? }] }.".to_string(),
        ]);
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    if custom_objects::get_object(&conn, id)
        .ok()
        .flatten()
        .is_none()
    {
        return not_found("Object not found.");
    }
    match custom_objects::add_link(&conn, id, link) {
        Ok(()) => match custom_objects::get_object(&conn, id) {
            Ok(Some(object)) => Json(json!({"ok": true, "object": object})).into_response(),
            _ => internal_500("object vanished after link"),
        },
        Err(crate::error::Error::Validation(m)) => validation_422(&m),
        Err(e) => internal_500(&e.to_string()),
    }
}

pub async fn delete_link(
    State(state): State<AppState>,
    Path((id, target_kind, target_local_id)): Path<(i64, String, i64)>,
) -> Response {
    if !LINK_TARGET_KINDS.contains(&target_kind.as_str()) {
        return bad_request(&["Unknown link target kind.".to_string()]);
    }
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match custom_objects::remove_link(&conn, id, &target_kind, target_local_id) {
        Ok(removed) => Json(json!({"ok": true, "removed": removed})).into_response(),
        Err(e) => internal_500(&e.to_string()),
    }
}

pub async fn list_for_target(
    State(state): State<AppState>,
    Path((target_kind, target_id)): Path<(String, i64)>,
) -> Response {
    if !LINK_TARGET_KINDS.contains(&target_kind.as_str()) {
        return bad_request(&["Unknown link target kind.".to_string()]);
    }
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match custom_objects::objects_for_target(&conn, &target_kind, target_id, 50) {
        Ok(objects) => Json(json!({"objects": objects})).into_response(),
        Err(e) => internal_500(&e.to_string()),
    }
}

pub async fn report(State(state): State<AppState>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match custom_objects::report(&conn) {
        Ok(r) => Json(json!({"report": r})).into_response(),
        Err(e) => internal_500(&e.to_string()),
    }
}
