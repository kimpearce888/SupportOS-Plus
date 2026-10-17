use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use super::super::server::AppState;
use super::parse_json_body;
use axum::response::IntoResponse;

fn eval_rejected(msg: &str) -> (StatusCode, Json<Value>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({ "ok": false, "message": msg })),
    )
}

fn db_error_500(e: &rusqlite::Error) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "statusCode": 500,
            "error": "InternalError",
            "message": e.to_string()
        })),
    )
        .into_response()
}

fn clamp_list_param(raw: Option<&String>, fallback: i64, min: i64, max: i64) -> i64 {
    match raw {
        Some(s) if !s.is_empty() => s
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(|n| (n.trunc() as i64).clamp(min, max))
            .unwrap_or(fallback),
        _ => fallback,
    }
}

pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());

    let mut filters = crate::inbox::InboxFilters::default();
    if let Some(status) = params.get("view") {
        filters.status = Some(status.clone());
    }
    if let Some(priority) = params.get("priority") {
        filters.priority = Some(priority.clone());
    }
    if let Some(query) = params.get("q") {
        filters.query = Some(query.clone());
    }
    if let Some(channel) = params.get("channel").filter(|c| !c.is_empty()) {
        if channel != "email" && channel != "chat" {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": "channel must be 'email' or 'chat'."
                })),
            );
        }
        filters.channel = Some(channel.clone());
    }
    if let Some(tag) = params.get("tag").filter(|t| !t.is_empty()) {
        filters.tag = Some(tag.clone());
    }
    if let Some(number) = params.get("number") {
        filters.number = number.trim().parse::<i64>().ok();
    }
    let page = clamp_list_param(params.get("page"), 1, 1, 100000);
    let page_size = clamp_list_param(params.get("pageSize"), 50, 1, 100);
    filters.limit = Some(page_size as u32);
    filters.offset = Some(((page - 1) * page_size).max(0) as u32);
    if let Some(mailbox_id) = params
        .get("mailboxId")
        .map(|m| m.trim())
        .filter(|m| !m.is_empty())
        .and_then(|m| m.parse::<i64>().ok())
    {
        filters.mailbox_id = Some(mailbox_id);
    }
    let mut notes: Vec<String> = Vec::new();
    if let Some(ops) = params.get("ops").filter(|ops| !ops.is_empty()) {
        if !crate::operations::is_conversation_ops_tile(ops) {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": crate::operations::OPS_INVALID_MESSAGE,
                })),
            );
        }
        let threshold = crate::operations::waiting_threshold_minutes(&conn);
        let Some((frag_sql, frag_params)) = crate::operations::tile_fragment(ops, threshold) else {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": crate::operations::OPS_INVALID_MESSAGE,
                })),
            );
        };
        filters.extra_where = Some(frag_sql);
        filters.extra_params = frag_params;
        notes.push(format!("Operations Center tile '{ops}' applied."));
    }

    if let Some(raw_id) = params.get("savedViewId").filter(|v| !v.is_empty()) {
        let Ok(id) = raw_id.parse::<i64>() else {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": "Invalid",
                    "detail": ["savedViewId: Invalid"],
                })),
            );
        };
        let saved = match crate::saved_views::get_view(&conn, id) {
            Ok(Some(v)) => v,
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "statusCode": 404,
                        "error": "NotFound",
                        "message": "Saved view not found.",
                    })),
                );
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "statusCode": 500,
                        "error": "InternalError",
                        "message": e.to_string(),
                    })),
                );
            }
        };
        let tz = crate::saved_views::resolve_timezone(
            params.get("timezone").map(|s| s.as_str()),
            Some(
                crate::settings::get_string(&conn, "display_timezone")
                    .ok()
                    .flatten()
                    .unwrap_or_default()
                    .as_str(),
            ),
        );
        let alerts = crate::sla::sla_alerts(&conn).ok();
        let engine = crate::saved_views::ViewEngine::new(&tz).with_sla_resolver(Box::new(
            move |states: &[&str]| {
                alerts
                    .as_ref()
                    .map(|a| {
                        a.alerts
                            .iter()
                            .filter(|alert| states.contains(&alert.state.as_str()))
                            .map(|alert| alert.conversation_id)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            },
        ));
        match engine.compile(&saved.definition) {
            Ok(compiled) => {
                if compiled.where_sql != "1=1" {
                    filters.extra_where = Some(compiled.where_sql);
                    filters.extra_params = compiled.params;
                }
                notes.push(format!(
                    "Saved view '{}' (v{}) applied.",
                    saved.name, saved.version
                ));
                notes.extend(compiled.notes);
            }
            Err(e) => {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422,
                        "error": "ValidationError",
                        "message": format!(
                            "Saved view '{}' could not be evaluated: {}",
                            saved.name, e.message
                        ),
                    })),
                );
            }
        }
    }

    if let Some(attribute) = params.get("aiAttribute").filter(|v| !v.is_empty()) {
        let value = match params.get("aiAttrValue") {
            Some(v) if v.is_empty() => {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422,
                        "error": "ValidationError",
                        "message": "String must contain at least 1 character(s)",
                        "detail": ["aiAttrValue: String must contain at least 1 character(s)"],
                    })),
                );
            }
            Some(v) if v.chars().count() > 120 => {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422,
                        "error": "ValidationError",
                        "message": "String must contain at most 120 character(s)",
                        "detail": ["aiAttrValue: String must contain at most 120 character(s)"],
                    })),
                );
            }
            Some(v) => v.clone(),
            None => {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422,
                        "error": "ValidationError",
                        "message": "aiAttribute requires aiAttrValue (use 'unknown' to find tickets without a value).",
                    })),
                );
            }
        };
        let op = params
            .get("aiAttrOp")
            .map(|s| s.as_str())
            .unwrap_or("equals");
        const AI_ATTR_FILTER_OPS: [&str; 8] = [
            "equals",
            "not_equals",
            "contains",
            "not_contains",
            "gt",
            "gte",
            "lt",
            "lte",
        ];
        if !AI_ATTR_FILTER_OPS.contains(&op) {
            let expected = AI_ATTR_FILTER_OPS
                .iter()
                .map(|v| format!("'{v}'"))
                .collect::<Vec<_>>()
                .join(" | ");
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": format!(
                        "Invalid enum value. Expected {expected}, received '{op}'"
                    ),
                    "detail": [format!("aiAttrOp: Invalid enum value. Expected {expected}, received '{op}'")],
                })),
            );
        }
        let engine = crate::saved_views::ViewEngine::new("UTC");
        let definition = crate::saved_views::ViewDefinition {
            combinator: "all".into(),
            conditions: vec![crate::saved_views::ViewNode::AiAttribute {
                attribute: attribute.clone(),
                op: op.to_string(),
                value: value.clone(),
            }],
        };
        match engine.compile(&definition) {
            Ok(compiled) => {
                if compiled.where_sql != "1=1" {
                    filters.extra_where = Some(match filters.extra_where.take() {
                        Some(existing) => format!("({existing}) AND ({})", compiled.where_sql),
                        None => compiled.where_sql,
                    });
                    filters.extra_params.extend(compiled.params);
                }
                notes.push(format!(
                    "AI attribute filter: {attribute} {op} \"{value}\" (local layer; missing values read as unknown)."
                ));
            }
            Err(e) => {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422,
                        "error": "ValidationError",
                        "message": format!("AI attribute filter rejected: {}", e.message),
                    })),
                );
            }
        }
    }

    match crate::inbox::list_conversations(&conn, &filters) {
        Ok((items, total)) => {
            let items_json: Vec<Value> = items
                .iter()
                .filter_map(|i| serde_json::to_value(i).ok())
                .collect();
            (
                StatusCode::OK,
                Json(json!({
                    "conversations": items_json,
                    "total": total,
                    "page": page,
                    "page_size": page_size,
                    "view": params.get("view").cloned().unwrap_or_default(),
                    "notes": notes,
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"statusCode": 500, "error": "InternalError", "message": e.to_string()})),
        ),
    }
}

