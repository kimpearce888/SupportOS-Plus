use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use super::super::server::AppState;
use crate::incident_workspace::{IncidentListFilters, INCIDENT_SEVERITIES, INCIDENT_STATUSES};

fn notify_incident_update(
    conn: &rusqlite::Connection,
    bus: Option<&crate::http::EventBus>,
    incident_id: i64,
    title: &str,
    body: &str,
    severity: &str,
    actor_user_id: Option<i64>,
) {
    let Some(incident) = crate::intelligence_features::incident_row_json(conn, incident_id) else {
        return;
    };
    let code = incident
        .get("code")
        .and_then(|c| c.as_str())
        .unwrap_or("INC");
    let minute = chrono::Utc::now().format("%Y-%m-%dT%H:%M").to_string();
    let _ = crate::notifications::record_notification(
        conn,
        bus,
        &crate::notifications::NotificationInput {
            notification_type: spp_catalog::NotificationType::IncidentUpdate,
            severity: Some(severity),
            title: format!("{code}: {title}"),
            body: Some(body.to_string()),
            actor_user_local_id: actor_user_id,
            dedup_key: format!("incident_update:{incident_id}:{title}:{minute}"),
            ..Default::default()
        },
    );
}

pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    let status = params
        .get("status")
        .filter(|s| INCIDENT_STATUSES.contains(&s.as_str()))
        .cloned();
    let severity = params
        .get("severity")
        .filter(|s| INCIDENT_SEVERITIES.contains(&s.as_str()))
        .cloned();
    let open = matches!(
        params.get("open").map(String::as_str),
        Some("true") | Some("1")
    );
    let query = params
        .get("q")
        .map(|q| q.chars().take(120).collect::<String>());
    let page_size = clamp_list_param(params.get("pageSize"), 50, 1, 200);
    let page = clamp_list_param(params.get("page"), 1, 1, 100_000);
    let filters = IncidentListFilters {
        status,
        severity,
        open,
        query,
        limit: page_size,
        offset: (page - 1) * page_size,
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let (items, total) =
        crate::incident_workspace::list_incident_rows(&conn, &filters).unwrap_or_default();
    Json(json!({"incidents": items, "total": total}))
}

