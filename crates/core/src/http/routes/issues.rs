use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use super::super::server::AppState;

pub async fn list_clusters(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let clusters: Vec<Value> = conn
        .prepare(
            "SELECT id, title, summary, category, product, feature,
                    conversation_count, customer_count, first_seen_at, last_seen_at,
                    trend, known_issue_id, ai_generated, created_at, updated_at, provenance
             FROM issue_clusters ORDER BY conversation_count DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            let rows: Vec<std::result::Result<Value, _>> = stmt
                .query_map([], |r| {
                    Ok(json!({
                        "id": r.get::<_, i64>(0)?,
                        "title": r.get::<_, String>(1)?,
                        "summary": r.get::<_, Option<String>>(2)?,
                        "category": r.get::<_, Option<String>>(3)?,
                        "product": r.get::<_, Option<String>>(4)?,
                        "feature": r.get::<_, Option<String>>(5)?,
                        "conversation_count": r.get::<_, i64>(6)?,
                        "customer_count": r.get::<_, Option<i64>>(7)?,
                        "first_seen_at": r.get::<_, Option<String>>(8)?,
                        "last_seen_at": r.get::<_, Option<String>>(9)?,
                        "trend": r.get::<_, Option<String>>(10)?,
                        "known_issue_id": r.get::<_, Option<i64>>(11)?,
                        "ai_generated": r.get::<_, Option<i64>>(12)?,
                        "created_at": r.get::<_, Option<String>>(13)?,
                        "updated_at": r.get::<_, Option<String>>(14)?,
                        "provenance": r.get::<_, Option<String>>(15)?,
                        "conversation_ids": cluster_conversation_ids(&conn, r.get::<_, i64>(0)?)?,
                    }))
                })
                .map(|rows| rows.collect())
                .unwrap_or_default();
            Some(rows.into_iter().filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    Json(json!({"clusters": clusters}))
}

fn cluster_conversation_ids(
    conn: &rusqlite::Connection,
    cluster_id: i64,
) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn
        .prepare("SELECT conversation_id FROM issue_cluster_conversations WHERE cluster_id = ?1")?;
    let ids = stmt
        .query_map(rusqlite::params![cluster_id], |r| r.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ids)
}

pub async fn sla_alerts(State(state): State<AppState>) -> axum::response::Response {
    use axum::response::IntoResponse;
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::sla::sla_alerts(&conn) {
        Ok(alerts) => (
            StatusCode::OK,
            Json(serde_json::to_value(&alerts).unwrap_or(Value::Null)),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn get_cluster(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> axum::response::Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let row = conn.query_row(
        "SELECT id, title, summary, category, product, feature,
                conversation_count, customer_count, first_seen_at, last_seen_at,
                trend, known_issue_id, ai_generated, created_at, updated_at, provenance
         FROM issue_clusters WHERE id = ?1",
        rusqlite::params![id],
        |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "title": r.get::<_, String>(1)?,
                "summary": r.get::<_, Option<String>>(2)?,
                "category": r.get::<_, Option<String>>(3)?,
                "product": r.get::<_, Option<String>>(4)?,
                "feature": r.get::<_, Option<String>>(5)?,
                "conversation_count": r.get::<_, i64>(6)?,
                "customer_count": r.get::<_, Option<i64>>(7)?,
                "first_seen_at": r.get::<_, Option<String>>(8)?,
                "last_seen_at": r.get::<_, Option<String>>(9)?,
                "trend": r.get::<_, Option<String>>(10)?,
                "known_issue_id": r.get::<_, Option<i64>>(11)?,
                "ai_generated": r.get::<_, Option<i64>>(12)?,
                "created_at": r.get::<_, Option<String>>(13)?,
                "updated_at": r.get::<_, Option<String>>(14)?,
                "provenance": r.get::<_, Option<String>>(15)?,
            }))
        },
    );
    match row {
        Ok(mut cluster) => {
            let conversation_ids = cluster_conversation_ids(&conn, id).unwrap_or_default();
            cluster["conversation_ids"] = json!(conversation_ids);
            let placeholders = if conversation_ids.is_empty() {
                "NULL".to_string()
            } else {
                vec!["?"; conversation_ids.len()].join(",")
            };
            let conversations: Vec<Value> = conn
                .prepare(&format!(
                    "SELECT id, number, subject, status,
                            COALESCE(remote_created_at, created_at) AS remote_created_at
                     FROM conversations WHERE id IN ({placeholders})"
                ))
                .and_then(|mut stmt| {
                    let rows: Vec<std::result::Result<Value, _>> = stmt
                        .query_map(rusqlite::params_from_iter(conversation_ids.iter()), |r| {
                            Ok(json!({
                                "id": r.get::<_, i64>(0)?,
                                "number": r.get::<_, Option<i64>>(1)?,
                                "subject": r.get::<_, Option<String>>(2)?,
                                "status": r.get::<_, Option<String>>(3)?,
                                "remote_created_at": r.get::<_, Option<String>>(4)?,
                            }))
                        })?
                        .collect();
                    Ok(rows.into_iter().filter_map(|r| r.ok()).collect())
                })
                .unwrap_or_default();
            (
                StatusCode::OK,
                Json(json!({"cluster": cluster, "conversations": conversations})),
            )
                .into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Cluster not found."
            })),
        )
            .into_response(),
    }
}