pub async fn get(State(state): State<AppState>, Path(id): Path<i64>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::inbox::get_conversation(&conn, id) {
        Ok(Some(detail)) => match serde_json::to_value(&detail) {
            Ok(mut v) => {
                if let Some(threads) = v["thread"].as_array_mut() {
                    for t in threads {
                        if let Some(body) = t.get("body").and_then(|b| b.as_str()) {
                            t["body"] = serde_json::Value::String(
                                crate::security::sanitize_thread_html(body),
                            );
                        }
                    }
                }
                let ticket_state = crate::ticket_states::get_conversation_state(&conn, id)
                    .and_then(|s| serde_json::to_value(s).ok())
                    .unwrap_or(serde_json::Value::Null);
                let state_history =
                    crate::ticket_states::list_transitions(&conn, id, 50).unwrap_or_default();
                let state_lifecycle =
                    crate::ticket_states::state_lifecycle(&conn, id).unwrap_or(serde_json::json!({
                        "current_state": serde_json::Value::Null,
                        "time_in_current_state_min": serde_json::Value::Null,
                        "transitions": 0,
                        "per_state": [],
                    }));
                let ticket_states: Vec<serde_json::Value> =
                    crate::ticket_states::list_states(&conn)
                        .iter()
                        .filter_map(|s| serde_json::to_value(s).ok())
                        .collect();
                v["activity"] = serde_json::json!({
                    "ticket_state": ticket_state,
                    "state_history": state_history,
                    "state_lifecycle": state_lifecycle,
                });
                v["ticket_states"] = serde_json::Value::Array(ticket_states);
                (StatusCode::OK, Json(v))
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    json!({"statusCode": 500, "error": "InternalError", "message": e.to_string()}),
                ),
            ),
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(
                json!({"statusCode": 404, "error": "NotFound", "message": "Conversation not found locally."}),
            ),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"statusCode": 500, "error": "InternalError", "message": e.to_string()})),
        ),
    }
}

pub async fn reply(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (text, draft, cc, bcc, status_after, assign_to, ai_draft_id, original_ai_text) =
        match parse_reply_body(&body) {
            Ok(v) => v,
            Err(r) => return r,
        };
    crate::conversation_ops::op_send_reply(
        &state,
        crate::conversation_ops::ReplyInput {
            conversation_id,
            text,
            draft,
            cc,
            bcc,
            status_after,
            assign_to,
            ai_draft_id,
            original_ai_text,
        },
    )
    .await
}

