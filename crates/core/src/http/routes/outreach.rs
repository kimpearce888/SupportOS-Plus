use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use super::super::server::AppState;

pub async fn meta(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let _ = crate::outreach::ensure_segments_v2(&conn);

    let valid_type = |t: Option<&str>| -> &'static str {
        match t {
            Some("text") | Some("number") | Some("date") | Some("dropdown") | Some("url") => {
                match t {
                    Some("number") => "number",
                    Some("date") => "date",
                    Some("dropdown") => "dropdown",
                    Some("url") => "url",
                    _ => "text",
                }
            }
            _ => "text",
        }
    };

    let mut prop_defs: Vec<Value> = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, remote_id, name, slug, type, sort_order
           FROM customer_property_definitions ORDER BY sort_order, name",
    ) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        }) {
            for row in rows.flatten() {
                let (id, _remote_id, name, slug, ptype) = row;
                let mut observed: Vec<String> = Vec::new();
                let mut populated: i64 = 0;
                if let Ok(mut s2) = conn.prepare(
                    "SELECT value, COUNT(*) AS n FROM customer_properties
                      WHERE definition_id = ?1 AND value IS NOT NULL AND value <> ''
                      GROUP BY value ORDER BY n DESC LIMIT 25",
                ) {
                    if let Ok(vals) =
                        s2.query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
                    {
                        for (v, n) in vals.flatten() {
                            observed.push(v);
                            populated += n;
                        }
                    }
                }
                prop_defs.push(json!({
                    "id": id,
                    "remote_id": _remote_id,
                    "name": name,
                    "slug": slug,
                    "type": valid_type(ptype.as_deref()),
                    "observed_values": observed,
                    "populated": populated,
                }));
            }
        }
    }

    let mut org_defs: Vec<Value> = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, remote_id, name, slug, type, sort_order
           FROM organization_property_definitions ORDER BY sort_order, name",
    ) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        }) {
            for row in rows.flatten() {
                let (id, _remote_id, name, slug, ptype) = row;
                let mut observed: Vec<String> = Vec::new();
                if let Ok(mut s2) = conn.prepare(
                    "SELECT DISTINCT value FROM organization_properties
                      WHERE definition_id = ?1 AND value IS NOT NULL AND value <> ''
                      ORDER BY value LIMIT 20",
                ) {
                    if let Ok(vals) = s2.query_map([id], |r| r.get::<_, String>(0)) {
                        observed = vals.flatten().collect();
                    }
                }
                let populated: i64 = conn
                    .query_row(
                        "SELECT COUNT(DISTINCT organization_id) FROM organization_properties
                          WHERE definition_id = ?1 AND value IS NOT NULL AND value <> ''",
                        [id],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                org_defs.push(json!({
                    "id": id,
                    "remote_id": _remote_id,
                    "name": name,
                    "slug": slug,
                    "type": valid_type(ptype.as_deref()),
                    "observed_values": observed,
                    "populated": populated,
                }));
            }
        }
    }

    let str_list = |sql: &str| -> Vec<String> {
        conn.prepare(sql)
            .and_then(|mut s| {
                let rows = s.query_map([], |r| r.get::<_, String>(0))?;
                Ok(rows.filter_map(|x| x.ok()).collect())
            })
            .unwrap_or_default()
    };

    let tags = str_list("SELECT name FROM tags WHERE deleted_at IS NULL ORDER BY name");
    let mailboxes: Vec<Value> = conn
        .prepare("SELECT id, name, email FROM mailboxes WHERE deleted_at IS NULL ORDER BY name")
        .and_then(|mut s| {
            let rows = s.query_map([], |r| {
                Ok(json!({
                    "local_id": r.get::<_, i64>(0)?,
                    "name": r.get::<_, String>(1)?,
                    "email": r.get::<_, Option<String>>(2)?,
                }))
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();
    let assignees: Vec<Value> = conn
        .prepare(
            "SELECT id, first_name, last_name FROM users
                   WHERE deleted_at IS NULL AND type = 'user' ORDER BY last_name",
        )
        .and_then(|mut s| {
            let rows = s.query_map([], |r| {
                let first: Option<String> = r.get(1)?;
                let last: Option<String> = r.get(2)?;
                let name = [first, last]
                    .iter()
                    .flatten()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                Ok(json!({
                    "local_id": r.get::<_, i64>(0)?,
                    "name": name,
                }))
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();
    let ticket_custom_fields: Vec<Value> = conn
        .prepare("SELECT id, name, type FROM inbox_fields WHERE deleted_at IS NULL ORDER BY sort_order, name")
        .and_then(|mut s| {
            let rows = s.query_map([], |r| {
                Ok(json!({
                    "local_id": r.get::<_, i64>(0)?,
                    "name": r.get::<_, String>(1)?,
                    "type": r.get::<_, Option<String>>(2)?,
                }))
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();
    let channels = str_list(
        "SELECT DISTINCT source_type FROM conversations
          WHERE source_type IS NOT NULL AND source_type <> '' AND deleted_at IS NULL
          ORDER BY source_type",
    );
    let mut issues: Vec<Value> = Vec::new();
    if let Ok(mut stmt) =
        conn.prepare("SELECT id, title FROM known_issues ORDER BY conversation_count DESC LIMIT 50")
    {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok(json!({"id": r.get::<_, i64>(0)?, "kind": "known_issue", "label": r.get::<_, String>(1)?}))
        }) {
            issues.extend(rows.flatten());
        }
    }
    if let Ok(mut stmt) = conn
        .prepare("SELECT id, name FROM issue_clusters ORDER BY conversation_count DESC LIMIT 50")
    {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok(json!({"id": r.get::<_, i64>(0)?, "kind": "cluster", "label": r.get::<_, String>(1)?}))
        }) {
            issues.extend(rows.flatten());
        }
    }
    let incidents: Vec<Value> = conn
        .prepare("SELECT id, code, title, status FROM incidents ORDER BY updated_at DESC LIMIT 50")
        .and_then(|mut s| {
            let rows = s.query_map([], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "code": r.get::<_, String>(1)?,
                    "title": r.get::<_, String>(2)?,
                    "status": r.get::<_, String>(3)?,
                }))
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();
    let campaigns_meta: Vec<Value> = conn
        .prepare(
            "SELECT id, name, status FROM outreach_campaigns ORDER BY updated_at DESC LIMIT 50",
        )
        .and_then(|mut s| {
            let rows = s.query_map([], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "name": r.get::<_, String>(1)?,
                    "status": r.get::<_, String>(2)?,
                }))
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();
    let custom_object_types: Vec<Value> = conn
        .prepare(
            "SELECT id, name, slug FROM custom_object_types WHERE deleted_at IS NULL ORDER BY name",
        )
        .and_then(|mut s| {
            let rows = s.query_map([], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "name": r.get::<_, String>(1)?,
                    "slug": r.get::<_, String>(2)?,
                }))
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();

    Json(json!({
        "property_definitions": prop_defs,
        "tags": tags,
        "mailboxes": mailboxes,
        "assignees": assignees,
        "contact_fields": [
            "name", "email", "email_domain", "organization", "job_title", "location",
            "background", "has_email", "has_phone", "has_multiple_emails"
        ],
        "ticket_statuses": ["active", "pending", "closed", "spam"],
        "operators_by_type": {
            "text": ["equals", "not_equals", "contains", "not_contains", "starts_with", "ends_with", "is_empty", "is_not_empty"],
            "url": ["equals", "contains", "starts_with", "ends_with", "is_empty", "is_not_empty"],
            "number": ["equals", "not_equals", "gt", "gte", "lt", "lte", "between", "is_empty", "is_not_empty"],
            "date": ["equals", "before", "after", "between", "is_empty", "is_not_empty"],
            "dropdown": ["equals", "not_equals", "is_any_of", "is_none_of", "is_empty", "is_not_empty"]
        },
        "personalization_variables": ["first_name", "last_name", "company", "organization", "last_ticket_number", "last_ticket_subject"],
        "organization_fields": ["name", "domains"],
        "organization_property_definitions": org_defs,
        "ticket_custom_fields": ticket_custom_fields,
        "channels": channels,
        "issues": issues,
        "incidents": incidents,
        "campaigns": campaigns_meta,
        "custom_object_types": custom_object_types,
        "customer_event_kinds": [
            "signup", "support_conversation", "customer_message", "campaign", "campaign_reply",
            "rating", "incident_exposure", "custom_object_event"
        ],
        "support_health_metrics": [
            "avg_rating", "avg_effort_score", "first_response_resolution_rate", "high_friction_rate"
        ],
    }))
}

pub async fn preview_segment(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let Ok(tree) = crate::segment::parse_segment_tree(&body) else {
        return (
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422, "error": "ValidationError",
                "message": "Body must be { combinator, conditions[], exclude[] }."
            })),
        )
            .into_response();
    };
    let page = body
        .get("page")
        .and_then(|v| v.as_i64())
        .unwrap_or(1)
        .max(1);
    let page_size = body
        .get("pageSize")
        .and_then(|v| v.as_i64())
        .unwrap_or(25)
        .clamp(5, 100);
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let engine = crate::segment::SegmentEngine::new(&conn);
    let mut result = engine.preview(&tree, page, page_size);
    if let Some(obj) = result.as_object_mut() {
        obj.insert("page".into(), json!(page));
        obj.insert("page_size".into(), json!(page_size));
    }
    Json(result).into_response()
}

pub async fn estimate_segment(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let Ok(tree) = crate::segment::parse_segment_tree(&body) else {
        return Json(json!({"matched": 0}));
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let engine = crate::segment::SegmentEngine::new(&conn);
    Json(json!({"matched": engine.count(&tree)}))
}

pub async fn suggest_segment(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let request = body.get("request").and_then(|v| v.as_str()).unwrap_or("");
    if request.len() < 3 || request.len() > 500 {
        return (
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422, "error": "ValidationError",
                "message": "request must be a string of 3-500 characters."
            })),
        )
            .into_response();
    }
    let (backend, catalog) = {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        if !crate::settings::get_bool(&conn, "ai_enabled", true).unwrap_or(true) {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "statusCode": 503, "error": "ServiceUnavailable",
                    "message": "AI is disabled in Settings. The natural-language suggestion needs the local model; you can build the segment manually."
                })),
            )
                .into_response();
        }
        (
            crate::ai_pipeline::backend_from_settings(&conn),
            crate::segment::suggest_catalog_context(&conn),
        )
    };
    let system = crate::segment::suggest_system_prompt();
    let user = format!("Catalog:\n{catalog}\n\nRequest: {request}");
    use crate::ai_lm_studio::ChatOpts;
    use crate::ai_provider::ChatMessage;
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: system,
        },
        ChatMessage {
            role: "user".into(),
            content: user,
        },
    ];
    let chat_result = match backend {
        crate::ai_pipeline::AiBackend::LmStudio { client, model } => {
            client
                .chat_with_opts(
                    model.as_deref(),
                    &messages,
                    ChatOpts {
                        temperature: Some(0.1),
                        max_tokens: Some(900),
                        json_mode: true,
                    },
                )
                .await
        }
        crate::ai_pipeline::AiBackend::Disabled => {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "statusCode": 503, "error": "ServiceUnavailable",
                    "message": "AI is disabled in Settings. The natural-language suggestion needs the local model; you can build the segment manually."
                })),
            )
                .into_response();
        }
    };
    let res = match chat_result {
        Ok(r) => r,
        Err(e) => {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "statusCode": 503, "error": "ServiceUnavailable",
                    "message": e.to_string()
                })),
            )
                .into_response();
        }
    };
    let parsed: Option<Value> = res
        .content
        .as_deref()
        .and_then(crate::ai_pipeline::extract_json);
    let Some(parsed) = parsed else {
        return (
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422, "error": "ValidationError",
                "message": "The model returned unparseable JSON; no segment was created. Try rephrasing or build the segment manually."
            })),
        )
            .into_response();
    };
    let tree = match crate::segment::validate_segment_tree(&parsed) {
        Ok(t) => t,
        Err(e) => {
            return (
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422, "error": "ValidationError",
                    "message": format!("The suggested definition failed validation ({e}); nothing was saved. Try rephrasing or build the segment manually.")
                })),
            )
                .into_response();
        }
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let engine = crate::segment::SegmentEngine::new(&conn);
    let preview = engine.preview(&tree, 1, 25);
    Json(json!({
        "ok": true,
        "definition": tree.to_json(),
        "model": res.model,
        "preview": preview,
        "notes": [
            "The model only PROPOSED this definition; the deterministic engine selects recipients.",
            "The model proposed the DEFINITION only; the deterministic segment engine selected the recipients. Nothing was saved - review and save explicitly."
        ],
    }))
    .into_response()
}

