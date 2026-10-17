use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::super::server::AppState;
use crate::people_store::{self, CustomerPatch, NewCustomer};

fn bad_request(message: String) -> Response {
    let capped: String = message.chars().take(300).collect();
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "statusCode": 400,
            "error": "BadRequest",
            "message": capped
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
            "message": message
        })),
    )
        .into_response()
}

fn internal(e: crate::error::Error) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "statusCode": 500,
            "error": "Internal",
            "message": e.to_string()
        })),
    )
        .into_response()
}

pub async fn list_customers(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let page = clamp_list_param(params.get("page"), 1, 1, 100_000);
    let page_size = clamp_list_param(params.get("pageSize"), 50, 1, 200);
    let query = params
        .get("q")
        .map(|s| s.as_str())
        .filter(|s| !s.trim().is_empty());
    match people_store::list_customers_summary(&conn, query, page, page_size) {
        Ok((customers, total)) => (
            StatusCode::OK,
            Json(json!({"customers": customers, "total": total, "page": page})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string(), "customers": [], "total": 0, "page": page})),
        ),
    }
}

pub async fn get_customer(State(state): State<AppState>, Path(id): Path<i64>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match people_store::get_customer_detail(&conn, id) {
        Ok(Some(detail)) => (StatusCode::OK, Json(detail)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"message": "Customer not found."})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        )
            .into_response(),
    }
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

pub async fn customer_timeline(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM customers WHERE id = ?1",
            rusqlite::params![id],
            |_| Ok(()),
        )
        .is_ok();
    if !exists {
        return not_found("Customer not found.");
    }
    let kind: Option<String> = params
        .get("kind")
        .map(|k| k.chars().take(40).collect::<String>())
        .filter(|k| !k.is_empty());
    let page_size = clamp_list_param(params.get("pageSize"), 100, 1, 200);
    let offset = (clamp_list_param(params.get("page"), 1, 1, 100000) - 1) * page_size;
    let (events, total) = match crate::customer_events::list_for_customer(
        &conn,
        id,
        kind.as_deref(),
        page_size,
        offset,
    ) {
        Ok(v) => v,
        Err(e) => return internal(e),
    };
    let kind_counts = match crate::customer_events::kind_counts(&conn, id) {
        Ok(v) => v,
        Err(e) => return internal(e),
    };
    (
        StatusCode::OK,
        Json(json!({"events": events, "total": total, "kind_counts": kind_counts})),
    )
        .into_response()
}

pub async fn customer_support_health(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match people_store::customer_support_health(&conn, id) {
        Ok(Some(report)) => (StatusCode::OK, Json(json!({ "report": report }))).into_response(),
        Ok(None) => not_found("Customer not found."),
        Err(e) => internal(e),
    }
}

pub async fn list_organizations(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<u32>().ok())
        .unwrap_or(20);
    let query = params
        .get("q")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty());
    match people_store::list_organizations(&conn, query, limit) {
        Ok(orgs) => (
            StatusCode::OK,
            Json(json!({"organizations": orgs, "total": orgs.len() as i64})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"organizations": [], "total": 0, "message": e.to_string()})),
        ),
    }
}

pub async fn get_organization(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match people_store::get_organization_detail(&conn, id) {
        Ok(Some(org)) => (StatusCode::OK, Json(org)).into_response(),
        Ok(None) => not_found("Organization not found."),
        Err(e) => internal(e),
    }
}

pub async fn organization_timeline(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM organizations WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![id],
            |_| Ok(()),
        )
        .is_ok();
    if !exists {
        return not_found("Organization not found.");
    }
    let kind: Option<String> = params
        .get("kind")
        .map(|k| k.chars().take(40).collect::<String>())
        .filter(|k| !k.is_empty());
    let page_size = clamp_list_param(params.get("pageSize"), 100, 1, 200);
    let offset = (clamp_list_param(params.get("page"), 1, 1, 100000) - 1) * page_size;
    match crate::customer_events::list_for_organization(
        &conn,
        id,
        kind.as_deref(),
        page_size,
        offset,
    ) {
        Ok((events, total)) => (
            StatusCode::OK,
            Json(json!({"events": events, "total": total})),
        )
            .into_response(),
        Err(e) => internal(e),
    }
}

pub async fn organization_support_health(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match people_store::organization_support_health(&conn, id) {
        Ok(Some(report)) => (StatusCode::OK, Json(json!({ "report": report }))).into_response(),
        Ok(None) => not_found("Organization not found."),
        Err(e) => internal(e),
    }
}