pub async fn note(State(state): State<AppState>, Path(id): Path<String>, body: Bytes) -> Response {
    let body = match parse_json_body(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let text = match parse_text_min1(&body, "text") {
        Ok(v) => v,
        Err(r) => return r,
    };
    crate::conversation_ops::op_add_note(&state, conversation_id, &text, false).await
}

pub async fn status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let body = match parse_json_body(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    const STATUSES: [&str; 4] = ["active", "closed", "pending", "spam"];
    let new_status = match body.get("status") {
        Some(Value::String(s)) => {
            if !STATUSES.contains(&s.as_str()) {
                return zod_422("status", &zod_enum_message(&STATUSES, s));
            }
            s.clone()
        }
        None | Some(Value::Null) => return zod_422("status", "Required"),
        Some(v) => {
            return zod_422(
                "status",
                &format!("Expected string, received {}", zod_type_name(v)),
            )
        }
    };
    crate::conversation_ops::op_change_status(&state, conversation_id, &new_status).await
}

pub async fn assign(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let body = match parse_json_body(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let user_id = match parse_required_nullable_int(&body, "userId") {
        Ok(v) => v,
        Err(r) => return r,
    };
    crate::conversation_ops::op_assign(&state, conversation_id, user_id).await
}

pub async fn priority(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let body = match parse_json_body(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    const PRIORITIES: [&str; 5] = ["none", "low", "medium", "high", "urgent"];
    let priority_str = match body.get("priority") {
        Some(Value::String(s)) => {
            if !PRIORITIES.contains(&s.as_str()) {
                return zod_422("priority", &zod_enum_message(&PRIORITIES, s));
            }
            s.clone()
        }
        None | Some(Value::Null) => return zod_422("priority", "Required"),
        Some(v) => {
            return zod_422(
                "priority",
                &format!("Expected string, received {}", zod_type_name(v)),
            )
        }
    };
    let mut conn = state.conn_lock();
    let Some(conv) = crate::conversation_ops::conv_full_by_local_id(&conn, conversation_id) else {
        return rejected("Conversation not found locally.");
    };
    let previous: Option<String> = conn
        .query_row(
            "SELECT supportos_priority FROM conversations WHERE id = ?1",
            rusqlite::params![conv.id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let tx = match conn.transaction() {
        Ok(tx) => tx,
        Err(e) => return db_error_500(&e),
    };
    if let Err(e) = tx.execute(
        "UPDATE conversations SET supportos_priority = ?1 WHERE id = ?2",
        rusqlite::params![priority_str, conv.id],
    ) {
        return db_error_500(&e);
    }
    if let Err(e) = tx.execute(
        "INSERT OR IGNORE INTO activity_events (conversation_id, event_type,
             actor_type, actor_id, occurred_at, source, metadata, dedup_key)
         VALUES (?1, 'priority_changed', 'user', NULL, ?2, 'local', ?3,
             'priority_changed:' || ?1 || ':' || ?2)",
        rusqlite::params![
            conv.id,
            at,
            json!({ "previous": previous, "next": priority_str, "hs_field_synced": false })
                .to_string()
        ],
    ) {
        return db_error_500(&e);
    }
    if let Err(e) = tx.commit() {
        return db_error_500(&e);
    }
    let _ = crate::jobs::audit(
        &conn,
        "user",
        "priority_changed",
        Some(conv.id),
        Some(&json!({ "priority": previous }).to_string()),
        Some(&json!({ "priority": priority_str, "hs_field_synced": false }).to_string()),
        None,
        None,
        false,
    );
    drop(conn);
    crate::http::event_bus::notify_conversation_updated(
        &state.bus,
        &crate::events::ConversationUpdatedEvent {
            conversation_id: Some(conv.id),
            conversation_number: None,
            mailbox_id: None,
            subject: None,
            reason: "sync".into(),
            at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
        },
    );
    ok_result(
        &format!("Priority set to {priority_str}."),
        Some(json!({ "priority": priority_str, "hs_field_synced": false })),
    )
}

pub async fn subject(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let new_subject = match parse_text_min1(&body, "subject") {
        Ok(v) => v,
        Err(r) => return r,
    };
    crate::conversation_ops::op_change_subject(&state, conversation_id, &new_subject).await
}

pub async fn set_state(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let state_id: Option<i64> = match body.get("stateId") {
        None => return zod_422("stateId", "Required"),
        Some(Value::Null) => None,
        Some(v) => match zod_int_value(v) {
            Ok(n) if n > 0 => Some(n),
            Ok(_) => return zod_422("stateId", "Number must be greater than 0"),
            Err(m) => return zod_422("stateId", &m),
        },
    };
    let reason: Option<String> = match body.get("reason") {
        None => None,
        Some(Value::String(s)) => {
            if s.chars().count() > 500 {
                return zod_422("reason", "String must contain at most 500 character(s)");
            }
            Some(s.clone())
        }
        Some(Value::Null) => return zod_422("reason", "Expected string, received null"),
        Some(v) => {
            return zod_422(
                "reason",
                &format!("Expected string, received {}", zod_type_name(v)),
            )
        }
    };
    let mut conn = state.conn_lock();
    let Some(conv) = crate::conversation_ops::conv_full_by_local_id(&conn, conversation_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Conversation not found locally."
            })),
        )
            .into_response();
    };
    let current: Option<i64> = conn
        .query_row(
            "SELECT supportos_state_id FROM conversations WHERE id = ?1",
            rusqlite::params![conv.id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let state_name: Option<String> = match state_id {
        Some(sid) => conn
            .query_row(
                "SELECT name FROM ticket_states WHERE id = ?1",
                rusqlite::params![sid],
                |r| r.get(0),
            )
            .ok(),
        None => None,
    };
    if state_id.is_some() && state_name.is_none() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "ok": false,
                "message": "State not found.",
                "previousStateId": current,
            })),
        )
            .into_response();
    }
    if current == state_id {
        return (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "message": "Conversation already in this state.",
                "previousStateId": current,
            })),
        )
            .into_response();
    }
    let occurred_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let tx = match conn.transaction() {
        Ok(tx) => tx,
        Err(e) => return db_error_500(&e),
    };
    if let Err(e) = tx.execute(
        "INSERT INTO state_transitions (conversation_id, previous_state_id,
             new_state_id, actor_type, actor_local_id, reason, occurred_at, source)
         VALUES (?1, ?2, ?3, 'user', NULL, ?4, ?5, 'local')",
        rusqlite::params![conv.id, current, state_id, reason, occurred_at],
    ) {
        return db_error_500(&e);
    }
    let transition_rowid = tx.last_insert_rowid();
    if let Err(e) = tx.execute(
        "UPDATE conversations SET supportos_state_id = ?1 WHERE id = ?2",
        rusqlite::params![state_id, conv.id],
    ) {
        return db_error_500(&e);
    }
    if let Err(e) = tx.execute(
        "INSERT OR IGNORE INTO activity_events (conversation_id, event_type,
             actor_type, actor_id, occurred_at, source, metadata, dedup_key)
         VALUES (?1, 'ticket_state_changed', 'user', NULL, ?2, 'local', ?3,
             'ticket_state_changed:' || ?1 || ':' || ?4)",
        rusqlite::params![
            conv.id,
            occurred_at,
            json!({
                "previous_state_id": current,
                "new_state_id": state_id,
                "reason": reason,
            })
            .to_string(),
            transition_rowid
        ],
    ) {
        return db_error_500(&e);
    }
    if let Err(e) = tx.commit() {
        return db_error_500(&e);
    }
    let _ = crate::jobs::audit(
        &conn,
        "user",
        "ticket_state_changed",
        Some(conv.id),
        Some(&json!({ "state_id": current }).to_string()),
        Some(&json!({ "state_id": state_id, "reason": reason }).to_string()),
        None,
        None,
        false,
    );
    drop(conn);
    crate::http::event_bus::notify_conversation_updated(
        &state.bus,
        &crate::events::ConversationUpdatedEvent {
            conversation_id: Some(conv.id),
            conversation_number: None,
            mailbox_id: None,
            subject: None,
            reason: "sync".into(),
            at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
        },
    );
    let message = match (&state_name, state_id) {
        (Some(name), _) => format!("State set to {name}."),
        (None, Some(_)) => "State set.".to_string(),
        (None, None) => "State cleared.".to_string(),
    };
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "message": message,
            "previousStateId": current,
        })),
    )
        .into_response()
}

const ZOD_EMAIL_RE: &str =
    r"^([A-Za-z0-9_'+\-\.]*)[A-Za-z0-9_+-]@([A-Za-z0-9][A-Za-z0-9\-]*\.)+[A-Za-z]{2,}$";

fn zod_email_ok(email: &str) -> bool {
    if email.starts_with('.') || email.contains("..") {
        return false;
    }
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(ZOD_EMAIL_RE).expect("static email regex"))
        .is_match(email)
}