pub async fn list_segments(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let segments = crate::outreach::list_segments_v2(&conn).unwrap_or_default();
    Json(json!({"segments": segments}))
}

pub async fn create_segment(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if name.is_empty() || name.len() > 200 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422, "error": "ValidationError",
                "message": "name must be a string of 1-200 characters."
            })),
        )
            .into_response();
    }
    let description: Option<String> = body
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.chars().take(2000).collect());
    let id = body.get("id").and_then(|v| v.as_i64()).filter(|id| *id > 0);
    let definition = body
        .get("definition")
        .cloned()
        .unwrap_or_else(|| json!({"combinator": "all", "conditions": [], "exclude": []}));
    let Ok(tree) = crate::segment::parse_segment_tree(&definition) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422, "error": "ValidationError",
                "message": "definition must be { combinator, conditions[], exclude[] }."
            })),
        )
            .into_response();
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::outreach::save_segment_v2(
        &conn,
        id,
        &name,
        description.as_deref(),
        &tree.to_json().to_string(),
    ) {
        Ok(saved_id) => {
            let _ = crate::jobs::audit(
                &conn,
                "user",
                if id.is_some() { "segment_updated" } else { "segment_created" },
                None,
                None,
                Some(&serde_json::to_string(&json!({"id": saved_id, "name": name})).unwrap_or_default()),
                None,
                None,
                false,
            );
            Json(json!({
                "ok": true,
                "id": saved_id,
                "message": if id.is_some() {
                    "Segment updated (version incremented)."
                } else {
                    "Segment saved. Saved segments are reusable rules; campaigns always snapshot their recipients at creation."
                },
            }))
        }
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
    .into_response()
}