fn clamp_list_param(raw: Option<&String>, fallback: i64, min: i64, max: i64) -> i64 {
    raw.and_then(|v| v.parse::<i64>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(fallback)
}

pub async fn create(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let mut issues: Vec<String> = Vec::new();

    let title = match body.get("title") {
        Some(Value::String(t)) => {
            let trimmed = t.trim();
            if trimmed.is_empty() {
                issues.push("Title is required.".to_string());
                None
            } else if trimmed.chars().count() > 200 {
                issues.push("Title must be at most 200 characters.".to_string());
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        _ => {
            issues.push("Title is required.".to_string());
            None
        }
    };
    let status = match body.get("status") {
        None | Some(Value::Null) => Some("investigating".to_string()),
        Some(Value::String(s)) => {
            if INCIDENT_STATUSES.contains(&s.as_str()) {
                Some(s.clone())
            } else {
                issues.push(format!(
                    "Invalid enum value. Expected 'investigating' | 'identified' | 'fix_in_progress' | 'monitoring' | 'resolved', received '{s}'."
                ));
                None
            }
        }
        Some(_) => {
            issues.push("Expected string, received non-string.".to_string());
            None
        }
    };
    let severity = match body.get("severity") {
        None | Some(Value::Null) => Some("sev3".to_string()),
        Some(Value::String(s)) => {
            if INCIDENT_SEVERITIES.contains(&s.as_str()) {
                Some(s.clone())
            } else {
                issues.push(format!(
                    "Invalid enum value. Expected 'sev1' | 'sev2' | 'sev3' | 'sev4', received '{s}'."
                ));
                None
            }
        }
        Some(_) => {
            issues.push("Expected string, received non-string.".to_string());
            None
        }
    };

    fn bounded_text(
        body: &Value,
        key: &str,
        max: usize,
        issues: &mut Vec<String>,
    ) -> Option<Option<String>> {
        match body.get(key) {
            None | Some(Value::Null) => Some(None),
            Some(Value::String(s)) => {
                let trimmed = s.trim();
                if trimmed.chars().count() > max {
                    issues.push(format!("{key} must be at most {max} characters."));
                    None
                } else {
                    Some(Some(trimmed.to_string()))
                }
            }
            Some(_) => {
                issues.push(format!("Expected string, received non-string for {key}."));
                None
            }
        }
    }
    let owner_user_local_id = match body.get("ownerUserId") {
        None | Some(Value::Null) => Some(None),
        Some(Value::Number(n)) if n.as_i64().is_some_and(|v| v > 0) => Some(n.as_i64()),
        Some(_) => {
            issues.push("ownerUserId must be a positive integer or null.".to_string());
            None
        }
    };
    let product = bounded_text(&body, "product", 120, &mut issues);
    let feature = bounded_text(&body, "feature", 120, &mut issues);
    let description = bounded_text(&body, "description", 4000, &mut issues);
    let internal_explanation = bounded_text(&body, "internalExplanation", 8000, &mut issues);
    let customer_safe_explanation =
        bounded_text(&body, "customerSafeExplanation", 8000, &mut issues);
    let known_cause = bounded_text(&body, "knownCause", 4000, &mut issues);
    let workaround = bounded_text(&body, "workaround", 4000, &mut issues);
    let resolution = bounded_text(&body, "resolution", 4000, &mut issues);
    let started_at = match body.get("startedAt") {
        None | Some(Value::Null) => Some(None),
        Some(Value::String(s)) => {
            if is_iso_dateish(s) {
                Some(Some(s.clone()))
            } else {
                issues.push("startedAt must be an ISO 8601 date string or null.".to_string());
                None
            }
        }
        Some(_) => {
            issues.push("startedAt must be a string or null.".to_string());
            None
        }
    };
    let conversation_ids: Option<Vec<i64>> = match body.get("conversationIds") {
        None => Some(Vec::new()),
        Some(Value::Array(arr)) => {
            if arr.len() > 500 {
                issues.push("conversationIds must contain at most 500 items.".to_string());
                None
            } else {
                let mut ids = Vec::with_capacity(arr.len());
                let mut ok = true;
                for v in arr {
                    match v.as_i64() {
                        Some(id) if id > 0 => ids.push(id),
                        _ => {
                            issues.push(
                                "conversationIds must contain positive integers.".to_string(),
                            );
                            ok = false;
                            break;
                        }
                    }
                }
                if ok {
                    Some(ids)
                } else {
                    None
                }
            }
        }
        Some(_) => {
            issues.push("conversationIds must be an array.".to_string());
            None
        }
    };

    if !issues.is_empty() {
        let message: String = issues.join("; ");
        let message = message.chars().take(300).collect::<String>();
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"statusCode": 400, "error": "BadRequest", "message": message})),
        )
            .into_response();
    }

    let (
        title,
        status,
        severity,
        owner_user_local_id,
        product,
        feature,
        description,
        internal_explanation,
        customer_safe_explanation,
        known_cause,
        workaround,
        resolution,
        started_at,
        conversation_ids,
    ) = (
        title.unwrap(),
        status.unwrap(),
        severity.unwrap(),
        owner_user_local_id.flatten(),
        product.flatten(),
        feature.flatten(),
        description.flatten(),
        internal_explanation.flatten(),
        customer_safe_explanation.flatten(),
        known_cause.flatten(),
        workaround.flatten(),
        resolution.flatten(),
        started_at.flatten(),
        conversation_ids.unwrap_or_default(),
    );

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let mut valid_ids = Vec::with_capacity(conversation_ids.len());
    for id in &conversation_ids {
        let exists: Option<i64> = conn
            .query_row(
                "SELECT id FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .ok();
        if exists.is_some() {
            valid_ids.push(*id);
        }
    }

    let incident = crate::intelligence_features::ManualIncident {
        title,
        status,
        severity,
        owner_user_local_id,
        product,
        feature,
        description,
        internal_explanation,
        customer_safe_explanation,
        known_cause,
        workaround,
        resolution,
        started_at,
        conversation_ids: valid_ids,
    };
    match crate::intelligence_features::create_manual_incident(&conn, &incident) {
        Ok(id) => {
            let incident_json =
                crate::intelligence_features::incident_row_json(&conn, id).unwrap_or(json!({}));
            let _ = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("incident_created").with_after_state(json!({
                    "id": id,
                    "code": incident_json.get("code"),
                    "title": incident_json.get("title"),
                })),
            );
            let linked = conversation_ids.len();
            let body = if linked > 0 {
                format!(
                    "Severity {}, status {}, {linked} linked conversation(s).",
                    incident.severity, incident.status
                )
            } else {
                format!(
                    "Severity {}, status {}.",
                    incident.severity, incident.status
                )
            };
            notify_incident_update(
                &conn,
                Some(&state.bus),
                id,
                "incident declared",
                &body,
                "warning",
                owner_user_local_id,
            );
            (
                StatusCode::OK,
                Json(json!({"ok": true, "incident": incident_json})),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"ok": false, "message": format!("Could not create incident: {e}")})),
        )
            .into_response(),
    }
}

