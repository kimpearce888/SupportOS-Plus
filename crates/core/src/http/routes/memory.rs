use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use super::super::server::AppState;

pub async fn meta(State(state): State<AppState>) -> Json<Value> {
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM customer_memory", [], |r| r.get(0))
        .unwrap_or(0);
    let by_source: Vec<Value> = conn
        .prepare("SELECT source, COUNT(*) FROM customer_memory GROUP BY source ORDER BY 2 DESC")
        .ok()
        .map(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "source": r.get::<_, String>(0)?,
                    "count": r.get::<_, i64>(1)?,
                }))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        })
        .unwrap_or_default();
    Json(json!({
        "total": total,
        "by_source": by_source,
        "sections": by_source.iter().map(|s| s["source"].clone()).collect::<Vec<_>>(),
        "section_labels": by_source.iter().map(|s| {
            let src = s["source"].as_str().unwrap_or("manual");
            let label = match src {
                "ai" => "AI-generated",
                "manual" => "Manually added",
                "imported" => "Imported",
                _ => src,
            };
            json!({src: label})
        }).collect::<Vec<_>>(),
        "entry_kinds": ["preference", "history", "signal", "note"],
        "notes": "Memory stats aggregated from the local customer_memory table."
    }))
}

pub async fn get(State(state): State<AppState>, Path(customer_id): Path<String>) -> Response {
    let Some(id) =
        crate::conversation_ops::js_number(&customer_id).filter(|v| v.fract() == 0.0 && *v > 0.0)
    else {
        return memory_id_422_customer();
    };
    let id = id as i64;
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM customers WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![id],
            |_| Ok(()),
        )
        .is_ok();
    if !exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Customer not found."
            })),
        )
            .into_response();
    }

    let rows: Vec<(i64, String, Option<String>, String, String)> = conn
        .prepare(
            "SELECT id, memory_key, memory_value, source, created_at
             FROM customer_memory WHERE customer_id = ?1 ORDER BY created_at DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .ok()
        })
        .unwrap_or_default();
    let entries: Vec<Value> = rows
        .into_iter()
        .filter(|(_, key, value, _, _)| !is_quarantined(key, value.as_deref()))
        .map(|(entry_id, key, value, source, created_at)| {
            json!({
                "id": entry_id,
                "customer_id": id,
                "key": key,
                "value": value,
                "source": source,
                "created_at": created_at,
            })
        })
        .collect();

    let profile = compose_memory_profile(&conn, id);

    (
        StatusCode::OK,
        Json(json!({
            "customerId": id,
            "sections": profile.sections,
            "quarantined": profile.quarantined,
            "notes": profile.notes,
            "freshness": profile.freshness,
            "entries": entries,
        })),
    )
        .into_response()
}

const MEMORY_ENTRY_KINDS: [&str; 5] = ["fact", "account", "preference", "issue_history", "context"];