pub async fn delete_segment(State(state): State<AppState>, Path(id): Path<i64>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let _ = crate::outreach::ensure_segments_v2(&conn);
    let ok = conn
        .execute(
            "DELETE FROM saved_segments WHERE id = ?1",
            rusqlite::params![id],
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    Json(json!({
        "ok": ok,
        "message": if ok {
            "Segment deleted. Existing campaigns keep their recipient snapshots."
        } else {
            "Segment not found."
        },
    }))
}

pub async fn create_campaign(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    let bad = |msg: &str| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422, "error": "ValidationError",
                "message": msg
            })),
        )
            .into_response()
    };
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if name.is_empty() || name.len() > 200 {
        return bad("name must be a string of 1-200 characters.");
    }
    let Some(subject) = body
        .get("subject")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
    else {
        return bad("subject is required.");
    };
    if subject.is_empty() || subject.len() > 500 {
        return bad("subject must be a string of 1-500 characters.");
    }
    let Some(body_text) = body
        .get("body")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    else {
        return bad("body is required.");
    };
    if body_text.is_empty() || body_text.len() > 200000 {
        return bad("body must be a string of 1-200000 characters.");
    }
    let Some(mailbox_local_id) = body
        .get("mailbox_local_id")
        .and_then(|v| v.as_i64())
        .filter(|id| *id > 0)
    else {
        return bad("A valid sending mailbox is required.");
    };
    let tags: Vec<String> = body
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|t| t.as_str())
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty() && t.len() <= 100)
                .take(10)
                .collect()
        })
        .unwrap_or_default();
    let segment_id = body
        .get("segment_id")
        .and_then(|v| v.as_i64())
        .filter(|id| *id > 0);
    let customer_ids: Vec<i64> = body
        .get("customer_ids")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_i64())
                .filter(|id| *id > 0)
                .collect()
        })
        .unwrap_or_default();

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let mailbox_ok: bool = conn
        .query_row(
            "SELECT 1 FROM mailboxes WHERE id = ?1 AND deleted_at IS NULL",
            [mailbox_local_id],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !mailbox_ok {
        return bad("A valid sending mailbox is required.");
    }
    let mut tree: Option<crate::segment::SegmentDefinition> = None;
    if let Some(def) = body.get("definition").filter(|v| !v.is_null()) {
        match crate::segment::parse_segment_tree(def) {
            Ok(t) => tree = Some(t),
            Err(_) => return bad("definition must be { combinator, conditions[], exclude[] }."),
        }
    } else if let Some(sid) = segment_id {
        if let Ok(Some(saved)) = crate::outreach::get_segment_v2(&conn, sid) {
            if let Ok(t) = crate::segment::parse_segment_tree(&saved["definition"]) {
                tree = Some(t);
            }
        }
    }
    let Some(tree) = tree else {
        return bad(
            "A segment definition or saved segment is required (campaigns never target the whole address book by accident).",
        );
    };
    const SNAPSHOT_LIMIT: i64 = 5000;
    let engine = crate::segment::SegmentEngine::new(&conn);
    let preview = engine.preview(&tree, 1, SNAPSHOT_LIMIT);
    let rows = preview
        .get("rows")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let selected_ids: Vec<i64> = if !customer_ids.is_empty() {
        customer_ids
    } else {
        rows.iter()
            .filter_map(|r| r.get("customer_local_id").and_then(|v| v.as_i64()))
            .collect()
    };
    let selected_rows: Vec<&Value> = rows
        .iter()
        .filter(|r| {
            r.get("customer_local_id")
                .and_then(|v| v.as_i64())
                .map(|id| selected_ids.contains(&id))
                .unwrap_or(false)
        })
        .collect();
    if selected_rows.is_empty() {
        return bad("No recipients selected.");
    }
    let row_ids: std::collections::HashSet<i64> = rows
        .iter()
        .filter_map(|r| r.get("customer_local_id").and_then(|v| v.as_i64()))
        .collect();
    let dropped_selected = selected_ids
        .iter()
        .filter(|id| !row_ids.contains(id))
        .count();
    let matched = preview.get("matched").and_then(|v| v.as_i64()).unwrap_or(0);
    let recipients_json: Vec<Value> = selected_rows.into_iter().cloned().collect();
    let count = recipients_json.len();
    match crate::outreach::create_outreach_campaign(
        &conn,
        &name,
        &subject,
        &body_text,
        mailbox_local_id,
        &tags,
        segment_id,
        Some(&tree.to_json().to_string()),
        &recipients_json,
    ) {
        Ok(campaign_id) => {
            let _ = crate::jobs::audit(
                &conn,
                "user",
                "campaign_created",
                None,
                None,
                Some(&serde_json::to_string(&json!({
                    "id": campaign_id,
                    "name": name,
                    "recipients": count
                }))
                .unwrap_or_default()),
                None,
                None,
                false,
            );
            let truncated = matched > rows.len() as i64 || dropped_selected > 0;
            Json(json!({
                "ok": true,
                "id": campaign_id,
                "recipients": count,
                "message": if truncated {
                    format!(
                        "Campaign created with {count} recipients (segment matched {matched}; the recipient snapshot is capped at {SNAPSHOT_LIMIT}{}). Recipients are a static snapshot - later segment changes will not alter this campaign.",
                        if dropped_selected > 0 {
                            format!(", {dropped_selected} explicitly-selected customer(s) beyond the cap were not included")
                        } else {
                            String::new()
                        }
                    )
                } else {
                    format!("Campaign created with {count} recipients. Recipients are a static snapshot - later segment changes will not alter this campaign.")
                },
            }))
        }
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
    .into_response()
}

pub async fn list_campaigns(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let campaigns = crate::outreach::list_outreach_campaigns(&conn).unwrap_or_default();
    Json(json!({"campaigns": campaigns}))
}

pub async fn get_campaign(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::outreach::get_campaign_full(&conn, id) {
        Ok(Some(campaign)) => {
            let events = crate::outreach::list_campaign_events(&conn, id).unwrap_or_default();
            Json(json!({"campaign": campaign, "events": events})).into_response()
        }
        _ => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404, "error": "NotFound",
                "message": "Campaign not found."
            })),
        )
            .into_response(),
    }
}

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub async fn campaign_validate_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return campaign_not_found();
    };
    let conn = state.conn_lock();
    Json(crate::outreach::campaign_validate(&conn, id)).into_response()
}