fn zod_type_name(v: &Value) -> &'static str {
    match v {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        Value::Null => "null",
    }
}

fn zod_int_value(v: &Value) -> std::result::Result<i64, String> {
    if let Some(f) = v.as_f64() {
        if f.fract() == 0.0 && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
            return Ok(f as i64);
        }
        return Err("Expected integer, received float".into());
    }
    Err(format!("Expected number, received {}", zod_type_name(v)))
}

fn parse_text_min1(body: &Value, field: &str) -> Result<String, Response> {
    match crate::conversation_ops::zod_string_msg(body, field) {
        Ok(t) if t.is_empty() => Err(zod_422(
            field,
            "String must contain at least 1 character(s)",
        )),
        Ok(t) => Ok(t),
        Err(m) => Err(zod_422(field, &m)),
    }
}

fn parse_email_array(body: &Value, field: &str) -> Result<Vec<String>, Response> {
    match body.get(field) {
        None => Ok(vec![]),
        Some(Value::Null) => Err(zod_422(field, "Expected array, received null")),
        Some(Value::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for (i, v) in items.iter().enumerate() {
                let Some(s) = v.as_str() else {
                    return Err(zod_422(
                        &format!("{field}[{i}]"),
                        &format!("Expected string, received {}", zod_type_name(v)),
                    ));
                };
                if !zod_email_ok(s) {
                    return Err(zod_422(&format!("{field}[{i}]"), "Invalid email"));
                }
                out.push(s.to_string());
            }
            Ok(out)
        }
        Some(v) => Err(zod_422(
            field,
            &format!("Expected array, received {}", zod_type_name(v)),
        )),
    }
}

fn parse_optional_int(body: &Value, field: &str) -> Result<Option<i64>, Response> {
    match body.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => zod_int_value(v).map(Some).map_err(|m| zod_422(field, &m)),
    }
}

fn parse_required_nullable_int(body: &Value, field: &str) -> Result<Option<i64>, Response> {
    match body.get(field) {
        None => Err(zod_422(field, "Required")),
        Some(Value::Null) => Ok(None),
        Some(v) => zod_int_value(v).map(Some).map_err(|m| zod_422(field, &m)),
    }
}

fn parse_reply_body(
    body: &Value,
) -> Result<
    (
        String,
        bool,
        Vec<String>,
        Vec<String>,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<String>,
    ),
    Response,
> {
    let text = parse_text_min1(body, "text")?;
    let draft = match body.get("draft") {
        None => false,
        Some(Value::Null) => return Err(zod_422("draft", "Expected boolean, received null")),
        Some(_) => match crate::conversation_ops::zod_bool_msg(body, "draft") {
            Ok(b) => b,
            Err(m) => return Err(zod_422("draft", &m)),
        },
    };
    let cc = parse_email_array(body, "cc")?;
    let bcc = parse_email_array(body, "bcc")?;
    const STATUS_AFTER: [&str; 6] = [
        "active",
        "closed",
        "pending",
        "spam",
        "open",
        "inbox_predefined",
    ];
    let status_after = match body.get("statusAfter") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => {
            if !STATUS_AFTER.contains(&s.as_str()) {
                return Err(zod_422("statusAfter", &zod_enum_message(&STATUS_AFTER, s)));
            }
            Some(s.clone())
        }
        Some(v) => {
            return Err(zod_422(
                "statusAfter",
                &format!("Expected string, received {}", zod_type_name(v)),
            ))
        }
    };
    let assign_to = parse_optional_int(body, "assignTo")?;
    let ai_draft_id = match body.get("aiDraftId") {
        None | Some(Value::Null) => None,
        Some(v) => match zod_int_value(v) {
            Ok(n) if n > 0 => Some(n),
            Ok(_) => return Err(zod_422("aiDraftId", "Number must be greater than 0")),
            Err(m) => return Err(zod_422("aiDraftId", &m)),
        },
    };
    let original_ai_text = match body.get("originalAiText") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(v) => {
            return Err(zod_422(
                "originalAiText",
                &format!("Expected string, received {}", zod_type_name(v)),
            ))
        }
    };
    match body.get("attachmentIds") {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            for (i, v) in items.iter().enumerate() {
                if let Err(m) = zod_int_value(v) {
                    return Err(zod_422(&format!("attachmentIds[{i}]"), &m));
                }
            }
        }
        Some(v) => {
            return Err(zod_422(
                "attachmentIds",
                &format!("Expected array, received {}", zod_type_name(v)),
            ))
        }
    }
    Ok((
        text,
        draft,
        cc,
        bcc,
        status_after,
        assign_to,
        ai_draft_id,
        original_ai_text,
    ))
}

pub async fn events(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let limit: i64 = match params
        .get("limit")
        .filter(|s| !s.is_empty())
        .and_then(|s| crate::conversation_ops::js_number(s))
    {
        Some(n) if n.is_finite() => (n.trunc() as i64).clamp(1, 1000),
        _ => 200,
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());

    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM conversations WHERE id = ?1",
            rusqlite::params![id],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Conversation not found locally."
            })),
        )
            .into_response();
    }

    let events: Vec<Value> = match conn.prepare(
        "SELECT e.id, e.conversation_id, e.thread_local_id, e.event_type, e.actor_type, e.actor_id,
                e.occurred_at, e.source, e.metadata, e.created_at,
                CASE
                  WHEN e.actor_type = 'user' THEN
                    (SELECT TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, ''))
                     FROM users u WHERE u.id = e.actor_id)
                  WHEN e.actor_type = 'customer' THEN
                    (SELECT TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, ''))
                     FROM customers cu WHERE cu.id = e.actor_id)
                  WHEN e.actor_type = 'system_user' THEN
                    (SELECT TRIM(COALESCE(su.first_name, '') || ' ' || COALESCE(su.last_name, ''))
                     FROM system_users su WHERE su.id = e.actor_id)
                  ELSE NULL
                END AS actor_name
         FROM activity_events e
         WHERE e.conversation_id = ?1
         ORDER BY COALESCE(e.occurred_at, e.created_at) ASC, e.id ASC
         LIMIT ?2",
    ) {
        Ok(mut stmt) => stmt
            .query_map(rusqlite::params![id, limit], |r| {
                let metadata = r
                    .get::<_, Option<String>>(8)?
                    .and_then(|m| serde_json::from_str::<Value>(&m).ok())
                    .filter(|v| v.is_object())
                    .unwrap_or_else(|| json!({}));
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "conversation_id": r.get::<_, i64>(1)?,
                    "thread_local_id": r.get::<_, Option<i64>>(2)?,
                    "event_type": r.get::<_, String>(3)?,
                    "actor_type": r.get::<_, Option<String>>(4)?,
                    "actor_local_id": r.get::<_, Option<i64>>(5)?,
                    "occurred_at": r.get::<_, Option<String>>(6)?,
                    "source": r.get::<_, Option<String>>(7)?,
                    "metadata": metadata,
                    "created_at": r.get::<_, Option<String>>(9)?,
                    "actor_name": r.get::<_, Option<String>>(10)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default(),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    json!({"statusCode": 500, "error": "InternalError", "message": e.to_string()}),
                ),
            )
                .into_response();
        }
    };

    let count_pairs: Vec<(String, i64)> = conn
        .prepare(
            "SELECT event_type, COUNT(*) FROM activity_events
             WHERE conversation_id = ?1 GROUP BY event_type",
        )
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let mut counts = serde_json::Map::new();
    for (kind, n) in count_pairs {
        counts.insert(kind, json!(n));
    }

    (
        StatusCode::OK,
        Json(json!({
            "conversation_id": id,
            "events": events,
            "counts": Value::Object(counts),
        })),
    )
        .into_response()
}