pub async fn add_entry(
    State(state): State<AppState>,
    Path(customer_id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let Some(id) =
        crate::conversation_ops::js_number(&customer_id).filter(|v| v.fract() == 0.0 && *v > 0.0)
    else {
        return memory_id_422_customer();
    };
    let id = id as i64;
    let body = body.map(|b| b.0).unwrap_or_else(|| json!({}));
    let key: String = match body.get("key") {
        Some(Value::String(s)) => {
            let n = s.chars().count();
            if n < 1 || n > 120 {
                return crate::conversation_ops::zod_422(
                    "key",
                    "String must contain at most 120 character(s)",
                );
            }
            s.trim().to_string()
        }
        _ => return crate::conversation_ops::zod_422("key", "Required"),
    };
    let value: Option<String> = match body.get("value") {
        None | Some(Value::Null) => None,
        Some(Value::String(v)) => {
            if v.chars().count() > 2000 {
                return crate::conversation_ops::zod_422(
                    "value",
                    "String must contain at most 2000 character(s)",
                );
            }
            Some(v.clone())
        }
        Some(_) => {
            return crate::conversation_ops::zod_422(
                "value",
                "Expected string, received non-string",
            );
        }
    };
    let kind: &str = match body.get("kind") {
        None | Some(Value::Null) => "fact",
        Some(Value::String(k)) => {
            if !MEMORY_ENTRY_KINDS.contains(&k.as_str()) {
                return crate::conversation_ops::zod_422(
                    "kind",
                    &crate::conversation_ops::zod_enum_message(&MEMORY_ENTRY_KINDS, k),
                );
            }
            MEMORY_ENTRY_KINDS
                .iter()
                .find(|k2| **k2 == *k)
                .copied()
                .unwrap_or("fact")
        }
        Some(_) => {
            return crate::conversation_ops::zod_422(
                "kind",
                "Expected string, received non-string",
            );
        }
    };
    let conversation_id: Option<i64> = match body.get("conversation_id") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let Some(n) = v.as_i64() else {
                return crate::conversation_ops::zod_422(
                    "conversation_id",
                    "Expected number, received non-number",
                );
            };
            if n < 1 {
                return crate::conversation_ops::zod_422(
                    "conversation_id",
                    "Number must be greater than or equal to 1",
                );
            }
            Some(n)
        }
    };
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let customer_exists: bool = conn
        .query_row(
            "SELECT 1 FROM customers WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![id],
            |_| Ok(()),
        )
        .is_ok();
    if !customer_exists {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Customer not found."
            })),
        )
            .into_response();
    }
    if let Some(conv) = conversation_id {
        let conv_exists: bool = conn
            .query_row(
                "SELECT 1 FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
                rusqlite::params![conv],
                |_| Ok(()),
            )
            .is_ok();
        if !conv_exists {
            return crate::conversation_ops::zod_422(
                "conversation_id",
                "Linked conversation not found.",
            );
        }
    }
    if is_quarantined(&key, value.as_deref()) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "statusCode": 422,
                "error": "ValidationError",
                "message": "SupportOS policy: psychological/personality judgments are never stored as customer memory. Rephrase as an observable fact."
            })),
        )
            .into_response();
    }
    let inserted = conn.execute(
        "INSERT INTO customer_memory (customer_id, memory_key, memory_value, source, origin,
                                     first_seen_at, last_seen_at, confidence, provenance, kind)
         VALUES (?1, ?2, ?3, 'human', 'manual', datetime('now'), datetime('now'), 'high',
                 'human_local', ?4)
         ON CONFLICT (customer_id, memory_key) DO UPDATE SET
             memory_value = excluded.memory_value,
             source = 'human',
             origin = 'manual',
             last_seen_at = datetime('now'),
             confidence = 'high',
             provenance = 'human_local',
             kind = excluded.kind",
        rusqlite::params![id, key, value, kind],
    );
    match inserted {
        Ok(_) => {
            let entry_id = conn.last_insert_rowid();
            (
                StatusCode::OK,
                Json(json!({ "ok": true, "entry_id": entry_id })),
            )
                .into_response()
        }
        Err(_) => (
            StatusCode::OK,
            Json(json!({ "ok": true, "customerId": id })),
        )
            .into_response(),
    }
}

const QUARANTINE_WORDS: [&str; 16] = [
    "personality",
    "temperament",
    "mental health",
    "emotional state",
    "emotional instability",
    "cognitive trait",
    "mood disorder",
    "anxiety disorder",
    "bipolar",
    "diagnosis",
    "diagnosed",
    "diagnoses",
    "depressed",
    "depression",
    "intelligence level",
    "intelligence score",
];

const QUARANTINE_PREFIXES: [&str; 8] = [
    "psycholog",
    "introvert",
    "extrovert",
    "extravert",
    "neurotic",
    "narcissis",
    "schizo",
    "autis",
];

fn is_js_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn has_lower_word(hay: &str, needle: &str) -> bool {
    let mut start = 0;
    while let Some(idx) = hay[start..].find(needle) {
        let at = start + idx;
        let before_ok = !hay[..at].chars().next_back().is_some_and(is_js_word_char);
        let after = at + needle.len();
        let after_ok = !hay[after..].chars().next().is_some_and(is_js_word_char);
        if before_ok && after_ok {
            return true;
        }
        start = at + needle.len();
    }
    false
}

fn has_lower_prefix(hay: &str, needle: &str) -> bool {
    let mut start = 0;
    while let Some(idx) = hay[start..].find(needle) {
        let at = start + idx;
        if !hay[..at].chars().next_back().is_some_and(is_js_word_char) {
            return true;
        }
        start = at + needle.len();
    }
    false
}

fn has_case_word(text: &str, word: &str) -> bool {
    let mut start = 0;
    while let Some(idx) = text[start..].find(word) {
        let at = start + idx;
        let before_ok = !text[..at].chars().next_back().is_some_and(is_js_word_char);
        let after = at + word.len();
        let after_ok = !text[after..].chars().next().is_some_and(is_js_word_char);
        if before_ok && after_ok {
            return true;
        }
        start = at + word.len();
    }
    false
}