pub async fn render_route(State(state): State<AppState>, body: Option<Json<Value>>) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let customer_id = match body.get("customer_local_id").and_then(|v| v.as_i64()) {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422, "error": "ValidationError",
                    "message": "customer_local_id is required."
                })),
            )
                .into_response();
        }
    };
    let conn = state.conn_lock();
    let customer: Option<(Option<String>, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT first_name, last_name, email FROM customers WHERE id = ?1",
            [customer_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    let Some((first_name, last_name, email)) = customer else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404, "error": "NotFound",
                "message": "Customer not found in the local mirror."
            })),
        )
            .into_response();
    };
    let tickets: Vec<Value> = match body.get("matching_tickets").and_then(|v| v.as_array()) {
        Some(t) => t.clone(),
        None => conn
            .prepare(
                "SELECT c.id, c.number, c.subject, c.status, c.created_at,
                        (SELECT GROUP_CONCAT(t.name) FROM conversation_tags ct
                          JOIN tags t ON t.id = ct.tag_id WHERE ct.conversation_id = c.id)
                   FROM conversations c WHERE c.customer_local_id = ?1
                  ORDER BY c.created_at DESC LIMIT 3",
            )
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map([customer_id], |r| {
                    let tags_raw: Option<String> = r.get(5)?;
                    let tags: Vec<String> = tags_raw
                        .map(|t| {
                            t.split(',')
                                .filter(|s| !s.is_empty())
                                .map(String::from)
                                .collect()
                        })
                        .unwrap_or_default();
                    Ok(json!({
                        "conversationId": r.get::<_, i64>(0)?,
                        "number": r.get::<_, i64>(1)?,
                        "subject": r.get::<_, Option<String>>(2)?,
                        "status": r.get::<_, String>(3)?,
                        "createdAt": r.get::<_, Option<String>>(4)?,
                        "tags": tags,
                    }))
                })
                .map(|rows| rows.filter_map(|x| x.ok()).collect())
                .ok()
            })
            .unwrap_or_default(),
    };
    let subject = body.get("subject").and_then(|v| v.as_str()).unwrap_or("");
    let body_text = body.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let rendered = crate::outreach::render_for(&conn, customer_id, &tickets, subject, body_text);
    let sources: Vec<Value> = tickets
        .iter()
        .take(3)
        .map(|t| json!({ "number": t["number"], "subject": t["subject"] }))
        .collect();
    Json(json!({
        "rendered": rendered,
        "customer": { "first_name": first_name, "last_name": last_name, "email": email },
        "sources": sources,
    }))
    .into_response()
}