pub async fn activity_rebuild(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    match crate::jobs::enqueue_on(&conn, "maintenance", "rebuild_activity", "{}", 4, 1) {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({"ok": true, "message": "Activity rebuild queued."})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "statusCode": 500,
                "error": "InternalError",
                "message": format!("Could not queue the activity rebuild: {e}"),
            })),
        ),
    }
}

pub async fn list_ticket_states(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let states: Vec<Value> = crate::ticket_states::list_states(&conn)
        .iter()
        .filter_map(|s| serde_json::to_value(s).ok())
        .collect();
    let bottlenecks = crate::ticket_states::state_bottlenecks(&conn);
    (
        StatusCode::OK,
        Json(json!({ "states": states, "bottlenecks": bottlenecks })),
    )
}

pub async fn create_ticket_state(
    State(state): State<AppState>,
    body: Option<Json<Value>>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let Some(name) = body.get("name").and_then(|v| v.as_str()) else {
        return crate::conversation_ops::zod_422("name", "Required");
    };
    if name.is_empty() {
        return crate::conversation_ops::zod_422(
            "name",
            "String must contain at least 1 character(s)",
        );
    }
    if name.len() > 80 {
        return crate::conversation_ops::zod_422(
            "name",
            "String must contain at most 80 character(s)",
        );
    }
    let key = body.get("key").and_then(|v| v.as_str());
    if let Some(k) = key {
        if k.is_empty()
            || k.len() > 60
            || !k
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return crate::conversation_ops::zod_422(
                "key",
                "key must be lowercase letters, digits and dashes",
            );
        }
    }
    let color = match body.get("color") {
        Some(serde_json::Value::Null) | None => None,
        Some(serde_json::Value::String(c)) => {
            let ok =
                c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|h| h.is_ascii_hexdigit());
            if !ok {
                return crate::conversation_ops::zod_422("color", "color must be a #rrggbb hex");
            }
            Some(c.as_str())
        }
        Some(_) => {
            return crate::conversation_ops::zod_422(
                "color",
                "Expected string, received non-string",
            )
        }
    };
    let sort_order = match body.get("sort_order") {
        None | Some(serde_json::Value::Null) => None,
        Some(v) => match v.as_i64() {
            Some(n) if (0..=9999).contains(&n) => Some(n),
            Some(_) => {
                return crate::conversation_ops::zod_422(
                    "sort_order",
                    "Number must be greater than or equal to 0",
                )
            }
            None => {
                return crate::conversation_ops::zod_422(
                    "sort_order",
                    "Expected number, received non-number",
                )
            }
        },
    };
    let is_resolved = body.get("is_resolved").and_then(|v| v.as_bool());
    let conn = state.conn_lock();
    match crate::ticket_states::create_state(&conn, name, key, color, sort_order, is_resolved) {
        Ok(st) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "message": format!("State '{}' created.", st.name),
                "state": st,
            })),
        )
            .into_response(),
        Err(msg) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "ok": false, "message": msg })),
        )
            .into_response(),
    }
}

pub async fn update_ticket_state(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let Ok(id) = id.parse::<i64>() else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "message": "State not found." })),
        )
            .into_response();
    };
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    if let Some(name) = body.get("name").and_then(|v| v.as_str()) {
        if name.is_empty() {
            return crate::conversation_ops::zod_422(
                "name",
                "String must contain at least 1 character(s)",
            );
        }
        if name.len() > 80 {
            return crate::conversation_ops::zod_422(
                "name",
                "String must contain at most 80 character(s)",
            );
        }
    }
    if let Some(color) = body.get("color").and_then(|v| v.as_str()) {
        let ok = color.len() == 7
            && color.starts_with('#')
            && color[1..].chars().all(|h| h.is_ascii_hexdigit());
        if !ok {
            return crate::conversation_ops::zod_422("color", "color must be a #rrggbb hex");
        }
    }
    let conn = state.conn_lock();
    match crate::ticket_states::update_state(&conn, id, &body) {
        Ok(Some(st)) => (
            StatusCode::OK,
            Json(json!({ "ok": true, "message": "State updated.", "state": st })),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "message": "State not found." })),
        )
            .into_response(),
        Err(msg) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "ok": false, "message": msg })),
        )
            .into_response(),
    }
}

pub async fn delete_ticket_state(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let Ok(id) = id.parse::<i64>() else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "ok": false, "message": "State not found." })),
        )
            .into_response();
    };
    let conn = state.conn_lock();
    let (ok, message) = crate::ticket_states::delete_state(&conn, id);
    let code = if ok {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    (code, Json(json!({ "ok": ok, "message": message }))).into_response()
}

pub async fn list_mailboxes(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let mailboxes: Vec<Value> = conn
        .prepare("SELECT id, remote_id, name, email FROM mailboxes ORDER BY id")
        .ok()
        .map(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "remote_id": r.get::<_, i64>(1)?,
                    "name": r.get::<_, String>(2)?,
                    "email": r.get::<_, Option<String>>(3)?,
                }))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
        .unwrap_or_default();
    (StatusCode::OK, Json(mailboxes))
}

