use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use super::super::server::AppState;
use axum::response::IntoResponse;

pub async fn get_settings(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    (StatusCode::OK, Json(all_settings_json(&conn)))
}

fn all_settings_json(conn: &rusqlite::Connection) -> Value {
    let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if let Ok(mut stmt) = conn.prepare("SELECT key, value FROM application_settings") {
        if let Ok(rows) =
            stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        {
            for row in rows.flatten() {
                map.insert(row.0, row.1);
            }
        }
    }
    fn get_str(
        map: &std::collections::HashMap<String, String>,
        key: &str,
        default: &str,
    ) -> String {
        map.get(key).cloned().unwrap_or_else(|| default.to_string())
    }
    fn get_bool(map: &std::collections::HashMap<String, String>, key: &str, default: bool) -> bool {
        map.get(key)
            .and_then(|v| match v.as_str() {
                "true" | "1" => Some(true),
                "false" | "0" => Some(false),
                _ => None,
            })
            .unwrap_or(default)
    }
    fn get_i64(map: &std::collections::HashMap<String, String>, key: &str, default: i64) -> i64 {
        map.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
    }
    fn get_opt_i64(
        map: &std::collections::HashMap<String, String>,
        key: &str,
    ) -> Option<serde_json::Value> {
        map.get(key).and_then(|v| {
            if v.is_empty() || v == "null" {
                None
            } else {
                v.parse::<i64>().ok().map(serde_json::Value::from)
            }
        })
    }

    json!({
        "sync_interval_minutes": get_i64(&map, "sync_interval_minutes", 5),
        "api_concurrency": get_i64(&map, "api_concurrency", 2),
        "ai_enabled": get_bool(&map, "ai_enabled", true),
        "automatic_analysis_enabled": get_bool(&map, "automatic_analysis_enabled", true),
        "automatic_note_enabled": get_bool(&map, "automatic_note_enabled", false),
        "automatic_draft_enabled": get_bool(&map, "automatic_draft_enabled", false),
        "automation_enabled": get_bool(&map, "automation_enabled", false),
        "automation_write_actions_enabled": get_bool(&map, "automation_write_actions_enabled", false),
        "qdrant_enabled": get_bool(&map, "qdrant_enabled", true),
        "attachment_auto_download": get_bool(&map, "attachment_auto_download", true),
        "automatic_reply_sending": get_bool(&map, "automatic_reply_sending", false),
        "retention_days": get_opt_i64(&map, "retention_days"),
        "backup_interval_hours": get_i64(&map, "backup_interval_hours", 24),
        "log_level": get_str(&map, "log_level", "info"),
        "display_timezone": get_str(&map, "display_timezone", "system"),
        "redaction_enabled": get_bool(&map, "redaction_enabled", true),
        "ai_evaluation_mode": get_bool(&map, "ai_evaluation_mode", false),
        "agent_language": get_str(&map, "agent_language", "en"),
    })
}

