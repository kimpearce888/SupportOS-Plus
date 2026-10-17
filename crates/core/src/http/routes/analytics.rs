use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde_json::{Value, json};

use super::super::server::AppState;
use axum::response::IntoResponse;

pub async fn dashboard(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    let days = clamp_days_param(params.get("days"), 30, 1, 3650);

    let mailbox_tokens: Vec<Option<f64>> = params
        .get("mailboxIds")
        .filter(|s| !s.is_empty())
        .map(|raw| {
            raw.split(',')
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .map(crate::conversation_ops::js_number)
                .collect::<Vec<Option<f64>>>()
        })
        .unwrap_or_default();
    if mailbox_tokens
        .iter()
        .any(|id| id.is_none_or(|v| !v.is_finite() || v.fract() != 0.0 || v <= 0.0))
    {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "mailboxIds must be a comma-separated list of positive integers."
            })),
        )
            .into_response();
    }
    let mailbox_ids: Vec<i64> = mailbox_tokens
        .into_iter()
        .map(|id| id.unwrap_or_default() as i64)
        .collect();

    let channel: Option<String> = match params.get("channel").filter(|s| !s.is_empty()) {
        Some(c) if c == "email" || c == "chat" => Some(c.clone()),
        Some(_) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": "channel must be 'email' or 'chat'."
                })),
            )
                .into_response();
        }
        None => None,
    };

    let from = params
        .get("from")
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| crate::business_hours::iso_days_ago(days));
    let to = params
        .get("to")
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(crate::business_hours::now_iso_millis);

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());

    let mailbox_in = if mailbox_ids.is_empty() {
        String::new()
    } else {
        format!(
            " AND c.mailbox_local_id IN ({})",
            mailbox_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let conv_channel = channel
        .as_ref()
        .map(|_| " AND c.type = ?3".to_string())
        .unwrap_or_default();
    let rating_channel = channel
        .as_ref()
        .map(|_| " AND c.type = ?3".to_string())
        .unwrap_or_default();

    let range_params = |with_channel: bool| -> Vec<String> {
        let mut p = vec![from.clone(), to.clone()];
        if with_channel {
            if let Some(c) = &channel {
                p.push(c.clone());
            }
        }
        p
    };

    let counts_sql = format!(
        "SELECT
          COALESCE(SUM(CASE WHEN c.created_at >= ?1 AND c.created_at <= ?2 THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN c.status = 'active' AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN c.status = 'pending' AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN c.closed_at IS NOT NULL AND c.closed_at >= ?1 AND c.closed_at <= ?2 THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN c.status IN ('active','pending') AND c.assignee_local_id IS NULL AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN c.status IN ('active','pending') AND (julianday('now') - COALESCE(julianday(c.updated_at), julianday(c.created_at))) >= 7 AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0)
         FROM conversations c WHERE c.deleted_at IS NULL{mailbox_in}{conv_channel}"
    );
    let (
        new_conversations,
        active_conversations,
        pending_conversations,
        closed_conversations,
        unassigned,
        backlog,
    ): (i64, i64, i64, i64, i64, i64) = conn
        .query_row(
            &counts_sql,
            rusqlite::params_from_iter(range_params(channel.is_some())),
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .unwrap_or((0, 0, 0, 0, 0, 0));

    let replies_sent: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM conversation_threads t
                 JOIN conversations c ON c.id = t.conversation_id
                 WHERE t.type = 'reply' AND t.state = 'published' AND t.deleted_at IS NULL
                   AND t.created_at >= ?1 AND t.created_at <= ?2{mailbox_in}{conv_channel}"
            ),
            rusqlite::params_from_iter(range_params(channel.is_some())),
            |r| r.get(0),
        )
        .unwrap_or(0);

    let first_reply_join = "LEFT JOIN (SELECT conversation_id, MIN(created_at) AS first_reply FROM conversation_threads WHERE type = 'reply' AND state = 'published' AND deleted_at IS NULL GROUP BY conversation_id) fr ON fr.conversation_id = c.id";
    let first_response_avg_min: Option<f64> = conn
        .query_row(
            &format!(
                "SELECT AVG((julianday(fr.first_reply) - julianday(COALESCE(c.first_customer_message_at, c.created_at))) * 1440)
                 FROM conversations c
                 {first_reply_join}
                 WHERE c.created_at >= ?1 AND c.created_at <= ?2 AND c.deleted_at IS NULL{mailbox_in}{conv_channel}"
            ),
            rusqlite::params_from_iter(range_params(channel.is_some())),
            |r| r.get(0),
        )
        .unwrap_or(None);
    let resolution_avg_min: Option<f64> = conn
        .query_row(
            &format!(
                "SELECT AVG((julianday(c.closed_at) - julianday(COALESCE(c.first_customer_message_at, c.created_at))) * 1440)
                 FROM conversations c
                 WHERE c.closed_at IS NOT NULL AND c.closed_at >= ?1 AND c.closed_at <= ?2 AND c.deleted_at IS NULL{mailbox_in}{conv_channel}"
            ),
            rusqlite::params_from_iter(range_params(channel.is_some())),
            |r| r.get(0),
        )
        .unwrap_or(None);

    let (great, okay, not_good): (i64, i64, i64) = conn
        .query_row(
            &format!(
                "SELECT
                   COALESCE(SUM(CASE WHEN r.rating = 'great' THEN 1 ELSE 0 END), 0),
                   COALESCE(SUM(CASE WHEN r.rating = 'okay' THEN 1 ELSE 0 END), 0),
                   COALESCE(SUM(CASE WHEN r.rating = 'not-good' THEN 1 ELSE 0 END), 0)
                 FROM ratings r LEFT JOIN conversations c ON c.id = r.conversation_id
                 WHERE r.remote_created_at >= ?1 AND r.remote_created_at <= ?2{mailbox_in}{rating_channel}"
            ),
            rusqlite::params_from_iter(range_params(channel.is_some())),
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, 0, 0));

    let name_count_rows = |sql: &str, params: Vec<String>| -> Vec<Value> {
        conn.prepare(sql)
            .and_then(|mut stmt| {
                stmt.query_map(rusqlite::params_from_iter(params), |r| {
                    Ok(json!({
                        "name": r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                        "count": r.get::<_, i64>(1)?,
                    }))
                })
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default()
    };
    let by_mailbox = name_count_rows(
        &format!(
            "SELECT m.name AS name, COUNT(*) AS count FROM conversations c
             JOIN mailboxes m ON m.id = c.mailbox_local_id
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{conv_channel}
             GROUP BY m.name ORDER BY count DESC"
        ),
        range_params(channel.is_some()),
    );
    let by_tag = name_count_rows(
        &format!(
            "SELECT t.name AS name, COUNT(*) AS count
             FROM conversation_tags ct
             JOIN tags t ON t.id = ct.tag_id
             JOIN conversations c ON c.id = ct.conversation_id
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{mailbox_in}{conv_channel}
             GROUP BY t.name ORDER BY count DESC LIMIT 12"
        ),
        range_params(channel.is_some()),
    );
    let by_agent = name_count_rows(
        &format!(
            "SELECT TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, '')) AS name, COUNT(*) AS count
             FROM conversations c JOIN users u ON u.id = c.assignee_local_id
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{mailbox_in}{conv_channel}
             GROUP BY u.id ORDER BY count DESC"
        ),
        range_params(channel.is_some()),
    );
    let by_team = name_count_rows(
        &format!(
            "SELECT tm.name AS name, COUNT(*) AS count
             FROM conversations c
             JOIN users u ON u.id = c.assignee_local_id
             JOIN team_members tem ON tem.user_id = u.id
             JOIN teams tm ON tm.id = tem.team_id
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{mailbox_in}{conv_channel}
             GROUP BY tm.id ORDER BY count DESC"
        ),
        range_params(channel.is_some()),
    );

    let daily_new: Vec<Value> = conn
        .prepare(&format!(
            "SELECT date(c.created_at) AS date, COUNT(*) AS value FROM conversations c
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{mailbox_in}{conv_channel}
             GROUP BY date(c.created_at) ORDER BY date"
        ))
        .and_then(|mut stmt| {
            stmt.query_map(
                rusqlite::params_from_iter(range_params(channel.is_some())),
                |r| {
                    Ok(json!({
                        "date": r.get::<_, Option<String>>(0)?,
                        "value": r.get::<_, i64>(1)?,
                    }))
                },
            )
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    let by_channel: Vec<Value> = conn
        .prepare(&format!(
            "SELECT COALESCE(c.type, 'unknown') AS channel, COUNT(*) AS count FROM conversations c
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{mailbox_in}
             GROUP BY COALESCE(c.type, 'unknown') ORDER BY count DESC"
        ))
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params_from_iter(range_params(false)), |r| {
                Ok(json!({
                    "channel": r.get::<_, Option<String>>(0)?.unwrap_or_else(|| "unknown".into()),
                    "count": r.get::<_, i64>(1)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let channel_metrics: Vec<Value> = conn
        .prepare(&format!(
            "SELECT COALESCE(c.type, 'unknown') AS channel, COUNT(*) AS count,
                    AVG((julianday(fr.first_reply) - julianday(COALESCE(c.first_customer_message_at, c.created_at))) * 1440) AS first_response_avg_min,
                    AVG((julianday(c.closed_at) - julianday(COALESCE(c.first_customer_message_at, c.created_at))) * 1440) AS resolution_avg_min
             FROM conversations c
             {first_reply_join}
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{mailbox_in}
             GROUP BY COALESCE(c.type, 'unknown') ORDER BY count DESC"
        ))
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params_from_iter(range_params(false)), |r| {
                Ok(json!({
                    "channel": r.get::<_, Option<String>>(0)?.unwrap_or_else(|| "unknown".into()),
                    "count": r.get::<_, i64>(1)?,
                    "first_response_avg_min": r.get::<_, Option<f64>>(2)?.map(round_min),
                    "resolution_avg_min": r.get::<_, Option<f64>>(3)?.map(round_min),
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    let cmp_channel_join = channel.as_ref().map(|_| " AND c.type = ?3").unwrap_or("");
    let cmp_channel_sub = channel.as_ref().map(|_| " AND c2.type = ?3").unwrap_or("");
    let mailbox_comparison: Vec<Value> = conn
        .prepare(&format!(
            "SELECT m.id AS mailbox_id, m.name AS name,
                    COALESCE(SUM(CASE WHEN c.created_at >= ?1 AND c.created_at <= ?2 THEN 1 ELSE 0 END), 0) AS new_conversations,
                    COALESCE(SUM(CASE WHEN c.status = 'active' AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0) AS active_conversations,
                    COALESCE(SUM(CASE WHEN c.closed_at IS NOT NULL AND c.closed_at >= ?1 AND c.closed_at <= ?2 THEN 1 ELSE 0 END), 0) AS closed_conversations,
                    COALESCE(SUM(CASE WHEN c.status IN ('active','pending') AND (julianday('now') - COALESCE(julianday(c.updated_at), julianday(c.created_at))) >= 7 AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0) AS backlog,
                    AVG((julianday(fr.first_reply) - julianday(COALESCE(c.first_customer_message_at, c.created_at))) * 1440) AS first_response_avg_min,
                    AVG((julianday(c.closed_at) - julianday(COALESCE(c.first_customer_message_at, c.created_at))) * 1440) AS resolution_avg_min,
                    (SELECT COUNT(*) FROM ratings r JOIN conversations c2 ON c2.id = r.conversation_id
                      WHERE c2.mailbox_local_id = m.id AND r.rating = 'great' AND r.remote_created_at >= ?1 AND r.remote_created_at <= ?2{cmp_channel_sub}) AS great_ratings,
                    (SELECT COUNT(*) FROM ratings r JOIN conversations c2 ON c2.id = r.conversation_id
                      WHERE c2.mailbox_local_id = m.id AND r.remote_created_at >= ?1 AND r.remote_created_at <= ?2{cmp_channel_sub}) AS total_ratings
             FROM mailboxes m
             LEFT JOIN conversations c ON c.mailbox_local_id = m.id AND c.deleted_at IS NULL{cmp_channel_join}
             {first_reply_join}
             WHERE 1 = 1{mailbox_in}
             GROUP BY m.id, m.name ORDER BY new_conversations DESC"
        ))
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params_from_iter(range_params(channel.is_some())), |r| {
                Ok(json!({
                    "mailbox_id": r.get::<_, i64>(0)?,
                    "name": r.get::<_, String>(1)?,
                    "new_conversations": r.get::<_, i64>(2)?,
                    "active_conversations": r.get::<_, i64>(3)?,
                    "closed_conversations": r.get::<_, i64>(4)?,
                    "backlog": r.get::<_, i64>(5)?,
                    "first_response_avg_min": r.get::<_, Option<f64>>(6)?.map(round_min),
                    "resolution_avg_min": r.get::<_, Option<f64>>(7)?.map(round_min),
                    "great_ratings": r.get::<_, i64>(8)?,
                    "total_ratings": r.get::<_, i64>(9)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    for point in &daily_new {
        if let (Some(date), Some(value)) = (
            point.get("date").and_then(|d| d.as_str()),
            point.get("value").and_then(|v| v.as_i64()),
        ) {
            let _ = conn.execute(
                "INSERT INTO daily_metrics (metric_key, date, value, updated_at)
                 VALUES ('new_conversations', ?1, ?2, datetime('now'))
                 ON CONFLICT(metric_key, date) DO UPDATE SET
                   value = excluded.value, updated_at = excluded.updated_at",
                rusqlite::params![date, value],
            );
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "range": { "from": from, "to": to },
            "new_conversations": new_conversations,
            "active_conversations": active_conversations,
            "pending_conversations": pending_conversations,
            "closed_conversations": closed_conversations,
            "unassigned": unassigned,
            "backlog": backlog,
            "first_response_time_avg_min": first_response_avg_min.map(round_min),
            "resolution_time_avg_min": resolution_avg_min.map(round_min),
            "replies_sent": replies_sent,
            "ratings": { "great": great, "okay": okay, "not-good": not_good },
            "by_mailbox": by_mailbox,
            "by_tag": by_tag,
            "by_agent": by_agent,
            "by_team": by_team,
            "daily_new": daily_new,
            "by_channel": by_channel,
            "channel_metrics": channel_metrics,
            "mailbox_comparison": mailbox_comparison,
            "source": ["local"],
        })),
    )
        .into_response()
}

fn round_min(v: f64) -> i64 {
    v.round() as i64
}

pub async fn ai_analytics(State(state): State<AppState>) -> impl IntoResponse {
    super::ai::ai_analytics(State(state)).await
}

pub async fn sla_report(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let days = clamp_days_param(params.get("days"), 30, 1, 3650);
    let mut mailbox_ids: Option<Vec<i64>> = None;
    if let Some(raw) = params.get("mailboxIds") {
        if !raw.is_empty() {
            let tokens: Vec<Option<f64>> = raw
                .split(',')
                .map(|t| crate::conversation_ops::js_number(t.trim()))
                .collect();
            if tokens
                .iter()
                .any(|id| id.is_none_or(|v| !v.is_finite() || v.fract() != 0.0 || v <= 0.0))
            {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422,
                        "error": "ValidationError",
                        "message": "mailboxIds must be a comma-separated list of positive integers."
                    })),
                );
            }
            mailbox_ids = Some(
                tokens
                    .iter()
                    .map(|id| id.unwrap_or_default() as i64)
                    .collect(),
            );
        }
    }
    let from = params
        .get("from")
        .cloned()
        .unwrap_or_else(|| crate::business_hours::iso_days_ago(days));
    let to = params
        .get("to")
        .cloned()
        .unwrap_or_else(crate::business_hours::now_iso_millis);
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::sla::sla_report(&conn, &from, &to, mailbox_ids.as_deref()) {
        Ok(report) => (
            StatusCode::OK,
            Json(serde_json::to_value(&report).unwrap_or(Value::Null)),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        ),
    }
}

fn clamp_days_param(raw: Option<&String>, fallback: i64, min: i64, max: i64) -> i64 {
    let Some(s) = raw.filter(|s| !s.is_empty()) else {
        return fallback;
    };
    match crate::conversation_ops::js_number(s) {
        Some(n) if n.is_finite() => (n.trunc() as i64).clamp(min, max),
        _ => fallback,
    }
}

pub async fn why_contacting(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let days = clamp_days_param(params.get("days"), 30, 1, 3650);
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let categories = why_contacting_impl(&conn, days);
    (
        StatusCode::OK,
        Json(json!({ "categories": categories, "source": "ai-derived" })),
    )
}

fn why_contacting_impl(conn: &rusqlite::Connection, days: i64) -> Vec<Value> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT json_extract(a.response_json, '$.issue_cluster_candidate') AS category,
                a.conversation_id
           FROM ai_runs a
          WHERE a.type = 'ticket_analysis' AND a.status = 'completed'
            AND a.conversation_id IS NOT NULL
            AND a.id IN (SELECT MAX(id) FROM ai_runs
                          WHERE type = 'ticket_analysis' AND status = 'completed'
                         GROUP BY conversation_id)
            AND json_extract(a.response_json, '$.issue_cluster_candidate') IS NOT NULL
            AND a.created_at >= datetime('now', '-' || ?1 || ' days')",
    ) else {
        return Vec::new();
    };
    let rows: Vec<(String, i64)> = stmt
        .query_map([days], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    let mut by_category: std::collections::HashMap<String, Vec<i64>> =
        std::collections::HashMap::new();
    for (category, conversation_id) in rows {
        let cat = category.trim().to_lowercase();
        by_category.entry(cat).or_default().push(conversation_id);
    }
    let mut out: Vec<Value> = by_category
        .into_iter()
        .map(|(category, ids)| {
            json!({ "category": category, "count": ids.len(), "conversation_ids": ids })
        })
        .collect();
    out.sort_by(|a, b| {
        b["count"]
            .as_i64()
            .unwrap_or(0)
            .cmp(&a["count"].as_i64().unwrap_or(0))
    });
    out
}

pub async fn top_questions(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let days = clamp_days_param(params.get("days"), 30, 1, 3650);
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let questions = top_questions_impl(&conn, days);
    (StatusCode::OK, Json(json!({ "questions": questions })))
}

fn top_questions_impl(conn: &rusqlite::Connection, days: i64) -> Vec<Value> {
    let mut out: Vec<Value> = grouped_questions(conn, days)
        .into_iter()
        .map(|(question, ids)| {
            json!({ "question": question, "count": ids.len(), "conversation_ids": ids })
        })
        .collect();
    out.truncate(20);
    out
}

fn grouped_questions(conn: &rusqlite::Connection, days: i64) -> Vec<(String, Vec<i64>)> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT LOWER(TRIM(json_extract(a.response_json, '$.primary_question'))) AS question,
                a.conversation_id
           FROM ai_runs a
          WHERE a.type = 'ticket_analysis' AND a.status = 'completed'
            AND a.conversation_id IS NOT NULL
            AND a.id IN (SELECT MAX(id) FROM ai_runs
                          WHERE type = 'ticket_analysis' AND status = 'completed'
                         GROUP BY conversation_id)
            AND json_extract(a.response_json, '$.primary_question') IS NOT NULL
            AND a.created_at >= datetime('now', '-' || ?1 || ' days')",
    ) else {
        return Vec::new();
    };
    let rows: Vec<(String, i64)> = stmt
        .query_map([days], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    let mut by_question: std::collections::HashMap<String, Vec<i64>> =
        std::collections::HashMap::new();
    for (question, conversation_id) in rows {
        by_question
            .entry(question)
            .or_default()
            .push(conversation_id);
    }
    let mut out: Vec<(String, Vec<i64>)> = by_question.into_iter().collect();
    out.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    out
}

fn fts_tokens(q: &str) -> String {
    let tokens = crate::search::tokenize(q, 3, 6);
    if tokens.is_empty() {
        return "\"\"".to_string();
    }
    tokens
        .iter()
        .map(|t| format!("\"{t}\"*"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn is_reference_date_time(s: &str) -> bool {
    let bytes = s.as_bytes();
    let date_ok = bytes.len() >= 10
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit);
    if !date_ok {
        return false;
    }
    if s.len() == 10 {
        return true;
    }
    let rest = &s[10..];
    let Some(time) = rest.strip_prefix('T') else {
        return false;
    };
    let (time, z) = match time.strip_suffix('Z') {
        Some(t) => (t, true),
        None => (time, false),
    };
    if time.is_empty()
        || !time
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'.' || b == b':')
    {
        return false;
    }
    !time.contains('Z') && (!z || !time.is_empty())
}

fn release_event_422(path: &str, message: &str) -> axum::response::Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "statusCode": 422,
            "error": "ValidationError",
            "message": format!("Invalid request ({path}): {message}"),
            "issues": [{ "path": path, "message": message }]
        })),
    )
        .into_response()
}

struct RadarCluster {
    id: i64,
    title: String,
    conversation_ids: Vec<i64>,
    customer_count: usize,
    summary: String,
    trend: &'static str,
}

struct ConvFacts {
    id: i64,
    customer_id: Option<i64>,
    mailbox_id: Option<i64>,
    created_at: Option<String>,
    jd: Option<f64>,
}

fn radar_alert(
    kind: &str,
    title: String,
    detail: String,
    conversation_ids: Vec<i64>,
    cluster_id: Option<i64>,
    severity: &str,
) -> Value {
    let capped: Vec<i64> = conversation_ids.into_iter().take(10).collect();
    json!({
        "kind": kind,
        "title": title,
        "detail": detail,
        "conversation_ids": capped,
        "cluster_id": cluster_id,
        "severity": severity,
    })
}

fn issue_radar_impl(conn: &rusqlite::Connection) -> Vec<Value> {
    let mut alerts: Vec<Value> = Vec::new();
    let clusters = radar_clusters(conn);

    for c in &clusters {
        if c.trend == "new" && c.conversation_ids.len() >= 2 {
            alerts.push(radar_alert(
                "new_cluster",
                format!("New issue cluster: {}", c.title),
                format!(
                    "{} conversations from {} customers first seen recently. {}",
                    c.conversation_ids.len(),
                    c.customer_count,
                    c.summary
                ),
                c.conversation_ids.clone(),
                Some(c.id),
                if c.conversation_ids.len() >= 5 {
                    "critical"
                } else {
                    "warning"
                },
            ));
        }
        if c.trend == "rising" && c.conversation_ids.len() >= 3 {
            alerts.push(radar_alert(
                "volume_spike",
                format!("Rising volume: {}", c.title),
                format!(
                    "Conversation volume in this cluster increased vs the previous 14 days. {}",
                    c.summary
                ),
                c.conversation_ids.clone(),
                Some(c.id),
                "warning",
            ));
        }
        if c.trend == "stable" && c.conversation_ids.len() >= 5 {
            alerts.push(radar_alert(
                "recurring_issue",
                format!("Recurring issue: {}", c.title),
                format!(
                    "{} conversations over time. Consider a knowledge article or known issue entry. {}",
                    c.conversation_ids.len(),
                    c.summary
                ),
                c.conversation_ids.clone(),
                Some(c.id),
                "info",
            ));
        }
    }

    for q in top_questions_impl(conn, 30).into_iter().take(3) {
        let count = q["count"].as_i64().unwrap_or(0);
        if count >= 3 {
            alerts.push(radar_alert(
                "high_volume_question",
                format!("High-volume question ({count} tickets)"),
                q["question"].as_str().unwrap_or_default().to_string(),
                q["conversation_ids"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_i64()).collect::<Vec<i64>>())
                    .unwrap_or_default(),
                None,
                "info",
            ));
        }
    }

    let escalated: Vec<i64> = conn
        .prepare(
            "SELECT c.id FROM conversations c
               JOIN conversation_tags ct ON ct.conversation_id = c.id
               JOIN tags t ON t.id = ct.tag_id
              WHERE t.name = 'escalated'
                AND julianday(c.created_at) >= julianday('now', '-30 days')
              LIMIT 1000",
        )
        .map(|mut stmt| {
            stmt.query_map([], |r| r.get::<_, i64>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    if escalated.len() >= 2 {
        alerts.push(radar_alert(
            "escalation_heavy",
            format!(
                "{} escalated tickets in the last 30 days",
                escalated.len()
            ),
            "Multiple tickets required escalation. Review the underlying causes - this is a correlation, not a causal claim.".to_string(),
            escalated,
            None,
            "warning",
        ));
    }

    let bad_ids: std::collections::HashSet<i64> = conn
        .prepare(
            "SELECT r.conversation_id FROM ratings r
              WHERE r.rating = 'not-good' AND r.conversation_id IS NOT NULL
              ORDER BY r.id DESC LIMIT 2000",
        )
        .map(|mut stmt| {
            stmt.query_map([], |r| r.get::<_, i64>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    if !bad_ids.is_empty() {
        for c in &clusters {
            let overlap: Vec<i64> = c
                .conversation_ids
                .iter()
                .copied()
                .filter(|id| bad_ids.contains(id))
                .collect();
            if overlap.len() >= 2 {
                alerts.push(radar_alert(
                    "rating_correlated",
                    format!("Cluster \"{}\" is associated with poor ratings", c.title),
                    format!(
                        "{} conversations in this cluster received \"not-good\" ratings. This is an association, not proof of causation.",
                        overlap.len()
                    ),
                    overlap,
                    Some(c.id),
                    "warning",
                ));
            }
        }
    }

    alerts.extend(radar_extensions(conn, &clusters));
    alerts
}

fn radar_clusters(conn: &rusqlite::Connection) -> Vec<RadarCluster> {
    let cluster_rows: Vec<(i64, String)> = conn
        .prepare("SELECT id, name FROM issue_clusters ORDER BY conversation_count DESC")
        .map(|mut stmt| {
            stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    if cluster_rows.is_empty() {
        return Vec::new();
    }

    let members: Vec<(i64, String, Vec<i64>)> = cluster_rows
        .into_iter()
        .filter_map(|(id, title)| {
            let member_ids: Vec<i64> = conn
                .prepare(
                    "SELECT conversation_id FROM issue_cluster_conversations WHERE cluster_id = ?1",
                )
                .and_then(|mut stmt| {
                    Ok(stmt
                        .query_map(rusqlite::params![id], |r| r.get::<_, i64>(0))?
                        .filter_map(|r| r.ok())
                        .collect())
                })
                .unwrap_or_default();
            Some((id, title, member_ids))
        })
        .collect();
    let all_conv_ids: Vec<i64> = members
        .iter()
        .flat_map(|(_, _, ids)| ids.iter().copied())
        .collect::<std::collections::HashSet<i64>>()
        .into_iter()
        .collect();
    if all_conv_ids.is_empty() {
        return Vec::new();
    }

    let placeholders = all_conv_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT c.id, c.customer_local_id, c.mailbox_local_id, c.created_at, julianday(c.created_at) AS jd
           FROM conversations c WHERE c.id IN ({placeholders})"
    );
    let facts: Vec<ConvFacts> = conn
        .prepare(&sql)
        .and_then(|mut stmt| {
            let params: Vec<&dyn rusqlite::ToSql> = all_conv_ids
                .iter()
                .map(|i| i as &dyn rusqlite::ToSql)
                .collect();
            Ok(stmt
                .query_map(params.as_slice(), |r| {
                    Ok(ConvFacts {
                        id: r.get(0)?,
                        customer_id: r.get(1)?,
                        mailbox_id: r.get(2)?,
                        created_at: r.get(3)?,
                        jd: r.get(4)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect())
        })
        .unwrap_or_default();
    let now_jd: f64 = conn
        .query_row("SELECT julianday('now')", [], |r| r.get(0))
        .unwrap_or(0.0);

    let mut clusters = Vec::new();
    for (id, title, member_ids) in members {
        let convs: Vec<&ConvFacts> = facts
            .iter()
            .filter(|f| member_ids.contains(&f.id))
            .collect();
        if convs.is_empty() {
            continue;
        }

        let recent = convs
            .iter()
            .filter(|f| f.jd.is_some_and(|d| now_jd - d <= 14.0))
            .count();
        let previous = convs
            .iter()
            .filter(|f| {
                f.jd.is_some_and(|d| now_jd - d > 14.0 && now_jd - d <= 28.0)
            })
            .count();
        let trend = if (recent as f64) > previous as f64 * 1.3 && recent >= 3 {
            "rising"
        } else if previous > 0 && (recent as f64) < previous as f64 * 0.7 {
            "falling"
        } else if previous == 0 && recent > 0 {
            "new"
        } else {
            "stable"
        };

        let customer_count = convs
            .iter()
            .filter_map(|f| f.customer_id)
            .collect::<std::collections::HashSet<i64>>()
            .len();
        let summary = format!(
            "{} conversations from {} customers.",
            convs.len(),
            customer_count
        );
        clusters.push(RadarCluster {
            id,
            title,
            conversation_ids: convs.iter().map(|f| f.id).collect(),
            customer_count,
            summary,
            trend,
        });
    }
    clusters
}

fn radar_extensions(conn: &rusqlite::Connection, clusters: &[RadarCluster]) -> Vec<Value> {
    let mut alerts: Vec<Value> = Vec::new();
    if clusters.is_empty() {
        return alerts;
    }
    let ids: Vec<i64> = clusters
        .iter()
        .flat_map(|c| c.conversation_ids.clone())
        .collect();
    if ids.is_empty() {
        return alerts;
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT c.id, c.customer_local_id, c.mailbox_local_id, c.created_at, julianday(c.created_at) AS jd
           FROM conversations c WHERE c.id IN ({placeholders})"
    );
    let facts: Vec<ConvFacts> = conn
        .prepare(&sql)
        .and_then(|mut stmt| {
            let params: Vec<&dyn rusqlite::ToSql> =
                ids.iter().map(|i| i as &dyn rusqlite::ToSql).collect();
            Ok(stmt
                .query_map(params.as_slice(), |r| {
                    Ok(ConvFacts {
                        id: r.get(0)?,
                        customer_id: r.get(1)?,
                        mailbox_id: r.get(2)?,
                        created_at: r.get(3)?,
                        jd: r.get(4)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect())
        })
        .unwrap_or_default();
    let now_jd: f64 = conn
        .query_row("SELECT julianday('now')", [], |r| r.get(0))
        .unwrap_or(0.0);

    for c in clusters {
        let convs: Vec<&ConvFacts> = facts
            .iter()
            .filter(|f| c.conversation_ids.contains(&f.id))
            .collect();
        if convs.len() < 2 {
            continue;
        }
        let conv_ids: Vec<i64> = convs.iter().map(|f| f.id).collect();

        let recent_count = convs
            .iter()
            .filter(|f| f.jd.is_some_and(|d| now_jd - d <= 30.0))
            .count();
        let older_count = convs
            .iter()
            .filter(|f| {
                f.jd.is_some_and(|d| now_jd - d > 30.0 && now_jd - d <= 60.0)
            })
            .count();
        if recent_count >= 2 && older_count >= 2 {
            alerts.push(radar_alert(
                "reappearing_issue",
                format!("Reappearing issue: {}", c.title),
                format!(
                    "{} conversations 30-60 days ago and {} in the last 30 days - the topic went quiet and came back. {}",
                    older_count, recent_count, c.summary
                ),
                conv_ids.clone(),
                Some(c.id),
                "warning",
            ));
        }

        let customers: std::collections::HashSet<i64> =
            convs.iter().filter_map(|f| f.customer_id).collect();
        if convs.len() >= 4 && !customers.is_empty() && customers.len() <= convs.len() / 2 {
            alerts.push(radar_alert(
                "customer_concentration",
                format!("Customer concentration: {}", c.title),
                format!(
                    "{} conversations from only {} distinct customers - a small group hitting the same problem repeatedly (an association, not a causal claim). {}",
                    convs.len(),
                    customers.len(),
                    c.summary
                ),
                conv_ids.clone(),
                Some(c.id),
                "info",
            ));
        }

        let mut mailbox_counts: std::collections::HashMap<i64, usize> =
            std::collections::HashMap::new();
        for f in &convs {
            if let Some(m) = f.mailbox_id {
                *mailbox_counts.entry(m).or_insert(0) += 1;
            }
        }
        let dominant = mailbox_counts
            .iter()
            .max_by_key(|(_, n)| **n)
            .map(|(m, n)| (*m, *n));
        if let Some((mailbox_id, n)) = dominant {
            if convs.len() >= 4 && n as f64 / convs.len() as f64 >= 0.7 {
                let mailbox_name: String = conn
                    .query_row(
                        "SELECT name FROM mailboxes WHERE id = ?1",
                        rusqlite::params![mailbox_id],
                        |r| r.get(0),
                    )
                    .unwrap_or_else(|_| format!("mailbox #{mailbox_id}"));
                alerts.push(radar_alert(
                    "inbox_concentration",
                    format!("Inbox concentration: {}", c.title),
                    format!(
                        "{} of {} conversations arrived in \"{}\" - the issue is concentrated in one channel. {}",
                        n,
                        convs.len(),
                        mailbox_name,
                        c.summary
                    ),
                    conv_ids.clone(),
                    Some(c.id),
                    "info",
                ));
            }
        }

        let mut jds: Vec<f64> = convs.iter().filter_map(|f| f.jd).collect();
        jds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let date_by_jd: Vec<(f64, &str)> = convs
            .iter()
            .filter_map(|f| f.jd.map(|d| (d, f.created_at.as_deref().unwrap_or(""))))
            .collect();
        if jds.len() >= 4 {
            let (mut best_start, mut best_count) = (None, 0usize);
            for &start in &jds {
                let in_window = jds
                    .iter()
                    .filter(|&&d| d >= start && d < start + 7.0)
                    .count();
                if in_window > best_count {
                    best_count = in_window;
                    best_start = Some(start);
                }
            }
            if let Some(best) = best_start {
                if best_count as f64 / jds.len() as f64 >= 0.6 {
                    let start_date = date_by_jd
                        .iter()
                        .find(|(d, _)| *d == best)
                        .map(|(_, s)| *s)
                        .unwrap_or("");
                    let releases: Vec<String> = conn
                        .prepare(
                            "SELECT version_label FROM incident_releases
                              WHERE released_at IS NOT NULL
                                AND julianday(released_at) >= ?1
                                AND julianday(released_at) < ?2
                              LIMIT 3",
                        )
                        .and_then(|mut stmt| {
                            Ok(stmt
                                .query_map(rusqlite::params![best, best + 7.0], |r| {
                                    r.get::<_, String>(0)
                                })?
                                .filter_map(|r| r.ok())
                                .collect())
                        })
                        .unwrap_or_default();
                    let release_note = if releases.is_empty() {
                        String::new()
                    } else {
                        format!(
                            " Releases recorded in this window: {} (association only).",
                            releases.join(", ")
                        )
                    };
                    alerts.push(radar_alert(
                        "release_correlation",
                        format!("Temporal burst: {}", c.title),
                        format!(
                            "{} of {} conversations started within one 7-day window starting {}. Temporal clustering suggests something changed - possibly a release - but this is an association, not a causal claim.{} {}",
                            best_count,
                            jds.len(),
                            truncate_chars(start_date, 10),
                            release_note,
                            c.summary
                        ),
                        conv_ids.clone(),
                        Some(c.id),
                        "warning",
                    ));
                }
            }
        }

        let mut by_customer: std::collections::HashMap<i64, (usize, f64, f64, Vec<i64>)> =
            std::collections::HashMap::new();
        for f in &convs {
            let (cid, jd) = match (f.customer_id, f.jd) {
                (Some(cid), Some(jd)) => (cid, jd),
                _ => continue,
            };
            let entry = by_customer.entry(cid).or_insert((0, jd, jd, Vec::new()));
            entry.0 += 1;
            entry.1 = entry.1.min(jd);
            entry.2 = entry.2.max(jd);
            entry.3.push(f.id);
        }
        let repeated: Vec<&(usize, f64, f64, Vec<i64>)> = by_customer
            .values()
            .filter(|e| e.0 >= 2 && e.2 - e.1 >= 14.0)
            .collect();
        if !repeated.is_empty() {
            alerts.push(radar_alert(
                "repeated_unresolved",
                format!("Repeated unresolved pattern: {}", c.title),
                format!(
                    "{} customer(s) wrote in {} times about this cluster across 14+ day spans - the underlying problem may not be resolved. {}",
                    repeated.len(),
                    repeated.iter().map(|e| e.0.to_string()).collect::<Vec<_>>().join(", "),
                    c.summary
                ),
                repeated.iter().flat_map(|e| e.3.clone()).collect(),
                Some(c.id),
                "warning",
            ));
        }
    }

    let recent7: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations
              WHERE julianday(created_at) >= julianday('now', '-7 days')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let prev7: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations
              WHERE julianday(created_at) >= julianday('now', '-14 days')
                AND julianday(created_at) < julianday('now', '-7 days')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if recent7 >= 10 && prev7 > 0 && recent7 as f64 / prev7 as f64 >= 1.4 {
        let sample: Vec<i64> = conn
            .prepare(
                "SELECT id FROM conversations
                  WHERE julianday(created_at) >= julianday('now', '-7 days')
                  ORDER BY created_at DESC LIMIT 10",
            )
            .map(|mut stmt| {
                stmt.query_map([], |r| r.get::<_, i64>(0))
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        alerts.push(radar_alert(
            "volume_spike",
            format!("Unusual support volume: {recent7} conversations in the last 7 days"),
            format!(
                "{} conversations in the last 7 days vs {} in the previous 7 (a {}% increase). An association with a change somewhere - not a causal claim.",
                recent7,
                prev7,
                ((recent7 as f64 / prev7 as f64 - 1.0) * 100.0).round() as i64
            ),
            sample,
            None,
            if recent7 as f64 / prev7 as f64 >= 2.0 {
                "critical"
            } else {
                "warning"
            },
        ));
    }

    alerts
}

pub async fn doc_gaps(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let days = clamp_days_param(params.get("days"), 90, 1, 3650);
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let mut gaps: Vec<Value> = grouped_questions(&conn, days)
        .into_iter()
        .filter(|(_, ids)| ids.len() >= 2)
        .map(|(question, ids)| {
            let knowledge_hits: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM fts_knowledge WHERE fts_knowledge MATCH ?1",
                    rusqlite::params![fts_tokens(&question)],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let known_answer: Option<String> = conn
                .query_row(
                    "SELECT body_text FROM conversation_threads
                      WHERE conversation_id = ?1 AND type = 'reply' AND state = 'published'
                      ORDER BY created_at ASC LIMIT 1",
                    rusqlite::params![ids[0]],
                    |r| r.get::<_, Option<String>>(0),
                )
                .unwrap_or(None);
            let coverage = if knowledge_hits == 0 {
                "missing"
            } else if knowledge_hits < 2 {
                "partial"
            } else {
                "ambiguous"
            };
            json!({
                "question": question,
                "conversation_count": ids.len(),
                "known_answer": known_answer.map(|b| truncate_chars(&b, 300)),
                "coverage": coverage,
                "suggested_doc_title": format!("Documentation: {}", truncate_chars(&question, 80)),
            })
        })
        .collect();
    gaps.truncate(15);
    (StatusCode::OK, Json(json!({ "gaps": gaps })))
}

pub async fn answer_reuse(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let days = clamp_days_param(params.get("days"), 90, 1, 3650);
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let mut candidates: Vec<Value> = grouped_questions(&conn, days)
        .into_iter()
        .filter(|(_, ids)| ids.len() >= 2)
        .map(|(question, ids)| {
            let common_resolution: Option<String> = conn
                .query_row(
                    "SELECT body_text FROM conversation_threads
                      WHERE conversation_id = ?1 AND type = 'reply' AND state = 'published'
                      ORDER BY created_at ASC LIMIT 1",
                    rusqlite::params![ids[0]],
                    |r| r.get::<_, Option<String>>(0),
                )
                .unwrap_or(None);
            let like = format!("%{}%", truncate_chars(&question, 40));
            let saved_reply_name: Option<String> = conn
                .query_row(
                    "SELECT name FROM saved_replies
                      WHERE deleted_at IS NULL AND (name LIKE ?1 OR preview LIKE ?1) LIMIT 1",
                    rusqlite::params![like],
                    |r| r.get(0),
                )
                .unwrap_or(None);
            let knowledge_doc_title: Option<String> = conn
                .query_row(
                    "SELECT title FROM knowledge_documents
                      WHERE title LIKE ?1 OR content LIKE ?1 LIMIT 1",
                    rusqlite::params![like],
                    |r| r.get(0),
                )
                .unwrap_or(None);
            json!({
                "question": question,
                "conversation_count": ids.len(),
                "common_resolution": common_resolution.map(|b| truncate_chars(&b, 300)),
                "saved_reply_name": saved_reply_name,
                "knowledge_doc_title": knowledge_doc_title,
            })
        })
        .collect();
    candidates.truncate(15);
    (StatusCode::OK, Json(json!({ "candidates": candidates })))
}

pub async fn issue_radar(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    (
        StatusCode::OK,
        Json(json!({ "alerts": issue_radar_impl(&conn) })),
    )
}

pub async fn metric_definitions(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let definitions: Vec<Value> = conn
        .prepare(
            "SELECT key, name, description, formula, source, limitations
             FROM metric_definitions ORDER BY key",
        )
        .map(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "key": r.get::<_, String>(0)?,
                    "name": r.get::<_, String>(1)?,
                    "description": r.get::<_, Option<String>>(2)?,
                    "formula": r.get::<_, Option<String>>(3)?,
                    "source": r.get::<_, String>(4)?,
                    "limitations": r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
        .unwrap_or_default();
    (StatusCode::OK, Json(json!({ "definitions": definitions })))
}

pub async fn release_correlation(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let releases: Vec<Value> = conn
        .prepare("SELECT name, version, occurred_at FROM release_events ORDER BY occurred_at DESC")
        .map(|mut stmt| {
            stmt.query_map([], |r| {
                let name: String = r.get(0)?;
                let version: Option<String> = r.get(1)?;
                let occurred_at: String = r.get(2)?;
                let before_7d: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM conversations
                          WHERE created_at IS NOT NULL
                            AND julianday(created_at) >= julianday(?1, '-7 days')
                            AND julianday(created_at) < julianday(?1)",
                        rusqlite::params![occurred_at],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                let after_7d: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM conversations
                          WHERE created_at IS NOT NULL
                            AND julianday(created_at) >= julianday(?1)
                            AND julianday(created_at) < julianday(?1, '+7 days')",
                        rusqlite::params![occurred_at],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                Ok(json!({
                    "release": name,
                    "version": version,
                    "occurred_at": occurred_at,
                    "before_7d": before_7d,
                    "after_7d": after_7d,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
        .unwrap_or_default();
    (
        StatusCode::OK,
        Json(json!({
            "releases": releases,
            "note": "Conversations before/after each release window. Timing overlap is described as \"potentially related\" - never as proven causation.",
            "source": "local"
        })),
    )
}

pub async fn release_events(
    State(state): State<AppState>,
    body: Option<Json<Value>>,
) -> axum::response::Response {
    let body = body.map(|Json(b)| b).unwrap_or_else(|| json!({}));

    let name = match body.get("name") {
        Some(Value::String(s)) => {
            let n = s.chars().count();
            if n == 0 {
                return release_event_422("name", "String must contain at least 1 character(s)");
            }
            if n > 200 {
                return release_event_422("name", "String must contain at most 200 character(s)");
            }
            s.clone()
        }
        None => return release_event_422("name", "Required"),
        Some(other) => {
            return release_event_422(
                "name",
                &format!("Expected string, received {}", zod_type_of(other)),
            );
        }
    };
    let version = match body.get("version") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => {
            if s.chars().count() > 100 {
                return release_event_422(
                    "version",
                    "String must contain at most 100 character(s)",
                );
            }
            s.clone()
        }
        Some(other) => {
            return release_event_422(
                "version",
                &format!("Expected string, received {}", zod_type_of(other)),
            );
        }
    };
    let occurred_at = match body.get("occurredAt") {
        Some(Value::String(s)) => {
            if !is_reference_date_time(s) {
                return release_event_422("occurredAt", "Invalid");
            }
            s.clone()
        }
        None => return release_event_422("occurredAt", "Required"),
        Some(other) => {
            return release_event_422(
                "occurredAt",
                &format!("Expected string, received {}", zod_type_of(other)),
            );
        }
    };
    let notes = match body.get("notes") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => {
            if s.chars().count() > 2000 {
                return release_event_422("notes", "String must contain at most 2000 character(s)");
            }
            Some(s.clone())
        }
        Some(other) => {
            return release_event_422(
                "notes",
                &format!("Expected string, received {}", zod_type_of(other)),
            );
        }
    };

    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match conn.execute(
        "INSERT INTO release_events (name, version, occurred_at, notes) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, version, occurred_at, notes],
    ) {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "ok": true, "message": "Release event recorded." })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "message": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn helpscout_report(
    State(state): State<AppState>,
    axum::extract::Path(report_key): axum::extract::Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> axum::response::Response {
    let days = clamp_days_param(params.get("days"), 30, 1, 3650);
    let now_iso = chrono::Utc::now().to_rfc3339();
    let days_ago_iso = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    let start: String = params
        .get("from")
        .filter(|s| !s.is_empty())
        .unwrap_or(&days_ago_iso)
        .chars()
        .take(10)
        .collect();
    let end: String = params
        .get("to")
        .filter(|s| !s.is_empty())
        .unwrap_or(&now_iso)
        .chars()
        .take(10)
        .collect();

    let provider: Option<std::sync::Arc<dyn crate::helpscout::HelpScoutProvider>> =
        if let Some(sync) = state.sync.as_ref() {
            Some(sync.provider().clone())
        } else if let Some(real) = state.real.clone() {
            let coerced: std::sync::Arc<dyn crate::helpscout::HelpScoutProvider> = real;
            Some(coerced)
        } else {
            None
        };

    let Some(provider) = provider else {
        return (
            StatusCode::OK,
            Json(json!({
                "ok": false,
                "message": "Help Scout report request failed: no provider configured"
            })),
        )
            .into_response();
    };

    let fetched = match report_key.as_str() {
        "company" => provider.get_company_overall_report(&start, &end).await,
        "conversations" => {
            provider
                .get_conversations_overall_report(&start, &end)
                .await
        }
        "happiness" => provider.get_happiness_ratings_report(&start, &end).await,
        "productivity" => provider.get_productivity_overall_report(&start, &end).await,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "statusCode": 404,
                    "error": "NotFound",
                    "message": "Unknown report. Available: company, conversations, happiness, productivity."
                })),
            )
                .into_response();
        }
    };

    match fetched {
        Ok(row) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "report": row,
                "source": "helpscout",
                "note": "Numbers come from Help Scout native reporting and use Help Scout definitions."
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::OK,
            Json(json!({
                "ok": false,
                "message": format!("Help Scout report request failed: {e}")
            })),
        )
            .into_response(),
    }
}

pub async fn narrative(
    State(state): State<AppState>,
    body: Option<Json<Value>>,
) -> axum::response::Response {
    let body = body.map(|Json(b)| b).unwrap_or_else(|| json!({}));

    let report_name = match body.get("reportName") {
        Some(Value::String(s)) => {
            let n = s.chars().count();
            if n == 0 {
                return narrative_422("reportName", "String must contain at least 1 character(s)");
            }
            if n > 200 {
                return narrative_422("reportName", "String must contain at most 200 character(s)");
            }
            s.clone()
        }
        None => return narrative_422("reportName", "Required"),
        Some(other) => {
            return narrative_422(
                "reportName",
                &format!("Expected string, received {}", zod_type_of(other)),
            );
        }
    };
    let facts = match body.get("facts") {
        None | Some(Value::Null) => json!({}),
        Some(v @ Value::Object(map)) => {
            for (k, val) in map {
                if !matches!(
                    val,
                    Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null
                ) {
                    return narrative_422(
                        &format!("facts.{k}"),
                        &format!("Expected string, received {}", zod_type_of(val)),
                    );
                }
            }
            v.clone()
        }
        Some(other) => {
            return narrative_422(
                "facts",
                &format!("Expected object, received {}", zod_type_of(other)),
            );
        }
    };

    let result = state
        .run_ai(move |conn| {
            Box::pin(async move {
                crate::ai_pipeline::ensure_pipeline_schema(conn).ok();
                let backend = crate::ai_pipeline::backend_from_settings(conn);
                crate::ai_pipeline::report_narrative(conn, &backend, &report_name, &facts).await
            })
        })
        .await;
    match result {
        Ok(Ok(narrative)) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "narrative": narrative,
                "ai_generated": true,
                "note": "This narrative was AI-generated locally from the computed facts above."
            })),
        )
            .into_response(),
        Ok(Err(e)) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "ok": false, "error": e.message })),
        )
            .into_response(),
        Err(join) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "ok": false, "error": join })),
        )
            .into_response(),
    }
}

fn narrative_422(path: &str, message: &str) -> axum::response::Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "statusCode": 422,
            "error": "ValidationError",
            "message": format!("Invalid request ({path}): {message}"),
            "issues": [{ "path": path, "message": message }]
        })),
    )
        .into_response()
}

fn zod_type_of(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
pub async fn effectiveness(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let days = super::quality::clamp_days_param(params.get("days"), 90, 1, 3650);
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    (
        StatusCode::OK,
        Json(crate::quality::effectiveness_report(&conn, days)),
    )
}

pub async fn report_catalog() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "metrics": crate::catalog::reporting::all_metric_entries(),
            "dimensions": crate::catalog::reporting::all_dimension_entries(),
            "origin": "local",
            "note": "This builder computes local SupportOS metrics only. Native Help Scout reports remain available under the Help Scout tab, clearly labeled as Help Scout-originated."
        })),
    )
}

pub async fn builder_run(
    State(state): State<AppState>,
    body: Option<Json<Value>>,
) -> impl IntoResponse {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    match crate::reports::parse_report_config(&body) {
        Err(issues) => {
            let message = issues
                .iter()
                .map(|(path, msg)| format!("{path}: {msg}"))
                .collect::<Vec<_>>()
                .join("; ");
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": message
                })),
            )
        }
        Ok(config) => {
            let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
            match crate::reports::run_builder_report(&conn, &config) {
                Ok(outcome) => (
                    StatusCode::OK,
                    Json(serde_json::to_value(&outcome).unwrap_or(json!({}))),
                ),
                Err(message) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422,
                        "error": "ValidationError",
                        "message": message
                    })),
                ),
            }
        }
    }
}