pub async fn campaign_preview_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return campaign_not_found();
    };
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let conn = state.conn_lock();
    let Some(c) = crate::outreach::get_outreach_campaign(&conn, id)
        .ok()
        .flatten()
    else {
        return campaign_not_found();
    };
    let customer_local_id = body.get("customer_local_id").and_then(|v| v.as_i64());
    let recipient: Option<(i64, Option<String>, Option<String>, String)> = customer_local_id
        .and_then(|cid| {
            conn.query_row(
                "SELECT customer_local_id, email, first_name, snapshot
                   FROM outreach_recipients
                  WHERE campaign_id = ?1 AND customer_local_id = ?2",
                rusqlite::params![id, cid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .ok()
        });
    let recipient: Option<(i64, String, String, String, Option<String>)> = customer_local_id
        .and_then(|cid| {
            conn.query_row(
                "SELECT r.customer_local_id, COALESCE(r.email, ''), r.snapshot, '',
                        (c.first_name || ' ' || c.last_name)
                   FROM outreach_recipients r
                   LEFT JOIN customers c ON c.id = r.customer_local_id
                  WHERE r.campaign_id = ?1 AND r.customer_local_id = ?2",
                rusqlite::params![id, cid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .ok()
        });
    let Some((_clid, email, snapshot, _unused, name)) = recipient else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404, "error": "NotFound",
                "message": "That customer is not a recipient of this campaign."
            })),
        )
            .into_response();
    };
    let _ = _unused;
    let tickets: Vec<Value> = serde_json::from_str::<Value>(&snapshot)
        .ok()
        .and_then(|v| v.get("matching_tickets").cloned())
        .and_then(|t| serde_json::from_value(t).ok())
        .unwrap_or_default();
    let rendered = crate::outreach::render_for(&conn, _clid, &tickets, &c.subject, &c.body);
    let sources: Vec<Value> = tickets
        .iter()
        .take(3)
        .map(|t| json!({ "number": t["number"], "subject": t["subject"] }))
        .collect();
    let _ = email;
    let name = name.unwrap_or_default();
    let (first, last) = match name.split_once(' ') {
        Some((f, l)) => (f.to_string(), l.trim().to_string()),
        None => (name.clone(), String::new()),
    };
    Json(json!({
        "rendered": rendered,
        "customer": { "first_name": first, "last_name": last, "email": email },
        "sources": sources,
    }))
    .into_response()
}