pub async fn update_settings(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let invalid = |cx: &str| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "ok": false,
                "message": "Invalid settings patch: unknown or badly typed keys were rejected.",
                "detail": cx,
            })),
        )
    };

    let Some(obj) = body.as_object() else {
        return invalid("body must be an object");
    };

    let is_int_in =
        |v: &Value, lo: i64, hi: i64| v.as_i64().is_some_and(|n| (lo..=hi).contains(&n));
    for (key, value) in obj {
        let ok = match key.as_str() {
            "sync_interval_minutes" => is_int_in(value, 1, 1440),
            "api_concurrency" => is_int_in(value, 1, 10),
            "ai_enabled"
            | "automatic_analysis_enabled"
            | "automatic_note_enabled"
            | "automatic_draft_enabled"
            | "automation_enabled"
            | "automation_write_actions_enabled"
            | "qdrant_enabled"
            | "attachment_auto_download"
            | "automatic_reply_sending"
            | "redaction_enabled"
            | "ai_evaluation_mode" => value.is_boolean(),
            "qdrant_url" => value.as_str().is_some_and(|s| {
                s.len() <= 500 && s.contains("://") && !s.contains(char::is_whitespace)
            }),
            "retention_days" => value.is_null() || is_int_in(value, 1, 3650),
            "backup_interval_hours" => value.is_null() || is_int_in(value, 1, 720),
            "log_level" => matches!(
                value.as_str(),
                Some("debug") | Some("info") | Some("warn") | Some("error")
            ),
            "display_timezone" => value.as_str().is_some_and(|s| s.len() <= 64),
            "agent_language" => value
                .as_str()
                .is_some_and(|s| s.len() == 2 && s.chars().all(|c| c.is_ascii_lowercase())),
            _ => false,
        };
        if !ok {
            return invalid(key);
        }
    }

    let qdrant_patch = (
        obj.get("qdrant_url")
            .and_then(|v| v.as_str())
            .map(String::from),
        obj.get("qdrant_enabled").and_then(|v| v.as_bool()),
    );
    let interval_changed = obj.contains_key("sync_interval_minutes");
    {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        for (key, value) in obj {
            let res = match value {
                Value::Bool(b) => {
                    let b = if key == "automatic_reply_sending" {
                        false
                    } else {
                        *b
                    };
                    crate::settings::set_bool(&conn, key, b)
                }
                Value::Number(_) => {
                    crate::settings::set_i64(&conn, key, value.as_i64().unwrap_or(0))
                }
                Value::String(s) => crate::settings::set_string(&conn, key, s),
                Value::Null => crate::settings::set_string(&conn, key, ""),
                _ => Ok(()),
            };
            let _ = res;
        }
        let entry = crate::audit::AuditEntry::user("settings_updated").with_after_state(json!({
            "keys": obj.keys().cloned().collect::<Vec<_>>(),
        }));
        let _ = crate::audit::audit(&conn, &entry);
    }

    if qdrant_patch.0.is_some() || qdrant_patch.1.is_some() {
        state.qdrant.reconfigure(qdrant_patch.0, qdrant_patch.1);
    }
    if interval_changed {
        if let Some(workers) = state.workers.clone() {
            workers.stop();
            workers.start();
        }
    }

    let settings = {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        all_settings_json(&conn)
    };
    (
        StatusCode::OK,
        Json(json!({"ok": true, "message": "Settings saved.", "settings": settings})),
    )
}

pub async fn get_lmstudio(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let base_url = crate::settings::get_string(&conn, "lmstudio_base_url")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            crate::ai_center::get_ai_status(&conn)
                .ok()
                .and_then(|s| s.base_url)
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "http://127.0.0.1:1234".to_string());
    let chat_model = crate::settings::get_string(&conn, "lmstudio_chat_model")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            crate::ai_center::get_ai_status(&conn)
                .ok()
                .and_then(|s| s.chat_model)
                .filter(|s| !s.is_empty())
        });
    let embedding_model = crate::settings::get_string(&conn, "lmstudio_embedding_model")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            crate::ai_center::get_ai_status(&conn)
                .ok()
                .and_then(|s| s.embedding_model)
                .filter(|s| !s.is_empty())
        });
    let timeout_ms =
        crate::settings::get_i64(&conn, "lmstudio_timeout_ms", 120_000).unwrap_or(120_000);
    let concurrency = crate::settings::get_i64(&conn, "lmstudio_concurrency", 2).unwrap_or(2);
    (
        StatusCode::OK,
        Json(json!({
            "base_url": base_url,
            "chat_model": chat_model,
            "embedding_model": embedding_model,
            "timeout_ms": timeout_ms,
            "concurrency": concurrency,
        })),
    )
}

pub async fn update_lmstudio(
    State(state): State<AppState>,
    body: Option<Json<Value>>,
) -> impl IntoResponse {
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let base_url = match body.get("base_url") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => {
            let ok = s.contains("://") && s.len() > "://".len() && !s.contains(' ');
            if !ok {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422,
                        "error": "ValidationError",
                        "message": "Invalid LM Studio settings."
                    })),
                );
            }
            Some(s.clone())
        }
        Some(_) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": "Invalid LM Studio settings."
                })),
            )
        }
    };
    let chat_model = match body.get("chat_model") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(s)) => Some(Some(s.clone())),
        Some(_) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": "Invalid LM Studio settings."
                })),
            )
        }
    };
    let embedding_model = match body.get("embedding_model") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(s)) => Some(Some(s.clone())),
        Some(_) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "statusCode": 422,
                    "error": "ValidationError",
                    "message": "Invalid LM Studio settings."
                })),
            )
        }
    };
    let timeout_ms = match body.get("timeout_ms") {
        None | Some(Value::Null) => None,
        Some(v) => match v.as_f64() {
            Some(f) if f.fract() == 0.0 && f >= 1000.0 && f <= i64::MAX as f64 => Some(f as i64),
            _ => {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422,
                        "error": "ValidationError",
                        "message": "Invalid LM Studio settings."
                    })),
                )
            }
        },
    };
    let concurrency = match body.get("concurrency") {
        None | Some(Value::Null) => None,
        Some(v) => match v.as_f64() {
            Some(f) if f.fract() == 0.0 && f >= 1.0 && f <= i64::MAX as f64 => Some(f as i64),
            _ => {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "statusCode": 422,
                        "error": "ValidationError",
                        "message": "Invalid LM Studio settings."
                    })),
                )
            }
        },
    };
    {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(url) = &base_url {
            let _ = crate::settings::set_string(&conn, "lmstudio_base_url", url);
        }
        if let Some(model) = &chat_model {
            let _ = crate::settings::set_string(
                &conn,
                "lmstudio_chat_model",
                model.as_deref().unwrap_or(""),
            );
        }
        if let Some(model) = &embedding_model {
            let _ = crate::settings::set_string(
                &conn,
                "lmstudio_embedding_model",
                model.as_deref().unwrap_or(""),
            );
        }
        if let Some(ms) = timeout_ms {
            let _ = crate::settings::set_i64(&conn, "lmstudio_timeout_ms", ms);
        }
        if let Some(c) = concurrency {
            let _ = crate::settings::set_i64(&conn, "lmstudio_concurrency", c);
        }
    }
    (
        StatusCode::OK,
        Json(json!({"ok": true, "message": "LM Studio settings saved."})),
    )
}