pub async fn builder_saved_list(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::reports::list_saved_reports(&conn) {
        Ok(saved) => (
            StatusCode::OK,
            Json(json!({
                "saved": saved
                    .iter()
                    .filter_map(|s| serde_json::to_value(s).ok())
                    .collect::<Vec<Value>>()
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        ),
    }
}

pub async fn builder_saved_create(
    State(state): State<AppState>,
    body: Option<Json<Value>>,
) -> impl IntoResponse {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() && n.len() <= 120 => n.to_string(),
        Some(n) if n.is_empty() => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": "name: String must contain at least 1 character(s)"
                })),
            );
        }
        Some(_) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": "name: String must contain at most 120 character(s)"
                })),
            );
        }
        None => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": "name: Required"
                })),
            );
        }
    };
    match crate::reports::parse_report_config(&body) {
        Err(issues) => {
            let message = issues
                .iter()
                .map(|(path, msg)| format!("{path}: {msg}"))
                .collect::<Vec<_>>()
                .join("; ");
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": message
                })),
            )
        }
        Ok(config) => {
            let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
            match crate::reports::save_report(&conn, &name, &config) {
                Ok(saved) => (
                    StatusCode::OK,
                    Json(json!({
                        "ok": true,
                        "saved": serde_json::to_value(&saved).unwrap_or(json!({}))
                    })),
                ),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"message": e.to_string()})),
                ),
            }
        }
    }
}

pub async fn builder_saved_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let id_num = crate::conversation_ops::js_number(&id);
    let Some(idv) = id_num.filter(|v| v.fract() == 0.0 && *v > 0.0) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "Report id must be a positive integer."
            })),
        );
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    match crate::reports::delete_saved_report(&conn, idv as i64) {
        Ok(true) => (StatusCode::OK, Json(json!({"ok": true}))),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Saved report not found."
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"message": e.to_string()})),
        ),
    }
}