pub async fn get(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::incident_workspace::incident_detail(&conn, id) {
        Some(payload) => Json(payload).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Incident not found."
            })),
        )
            .into_response(),
    }
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Response {
    let mut issues: Vec<String> = Vec::new();
    let patch = parse_incident_patch(&body, &mut issues);
    if !issues.is_empty() {
        let message: String = issues.join("; ");
        let message = message.chars().take(300).collect::<String>();
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"statusCode": 400, "error": "BadRequest", "message": message})),
        )
            .into_response();
    }
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let actor_user_id = body.get("actorUserId").and_then(|v| v.as_i64());
    let before = crate::intelligence_features::incident_row_json(&conn, id);
    let Some(before) = before else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Incident not found."
            })),
        )
            .into_response();
    };
    let changed_columns = patch.changed_columns();
    match crate::intelligence_features::patch_incident(&conn, id, &patch, actor_user_id) {
        Ok(Some(after)) => {
            let _ = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("incident_updated").with_after_state(json!({
                    "id": id,
                    "changes": changed_columns,
                })),
            );
            if let Some(status) = &patch.status {
                let before_status = before.get("status").and_then(|v| v.as_str()).unwrap_or("");
                if status != before_status {
                    let body = if status == "resolved" {
                        "Incident resolved.".to_string()
                    } else {
                        format!("Status moved from {before_status} to {status}.")
                    };
                    let severity = if status == "resolved" {
                        "info"
                    } else {
                        "warning"
                    };
                    notify_incident_update(
                        &conn,
                        Some(&state.bus),
                        id,
                        &format!("status changed to {status}"),
                        &body,
                        severity,
                        actor_user_id,
                    );
                }
            }
            if let Some(severity) = &patch.severity {
                let before_severity = before
                    .get("severity")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if severity != before_severity {
                    notify_incident_update(
                        &conn,
                        Some(&state.bus),
                        id,
                        &format!("severity changed to {severity}"),
                        &format!("Severity moved from {before_severity} to {severity}."),
                        "critical",
                        actor_user_id,
                    );
                }
            }
            (StatusCode::OK, Json(json!({"ok": true, "incident": after}))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Incident not found."
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "message": format!("Could not update incident: {e}")})),
        )
            .into_response(),
    }
}

fn is_iso_dateish(s: &str) -> bool {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"^\d{4}-\d{2}-\d{2}(T[\d:.]+(Z|[+-]\d{2}:?\d{2})?)?$").unwrap()
    });
    s.chars().count() <= 32 && re.is_match(s)
}