fn campaign_not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "statusCode": 404, "error": "NotFound",
            "message": "Campaign not found."
        })),
    )
        .into_response()
}

pub async fn campaign_queue_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return Json(json!({ "ok": false, "message": "Campaign not found." })).into_response();
    };
    let conn = state.conn_lock();
    let result = crate::outreach::campaign_queue(&conn, id);
    if result["ok"].as_bool().unwrap_or(false) {
        let _ = crate::jobs::audit(
            &conn,
            "user",
            "campaign_queued",
            None,
            None,
            Some(&json!({ "id": id }).to_string()),
            None,
            None,
            false,
        );
    }
    Json(result).into_response()
}

pub async fn campaign_pause_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return Json(json!({ "ok": false, "message": "Campaign not found." })).into_response();
    };
    let conn = state.conn_lock();
    Json(crate::outreach::campaign_pause(&conn, id)).into_response()
}

pub async fn campaign_resume_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return Json(json!({ "ok": false, "message": "Campaign not found." })).into_response();
    };
    let conn = state.conn_lock();
    Json(crate::outreach::campaign_resume(&conn, id)).into_response()
}

pub async fn campaign_cancel_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return Json(json!({ "ok": false, "message": "Campaign not found." })).into_response();
    };
    let conn = state.conn_lock();
    Json(crate::outreach::campaign_cancel_remaining(&conn, id)).into_response()
}

