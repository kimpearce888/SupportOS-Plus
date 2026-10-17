use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::{Value, json};

use super::super::server::AppState;
use crate::automation::{self, RuleUpdate, SqlValue};

fn internal_error(e: crate::error::Error) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "statusCode": 500,
            "error": "Internal Server Error",
            "message": e.to_string()
        })),
    )
}

pub async fn list_rules(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let rules = match automation::list_rule_records(&conn) {
        Ok(r) => r,
        Err(e) => return internal_error(e),
    };
    let runs = match automation::list_run_records(&conn, 50) {
        Ok(r) => r,
        Err(e) => return internal_error(e),
    };
    let automation_enabled =
        crate::settings::get_bool(&conn, "automation_enabled", false).unwrap_or(false);
    let rules_json: Vec<Value> = rules.iter().map(|r| r.to_main_json()).collect();
    let runs_json: Vec<Value> = runs.iter().map(|r| r.to_main_json()).collect();
    (
        StatusCode::OK,
        Json(json!({
            "rules": rules_json,
            "runs": runs_json,
            "risk_tiers": {
                "read": ["analyze_ticket", "search_similar", "check_known_issues"],
                "non_destructive": ["create_ai_note", "create_ai_draft", "add_tag", "manual_review_queue"],
                "higher_risk": ["set_status", "assign"],
                "note": "Higher-risk actions always require explicit approval. Help Scout workflows are a separate system (Automation screen shows both)."
            },
            "automation_enabled": automation_enabled,
        })),
    )
}

pub async fn create_rule(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let input = match automation::validate_rule(&body) {
        Ok(i) => i,
        Err(issues) => {
            let detail = issues
                .iter()
                .map(|i| format!("{}: {}", i.path, i.message))
                .collect::<Vec<_>>()
                .join("; ");
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "statusCode": 400,
                    "error": "BadRequest",
                    "message": "Invalid automation rule.",
                    "detail": detail,
                })),
            );
        }
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match automation::create_rule_record(&conn, &input) {
        Ok(id) => {
            let _ = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("automation_rule_created")
                    .with_after_state(json!({ "id": id, "name": input.name })),
            );
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "message": "Automation rule created (disabled by default - enable it when ready).",
                    "id": id,
                })),
            )
        }
        Err(e) => internal_error(e),
    }
}