fn parse_incident_patch(
    body: &Value,
    issues: &mut Vec<String>,
) -> crate::intelligence_features::IncidentPatch {
    use crate::intelligence_features::IncidentPatch;

    fn required_text(
        body: &Value,
        key: &str,
        max: usize,
        issues: &mut Vec<String>,
    ) -> Option<String> {
        match body.get(key) {
            None => None,
            Some(Value::String(s)) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    issues.push(format!("{key} must not be empty."));
                } else if trimmed.chars().count() > max {
                    issues.push(format!("{key} must be at most {max} characters."));
                } else {
                    return Some(trimmed.to_string());
                }
                None
            }
            Some(_) => {
                issues.push(format!("Expected string, received non-string for {key}."));
                None
            }
        }
    }

    fn trimmed_nullable(
        body: &Value,
        key: &str,
        max: usize,
        issues: &mut Vec<String>,
    ) -> Option<Option<String>> {
        match body.get(key) {
            None => None,
            Some(Value::Null) => Some(None),
            Some(Value::String(s)) => {
                if s.trim().chars().count() > max {
                    issues.push(format!("{key} must be at most {max} characters."));
                    None
                } else {
                    Some(Some(s.trim().to_string()))
                }
            }
            Some(_) => {
                issues.push(format!("Expected string, received non-string for {key}."));
                None
            }
        }
    }

    fn untrimmed_nullable(
        body: &Value,
        key: &str,
        max: usize,
        issues: &mut Vec<String>,
    ) -> Option<Option<String>> {
        match body.get(key) {
            None => None,
            Some(Value::Null) => Some(None),
            Some(Value::String(s)) => {
                if s.chars().count() > max {
                    issues.push(format!("{key} must be at most {max} characters."));
                    None
                } else {
                    Some(Some(s.clone()))
                }
            }
            Some(_) => {
                issues.push(format!("Expected string, received non-string for {key}."));
                None
            }
        }
    }

    let mut patch = IncidentPatch::default();
    patch.title = required_text(body, "title", 200, issues);
    if let Some(Value::String(s)) = body.get("status") {
        if INCIDENT_STATUSES.contains(&s.as_str()) {
            patch.status = Some(s.clone());
        } else {
            issues.push(format!(
                "Invalid enum value. Expected 'investigating' | 'identified' | 'fix_in_progress' | 'monitoring' | 'resolved', received '{s}'."
            ));
        }
    } else if let Some(other) = body.get("status") {
        if !other.is_null() {
            issues.push("Expected string, received non-string for status.".to_string());
        }
    }
    if let Some(Value::String(s)) = body.get("severity") {
        if INCIDENT_SEVERITIES.contains(&s.as_str()) {
            patch.severity = Some(s.clone());
        } else {
            issues.push(format!(
                "Invalid enum value. Expected 'sev1' | 'sev2' | 'sev3' | 'sev4', received '{s}'."
            ));
        }
    } else if let Some(other) = body.get("severity") {
        if !other.is_null() {
            issues.push("Expected string, received non-string for severity.".to_string());
        }
    }
    patch.owner_user_local_id = match body.get("ownerUserId") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::Number(n)) if n.as_i64().is_some_and(|v| v > 0) => Some(n.as_i64()),
        Some(_) => {
            issues.push("ownerUserId must be a positive integer or null.".to_string());
            None
        }
    };
    patch.product = trimmed_nullable(body, "product", 120, issues);
    patch.feature = trimmed_nullable(body, "feature", 120, issues);
    patch.description = untrimmed_nullable(body, "description", 4000, issues);
    patch.internal_explanation = untrimmed_nullable(body, "internalExplanation", 8000, issues);
    patch.customer_safe_explanation =
        untrimmed_nullable(body, "customerSafeExplanation", 8000, issues);
    patch.known_cause = untrimmed_nullable(body, "knownCause", 4000, issues);
    patch.workaround = untrimmed_nullable(body, "workaround", 4000, issues);
    patch.resolution = untrimmed_nullable(body, "resolution", 4000, issues);
    patch.started_at = match body.get("startedAt") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(s)) => {
            if is_iso_dateish(s) {
                Some(Some(s.clone()))
            } else {
                issues.push("Dates must be ISO (YYYY-MM-DD or with time).".to_string());
                None
            }
        }
        Some(_) => {
            issues.push("Expected string, received non-string for startedAt.".to_string());
            None
        }
    };
    patch
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let deleted = conn
        .execute("DELETE FROM incidents WHERE id = ?1", rusqlite::params![id])
        .map(|n| n > 0)
        .unwrap_or(false);
    if !deleted {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Incident not found."
            })),
        )
            .into_response();
    }
    let _ = crate::audit::audit(
        &conn,
        &crate::audit::AuditEntry::user("incident_deleted").with_after_state(json!({ "id": id })),
    );
    (
        StatusCode::OK,
        Json(json!({"ok": true, "message": "Incident deleted."})),
    )
        .into_response()
}

pub async fn link_conversation(
    State(state): State<AppState>,
    Path((id, conversation_id)): Path<(i64, i64)>,
) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let incident_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM incidents WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !incident_exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Incident not found."
            })),
        )
            .into_response();
    }
    let conversation: Option<i64> = conn
        .query_row(
            "SELECT number FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![conversation_id],
            |r| r.get(0),
        )
        .ok();
    let Some(number) = conversation else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Conversation not found."
            })),
        )
            .into_response();
    };
    let actor_user_id: Option<i64> = None;
    let created = crate::intelligence_features::link_incident_conversation(
        &conn,
        id,
        conversation_id,
        "human",
    )
    .unwrap_or(false);
    if created {
        let body = format!("Conversation #{number} is now counted in this incident.");
        notify_incident_update(
            &conn,
            Some(&state.bus),
            id,
            "conversation linked",
            &body,
            "info",
            actor_user_id,
        );
    }
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "linked": created,
            "message": if created {
                "Conversation linked."
            } else {
                "Conversation was already linked."
            },
        })),
    )
        .into_response()
}