pub async fn delete_cluster(State(state): State<AppState>, Path(id): Path<i64>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let _ = conn.execute(
        "DELETE FROM issue_clusters WHERE id = ?1",
        rusqlite::params![id],
    );
    Json(json!({"ok": true, "message": "Cluster deleted (conversations are untouched)."}))
}

pub async fn list_known(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let issues: Vec<Value> = conn
        .prepare(&format!("{KNOWN_ISSUE_SELECT} ORDER BY last_seen_at DESC"))
        .ok()
        .and_then(|mut stmt| {
            let rows: Vec<std::result::Result<Value, _>> = stmt
                .query_map([], |r| known_issue_json(&conn, r))
                .map(|rows| rows.collect())
                .unwrap_or_default();
            Some(rows.into_iter().filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    Json(json!({"known_issues": issues}))
}

fn known_issue_conversation_ids(
    conn: &rusqlite::Connection,
    known_issue_id: i64,
) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT conversation_id FROM known_issue_links WHERE known_issue_id = ?1 ORDER BY id",
    )?;
    let ids = stmt
        .query_map(rusqlite::params![known_issue_id], |r| r.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ids)
}

fn known_issue_refs(conn: &rusqlite::Connection, known_issue_id: i64) -> rusqlite::Result<Value> {
    let mut stmt = conn.prepare(
        "SELECT id, known_issue_id, system, reference_id, url, title, status, notes
           FROM known_issue_refs WHERE known_issue_id = ?1 ORDER BY id",
    )?;
    let refs: Vec<Value> = stmt
        .query_map(rusqlite::params![known_issue_id], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "known_issue_id": r.get::<_, i64>(1)?,
                "system": r.get::<_, Option<String>>(2)?,
                "reference_id": r.get::<_, Option<String>>(3)?,
                "url": r.get::<_, Option<String>>(4)?,
                "title": r.get::<_, Option<String>>(5)?,
                "status": r.get::<_, Option<String>>(6)?,
                "notes": r.get::<_, Option<String>>(7)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(json!(refs))
}

pub async fn create_known(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let input = match validate_known_issue_body(&body) {
        Ok(input) => input,
        Err(issues) => {
            let refs: Vec<(&str, &str)> = issues
                .iter()
                .map(|(path, message)| (path.as_str(), message.as_str()))
                .collect();
            return crate::conversation_ops::zod_422_multi(&refs);
        }
    };
    let mut conn = state.conn_lock();
    let tx = match conn.transaction() {
        Ok(tx) => tx,
        Err(e) => return db_error_500(&e),
    };
    let status = input
        .status
        .clone()
        .unwrap_or_else(|| "investigating".into());
    let provenance = input
        .provenance
        .clone()
        .unwrap_or_else(|| "human_local".into());
    if let Err(e) = tx.execute(
        "INSERT INTO known_issues (name, status, description, title, symptoms, product, feature,
                                    known_cause, workaround, customer_safe_explanation,
                                    internal_explanation, provenance)
         VALUES (?1, ?2, '', ?1, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            input.title,
            status,
            input.symptoms.clone().unwrap_or_default(),
            input.product,
            input.feature,
            input.known_cause,
            input.workaround,
            input.customer_safe_explanation,
            input.internal_explanation,
            provenance,
        ],
    ) {
        return db_error_500(&e);
    }
    let id = tx.last_insert_rowid();
    let link_source = if provenance == "ai_generated" {
        "ai"
    } else {
        "human"
    };
    for conv_id in &input.conversation_ids {
        if let Err(e) = tx.execute(
            "INSERT OR IGNORE INTO known_issue_links (known_issue_id, conversation_id, link_type)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![id, conv_id, link_source],
        ) {
            return db_error_500(&e);
        }
    }
    if let Err(e) = crate::intelligence_features::refresh_known_issue_counts(&tx, id) {
        return db_error_500(&e);
    }
    if let Err(e) = crate::search::index_known_issue_fts(&tx, id) {
        return db_error_500(&e);
    }
    if let Err(e) = tx.commit() {
        return db_error_500(&e);
    }
    if let Err(e) = crate::audit::audit(
        &conn,
        &crate::audit::AuditEntry::user("known_issue_created")
            .with_after_state(json!({"id": id, "title": input.title})),
    ) {
        return db_error_500(&e);
    }
    Json(json!({"ok": true, "message": "Known issue created.", "id": id})).into_response()
}

pub async fn get_known(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> axum::response::Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let row = conn.query_row(
        &format!("{KNOWN_ISSUE_SELECT} WHERE id = ?1"),
        rusqlite::params![id],
        |r| known_issue_json(&conn, r),
    );
    match row {
        Ok(known_issue) => {
            let conversation_ids: Vec<i64> = known_issue["conversation_ids"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_i64()).collect::<Vec<i64>>())
                .unwrap_or_default();
            let placeholders = if conversation_ids.is_empty() {
                "NULL".to_string()
            } else {
                vec!["?"; conversation_ids.len()].join(",")
            };
            let conversations: Vec<Value> = conn
                .prepare(&format!(
                    "SELECT id, number, subject, status FROM conversations WHERE id IN ({placeholders})"
                ))
                .and_then(|mut stmt| {
                    let rows: Vec<std::result::Result<Value, _>> = stmt
                        .query_map(rusqlite::params_from_iter(conversation_ids.iter()), |r| {
                            Ok(json!({
                                "id": r.get::<_, i64>(0)?,
                                "number": r.get::<_, Option<i64>>(1)?,
                                "subject": r.get::<_, Option<String>>(2)?,
                                "status": r.get::<_, Option<String>>(3)?,
                            }))
                        })?
                        .collect();
                    Ok(rows.into_iter().filter_map(|r| r.ok()).collect())
                })
                .unwrap_or_default();
            (
                StatusCode::OK,
                Json(json!({"known_issue": known_issue, "conversations": conversations})),
            )
                .into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Known issue not found."
            })),
        )
            .into_response(),
    }
}