pub async fn update_rule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    const ALLOWED: [&str; 7] = [
        "name",
        "enabled",
        "trigger",
        "conditions",
        "actions",
        "priority",
        "requires_approval",
    ];
    let obj = body.as_object();
    let has = |k: &str| obj.is_some_and(|o| o.contains_key(k));
    let get = |k: &str| obj.and_then(|o| o.get(k));
    if !ALLOWED.iter().any(|k| has(k)) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "No valid fields to update (name, enabled, trigger, conditions, actions, priority, requires_approval)."
            })),
        );
    }

    let mut upd = RuleUpdate::default();
    let full_revalidate = has("trigger") || has("conditions") || has("actions");
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());

    if full_revalidate {
        let existing = match automation::load_rule_record(&conn, id) {
            Ok(Some(r)) => r,
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "statusCode": 404,
                        "error": "NotFound",
                        "message": "Rule not found."
                    })),
                );
            }
            Err(e) => return internal_error(e),
        };
        let merged_name = get("name")
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(Value::String(existing.name.clone()));
        let merged_trigger = get("trigger")
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(Value::String(existing.trigger.clone()));
        let merged_conditions = get("conditions")
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(existing.conditions.clone());
        let merged_actions = get("actions")
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(existing.actions.clone());
        let merged_priority = get("priority")
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(json!(existing.priority));
        let merged_requires = get("requires_approval")
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(json!(existing.requires_approval));
        let candidate = json!({
            "name": merged_name,
            "trigger": merged_trigger,
            "conditions": merged_conditions,
            "actions": merged_actions,
            "priority": merged_priority,
            "requires_approval": merged_requires,
        });
        match automation::validate_rule(&candidate) {
            Ok(parsed) => {
                upd.name = Some(parsed.name);
                upd.trigger = Some(parsed.trigger);
                upd.conditions = Some(parsed.conditions);
                upd.actions = Some(parsed.actions);
                upd.priority = Some(SqlValue::Int(parsed.priority));
                upd.requires_approval = Some(parsed.requires_approval as i64);
            }
            Err(issues) => {
                let first = issues.first();
                let where_clause = first
                    .as_ref()
                    .filter(|i| !i.path.is_empty())
                    .map(|i| format!(" ({})", i.path));
                let message = format!(
                    "Invalid request{}: {}",
                    where_clause.as_deref().unwrap_or(""),
                    first
                        .map(|i| i.message.clone())
                        .unwrap_or_else(|| { "request body failed validation.".to_string() })
                );
                let issue_list: Vec<Value> = issues
                    .iter()
                    .take(10)
                    .map(|i| json!({ "path": i.path, "message": i.message }))
                    .collect();
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422,
                        "error": "ValidationError",
                        "message": message,
                        "issues": issue_list,
                    })),
                );
            }
        }
        if let Some(v) = get("enabled") {
            upd.enabled = Some(automation::js_truthy(v));
        }
    } else {
        if let Some(v) = get("name") {
            match v.as_str() {
                Some(s) if !s.trim().is_empty() && s.chars().count() <= 200 => {
                    upd.name = Some(s.to_string());
                }
                _ => {
                    return (
                        StatusCode::UNPROCESSABLE_ENTITY,
                        Json(json!({
                            "statusCode": 422,
                            "error": "ValidationError",
                            "message": "name must be a non-empty string (max 200 chars)."
                        })),
                    );
                }
            }
        }
        if let Some(v) = get("enabled") {
            match v.as_bool() {
                Some(b) => upd.enabled = Some(b as i64),
                None => {
                    return (
                        StatusCode::UNPROCESSABLE_ENTITY,
                        Json(json!({
                            "statusCode": 422,
                            "error": "ValidationError",
                            "message": "enabled must be a boolean."
                        })),
                    );
                }
            }
        }
        if let Some(v) = get("priority") {
            upd.priority = Some(match v {
                Value::Null => SqlValue::Null,
                Value::Number(n) => n
                    .as_i64()
                    .map(SqlValue::Int)
                    .unwrap_or_else(|| SqlValue::Real(n.as_f64().unwrap_or(0.0))),
                Value::Bool(b) => SqlValue::Int(i64::from(*b)),
                Value::String(s) => SqlValue::Text(s.clone()),
                _ => SqlValue::Text(v.to_string()),
            });
        }
        if let Some(v) = get("requires_approval") {
            upd.requires_approval = Some(automation::js_truthy(v));
        }
    }

    match automation::update_rule_record(&conn, id, &upd) {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "ok": true, "message": "Rule updated." })),
        ),
        Err(e) => internal_error(e),
    }
}

pub async fn delete_rule(State(state): State<AppState>, Path(id): Path<i64>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match automation::delete_rule_record(&conn, id) {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "ok": true, "message": "Rule deleted." })),
        ),
        Err(e) => internal_error(e),
    }
}

pub async fn trigger(
    State(state): State<AppState>,
    Path((id, conversation_id)): Path<(i64, i64)>,
) -> axum::response::Response {
    let mut conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let rule = match automation::load_rule_record(&conn, id) {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::OK,
                Json(json!({ "ok": false, "message": "Rule not found." })),
            )
                .into_response();
        }
        Err(e) => return internal_error(e).into_response(),
    };
    match automation::fire_trigger_for_conversation(&mut conn, &rule.trigger, conversation_id) {
        Ok(runs) => {
            let runs_json: Vec<Value> = runs.iter().map(|r| r.to_main_json()).collect();
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "message": format!("Trigger fired ({} runs recorded).", runs_json.len()),
                    "runs": runs_json,
                })),
            )
                .into_response()
        }
        Err(e) => internal_error(e).into_response(),
    }
}