pub async fn test_lmstudio(State(state): State<AppState>) -> axum::response::Response {
    let (base_url, timeout_ms) = {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        let base_url = crate::settings::get_string(&conn, "lmstudio_base_url")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                crate::ai_center::get_ai_status(&conn)
                    .ok()
                    .and_then(|s| s.base_url)
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or_else(|| "http://127.0.0.1:1234".to_string());
        let timeout_ms = u64::try_from(
            crate::settings::get_i64(&conn, "lmstudio_timeout_ms", 120_000).unwrap_or(120_000),
        )
        .unwrap_or(crate::ai_lm_studio::LM_STUDIO_DEFAULT_TIMEOUT_MS);
        (base_url, timeout_ms)
    };
    let client =
        crate::ai_lm_studio::OpenAiCompatibleClient::new_with_timeout(&base_url, timeout_ms);
    match client.list_models().await {
        Ok(models) => (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "connected": true,
                "models": models.iter().map(|m| m.id.clone()).collect::<Vec<_>>(),
                "message": format!("LM Studio reachable - {} model(s) available.", models.len()),
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::OK,
            Json(json!({
                "ok": false,
                "connected": false,
                "models": [],
                "message": e.to_string(),
            })),
        )
            .into_response(),
    }
}

pub async fn get_qdrant(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let enabled = crate::settings::get_bool(&conn, "qdrant_enabled", true).unwrap_or(true);
    let url = crate::settings::get_string(&conn, "qdrant_url")
        .ok()
        .flatten()
        .unwrap_or_else(|| crate::vectorstore_qdrant::DEFAULT_QDRANT_URL.to_string());
    (
        StatusCode::OK,
        Json(json!({"enabled": enabled, "url": url})),
    )
}

pub async fn update_qdrant(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let url = body.get("url");
    let enabled = body.get("enabled");
    if let Some(u) = url {
        let valid = u.as_str().is_some_and(|s| {
            (s.starts_with("http://") || s.starts_with("https://"))
                && s.len() > "http://".len()
                && !s.contains(char::is_whitespace)
        });
        if !valid {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"ok": false, "message": "Invalid Qdrant URL."})),
            );
        }
    }
    if let Some(e) = enabled {
        if !e.is_boolean() {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"ok": false, "message": "Invalid Qdrant enabled flag."})),
            );
        }
    }
    let (set_url, set_enabled) = {
        let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(u) = url.and_then(|v| v.as_str()) {
            let _ = crate::settings::set_string(&conn, "qdrant_url", u);
        }
        if let Some(e) = enabled.and_then(|v| v.as_bool()) {
            let _ = crate::settings::set_bool(&conn, "qdrant_enabled", e);
        }
        (
            url.and_then(|v| v.as_str()).map(String::from),
            enabled.and_then(|v| v.as_bool()),
        )
    };
    state.qdrant.reconfigure(set_url, set_enabled);
    (
        StatusCode::OK,
        Json(json!({"ok": true, "message": "Qdrant settings saved."})),
    )
}