pub async fn campaign_retry_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return Json(json!({ "ok": false, "message": "Campaign not found." })).into_response();
    };
    let conn = state.conn_lock();
    Json(crate::outreach::campaign_retry_failed(&conn, id)).into_response()
}

pub async fn campaign_reconcile_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return Json(json!({ "ok": true,
            "message": "Reconciliation finished: 0 resolved as sent, 0 returned to the queue, 0 still unknown.",
            "resolvedSent": 0, "returnedToQueue": 0, "stillUnknown": 0 })).into_response();
    };
    let conn = state.conn_lock();
    let r = crate::outreach::campaign_reconcile(&conn, id);
    let resolved = r["resolvedSent"].as_i64().unwrap_or(0);
    let returned = r["returnedToQueue"].as_i64().unwrap_or(0);
    let unknown = r["stillUnknown"].as_i64().unwrap_or(0);
    Json(json!({
        "ok": true,
        "message": format!("Reconciliation finished: {resolved} resolved as sent, {returned} returned to the queue, {unknown} still unknown."),
        "resolvedSent": resolved, "returnedToQueue": returned, "stillUnknown": unknown,
    }))
    .into_response()
}

pub async fn campaign_report_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return Json(crate::outreach::campaign_report_not_found()).into_response();
    };
    let conn = state.conn_lock();
    Json(crate::outreach::campaign_report(&conn, id)).into_response()
}