fn bounded_text(
    body: &Value,
    key: &str,
    max: usize,
    required: bool,
) -> Option<Result<String, String>> {
    match body.get(key) {
        None | Some(Value::Null) => {
            if required {
                Some(Err(format!("{key} is required.")))
            } else {
                None
            }
        }
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                if required {
                    Some(Err(format!("{key} is required.")))
                } else {
                    None
                }
            } else if trimmed.chars().count() > max {
                Some(Err(format!("{key} must be at most {max} characters.")))
            } else {
                Some(Ok(trimmed.to_string()))
            }
        }
        Some(_) => Some(Err(format!(
            "Expected string, received non-string for {key}."
        ))),
    }
}

fn patch_bounded_text(
    body: &Value,
    key: &str,
    max: usize,
) -> Result<Option<Option<String>>, String> {
    match body.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.chars().count() > max {
                Err(format!("{key} must be at most {max} characters."))
            } else if trimmed.is_empty() {
                Ok(Some(None))
            } else {
                Ok(Some(Some(trimmed.to_string())))
            }
        }
        Some(_) => Err(format!("Expected string, received non-string for {key}.")),
    }
}

fn check_email(email: &str) -> Result<(), String> {
    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || local.is_empty()
        || domain.is_empty()
        || !domain.contains('.')
        || email.chars().any(|c| c.is_whitespace())
    {
        Err("Email must be a valid email address.".to_string())
    } else {
        Ok(())
    }
}

fn parse_domains(body: &Value) -> Result<Option<Vec<String>>, String> {
    match body.get("domains") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => {
            if items.len() > 20 {
                return Err("domains must contain at most 20 entries.".to_string());
            }
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::String(s) => {
                        let trimmed = s.trim();
                        if trimmed.is_empty() {
                            return Err("domains entries must be non-empty strings.".to_string());
                        }
                        if trimmed.chars().count() > 200 {
                            return Err(
                                "domains entries must be at most 200 characters.".to_string()
                            );
                        }
                        out.push(trimmed.to_lowercase());
                    }
                    _ => return Err("Expected string, received non-string in domains.".to_string()),
                }
            }
            Ok(Some(out))
        }
        Some(_) => Err("Expected array, received non-array for domains.".to_string()),
    }
}

pub async fn create_customer(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let mut issues: Vec<String> = Vec::new();

    let first_name = match bounded_text(&body, "firstName", 80, false) {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => {
            issues.push(e);
            None
        }
        None => None,
    };
    let last_name = match bounded_text(&body, "lastName", 80, false) {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => {
            issues.push(e);
            None
        }
        None => None,
    };
    let email = match bounded_text(&body, "email", 200, false) {
        Some(Ok(v)) => {
            if let Err(e) = check_email(&v) {
                issues.push(e);
            }
            Some(v)
        }
        Some(Err(e)) => {
            issues.push(e);
            None
        }
        None => None,
    };
    if first_name.is_none() && last_name.is_none() && email.is_none() {
        issues.push("At least one of firstName, lastName or email is required.".to_string());
    }
    let organization = match bounded_text(&body, "organization", 200, false) {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => {
            issues.push(e);
            None
        }
        None => None,
    };
    let job_title = match bounded_text(&body, "jobTitle", 200, false) {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => {
            issues.push(e);
            None
        }
        None => None,
    };
    let phone = match bounded_text(&body, "phone", 60, false) {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => {
            issues.push(e);
            None
        }
        None => None,
    };

    if !issues.is_empty() {
        return bad_request(issues.join(" "));
    }

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let new = NewCustomer {
        first_name,
        last_name,
        email,
        organization,
        job_title,
        phone,
    };
    match people_store::create_customer(&conn, &new) {
        Ok(id) => match crate::customers::get_customer(&conn, id) {
            Ok(Some(c)) => {
                (StatusCode::OK, Json(json!({"ok": true, "customer": c}))).into_response()
            }
            _ => (StatusCode::OK, Json(json!({"ok": true, "customer_id": id}))).into_response(),
        },
        Err(e) => internal(e),
    }
}