pub async fn list_tags(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let tags: Vec<Value> = conn
        .prepare(
            "SELECT id, remote_id, name, slug, color, COALESCE(ticket_count, 0)
               FROM tags ORDER BY name",
        )
        .ok()
        .map(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "remote_id": r.get::<_, Option<i64>>(1)?,
                    "name": r.get::<_, String>(2)?,
                    "slug": r.get::<_, Option<String>>(3)?,
                    "color": r.get::<_, Option<String>>(4)?,
                    "ticket_count": r.get::<_, i64>(5)?,
                }))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
        .unwrap_or_default();
    (StatusCode::OK, Json(tags))
}

pub async fn list_users(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let users: Vec<Value> = conn
        .prepare("SELECT id, remote_id, first_name, last_name, email FROM users ORDER BY id")
        .ok()
        .map(|mut stmt| {
            stmt.query_map([], |r| {
                let first: String = r.get::<_, String>(2).unwrap_or_default();
                let last: String = r.get::<_, String>(3).unwrap_or_default();
                let display_name = format!("{first} {last}").trim().to_string();
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "remote_id": r.get::<_, i64>(1)?,
                    "display_name": display_name,
                    "email": r.get::<_, Option<String>>(4)?,
                }))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
        .unwrap_or_default();
    (
        StatusCode::OK,
        Json(json!({"users": users, "system_users": []})),
    )
}

pub async fn list_teams(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let teams: Vec<Value> = conn
        .prepare(
            "SELECT t.id, t.remote_id, t.name,
                    (SELECT COUNT(*) FROM users u WHERE u.team_id = t.id) AS member_count
             FROM teams t ORDER BY t.id",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "remote_id": r.get::<_, Option<i64>>(1)?,
                    "name": r.get::<_, String>(2)?,
                    "member_count": r.get::<_, i64>(3)?,
                }))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    (StatusCode::OK, Json(teams))
}

pub async fn list_saved_replies(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let conn = state.conn_lock();
    let q = params.get("q").map(String::as_str);
    let replies = crate::mirror_readouts::saved_replies(&conn, q).unwrap_or_default();
    (StatusCode::OK, Json(json!({ "saved_replies": replies })))
}

pub async fn inbox_fields(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let fields = crate::mirror_readouts::inbox_fields(&conn).unwrap_or_default();
    (StatusCode::OK, Json(fields))
}

pub async fn workflows(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let list = crate::mirror_readouts::workflows(&conn).unwrap_or_default();
    (StatusCode::OK, Json(list))
}

pub async fn user_statuses(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let list = crate::mirror_readouts::user_statuses(&conn).unwrap_or_default();
    (StatusCode::OK, Json(list))
}

pub async fn webhook_configs(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let list = crate::mirror_readouts::webhook_configs(&conn).unwrap_or_default();
    (StatusCode::OK, Json(list))
}

use crate::conversation_ops::{
    conv_by_local_id, eval_mode_blocked, js_number, ok_result, op_bulk_action, op_delete_schedule,
    op_move_to_inbox, op_publish_schedule, op_run_workflow, op_schedule_reply, op_snooze,
    op_unsnooze, op_update_custom_fields, op_update_tags, rejected, zod_422, zod_422_multi,
    zod_bool_msg, zod_enum_message, zod_int_msg, zod_string_msg,
};
use axum::response::Response;

fn path_id(raw: &str) -> Result<i64, Response> {
    match js_number(raw) {
        Some(f)
            if f.is_finite()
                && f.fract() == 0.0
                && f >= i64::MIN as f64
                && f <= i64::MAX as f64 =>
        {
            Ok(f as i64)
        }
        Some(_) => Err(zod_422(
            "conversationId",
            "Expected integer, received float",
        )),
        None => Err(zod_422("conversationId", "Expected number, received nan")),
    }
}

pub async fn move_to_inbox(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let mailbox_id = match zod_int_msg(&body, "mailboxId") {
        Ok(v) => v,
        Err(m) => return zod_422("mailboxId", &m),
    };
    {
        let conn = state.conn_lock();
        if let Some(msg) = eval_mode_blocked(&conn) {
            return rejected(msg);
        }
        if conv_by_local_id(&conn, conversation_id).is_none() {
            return rejected("Conversation not found locally.");
        }
    }
    if let Some(real) = state.real.clone() {
        let remote_id: i64 = {
            let conn = state.conn_lock();
            conv_by_local_id(&conn, conversation_id)
                .map(|c| c.remote_id)
                .unwrap_or_default()
        };
        if let Err(e) = real
            .request(
                &format!("/v2/conversations/{remote_id}"),
                "PATCH",
                Some(json!({ "op": "move", "path": "/mailboxId", "value": mailbox_id })),
            )
            .await
        {
            return rejected(&format!("Conversation was NOT moved. {e}"));
        }
    }
    let conn = state.conn_lock();
    op_move_to_inbox(&conn, conversation_id, mailbox_id)
}

pub async fn update_tags_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let mut change = json!({});
    for key in ["add", "remove"] {
        match body.get(key) {
            None | Some(Value::Null) => {
                change[key] = json!([]);
            }
            Some(Value::Array(a)) => {
                for v in a {
                    if !v.is_string() {
                        return zod_422(
                            key,
                            &format!(
                                "Expected string, received {}",
                                if v.is_number() {
                                    "number"
                                } else if v.is_boolean() {
                                    "boolean"
                                } else {
                                    "object"
                                }
                            ),
                        );
                    }
                }
                change[key] = json!(a);
            }
            Some(_) => return zod_422(key, "Expected array, received non-array"),
        }
    }
    match body.get("set") {
        Some(Value::Null) | None => {}
        Some(Value::Array(a)) => {
            change["set"] = json!(a);
        }
        Some(_) => return zod_422("set", "Expected array, received non-array"),
    }
    {
        let conn = state.conn_lock();
        if let Some(msg) = eval_mode_blocked(&conn) {
            return rejected(msg);
        }
        if conv_by_local_id(&conn, conversation_id).is_none() {
            return rejected("Conversation not found locally.");
        }
    }
    if let Some(provider) = crate::conversation_ops::op_provider(&state) {
        let remote_id: i64 = {
            let conn = state.conn_lock();
            conv_by_local_id(&conn, conversation_id)
                .map(|c| c.remote_id)
                .unwrap_or_default()
        };
        if let Ok(Some(remote)) = provider.get_conversation(remote_id).await {
            let current = remote.tags.clone();
            let desired = crate::conversation_ops::merge_tags(&current, &change);
            if let Err(e) = provider.update_tags(remote_id, desired).await {
                return rejected(&format!("Tags were NOT changed. {e}"));
            }
        }
    }
    let conn = state.conn_lock();
    op_update_tags(&conn, conversation_id, &change)
}