pub fn is_quarantined(key: &str, value: Option<&str>) -> bool {
    let text = value.map_or(key.to_string(), |v| format!("{key} {v}"));
    let key_lower = key.to_lowercase();
    let text_lower = text.to_lowercase();
    for w in QUARANTINE_WORDS {
        if has_lower_word(&key_lower, w) || has_lower_word(&text_lower, w) {
            return true;
        }
    }
    for p in QUARANTINE_PREFIXES {
        if has_lower_prefix(&key_lower, p) || has_lower_prefix(&text_lower, p) {
            return true;
        }
    }
    has_case_word(key, "IQ") || has_case_word(&text, "IQ") || has_case_word(&text, "EQ")
}

pub async fn delete_entry(
    State(state): State<AppState>,
    Path((customer_id, entry_id)): Path<(String, String)>,
) -> Response {
    let (Ok(customer_id), Ok(entry_id)) = (customer_id.parse::<i64>(), entry_id.parse::<i64>())
    else {
        return memory_ids_422();
    };
    if customer_id <= 0 || entry_id <= 0 {
        return memory_ids_422();
    }
    let conn = state.conn.lock().unwrap_or_else(|p| p.into_inner());
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT memory_key, memory_value FROM customer_memory
             WHERE id = ?1 AND customer_id = ?2",
            rusqlite::params![entry_id, customer_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let Some((key, value)) = row else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "statusCode": 404,
                "error": "NotFound",
                "message": "Memory entry not found for this customer."
            })),
        )
            .into_response();
    };
    let source: String = conn
        .query_row(
            "SELECT COALESCE(source, 'ai') FROM customer_memory WHERE id = ?1",
            rusqlite::params![entry_id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "ai".into());
    if source != "human" && !is_quarantined(&key, value.as_deref()) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "statusCode": 403,
                "error": "Forbidden",
                "message": "AI-extracted memories are immutable by design (they re-derive from conversations). Only human-written entries - and quarantined entries - can be deleted."
            })),
        )
            .into_response();
    }
    let _ = conn.execute(
        "DELETE FROM customer_memory WHERE id = ?1 AND customer_id = ?2",
        rusqlite::params![entry_id, customer_id],
    );
    (StatusCode::OK, Json(json!({"ok": true}))).into_response()
}

fn memory_ids_422() -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "statusCode": 422,
            "error": "ValidationError",
            "message": "Customer id and entry id must be positive integers."
        })),
    )
        .into_response()
}

fn memory_id_422_customer() -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "statusCode": 422,
            "error": "ValidationError",
            "message": "Customer id must be a positive integer."
        })),
    )
        .into_response()
}

const FRESHNESS_FRESH_DAYS: i64 = 30;
const FRESHNESS_AGING_DAYS: i64 = 90;

fn parse_profile_ts(ts: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let normalized = ts.trim().replace(' ', "T");
    let trimmed = normalized.strip_suffix('Z').unwrap_or(&normalized);
    chrono::DateTime::parse_from_rfc3339(&format!("{trimmed}Z"))
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

fn freshness_class(seen: Option<&str>, first: Option<&str>, at: Option<&str>) -> &'static str {
    fn pick(s: Option<&str>) -> Option<&str> {
        s.filter(|v| !v.is_empty())
    }
    let ts = pick(seen)
        .or_else(|| pick(first))
        .or_else(|| pick(at))
        .and_then(parse_profile_ts);
    let Some(dt) = ts else {
        return "unknown";
    };
    let age_days = (chrono::Utc::now() - dt).num_days();
    if age_days <= FRESHNESS_FRESH_DAYS {
        "fresh"
    } else if age_days <= FRESHNESS_AGING_DAYS {
        "aging"
    } else {
        "stale"
    }
}

struct ComposedProfile {
    sections: Vec<Value>,
    quarantined: Vec<Value>,
    notes: Vec<String>,
    freshness: Value,
}

struct ProfileEntry {
    id: i64,
    key: String,
    value: Option<String>,
    evidence: Option<String>,
    source_conversation_id: Option<i64>,
    source: String,
    origin: Option<String>,
    confidence: Option<String>,
    provenance: Option<String>,
    kind: String,
    first_seen_at: Option<String>,
    last_seen_at: Option<String>,
    created_at: String,
}