fn ensure_business_hours_table(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS mailbox_business_hours (
            mailbox_local_id INTEGER PRIMARY KEY,
            timezone TEXT NOT NULL DEFAULT 'UTC',
            days TEXT NOT NULL DEFAULT '[1,2,3,4,5]',
            start_minute INTEGER NOT NULL DEFAULT 540,
            end_minute INTEGER NOT NULL DEFAULT 1020,
            first_response_target_min INTEGER,
            resolution_target_min INTEGER,
            updated_at TEXT NOT NULL
        );",
    )
}

fn safe_parse_days(raw: &str) -> Option<Vec<i64>> {
    serde_json::from_str::<Value>(raw).ok().and_then(|v| {
        v.as_array()
            .and_then(|arr| arr.iter().map(|d| d.as_i64()).collect::<Option<Vec<i64>>>())
    })
}

pub async fn get_business_hours(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    if ensure_business_hours_table(&conn).is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                json!({"statusCode": 500, "error": "InternalServerError", "message": "Failed to read business hours."}),
            ),
        );
    }
    let mailboxes: Vec<(i64, String)> = match conn
        .prepare("SELECT id, name FROM mailboxes ORDER BY name")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                .map(|rows| rows.flatten().collect())
        }) {
        Ok(v) => v,
        Err(_) => Vec::new(),
    };
    let mut rows: std::collections::HashMap<
        i64,
        (String, String, i64, i64, Option<i64>, Option<i64>),
    > = std::collections::HashMap::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT mailbox_local_id, timezone, days, start_minute, end_minute, first_response_target_min, resolution_target_min
         FROM mailbox_business_hours",
    ) {
        if let Ok(mapped) = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                (
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                ),
            ))
        }) {
            for (id, cfg) in mapped.flatten() {
                rows.insert(id, cfg);
            }
        }
    }
    let mailboxes: Vec<Value> = mailboxes
        .into_iter()
        .map(|(id, name)| {
            let row = rows.get(&id);
            json!({
                "mailbox_id": id,
                "name": name,
                "configured": row.is_some(),
                "timezone": row.map(|r| json!(r.0)).unwrap_or(Value::Null),
                "days": row.and_then(|r| safe_parse_days(&r.1)).map(|d| json!(d)).unwrap_or(Value::Null),
                "start_minute": row.map(|r| json!(r.2)).unwrap_or(Value::Null),
                "end_minute": row.map(|r| json!(r.3)).unwrap_or(Value::Null),
                "first_response_target_min": row.and_then(|r| r.4).map(|v| json!(v)).unwrap_or(Value::Null),
                "resolution_target_min": row.and_then(|r| r.5).map(|v| json!(v)).unwrap_or(Value::Null),
            })
        })
        .collect();
    (StatusCode::OK, Json(json!({ "mailboxes": mailboxes })))
}

fn validate_business_hours(
    body: &Value,
) -> Option<(String, Vec<i64>, i64, i64, Option<i64>, Option<i64>)> {
    let obj = body.as_object()?;
    let allowed = [
        "timezone",
        "days",
        "start_minute",
        "end_minute",
        "first_response_target_min",
        "resolution_target_min",
    ];
    if obj.keys().any(|k| !allowed.contains(&k.as_str())) {
        return None;
    }
    let timezone = obj.get("timezone")?.as_str()?;
    if timezone.is_empty() || timezone.len() > 64 {
        return None;
    }
    let days_arr = obj.get("days")?.as_array()?;
    if days_arr.len() < 1 || days_arr.len() > 7 {
        return None;
    }
    let mut days = Vec::new();
    for d in days_arr {
        let n = d.as_i64()?;
        if !(0..=6).contains(&n) {
            return None;
        }
        days.push(n);
    }
    let start = obj.get("start_minute")?.as_i64()?;
    if !(0..=1439).contains(&start) {
        return None;
    }
    let end = obj.get("end_minute")?.as_i64()?;
    if !(1..=1440).contains(&end) {
        return None;
    }
    if end <= start {
        return None;
    }
    let targets = |key: &str| -> Option<Option<i64>> {
        match obj.get(key) {
            None | Some(Value::Null) => Some(None),
            Some(v) => {
                let n = v.as_i64()?;
                if !(1..=100000).contains(&n) {
                    None
                } else {
                    Some(Some(n))
                }
            }
        }
    };
    let first = targets("first_response_target_min")?;
    let resolution = targets("resolution_target_min")?;
    Some((timezone.to_string(), days, start, end, first, resolution))
}

fn is_valid_timezone(tz: &str) -> bool {
    tz.parse::<chrono_tz::Tz>().is_ok()
}