pub async fn update_known(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let mut patch = match validate_known_issue_patch(&body) {
        Ok(patch) => patch,
        Err(issues) => {
            let refs: Vec<(&str, &str)> = issues
                .iter()
                .map(|(path, message)| (path.as_str(), message.as_str()))
                .collect();
            return crate::conversation_ops::zod_422_multi(&refs);
        }
    };
    patch.id = id;
    let mut conn = state.conn_lock();
    if !patch.is_empty() {
        let tx = match conn.transaction() {
            Ok(tx) => tx,
            Err(e) => return db_error_500(&e),
        };
        if let Err(e) = tx.execute(&patch.sql(), rusqlite::params_from_iter(patch.params())) {
            return db_error_500(&e);
        }
        if patch.refreshes_fts() {
            if let Err(e) = crate::search::index_known_issue_fts(&tx, id) {
                return db_error_500(&e);
            }
        }
        if let Err(e) = tx.commit() {
            return db_error_500(&e);
        }
    }
    if let Err(e) = crate::audit::audit(
        &conn,
        &crate::audit::AuditEntry::user("known_issue_updated").with_before_state(json!({"id": id})),
    ) {
        return db_error_500(&e);
    }
    Json(json!({"ok": true, "message": "Known issue updated."})).into_response()
}