pub async fn add_note(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Response {
    let note = body
        .get("body")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");
    if note.is_empty() || note.chars().count() > 4000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "statusCode": 400,
                "error": "BadRequest",
                "message": "A non-empty note body is required."
            })),
        )
            .into_response();
    }
    let author = body.get("authorUserId").and_then(|v| v.as_i64());
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let incident_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM incidents WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !incident_exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Incident not found."
            })),
        )
            .into_response();
    }
    match crate::intelligence_features::add_incident_note(&conn, id, author, note) {
        Ok(note_id) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "note": crate::incident_workspace::incident_note_json(&conn, note_id)
                    .unwrap_or_else(|| json!({"id": note_id, "body": note}))
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "message": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn add_ref(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Response {
    let system = body.get("system").and_then(|v| v.as_str()).map(str::trim);
    let reference = body
        .get("reference")
        .and_then(|v| v.as_str())
        .map(str::trim);
    let (Some(system), Some(reference)) = (system, reference) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "statusCode": 400,
                "error": "BadRequest",
                "message": "Both a reference system and a reference id are required."
            })),
        )
            .into_response();
    };
    if system.is_empty()
        || system.chars().count() > 60
        || reference.is_empty()
        || reference.chars().count() > 200
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "statusCode": 400,
                "error": "BadRequest",
                "message": "Invalid reference system or id."
            })),
        )
            .into_response();
    }
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let incident_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM incidents WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !incident_exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Incident not found."
            })),
        )
            .into_response();
    }
    match crate::intelligence_features::add_incident_ref(&conn, id, system, reference) {
        Ok(ref_id) => (StatusCode::OK, Json(json!({"ok": true, "ref_id": ref_id}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "message": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn add_release(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Response {
    let label = body
        .get("versionLabel")
        .and_then(|v| v.as_str())
        .map(str::trim);
    let Some(label) = label.filter(|l| !l.is_empty()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "statusCode": 400,
                "error": "BadRequest",
                "message": "A version label is required."
            })),
        )
            .into_response();
    };
    if label.chars().count() > 120 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "statusCode": 400,
                "error": "BadRequest",
                "message": "Version label must be at most 120 characters."
            })),
        )
            .into_response();
    }
    let released_at = match body.get("releasedAt") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.trim().is_empty() => None,
        Some(Value::String(s)) if is_iso_dateish(s) => Some(s.clone()),
        Some(Value::String(_)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "statusCode": 400,
                    "error": "BadRequest",
                    "message": "Dates must be ISO (YYYY-MM-DD or with time)."
                })),
            )
                .into_response();
        }
        Some(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "statusCode": 400,
                    "error": "BadRequest",
                    "message": "releasedAt must be a string or null."
                })),
            )
                .into_response();
        }
    };
    let notes = body
        .get("notes")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let incident_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM incidents WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !incident_exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Incident not found."
            })),
        )
            .into_response();
    }
    match crate::intelligence_features::add_incident_release(
        &conn,
        id,
        label,
        released_at.as_deref(),
        notes,
    ) {
        Ok(0) => (
            StatusCode::OK,
            Json(json!({
                "ok": false,
                "message": "That version label is already attached to this incident."
            })),
        )
            .into_response(),
        Ok(release_id) => (
            StatusCode::OK,
            Json(json!({"ok": true, "release_id": release_id})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "message": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn delete_release(
    State(state): State<AppState>,
    Path((id, release_id)): Path<(i64, i64)>,
) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::intelligence_features::delete_incident_release(&conn, id, release_id) {
        Ok(true) => (
            StatusCode::OK,
            Json(json!({"ok": true, "message": "Release removed."})),
        )
            .into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Release not found on this incident."
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "message": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn add_related(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Response {
    let Some(kind) = body.get("targetKind").and_then(|v| v.as_str()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "statusCode": 400,
                "error": "BadRequest",
                "message": "targetKind is required."
            })),
        )
            .into_response();
    };
    let table = match kind {
        "known_issue" => "known_issues",
        "knowledge_doc" => "knowledge_documents",
        "campaign" => "outreach_campaigns",
        "custom_object" => "custom_objects",
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "statusCode": 400,
                    "error": "BadRequest",
                    "message": "Invalid targetKind."
                })),
            )
                .into_response();
        }
    };
    let Some(target_local_id) = body
        .get("targetLocalId")
        .and_then(|v| v.as_i64())
        .filter(|v| *v > 0)
    else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "statusCode": 400,
                "error": "BadRequest",
                "message": "targetLocalId must be a positive integer."
            })),
        )
            .into_response();
    };
    let note = body
        .get("note")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let incident_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM incidents WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !incident_exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Incident not found."
            })),
        )
            .into_response();
    }
    let target_exists: bool = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE id = ?1"),
            rusqlite::params![target_local_id],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !target_exists {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": format!("{kind} #{target_local_id} does not exist.")
            })),
        )
            .into_response();
    }
    match crate::intelligence_features::add_incident_related(&conn, id, kind, target_local_id, note)
    {
        Ok(linked) => (StatusCode::OK, Json(json!({"ok": true, "linked": linked}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "message": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn delete_related(
    State(state): State<AppState>,
    Path((id, target_kind, target_local_id)): Path<(i64, String, i64)>,
) -> Response {
    if !matches!(
        target_kind.as_str(),
        "known_issue" | "knowledge_doc" | "campaign" | "custom_object"
    ) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "statusCode": 400,
                "error": "BadRequest",
                "message": "Unknown related target kind."
            })),
        )
            .into_response();
    }
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::intelligence_features::delete_incident_related(
        &conn,
        id,
        &target_kind,
        target_local_id,
    ) {
        Ok(removed) => (
            StatusCode::OK,
            Json(json!({"ok": true, "removed": removed})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "message": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn impact(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::issue_impact::for_incident(&conn, id) {
        Some(impact) => (StatusCode::OK, Json(json!({"impact": impact}))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Incident not found."
            })),
        )
            .into_response(),
    }
}