pub async fn set_business_hours(
    State(state): State<AppState>,
    axum::extract::Path(mailbox_id): axum::extract::Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let conn = state.conn_lock();
    let id: i64 = match mailbox_id.parse() {
        Ok(n) if n > 0 => n,
        _ => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(
                    json!({"statusCode": 422, "error": "ValidationError", "message": "mailboxId must be a positive integer."}),
                ),
            )
        }
    };
    let mailbox_name: Option<String> = conn
        .query_row("SELECT name FROM mailboxes WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .ok();
    let Some(mailbox_name) = mailbox_name else {
        return (
            StatusCode::NOT_FOUND,
            Json(
                json!({"statusCode": 404, "error": "NotFound", "message": "Mailbox not found in the local mirror."}),
            ),
        );
    };
    let Some((timezone, days, start, end, first, resolution)) = validate_business_hours(&body)
    else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({"ok": false, "message": "Invalid business hours: check timezone, days (0-6), and that the end time is after the start time."}),
            ),
        );
    };
    if !is_valid_timezone(&timezone) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({"ok": false, "message": "Unknown IANA timezone (e.g. America/New_York, Europe/Berlin, Asia/Kolkata)."}),
            ),
        );
    }
    if ensure_business_hours_table(&conn).is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                json!({"statusCode": 500, "error": "InternalServerError", "message": "Failed to save business hours."}),
            ),
        );
    }
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let days_json = serde_json::to_string(&days).unwrap_or_else(|_| "[1,2,3,4,5]".into());
    let res = conn.execute(
        "INSERT INTO mailbox_business_hours (mailbox_local_id, timezone, days, start_minute, end_minute, first_response_target_min, resolution_target_min, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(mailbox_local_id) DO UPDATE SET
           timezone=excluded.timezone, days=excluded.days, start_minute=excluded.start_minute,
           end_minute=excluded.end_minute, first_response_target_min=excluded.first_response_target_min,
           resolution_target_min=excluded.resolution_target_min, updated_at=excluded.updated_at",
        rusqlite::params![id, timezone, days_json, start, end, first, resolution, now],
    );
    if res.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                json!({"statusCode": 500, "error": "InternalServerError", "message": "Failed to save business hours."}),
            ),
        );
    }
    let _ = crate::audit::audit(
        &conn,
        &crate::audit::AuditEntry::user("business_hours_updated").with_after_state(json!({
            "mailboxId": id, "timezone": timezone, "days": days, "start_minute": start, "end_minute": end,
            "first_response_target_min": first, "resolution_target_min": resolution,
        })),
    );
    (
        StatusCode::OK,
        Json(
            json!({"ok": true, "message": format!("Business hours saved for {mailbox_name}. SLA reports now measure this mailbox in business minutes.")}),
        ),
    )
}

pub async fn delete_business_hours(
    State(state): State<AppState>,
    axum::extract::Path(mailbox_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let conn = state.conn_lock();
    let id: i64 = match mailbox_id.parse() {
        Ok(n) if n > 0 => n,
        _ => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(
                    json!({"statusCode": 422, "error": "ValidationError", "message": "mailboxId must be a positive integer."}),
                ),
            )
        }
    };
    let _ = ensure_business_hours_table(&conn);
    let _ = conn.execute(
        "DELETE FROM mailbox_business_hours WHERE mailbox_local_id = ?1",
        [id],
    );
    (
        StatusCode::OK,
        Json(
            json!({"ok": true, "message": "Business hours cleared - this mailbox falls back to wall-clock minutes in SLA reports."}),
        ),
    )
}

pub async fn appearance(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.conn_lock();
    let tz = crate::settings::get_string(&conn, "display_timezone")
        .ok()
        .flatten()
        .unwrap_or_else(|| "system".to_string());
    (
        StatusCode::OK,
        Json(json!({ "display_timezone": tz, "themes": ["light", "dark"], "default": "light" })),
    )
}

pub async fn qdrant_test(State(state): State<AppState>) -> impl IntoResponse {
    let health = state.qdrant.health();
    let message = if health.connected {
        format!(
            "Qdrant reachable at {} ({} collections).",
            health.url,
            health.collections.len()
        )
    } else {
        format!("Qdrant is not reachable at {}. Keyword search (FTS) remains fully functional without it.", health.url)
    };
    (
        StatusCode::OK,
        Json(json!({
            "ok": health.connected,
            "connected": health.connected,
            "url": health.url,
            "collections": health.collections,
            "error": health.error,
            "message": message,
        })),
    )
}