pub async fn update_customer(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Response {
    let mut issues: Vec<String> = Vec::new();

    let first_name = match patch_bounded_text(&body, "firstName", 80) {
        Ok(v) => v,
        Err(e) => {
            issues.push(e);
            None
        }
    };
    let last_name = match patch_bounded_text(&body, "lastName", 80) {
        Ok(v) => v,
        Err(e) => {
            issues.push(e);
            None
        }
    };
    let email = match patch_bounded_text(&body, "email", 200) {
        Ok(v) => {
            if let Some(Some(e)) = v.as_ref() {
                if let Err(msg) = check_email(e) {
                    issues.push(msg);
                }
            }
            v
        }
        Err(e) => {
            issues.push(e);
            None
        }
    };
    let organization = match patch_bounded_text(&body, "organization", 200) {
        Ok(v) => v,
        Err(e) => {
            issues.push(e);
            None
        }
    };
    let job_title = match patch_bounded_text(&body, "jobTitle", 200) {
        Ok(v) => v,
        Err(e) => {
            issues.push(e);
            None
        }
    };
    let phone = match patch_bounded_text(&body, "phone", 60) {
        Ok(v) => v,
        Err(e) => {
            issues.push(e);
            None
        }
    };

    if !issues.is_empty() {
        return bad_request(issues.join(" "));
    }

    let patch = CustomerPatch {
        first_name,
        last_name,
        email,
        organization,
        job_title,
        phone,
    };
    if patch.is_empty() {
        return bad_request("No fields to update.".to_string());
    }

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match people_store::update_customer(&conn, id, &patch) {
        Ok(true) => match crate::customers::get_customer(&conn, id) {
            Ok(Some(c)) => {
                (StatusCode::OK, Json(json!({"ok": true, "customer": c}))).into_response()
            }
            _ => (StatusCode::OK, Json(json!({"ok": true, "customer_id": id}))).into_response(),
        },
        Ok(false) => not_found("Customer not found."),
        Err(e) => internal(e),
    }
}

pub async fn create_organization(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let mut issues: Vec<String> = Vec::new();

    let name = match bounded_text(&body, "name", 200, true) {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => {
            issues.push(e);
            None
        }
        None => None,
    };
    let domains = match parse_domains(&body) {
        Ok(v) => v,
        Err(e) => {
            issues.push(e);
            None
        }
    };

    if !issues.is_empty() {
        return bad_request(issues.join(" "));
    }
    let Some(name) = name else {
        return bad_request("name is required.".to_string());
    };

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match people_store::create_organization(&conn, &name, domains.as_deref().unwrap_or(&[])) {
        Ok(id) => match people_store::get_organization_detail(&conn, id) {
            Ok(Some(org)) => (
                StatusCode::OK,
                Json(json!({"ok": true, "organization": org})),
            )
                .into_response(),
            _ => (
                StatusCode::OK,
                Json(json!({"ok": true, "organization_id": id})),
            )
                .into_response(),
        },
        Err(e) => internal(e),
    }
}

pub async fn update_organization(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> Response {
    let mut issues: Vec<String> = Vec::new();

    let name = match bounded_text(&body, "name", 200, false) {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => {
            issues.push(e);
            None
        }
        None => None,
    };
    let domains = match parse_domains(&body) {
        Ok(v) => v,
        Err(e) => {
            issues.push(e);
            None
        }
    };

    if !issues.is_empty() {
        return bad_request(issues.join(" "));
    }
    if name.is_none() && domains.is_none() {
        return bad_request("No fields to update.".to_string());
    }

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match people_store::update_organization(&conn, id, name.as_deref(), domains.as_deref()) {
        Ok(true) => match people_store::get_organization_detail(&conn, id) {
            Ok(Some(org)) => (
                StatusCode::OK,
                Json(json!({"ok": true, "organization": org})),
            )
                .into_response(),
            _ => (
                StatusCode::OK,
                Json(json!({"ok": true, "organization_id": id})),
            )
                .into_response(),
        },
        Ok(false) => not_found("Organization not found."),
        Err(e) => internal(e),
    }
}

pub async fn timeline_rebuild(State(state): State<AppState>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match people_store::timeline_rebuild(&conn) {
        Ok(created) => {
            if let Err(e) = crate::audit::audit(
                &conn,
                &crate::audit::AuditEntry::user("customer_events_rebuilt")
                    .with_after_state(json!({"created": created})),
            ) {
                return internal(e);
            }
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "created": created,
                    "message": format!("Timeline rebuilt; {created} new event(s) derived.")
                })),
            )
                .into_response()
        }
        Err(e) => internal(e),
    }
}