struct IncidentOverrides {
    title: Option<String>,
    severity: Option<String>,
    status: Option<String>,
    description: Option<String>,
}

fn parse_incident_overrides(body: Option<Json<Value>>) -> Option<IncidentOverrides> {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let title = match body.get("title") {
        None | Some(Value::Null) => None,
        Some(Value::String(t)) => {
            let t = t.trim();
            if t.is_empty() || t.chars().count() > 200 {
                return None;
            }
            Some(t.to_string())
        }
        Some(_) => return None,
    };
    let status = match body.get("status") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => {
            if !INCIDENT_STATUSES.contains(&s.as_str()) {
                return None;
            }
            Some(s.clone())
        }
        Some(_) => return None,
    };
    let severity = match body.get("severity") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => {
            if !INCIDENT_SEVERITIES.contains(&s.as_str()) {
                return None;
            }
            Some(s.clone())
        }
        Some(_) => return None,
    };
    let description = match body.get("description") {
        None | Some(Value::Null) => None,
        Some(Value::String(d)) => {
            if d.chars().count() > 4000 {
                return None;
            }
            Some(d.clone())
        }
        Some(_) => return None,
    };
    Some(IncidentOverrides {
        title,
        severity,
        status,
        description,
    })
}

pub async fn from_cluster(
    State(state): State<AppState>,
    Path(cluster_id): Path<i64>,
    body: Option<Json<Value>>,
) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let cluster: Option<(String, Option<i64>)> = conn
        .query_row(
            "SELECT name, known_issue_id FROM issue_clusters WHERE id = ?1",
            rusqlite::params![cluster_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let Some((cluster_name, cluster_known_issue_id)) = cluster else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Issue cluster not found."
            })),
        )
            .into_response();
    };
    let conversation_ids =
        crate::intelligence_features::cluster_conversation_ids(&conn, cluster_id);
    let overrides = parse_incident_overrides(body);
    let title = overrides
        .as_ref()
        .and_then(|o| o.title.clone())
        .unwrap_or_else(|| format!("Issue cluster: {cluster_name}"));
    let severity = overrides
        .as_ref()
        .and_then(|o| o.severity.clone())
        .unwrap_or_else(|| "sev3".into());
    let status = overrides
        .as_ref()
        .and_then(|o| o.status.clone())
        .unwrap_or_else(|| "investigating".into());
    let description = overrides
        .as_ref()
        .and_then(|o| o.description.clone())
        .unwrap_or_else(|| format!("Declared from issue cluster \"{cluster_name}\"."));
    let incident_id = match crate::intelligence_features::create_incident_from_source(
        &conn,
        cluster_known_issue_id,
        &severity,
        &status,
        "cluster",
        &title,
        Some(&description),
        &conversation_ids,
    ) {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"ok": false, "message": format!("Could not create incident: {e}")})),
            )
                .into_response();
        }
    };
    let incident = crate::intelligence_features::incident_row_json(&conn, incident_id)
        .unwrap_or_else(|| json!({}));
    let _ = crate::audit::audit(
        &conn,
        &crate::audit::AuditEntry::user("incident_created_from_cluster").with_after_state(json!({
            "id": incident_id,
            "code": incident.get("code"),
            "cluster_id": cluster_id,
        })),
    );
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "incident": incident,
            "linked_conversations": conversation_ids.len(),
        })),
    )
        .into_response()
}