pub async fn update_fields_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(fields) = body.get("fields").and_then(|f| f.as_array()) else {
        return zod_422("fields", "Required");
    };
    let mut parsed: Vec<(i64, Option<String>)> = Vec::new();
    for f in fields {
        let Some(fid) = f.get("id").and_then(|v| v.as_i64()) else {
            return zod_422("fields", "Expected object with integer id");
        };
        let value: Option<String> = match f.get("value") {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Null) | None => None,
            Some(_) => {
                return zod_422("value", "Expected string, received non-string");
            }
        };
        parsed.push((fid, value));
    }
    {
        let conn = state.conn_lock();
        if let Some(msg) = eval_mode_blocked(&conn) {
            return rejected(msg);
        }
        if conv_by_local_id(&conn, conversation_id).is_none() {
            return rejected("Conversation not found locally.");
        }
    }
    if let Some(provider) = crate::conversation_ops::op_provider(&state) {
        let remote_id: i64 = {
            let conn = state.conn_lock();
            conv_by_local_id(&conn, conversation_id)
                .map(|c| c.remote_id)
                .unwrap_or_default()
        };
        if let Ok(Some(remote)) = provider.get_conversation(remote_id).await {
            let changed_ids: std::collections::HashSet<i64> =
                parsed.iter().map(|(id, _)| *id).collect();
            let mut merged: Vec<(i64, Option<String>)> = Vec::new();
            for f in &remote.custom_fields {
                if f.system_type.is_some() {
                    merged.push((f.field_id, f.value.clone()));
                } else if !changed_ids.contains(&f.field_id) {
                    merged.push((f.field_id, f.value.clone()));
                }
            }
            merged.extend(parsed.iter().cloned());
            if let Err(e) = provider.update_custom_fields(remote_id, merged).await {
                return rejected(&format!("Fields were NOT changed. {e}"));
            }
        }
    }
    let conn = state.conn_lock();
    op_update_custom_fields(&conn, conversation_id, &parsed)
}

pub async fn snooze_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Ok(snoozed_until) = zod_string_msg(&body, "snoozedUntil") else {
        return zod_422("snoozedUntil", "Required");
    };
    let _unsnooze = match body.get("unsnoozeOnCustomerReply") {
        None | Some(Value::Null) => true,
        _ => match zod_bool_msg(&body, "unsnoozeOnCustomerReply") {
            Ok(b) => b,
            Err(m) => return zod_422("unsnoozeOnCustomerReply", &m),
        },
    };
    {
        let conn = state.conn_lock();
        if let Some(msg) = eval_mode_blocked(&conn) {
            return rejected(msg);
        }
        if conv_by_local_id(&conn, conversation_id).is_none() {
            return rejected("Conversation not found locally.");
        }
    }
    if let Some(provider) = crate::conversation_ops::op_provider(&state) {
        let remote_id: i64 = {
            let conn = state.conn_lock();
            conv_by_local_id(&conn, conversation_id)
                .map(|c| c.remote_id)
                .unwrap_or_default()
        };
        if let Err(e) = provider
            .snooze_conversation(remote_id, snoozed_until.clone(), _unsnooze)
            .await
        {
            return rejected(&format!("Snooze was NOT applied. {e}"));
        }
    }
    let conn = state.conn_lock();
    op_snooze(&conn, conversation_id, &snoozed_until)
}

pub async fn unsnooze_route(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    {
        let conn = state.conn_lock();
        if let Some(msg) = eval_mode_blocked(&conn) {
            return rejected(msg);
        }
        if conv_by_local_id(&conn, conversation_id).is_none() {
            return rejected("Conversation not found locally.");
        }
    }
    if let Some(provider) = crate::conversation_ops::op_provider(&state) {
        let remote_id: i64 = {
            let conn = state.conn_lock();
            conv_by_local_id(&conn, conversation_id)
                .map(|c| c.remote_id)
                .unwrap_or_default()
        };
        if let Err(e) = provider.unsnooze_conversation(remote_id).await {
            return rejected(&format!("Snooze was NOT removed. {e}"));
        }
    }
    let conn = state.conn_lock();
    op_unsnooze(&conn, conversation_id)
}

pub async fn schedule_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let mut issues: Vec<(&str, String)> = Vec::new();
    let thread_id = match zod_int_msg(&body, "threadId") {
        Ok(v) => Some(v),
        Err(m) => {
            issues.push(("threadId", m));
            None
        }
    };
    let scheduled_for = match zod_string_msg(&body, "scheduledFor") {
        Ok(v) => Some(v),
        Err(m) => {
            issues.push(("scheduledFor", m));
            None
        }
    };
    if !issues.is_empty() {
        let owned: Vec<(&str, &str)> = issues.iter().map(|(p, m)| (*p, m.as_str())).collect();
        return zod_422_multi(&owned);
    }
    let thread_id = thread_id.unwrap_or_default();
    let scheduled_for = scheduled_for.unwrap_or_default();
    let _u = match body.get("unscheduleOnCustomerReply") {
        None | Some(Value::Null) => true,
        _ => match zod_bool_msg(&body, "unscheduleOnCustomerReply") {
            Ok(b) => b,
            Err(m) => return zod_422("unscheduleOnCustomerReply", &m),
        },
    };
    let _ = _u;
    {
        let conn = state.conn_lock();
        if let Some(msg) = eval_mode_blocked(&conn) {
            return rejected(msg);
        }
    }
    if let Some(provider) = crate::conversation_ops::op_provider(&state) {
        let remote_ids = {
            let conn = state.conn_lock();
            let c = conv_by_local_id(&conn, conversation_id);
            let t_remote: Option<i64> = conn
                .query_row(
                    "SELECT remote_id FROM conversation_threads WHERE id = ?1",
                    rusqlite::params![thread_id],
                    |r| r.get(0),
                )
                .ok();
            (c.map(|c| c.remote_id), t_remote)
        };
        match remote_ids {
            (Some(conv_remote), Some(thread_remote)) => {
                if let Err(e) = provider
                    .schedule_thread(conv_remote, thread_remote, scheduled_for.clone(), _u)
                    .await
                {
                    return rejected(&format!("Schedule was NOT applied. {e}"));
                }
            }
            _ => return rejected("Conversation or draft thread not found locally."),
        }
    }
    let conn = state.conn_lock();
    op_schedule_reply(&conn, conversation_id, thread_id, &scheduled_for)
}