pub async fn delete_known(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> axum::response::Response {
    let mut conn = state.conn_lock();
    let tx = match conn.transaction() {
        Ok(tx) => tx,
        Err(e) => return db_error_500(&e),
    };
    for sql in [
        "DELETE FROM fts_known_issues WHERE known_issue_id = ?1",
        "DELETE FROM known_issue_refs WHERE known_issue_id = ?1",
        "DELETE FROM known_issue_links WHERE known_issue_id = ?1",
        "DELETE FROM known_issues WHERE id = ?1",
    ] {
        if let Err(e) = tx.execute(sql, rusqlite::params![id]) {
            return db_error_500(&e);
        }
    }
    if let Err(e) = tx.commit() {
        return db_error_500(&e);
    }
    Json(json!({"ok": true, "message": "Known issue deleted."})).into_response()
}

pub async fn link_known(
    State(state): State<AppState>,
    Path((id, conversation_id)): Path<(i64, i64)>,
) -> axum::response::Response {
    let mut conn = state.conn_lock();
    let tx = match conn.transaction() {
        Ok(tx) => tx,
        Err(e) => return db_error_500(&e),
    };
    if let Err(e) = tx.execute(
        "INSERT OR IGNORE INTO known_issue_links (known_issue_id, conversation_id, link_type)
         VALUES (?1, ?2, 'human')",
        rusqlite::params![id, conversation_id],
    ) {
        return db_error_500(&e);
    }
    if let Err(e) = crate::intelligence_features::refresh_known_issue_counts(&tx, id) {
        return db_error_500(&e);
    }
    if let Err(e) = tx.commit() {
        return db_error_500(&e);
    }
    if let Err(e) = crate::audit::audit(
        &conn,
        &crate::audit::AuditEntry::user("known_issue_linked")
            .with_conversation_id(conversation_id)
            .with_after_state(json!({"known_issue_id": id})),
    ) {
        return db_error_500(&e);
    }
    Json(json!({"ok": true, "message": "Conversation linked to known issue."})).into_response()
}

pub async fn unlink_known(
    State(state): State<AppState>,
    Path((id, conversation_id)): Path<(i64, i64)>,
) -> axum::response::Response {
    let mut conn = state.conn_lock();
    let tx = match conn.transaction() {
        Ok(tx) => tx,
        Err(e) => return db_error_500(&e),
    };
    if let Err(e) = tx.execute(
        "DELETE FROM known_issue_links WHERE known_issue_id = ?1 AND conversation_id = ?2",
        rusqlite::params![id, conversation_id],
    ) {
        return db_error_500(&e);
    }
    if let Err(e) = crate::intelligence_features::refresh_known_issue_counts(&tx, id) {
        return db_error_500(&e);
    }
    if let Err(e) = tx.commit() {
        return db_error_500(&e);
    }
    Json(json!({"ok": true, "message": "Conversation unlinked."})).into_response()
}

const KNOWN_ISSUE_SELECT: &str = "SELECT id, COALESCE(title, name), symptoms, product, feature,
       known_cause, workaround, customer_safe_explanation, internal_explanation, status,
       first_seen_at, last_seen_at, COALESCE(conversation_count, 0),
       COALESCE(provenance, 'human_local'), created_at, updated_at
 FROM known_issues";

fn known_issue_json(conn: &rusqlite::Connection, r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let id: i64 = r.get(0)?;
    Ok(json!({
        "id": id,
        "title": r.get::<_, String>(1)?,
        "symptoms": r.get::<_, Option<String>>(2)?,
        "product": r.get::<_, Option<String>>(3)?,
        "feature": r.get::<_, Option<String>>(4)?,
        "known_cause": r.get::<_, Option<String>>(5)?,
        "workaround": r.get::<_, Option<String>>(6)?,
        "customer_safe_explanation": r.get::<_, Option<String>>(7)?,
        "internal_explanation": r.get::<_, Option<String>>(8)?,
        "status": r.get::<_, String>(9)?,
        "first_seen_at": r.get::<_, Option<String>>(10)?,
        "last_seen_at": r.get::<_, Option<String>>(11)?,
        "conversation_count": r.get::<_, i64>(12)?,
        "provenance": r.get::<_, String>(13)?,
        "created_at": r.get::<_, Option<String>>(14)?,
        "updated_at": r.get::<_, Option<String>>(15)?,
        "conversation_ids": known_issue_conversation_ids(conn, id)?,
        "engineering_refs": known_issue_refs(conn, id)?,
    }))
}

fn db_error_500(e: &dyn std::fmt::Display) -> Response {
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

const KNOWN_ISSUE_STATUSES: [&str; 5] = [
    "open",
    "investigating",
    "identified",
    "monitoring",
    "resolved",
];
const KNOWN_ISSUE_PROVENANCES: [&str; 2] = ["human_local", "ai_generated"];

fn received_type(v: &Value) -> &'static str {
    match v {
        Value::String(_) => "string",
        Value::Number(n) if n.as_f64().map(|f| f.fract() != 0.0).unwrap_or(false) => "float",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        Value::Null => "null",
    }
}

fn check_string_field(
    body: &Value,
    field: &'static str,
    max_chars: usize,
    min_one: bool,
    nullable: bool,
    required: bool,
    issues: &mut Vec<(String, String)>,
) -> std::result::Result<Option<String>, ()> {
    match body.get(field) {
        None if required => {
            issues.push((field.to_string(), "Required".to_string()));
            Err(())
        }
        None => Err(()),
        Some(Value::Null) if nullable => Ok(None),
        Some(Value::Null) => {
            issues.push((
                field.to_string(),
                "Expected string, received null".to_string(),
            ));
            Err(())
        }
        Some(Value::String(s)) => {
            let len = s.chars().count();
            if min_one && len < 1 {
                issues.push((
                    field.to_string(),
                    "String must contain at least 1 character(s)".to_string(),
                ));
                Err(())
            } else if len > max_chars {
                issues.push((
                    field.to_string(),
                    format!("String must contain at most {max_chars} character(s)"),
                ));
                Err(())
            } else {
                Ok(Some(s.clone()))
            }
        }
        Some(v) => {
            issues.push((
                field.to_string(),
                format!("Expected string, received {}", received_type(v)),
            ));
            Err(())
        }
    }
}

fn check_enum_field(
    body: &Value,
    field: &'static str,
    variants: &[&str],
    issues: &mut Vec<(String, String)>,
) -> std::result::Result<String, ()> {
    let expected = variants
        .iter()
        .map(|v| format!("'{v}'"))
        .collect::<Vec<_>>()
        .join(" | ");
    match body.get(field) {
        None => Err(()),
        Some(Value::String(s)) if variants.contains(&s.as_str()) => Ok(s.clone()),
        Some(Value::String(s)) => {
            issues.push((
                field.to_string(),
                format!("Invalid enum value. Expected {expected}, received '{s}'"),
            ));
            Err(())
        }
        Some(v) => {
            issues.push((
                field.to_string(),
                format!("Expected {expected}, received {}", received_type(v)),
            ));
            Err(())
        }
    }
}

fn check_conversation_ids(
    body: &Value,
    issues: &mut Vec<(String, String)>,
) -> std::result::Result<Vec<i64>, ()> {
    match body.get("conversation_ids") {
        None => Err(()),
        Some(Value::Array(items)) => {
            let mut ids = Vec::new();
            for (idx, item) in items.iter().enumerate() {
                let path = format!("conversation_ids.{idx}");
                match item.as_f64() {
                    Some(f) if f.fract() != 0.0 || f < i64::MIN as f64 || f > i64::MAX as f64 => {
                        issues.push((path, "Expected integer, received float".to_string()));
                    }
                    Some(f) if f <= 0.0 => {
                        issues.push((path, "Number must be greater than 0".to_string()));
                    }
                    Some(f) => ids.push(f as i64),
                    None => {
                        issues.push((
                            path,
                            format!("Expected number, received {}", received_type(item)),
                        ));
                    }
                }
            }
            Ok(ids)
        }
        Some(v) => {
            issues.push((
                "conversation_ids".to_string(),
                format!("Expected array, received {}", received_type(v)),
            ));
            Err(())
        }
    }
}

struct KnownIssueInput {
    title: String,
    symptoms: Option<String>,
    product: Option<String>,
    feature: Option<String>,
    known_cause: Option<String>,
    workaround: Option<String>,
    customer_safe_explanation: Option<String>,
    internal_explanation: Option<String>,
    status: Option<String>,
    conversation_ids: Vec<i64>,
    provenance: Option<String>,
}

fn validate_known_issue_body(
    body: &Value,
) -> std::result::Result<KnownIssueInput, Vec<(String, String)>> {
    let mut issues: Vec<(String, String)> = Vec::new();
    if !body.is_object() && !body.is_null() {
        return Err(vec![(
            String::new(),
            format!("Expected object, received {}", received_type(body)),
        )]);
    }
    let title = check_string_field(body, "title", 300, true, false, true, &mut issues);
    let symptoms = check_string_field(body, "symptoms", 5000, false, false, false, &mut issues);
    let product = check_string_field(body, "product", 200, false, true, false, &mut issues);
    let feature = check_string_field(body, "feature", 200, false, true, false, &mut issues);
    let known_cause =
        check_string_field(body, "known_cause", 10000, false, true, false, &mut issues);
    let workaround = check_string_field(body, "workaround", 10000, false, true, false, &mut issues);
    let customer_safe_explanation = check_string_field(
        body,
        "customer_safe_explanation",
        10000,
        false,
        true,
        false,
        &mut issues,
    );
    let internal_explanation = check_string_field(
        body,
        "internal_explanation",
        20000,
        false,
        true,
        false,
        &mut issues,
    );
    let status = check_enum_field(body, "status", &KNOWN_ISSUE_STATUSES, &mut issues);
    let conversation_ids = check_conversation_ids(body, &mut issues);
    let provenance = check_enum_field(body, "provenance", &KNOWN_ISSUE_PROVENANCES, &mut issues);
    if !issues.is_empty() {
        return Err(issues);
    }
    Ok(KnownIssueInput {
        title: title.ok().flatten().unwrap_or_default(),
        symptoms: symptoms.ok().flatten(),
        product: product.ok().flatten(),
        feature: feature.ok().flatten(),
        known_cause: known_cause.ok().flatten(),
        workaround: workaround.ok().flatten(),
        customer_safe_explanation: customer_safe_explanation.ok().flatten(),
        internal_explanation: internal_explanation.ok().flatten(),
        status: status.ok(),
        conversation_ids: conversation_ids.unwrap_or_default(),
        provenance: provenance.ok(),
    })
}

enum PatchField {
    Title(String),
    Symptoms(String),
    Product(Option<String>),
    Feature(Option<String>),
    KnownCause(Option<String>),
    Workaround(Option<String>),
    CustomerSafeExplanation(Option<String>),
    InternalExplanation(Option<String>),
    Status(String),
}

struct KnownIssuePatch {
    id: i64,
    fields: Vec<PatchField>,
}

impl KnownIssuePatch {
    fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    fn sql(&self) -> String {
        let mut sets: Vec<String> = Vec::with_capacity(self.fields.len() + 2);
        for (idx, field) in self.fields.iter().enumerate() {
            let column = match field {
                PatchField::Title(_) => "title",
                PatchField::Symptoms(_) => "symptoms",
                PatchField::Product(_) => "product",
                PatchField::Feature(_) => "feature",
                PatchField::KnownCause(_) => "known_cause",
                PatchField::Workaround(_) => "workaround",
                PatchField::CustomerSafeExplanation(_) => "customer_safe_explanation",
                PatchField::InternalExplanation(_) => "internal_explanation",
                PatchField::Status(_) => "status",
            };
            sets.push(format!("{column} = ?{}", idx + 1));
        }
        sets.push("updated_at = datetime('now')".to_string());
        format!(
            "UPDATE known_issues SET {} WHERE id = ?{}",
            sets.join(", "),
            self.fields.len() + 1
        )
    }

    fn params(&self) -> Vec<Box<dyn rusqlite::ToSql>> {
        let mut out: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(self.fields.len() + 1);
        for field in &self.fields {
            match field {
                PatchField::Title(s) | PatchField::Symptoms(s) | PatchField::Status(s) => {
                    out.push(Box::new(s.clone()))
                }
                PatchField::Product(v)
                | PatchField::Feature(v)
                | PatchField::KnownCause(v)
                | PatchField::Workaround(v)
                | PatchField::CustomerSafeExplanation(v)
                | PatchField::InternalExplanation(v) => out.push(Box::new(v.clone())),
            }
        }
        out.push(Box::new(self.id));
        out
    }

    fn refreshes_fts(&self) -> bool {
        self.fields.iter().any(|field| match field {
            PatchField::Title(s) | PatchField::Symptoms(s) => !s.is_empty(),
            PatchField::Workaround(v) | PatchField::CustomerSafeExplanation(v) => {
                v.as_deref().is_some_and(|s| !s.is_empty())
            }
            _ => false,
        })
    }
}

fn validate_known_issue_patch(
    body: &Value,
) -> std::result::Result<KnownIssuePatch, Vec<(String, String)>> {
    let mut issues: Vec<(String, String)> = Vec::new();
    if !body.is_object() && !body.is_null() {
        return Err(vec![(
            String::new(),
            format!("Expected object, received {}", received_type(body)),
        )]);
    }
    let mut fields: Vec<PatchField> = Vec::new();
    if let Ok(Some(s)) = check_string_field(body, "title", 300, true, false, false, &mut issues) {
        fields.push(PatchField::Title(s));
    }
    if let Ok(Some(s)) =
        check_string_field(body, "symptoms", 5000, false, false, false, &mut issues)
    {
        fields.push(PatchField::Symptoms(s));
    }
    if let Ok(v) = check_string_field(body, "product", 200, false, true, false, &mut issues) {
        fields.push(PatchField::Product(v));
    }
    if let Ok(v) = check_string_field(body, "feature", 200, false, true, false, &mut issues) {
        fields.push(PatchField::Feature(v));
    }
    if let Ok(v) = check_string_field(body, "known_cause", 10000, false, true, false, &mut issues) {
        fields.push(PatchField::KnownCause(v));
    }
    if let Ok(v) = check_string_field(body, "workaround", 10000, false, true, false, &mut issues) {
        fields.push(PatchField::Workaround(v));
    }
    if let Ok(v) = check_string_field(
        body,
        "customer_safe_explanation",
        10000,
        false,
        true,
        false,
        &mut issues,
    ) {
        fields.push(PatchField::CustomerSafeExplanation(v));
    }
    if let Ok(v) = check_string_field(
        body,
        "internal_explanation",
        20000,
        false,
        true,
        false,
        &mut issues,
    ) {
        fields.push(PatchField::InternalExplanation(v));
    }
    if let Ok(s) = check_enum_field(body, "status", &KNOWN_ISSUE_STATUSES, &mut issues) {
        fields.push(PatchField::Status(s));
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    Ok(KnownIssuePatch { id: 0, fields })
}

pub async fn add_ref(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let mut issues: Vec<(String, String)> = Vec::new();
    if !body.is_object() && !body.is_null() {
        return crate::conversation_ops::zod_422_multi(&[(
            "",
            &format!("Expected object, received {}", received_type(&body)),
        )]);
    }
    let system = check_string_field(&body, "system", 100, true, false, true, &mut issues);
    let reference_id =
        check_string_field(&body, "reference_id", 200, true, false, true, &mut issues);
    let url = check_string_field(&body, "url", 2000, false, false, false, &mut issues);
    let title = check_string_field(&body, "title", 500, false, false, false, &mut issues);
    let status = check_string_field(&body, "status", 100, false, false, false, &mut issues);
    let notes = check_string_field(&body, "notes", 5000, false, false, false, &mut issues);
    if !issues.is_empty() {
        let refs: Vec<(&str, &str)> = issues
            .iter()
            .map(|(path, message)| (path.as_str(), message.as_str()))
            .collect();
        return crate::conversation_ops::zod_422_multi(&refs);
    }
    let system = system.ok().flatten().unwrap_or_default();
    let reference_id = reference_id.ok().flatten().unwrap_or_default();
    let (url, title, status, notes) = (
        url.ok().flatten(),
        title.ok().flatten(),
        status.ok().flatten(),
        notes.ok().flatten(),
    );
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match conn.execute(
        "INSERT INTO known_issue_refs (known_issue_id, system, reference_id, url, title, status, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, system, reference_id, url, title, status, notes],
    ) {
        Ok(_) => {
            Json(json!({"ok": true, "message": "Engineering reference added."})).into_response()
        }
        Err(e) => db_error_500(&e),
    }
}

fn support_case_json(
    r: &rusqlite::Row<'_>,
) -> rusqlite::Result<std::result::Result<Value, String>> {
    let tags: Option<String> = r.get(9)?;
    let tags = match serde_json::from_str::<Value>(tags.as_deref().unwrap_or("[]")) {
        Ok(v) if v.is_array() => Ok(v),
        Ok(_) | Err(_) => Err(tags.unwrap_or_default()),
    };
    Ok(match tags {
        Ok(tags) => Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "conversation_id": r.get::<_, Option<i64>>(1)?,
            "customer_id": r.get::<_, Option<i64>>(2)?,
            "problem": r.get::<_, Option<String>>(3)?,
            "root_question": r.get::<_, Option<String>>(4)?,
            "resolution": r.get::<_, Option<String>>(5)?,
            "answer": r.get::<_, Option<String>>(6)?,
            "product": r.get::<_, Option<String>>(7)?,
            "feature": r.get::<_, Option<String>>(8)?,
            "tags": tags,
            "fields": r.get::<_, Option<String>>(10)?,
            "agent_user_id": r.get::<_, Option<i64>>(11)?,
            "resolution_time_min": r.get::<_, Option<f64>>(12)?,
            "rating": r.get::<_, Option<String>>(13)?,
            "created_at": r.get::<_, String>(14)?,
            "provenance": r.get::<_, Option<String>>(15)?,
        })),
        Err(bad) => Err(bad),
    })
}