pub async fn from_known_issue(
    State(state): State<AppState>,
    Path(known_issue_id): Path<i64>,
    body: Option<Json<Value>>,
) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let known_issue: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT name, description FROM known_issues WHERE id = ?1",
            rusqlite::params![known_issue_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let Some((issue_name, issue_description)) = known_issue else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Known issue not found."
            })),
        )
            .into_response();
    };
    let conversation_ids =
        crate::intelligence_features::known_issue_conversation_ids(&conn, known_issue_id);
    let overrides = parse_incident_overrides(body);
    let title = overrides
        .as_ref()
        .and_then(|o| o.title.clone())
        .unwrap_or_else(|| format!("Known issue: {issue_name}"));
    let severity = overrides
        .as_ref()
        .and_then(|o| o.severity.clone())
        .unwrap_or_else(|| "sev3".into());
    let status = overrides
        .as_ref()
        .and_then(|o| o.status.clone())
        .unwrap_or_else(|| "investigating".into());
    let description = overrides
        .as_ref()
        .and_then(|o| o.description.clone())
        .unwrap_or_else(|| match issue_description.as_deref().map(str::trim) {
            Some(d) if !d.is_empty() => {
                format!("Declared from known issue \"{issue_name}\". {d}")
            }
            _ => format!("Declared from known issue \"{issue_name}\"."),
        });
    let incident_id = match crate::intelligence_features::create_incident_from_source(
        &conn,
        Some(known_issue_id),
        &severity,
        &status,
        "known_issue",
        &title,
        Some(&description),
        &conversation_ids,
    ) {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"ok": false, "message": format!("Could not create incident: {e}")})),
            )
                .into_response();
        }
    };
    let _ = crate::intelligence_features::add_incident_related(
        &conn,
        incident_id,
        "known_issue",
        known_issue_id,
        Some("Declared from this known issue"),
    );
    let incident = crate::intelligence_features::incident_row_json(&conn, incident_id)
        .unwrap_or_else(|| json!({}));
    let _ = crate::audit::audit(
        &conn,
        &crate::audit::AuditEntry::user("incident_created_from_known_issue").with_after_state(
            json!({
                "id": incident_id,
                "code": incident.get("code"),
                "known_issue_id": known_issue_id,
            }),
        ),
    );
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "incident": incident,
            "linked_conversations": conversation_ids.len(),
        })),
    )
        .into_response()
}

pub async fn unlink_conversation(
    State(state): State<AppState>,
    Path((id, conversation_id)): Path<(i64, i64)>,
) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM incidents WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if exists == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Incident not found."
            })),
        )
            .into_response();
    }
    let removed =
        crate::intelligence_features::unlink_incident_conversation(&conn, id, conversation_id)
            .unwrap_or(false);
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "removed": removed,
            "message": if removed {
                "Conversation unlinked."
            } else {
                "Conversation was not linked."
            },
        })),
    )
        .into_response()
}

pub async fn delete_ref(
    State(state): State<AppState>,
    Path((id, ref_id)): Path<(i64, i64)>,
) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let removed =
        crate::intelligence_features::delete_incident_ref(&conn, id, ref_id).unwrap_or(false);
    if !removed {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Reference not found on this incident."
            })),
        )
            .into_response();
    }
    (
        StatusCode::OK,
        Json(json!({"ok": true, "message": "Reference removed."})),
    )
        .into_response()
}