pub async fn schedule_publish_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Ok(thread_id) = zod_int_msg(&body, "threadId") else {
        return zod_422("threadId", "Required");
    };
    {
        let conn = state.conn_lock();
        if let Some(msg) = eval_mode_blocked(&conn) {
            return rejected(msg);
        }
    }
    if let Some(provider) = crate::conversation_ops::op_provider(&state) {
        let (conv_remote, thread_remote): (i64, Option<i64>) = {
            let conn = state.conn_lock();
            let c = conv_by_local_id(&conn, conversation_id).map(|c| c.remote_id);
            let t = conn
                .query_row(
                    "SELECT remote_id FROM conversation_threads WHERE id = ?1",
                    rusqlite::params![thread_id],
                    |r| r.get(0),
                )
                .ok();
            (c.unwrap_or_default(), t)
        };
        let Some(thread_remote) = thread_remote else {
            return rejected("Conversation or thread not found locally.");
        };
        if let Err(e) = provider
            .publish_scheduled_thread(conv_remote, thread_remote)
            .await
        {
            return rejected(&format!("Publish failed. {e}"));
        }
    }
    let conn = state.conn_lock();
    op_publish_schedule(&conn, conversation_id, thread_id)
}

pub async fn schedule_delete_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Ok(thread_id) = zod_int_msg(&body, "threadId") else {
        return zod_422("threadId", "Required");
    };
    {
        let conn = state.conn_lock();
        if let Some(msg) = eval_mode_blocked(&conn) {
            return rejected(msg);
        }
    }
    if let Some(provider) = crate::conversation_ops::op_provider(&state) {
        let (conv_remote, thread_remote): (i64, Option<i64>) = {
            let conn = state.conn_lock();
            let c = conv_by_local_id(&conn, conversation_id).map(|c| c.remote_id);
            let t = conn
                .query_row(
                    "SELECT remote_id FROM conversation_threads WHERE id = ?1",
                    rusqlite::params![thread_id],
                    |r| r.get(0),
                )
                .ok();
            (c.unwrap_or_default(), t)
        };
        let Some(thread_remote) = thread_remote else {
            return rejected("Conversation or thread not found locally.");
        };
        if let Err(e) = provider
            .delete_thread_schedule(conv_remote, thread_remote)
            .await
        {
            return rejected(&format!("Delete failed. {e}"));
        }
    }
    let conn = state.conn_lock();
    op_delete_schedule(&conn, conversation_id, thread_id)
}

pub async fn bulk_route(State(state): State<AppState>, body: Option<Json<Value>>) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let Some(ids) = body.get("conversationIds").and_then(|v| v.as_array()) else {
        return zod_422("conversationIds", "Required");
    };
    if ids.is_empty() {
        return zod_422(
            "conversationIds",
            "Array must contain at least 1 element(s)",
        );
    }
    let mut conversation_ids = Vec::new();
    for v in ids {
        match v.as_i64() {
            Some(i) => conversation_ids.push(i),
            None => {
                return zod_422(
                    "conversationIds",
                    &format!(
                        "Expected number, received {}",
                        if v.is_string() { "string" } else { "object" }
                    ),
                )
            }
        }
    }
    let Some(action) = body.get("action").and_then(|v| v.as_str()) else {
        return zod_422("action", "Required");
    };
    const ACTIONS: [&str; 6] = ["tag", "untag", "assign", "unassign", "status", "close"];
    if !ACTIONS.contains(&action) {
        return zod_422("action", &zod_enum_message(&ACTIONS, action));
    }
    let params_body = body.get("params").cloned().unwrap_or_else(|| json!({}));
    let conn = state.conn_lock();
    op_bulk_action(&conn, &conversation_ids, action, &params_body)
}

pub async fn refresh_route(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let remote_id = {
        let conn = state.conn_lock();
        match conv_by_local_id(&conn, conversation_id) {
            Some(c) => c.remote_id,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "statusCode": 404, "error": "NotFound",
                        "message": "Conversation not found locally."
                    })),
                )
                    .into_response()
            }
        }
    };
    if let Some(sync) = state.sync.clone() {
        match sync.sync_single_conversation(remote_id).await {
            Ok(_) => (
                StatusCode::OK,
                Json(json!({ "ok": true, "message": "Conversation refreshed from Help Scout." })),
            )
                .into_response(),
            Err(e) => (
                StatusCode::OK,
                Json(json!({ "ok": false, "message": format!("Refresh failed: {e}") })),
            )
                .into_response(),
        }
    } else {
        (
            StatusCode::OK,
            Json(json!({ "ok": true, "message": "Conversation refreshed from Help Scout." })),
        )
            .into_response()
    }
}

pub async fn workflow_route(
    State(state): State<AppState>,
    Path((id, workflow_id)): Path<(String, String)>,
) -> Response {
    let conversation_id = match path_id(&id) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let workflow_id = match js_number(&workflow_id) {
        Some(f) if f.is_finite() && f.fract() == 0.0 => f as i64,
        Some(_) => return zod_422("workflowId", "Expected integer, received float"),
        None => return zod_422("workflowId", "Expected number, received nan"),
    };
    {
        let conn = state.conn_lock();
        if let Some(msg) = eval_mode_blocked(&conn) {
            return rejected(msg);
        }
        if conv_by_local_id(&conn, conversation_id).is_none() {
            return rejected("Conversation not found locally.");
        }
    }
    if let Some(provider) = crate::conversation_ops::op_provider(&state) {
        let remote_id: i64 = {
            let conn = state.conn_lock();
            conv_by_local_id(&conn, conversation_id)
                .map(|c| c.remote_id)
                .unwrap_or_default()
        };
        if let Err(e) = provider.run_workflow(workflow_id, remote_id).await {
            return rejected(&format!("Workflow failed. {e}"));
        }
    }
    let conn = state.conn_lock();
    op_run_workflow(&conn, conversation_id, workflow_id)
}

pub async fn attachment_download_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let attachment_id = match js_number(&id) {
        Some(f) if f.is_finite() && f.fract() == 0.0 => f as i64,
        _ => return zod_422("id", "Expected number, received nan"),
    };
    let attachments_dir = state.data_dir.join("attachments");
    crate::conversation_ops::op_download_attachment(&state, &attachments_dir, attachment_id).await
}