pub async fn list_cases(State(state): State<AppState>) -> axum::response::Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let rows: Vec<std::result::Result<Value, String>> = match conn.prepare(
        "SELECT id, conversation_id, customer_id, problem, root_question, resolution,
                answer, product, feature, tags, fields, agent_user_id, resolution_time_min,
                rating, created_at, provenance
         FROM support_cases ORDER BY created_at DESC LIMIT 500",
    ) {
        Ok(mut stmt) => match stmt.query_map([], support_case_json) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => return db_error_500(&e),
        },
        Err(e) => return db_error_500(&e),
    };
    let mut cases = Vec::with_capacity(rows.len());
    for row in rows {
        match row {
            Ok(v) => cases.push(v),
            Err(row) => {
                return db_error_500(&format!("support_cases.tags is not valid JSON: {row}"));
            }
        }
    }
    Json(json!({ "cases": cases })).into_response()
}

pub async fn case_from_conversation(
    State(state): State<AppState>,
    Path(conversation_id): Path<i64>,
) -> axum::response::Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let conv: Option<(Option<String>, Option<i64>, Option<i64>)> = conn
        .query_row(
            "SELECT subject, customer_local_id, assignee_local_id FROM conversations WHERE id = ?1",
            rusqlite::params![conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    let Some((subject, customer_id, agent_user_id)) = conv else {
        return Json(json!({"ok": false, "message": "Conversation not found."})).into_response();
    };
    let analysis = crate::ai_pipeline::get_latest_analysis(&conn, conversation_id)
        .ok()
        .flatten()
        .map(|la| la.analysis);
    let last_reply: Option<String> = conn
        .query_row(
            "SELECT body_text FROM conversation_threads
              WHERE conversation_id = ?1 AND type = 'reply' AND state = 'published'
              ORDER BY COALESCE(remote_created_at, created_at) DESC LIMIT 1",
            rusqlite::params![conversation_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let truncated: Option<String> = last_reply
        .as_deref()
        .map(|b| b.chars().take(2000).collect::<String>());
    let tags: Vec<String> = conn
        .prepare(
            "SELECT t.name FROM conversation_tags ct JOIN tags t ON t.id = ct.tag_id
              WHERE ct.conversation_id = ?1",
        )
        .and_then(|mut stmt| {
            let rows = stmt.query_map(rusqlite::params![conversation_id], |r| {
                r.get::<_, String>(0)
            })?;
            Ok(rows.filter_map(std::result::Result::ok).collect())
        })
        .unwrap_or_default();
    let rating: Option<String> = conn
        .query_row(
            "SELECT rating FROM ratings WHERE conversation_id = ?1 LIMIT 1",
            rusqlite::params![conversation_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let problem = analysis
        .as_ref()
        .and_then(|a| a.customer_goal.clone())
        .or(subject);
    let root_question = analysis.as_ref().and_then(|a| a.primary_question.clone());
    let product = analysis.as_ref().and_then(|a| a.product.clone());
    let feature = analysis.as_ref().and_then(|a| a.feature.clone());
    match conn.execute(
        "INSERT INTO support_cases (conversation_id, customer_id, problem, root_question,
                                    resolution, answer, product, feature, tags, agent_user_id,
                                    resolution_time_min, rating)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, ?11)
         ON CONFLICT(conversation_id) DO UPDATE SET
           customer_id=excluded.customer_id, problem=excluded.problem,
           root_question=excluded.root_question, resolution=excluded.resolution,
           answer=excluded.answer, product=excluded.product, feature=excluded.feature,
           tags=excluded.tags, agent_user_id=excluded.agent_user_id,
           resolution_time_min=excluded.resolution_time_min, rating=excluded.rating",
        rusqlite::params![
            conversation_id,
            customer_id,
            problem,
            root_question,
            truncated,
            truncated,
            product,
            feature,
            serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into()),
            agent_user_id,
            rating,
        ],
    ) {
        Ok(_) => {
            Json(json!({"ok": true, "message": "Support case captured from this conversation."}))
                .into_response()
        }
        Err(e) => db_error_500(&e),
    }
}

pub async fn known_impact(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::issue_impact::for_known_issue(&conn, id) {
        Some(impact) => (StatusCode::OK, Json(json!({"impact": impact}))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Known issue not found."
            })),
        )
            .into_response(),
    }
}