pub async fn campaign_delete_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return Json(json!({ "ok": false, "message": "Campaign not found." })).into_response();
    };
    let conn = state.conn_lock();
    Json(crate::outreach::campaign_delete(&conn, id)).into_response()
}

pub async fn list_dnc_route(State(state): State<AppState>) -> Response {
    let conn = state.conn_lock();
    let dnc: Vec<Value> = conn
        .prepare(
            "SELECT d.customer_id, c.first_name, c.last_name, d.reason, d.created_at
               FROM do_not_contact d
               LEFT JOIN customers c ON c.id = d.customer_id
              ORDER BY d.created_at DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "customer_local_id": r.get::<_, i64>(0)?,
                    "first_name": r.get::<_, Option<String>>(1)?,
                    "last_name": r.get::<_, Option<String>>(2)?,
                    "reason": r.get::<_, Option<String>>(3)?,
                    "created_at": r.get::<_, String>(4)?,
                }))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
            .ok()
        })
        .unwrap_or_default();
    Json(json!({ "dnc": dnc })).into_response()
}

pub async fn add_dnc_route(State(state): State<AppState>, body: Option<Json<Value>>) -> Response {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let id = body
        .get("customer_local_id")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if id <= 0 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422, "error": "ValidationError",
                "message": "customer_local_id must be a positive integer."
            })),
        )
            .into_response();
    }
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .map(String::from);
    let conn = state.conn_lock();
    let _ = conn.execute(
        "INSERT INTO do_not_contact (customer_id, reason, created_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
         ON CONFLICT(customer_id) DO UPDATE SET reason = excluded.reason",
        rusqlite::params![id, reason],
    );
    let _ = crate::jobs::audit(
        &conn,
        "user",
        "dnc_added",
        None,
        None,
        Some(&json!({ "customer_local_id": id }).to_string()),
        None,
        None,
        false,
    );
    Json(json!({
        "ok": true,
        "message": "Added to Do-Not-Contact. Every future campaign skips this customer."
    }))
    .into_response()
}

pub async fn remove_dnc_route(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Ok(id) = id.parse::<i64>() else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422, "error": "ValidationError",
                "message": "A positive numeric customer id is required."
            })),
        )
            .into_response();
    };
    if id <= 0 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422, "error": "ValidationError",
                "message": "A positive numeric customer id is required."
            })),
        )
            .into_response();
    }
    let conn = state.conn_lock();
    let removed = conn
        .execute("DELETE FROM do_not_contact WHERE customer_id = ?1", [id])
        .map(|n| n > 0)
        .unwrap_or(false);
    if !removed {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404, "error": "NotFound",
                "message": "Customer is not on the Do-Not-Contact list."
            })),
        )
            .into_response();
    }
    Json(json!({ "ok": true, "message": "Removed from Do-Not-Contact." })).into_response()
}