const PROFILE_ENTRY_SQL: &str = "SELECT id, memory_key, memory_value, evidence_excerpt,
     source_conversation_id, source, origin, confidence, provenance, kind,
     first_seen_at, last_seen_at, created_at
     FROM customer_memory WHERE customer_id = ?1 ORDER BY created_at DESC, id DESC";

fn load_profile_entries(conn: &rusqlite::Connection, customer_id: i64) -> Vec<ProfileEntry> {
    conn.prepare(PROFILE_ENTRY_SQL)
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![customer_id], |r| {
                Ok(ProfileEntry {
                    id: r.get(0)?,
                    key: r.get(1)?,
                    value: r.get(2)?,
                    evidence: r.get(3)?,
                    source_conversation_id: r.get(4)?,
                    source: r.get(5)?,
                    origin: r.get(6)?,
                    confidence: r.get(7)?,
                    provenance: r.get(8)?,
                    kind: r.get(9)?,
                    first_seen_at: r.get(10)?,
                    last_seen_at: r.get(11)?,
                    created_at: r.get(12)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .ok()
        })
        .unwrap_or_default()
}

fn conversation_numbers(
    conn: &rusqlite::Connection,
    ids: &[i64],
) -> std::collections::HashMap<i64, i64> {
    let mut map = std::collections::HashMap::new();
    for id in ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<i64>>()
    {
        if let Ok(number) = conn.query_row(
            "SELECT number FROM conversations WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        ) {
            map.insert(id, number);
        }
    }
    map
}

struct SectionBuilder {
    section: &'static str,
    label: &'static str,
    entries: Vec<Value>,
}

impl SectionBuilder {
    fn new(section: &'static str, label: &'static str) -> Self {
        Self {
            section,
            label,
            entries: Vec::new(),
        }
    }

    fn push(&mut self, entry: Value) {
        self.entries.push(entry);
    }

    fn build(self) -> Value {
        json!({
            "section": self.section,
            "label": self.label,
            "entries": self.entries,
        })
    }
}

fn compose_memory_profile(conn: &rusqlite::Connection, customer_id: i64) -> ComposedProfile {
    let all = load_profile_entries(conn, customer_id);
    let conv_numbers = conversation_numbers(
        conn,
        &all.iter()
            .filter_map(|e| e.source_conversation_id)
            .collect::<Vec<_>>(),
    );
    let mut quarantined = Vec::new();
    let mut usable: Vec<&ProfileEntry> = Vec::new();
    for e in &all {
        if is_quarantined(&e.key, e.value.as_deref()) {
            quarantined.push(json!({
                "entry_id": e.id,
                "key": e.key,
                "reason": "psychological/personality judgment (policy)",
            }));
        } else {
            usable.push(e);
        }
    }
    let memory_entry_json = |e: &ProfileEntry, editable: bool| -> Value {
        json!({
            "entry_id": e.id,
            "title": e.key,
            "value": e.value,
            "source": if e.source == "human" { "human_local" } else { "ai_derived" },
            "confidence": e.confidence.as_deref().unwrap_or(if e.source == "human" { "human" } else { "unknown" }),
            "freshness": freshness_class(e.last_seen_at.as_deref(), e.first_seen_at.as_deref(), Some(e.created_at.as_str())),
            "evidence": e.evidence.as_deref().map(|ev| {
                let number = e.source_conversation_id.and_then(|c| conv_numbers.get(&c).copied());
                json!([{ "description": ev, "conversation_number": number }])
            }).unwrap_or_else(|| json!([])),
            "editable": editable,
            "first_seen_at": e.first_seen_at,
            "last_seen_at": e.last_seen_at,
        })
    };

    let mut account = SectionBuilder::new("account", "Account facts");
    let identity: Option<(
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = conn
        .query_row(
            "SELECT o.name, c.job_title, c.email, c.phone
             FROM customers c LEFT JOIN organizations o ON o.id = c.organization_id
             WHERE c.id = ?1",
            rusqlite::params![customer_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .ok();
    if let Some((org, job_title, email, phone)) = identity {
        for (title, value) in [
            ("Organization", org),
            ("Job title", job_title),
            ("Email", email),
            ("Phone", phone),
        ] {
            if let Some(v) = value.filter(|s| !s.is_empty()) {
                account.push(json!({
                    "entry_id": null,
                    "title": title,
                    "value": v,
                    "source": "helpscout_mirror",
                    "confidence": "mirror",
                    "freshness": "unknown",
                    "evidence": [],
                    "editable": false,
                    "first_seen_at": null,
                    "last_seen_at": null,
                }));
            }
        }
    }
    for e in usable.iter().filter(|e| e.kind == "account") {
        account.push(memory_entry_json(e, e.source == "human"));
    }

    let mut issue_history = SectionBuilder::new("issue_history", "Issue history");
    let issue_rows: Vec<(i64, String, String, i64, Option<String>)> = conn
        .prepare(
            "SELECT ki.id, COALESCE(ki.title, ki.name) AS label, ki.status,
                    COUNT(kil.conversation_id) AS linked, MAX(ki.updated_at)
             FROM known_issues ki
             JOIN known_issue_links kil ON kil.known_issue_id = ki.id
             JOIN conversations cv ON cv.id = kil.conversation_id
             WHERE cv.customer_local_id = ?1
             GROUP BY ki.id
             ORDER BY ki.updated_at DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![customer_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .ok()
        })
        .unwrap_or_default();
    for (issue_id, label, status, linked, updated_at) in issue_rows {
        issue_history.push(json!({
            "entry_id": null,
            "title": format!("{label} ({status})"),
            "value": format!("Affects {linked} of this customer's conversations."),
            "source": "helpscout_mirror",
            "confidence": "mirror",
            "freshness": freshness_class(None, None, updated_at.as_deref()),
            "evidence": [{ "description": format!("known issue #{issue_id}"), "conversation_number": null }],
            "editable": false,
            "first_seen_at": null,
            "last_seen_at": updated_at,
        }));
    }
    let cluster_rows: Vec<(i64, String, i64, Option<String>)> = conn
        .prepare(
            "SELECT ic.id, ic.name, COUNT(icm.conversation_id) AS linked, MAX(ic.last_seen_at)
             FROM issue_clusters ic
             JOIN issue_cluster_conversations icm ON icm.cluster_id = ic.id
             JOIN conversations cv ON cv.id = icm.conversation_id
             WHERE cv.customer_local_id = ?1
             GROUP BY ic.id
             ORDER BY ic.last_seen_at DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![customer_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .ok()
        })
        .unwrap_or_default();
    for (cluster_id, name, linked, last_seen) in cluster_rows {
        issue_history.push(json!({
            "entry_id": null,
            "title": format!("Cluster: {name}"),
            "value": format!("{linked} of this customer's conversations sit in the cluster."),
            "source": "helpscout_mirror",
            "confidence": "mirror",
            "freshness": freshness_class(last_seen.as_deref(), None, None),
            "evidence": [{ "description": format!("issue cluster #{cluster_id}"), "conversation_number": null }],
            "editable": false,
            "first_seen_at": null,
            "last_seen_at": last_seen,
        }));
    }

    let mut outcomes = SectionBuilder::new("outcomes", "Outcomes");
    let outcome_rows: Vec<(
        i64,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = conn
        .prepare(
            "SELECT cv.id, cv.number, cv.subject, cv.status, r.rating, r.comments
             FROM conversations cv
             LEFT JOIN ratings r ON r.conversation_id = cv.id
             WHERE cv.customer_local_id = ?1 AND cv.deleted_at IS NULL AND cv.status = 'closed'
             ORDER BY COALESCE(cv.updated_at, cv.created_at) DESC LIMIT 25",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![customer_id], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .ok()
        })
        .unwrap_or_default();
    let mut rated_count = 0i64;
    for (_conv_id, number, subject, _status, rating, comments) in outcome_rows {
        let value = match (&rating, &comments) {
            (Some(r), Some(c)) => format!("Closed — rated {r} ({c})."),
            (Some(r), None) => format!("Closed — rated {r}."),
            (None, _) => "Closed.".to_string(),
        };
        if rating.is_some() {
            rated_count += 1;
        }
        outcomes.push(json!({
            "entry_id": null,
            "title": format!("#{number} {}", subject.unwrap_or_else(|| "(no subject)".into())),
            "value": value,
            "source": "helpscout_mirror",
            "confidence": "mirror",
            "freshness": "unknown",
            "evidence": [{ "description": "closed conversation", "conversation_number": number }],
            "editable": false,
            "first_seen_at": null,
            "last_seen_at": null,
        }));
    }

    let mut interaction = SectionBuilder::new("interaction", "Interaction patterns");
    let baseline_rows: Vec<(String, String, String, Option<String>)> = conn
        .prepare(
            "SELECT dimension, typical_value, confidence, last_observed
             FROM client_behavior_baselines WHERE customer_id = ?1
             ORDER BY dimension",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![customer_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .ok()
        })
        .unwrap_or_default();
    for (dimension, typical, confidence, last_observed) in baseline_rows {
        interaction.push(json!({
            "entry_id": null,
            "title": dimension.replace('_', " "),
            "value": typical,
            "source": "ai_derived",
            "confidence": confidence,
            "freshness": freshness_class(last_observed.as_deref(), None, None),
            "evidence": [],
            "editable": false,
            "first_seen_at": null,
            "last_seen_at": last_observed,
        }));
    }

    let mut preferences = SectionBuilder::new("preferences", "Preferences");
    for e in usable.iter().filter(|e| e.kind == "preference") {
        preferences.push(memory_entry_json(e, e.source == "human"));
    }

    let mut campaigns = SectionBuilder::new("campaigns", "Campaigns");
    let campaign_rows: Vec<(String, String, Option<String>, Option<String>)> = conn
        .prepare(
            "SELECT oc.name, r.state, r.sent_at, r.replied_at
             FROM outreach_recipients r
             JOIN outreach_campaigns oc ON oc.id = r.campaign_id
             WHERE r.customer_local_id = ?1
             ORDER BY COALESCE(r.sent_at, r.id) DESC LIMIT 25",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![customer_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .ok()
        })
        .unwrap_or_default();
    for (name, state, sent_at, replied_at) in campaign_rows {
        let value = if replied_at.is_some() {
            "Sent — customer replied.".to_string()
        } else {
            format!("Recipient state: {state}.")
        };
        campaigns.push(json!({
            "entry_id": null,
            "title": name,
            "value": value,
            "source": "helpscout_mirror",
            "confidence": "mirror",
            "freshness": freshness_class(sent_at.as_deref(), None, None),
            "evidence": [],
            "editable": false,
            "first_seen_at": null,
            "last_seen_at": sent_at,
        }));
    }

    let mut ai_entries = SectionBuilder::new("ai_entries", "AI-extracted entries");
    for e in usable.iter().filter(|e| {
        e.source != "human" && !matches!(e.kind.as_str(), "account" | "preference" | "context")
    }) {
        ai_entries.push(memory_entry_json(e, false));
    }

    let mut human_entries = SectionBuilder::new("human_entries", "Human entries");
    for e in usable.iter().filter(|e| e.source == "human") {
        human_entries.push(memory_entry_json(e, true));
    }

    let mut context = SectionBuilder::new("context", "Context");
    for e in usable.iter().filter(|e| e.kind == "context") {
        context.push(memory_entry_json(e, e.source == "human"));
    }

    let sections = vec![
        account.build(),
        issue_history.build(),
        outcomes.build(),
        interaction.build(),
        preferences.build(),
        campaigns.build(),
        ai_entries.build(),
        human_entries.build(),
        context.build(),
    ];

    let conversation_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations WHERE customer_local_id = ?1 AND deleted_at IS NULL",
            rusqlite::params![customer_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let mut notes = vec![
        format!("Composed from {conversation_count} conversations."),
        "Sections compose at read time; only human entries are stored rows.".to_string(),
    ];
    if !quarantined.is_empty() {
        notes.push(format!(
            "{} quarantined entr{} never used as memory.",
            quarantined.len(),
            if quarantined.len() == 1 {
                "y is"
            } else {
                "ies are"
            }
        ));
    }
    let mut fresh_counts = std::collections::BTreeMap::<&str, i64>::new();
    for section in &sections {
        if let Some(list) = section["entries"].as_array() {
            for e in list {
                let state = e["freshness"].as_str().unwrap_or("unknown");
                *fresh_counts.entry(state).or_insert(0) += 1;
            }
        }
    }
    let total_section_entries: i64 = fresh_counts.values().sum();
    let freshness = json!({
        "thresholds_days": { "fresh": FRESHNESS_FRESH_DAYS, "aging": FRESHNESS_AGING_DAYS },
        "entry_states": {
            "fresh": fresh_counts.get("fresh").copied().unwrap_or(0),
            "aging": fresh_counts.get("aging").copied().unwrap_or(0),
            "stale": fresh_counts.get("stale").copied().unwrap_or(0),
            "unknown": fresh_counts.get("unknown").copied().unwrap_or(0),
        },
        "rated_conversations": rated_count,
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "total_section_entries": total_section_entries,
    });

    ComposedProfile {
        sections,
        quarantined,
        notes,
        freshness,
    }
}
