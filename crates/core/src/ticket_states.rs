use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketPriority {
    Urgent,
    High,
    Normal,
    Low,
}

impl TicketPriority {
    pub const ALL: [Self; 4] = [Self::Urgent, Self::High, Self::Normal, Self::Low];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Urgent => "urgent",
            Self::High => "high",
            Self::Normal => "normal",
            Self::Low => "low",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "urgent" => Some(Self::Urgent),
            "high" => Some(Self::High),
            "normal" => Some(Self::Normal),
            "low" => Some(Self::Low),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub id: Option<i64>,
    pub conversation_remote_id: i64,
    pub from_state: Option<String>,
    pub to_state: String,
    pub from_priority: Option<String>,
    pub to_priority: Option<String>,
    pub actor_type: String,
    pub actor_id: Option<i64>,
    pub transitioned_at: String,
}

pub const M004_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS ticket_state_transitions (
        id                     INTEGER PRIMARY KEY AUTOINCREMENT,
        conversation_remote_id INTEGER NOT NULL,
        from_state             TEXT,
        to_state               TEXT NOT NULL,
        from_priority          TEXT,
        to_priority            TEXT,
        actor_type             TEXT NOT NULL,
        actor_id               INTEGER,
        transitioned_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_transitions_conv
        ON ticket_state_transitions (conversation_remote_id, transitioned_at);

    -- Add the local priority column to conversations.
    -- This is NOT the Help Scout status — it's a SupportOS++-local addition.
    ALTER TABLE conversations ADD COLUMN supportos_priority TEXT;
    ALTER TABLE conversations ADD COLUMN supportos_state TEXT;
"#;

pub fn apply_m004(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ticket_state_transitions (
            id                     INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_remote_id INTEGER NOT NULL,
            from_state             TEXT,
            to_state               TEXT,
            from_priority          TEXT,
            to_priority            TEXT,
            actor_type             TEXT NOT NULL,
            actor_id               INTEGER,
            transitioned_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_transitions_conv
            ON ticket_state_transitions (conversation_remote_id, transitioned_at);",
    )?;

    for col in ["supportos_priority TEXT", "supportos_state TEXT"] {
        let sql = format!("ALTER TABLE conversations ADD COLUMN {col}");
        if let Err(e) = conn.execute(&sql, [])
            && !e.to_string().contains("duplicate column name")
        {
            return Err(crate::error::Error::Sqlite(e));
        }
    }

    Ok(())
}

pub fn set_priority(
    conn: &mut Connection,
    conversation_remote_id: i64,
    new_priority: TicketPriority,
    actor_type: &str,
    actor_id: Option<i64>,
) -> Result<()> {
    let tx = conn.transaction()?;

    let old_priority: Option<String> = tx
        .query_row(
            "SELECT supportos_priority FROM conversations WHERE remote_id = ?1",
            params![conversation_remote_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    tx.execute(
        "UPDATE conversations SET supportos_priority = ?1 WHERE remote_id = ?2",
        params![new_priority.as_str(), conversation_remote_id],
    )?;

    tx.execute(
        "INSERT INTO ticket_state_transitions
            (conversation_remote_id, from_priority, to_priority, actor_type, actor_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            conversation_remote_id,
            old_priority,
            new_priority.as_str(),
            actor_type,
            actor_id,
        ],
    )?;

    tx.commit()?;
    Ok(())
}

pub fn set_ticket_state(
    conn: &mut Connection,
    conversation_remote_id: i64,
    new_state: &str,
    actor_type: &str,
    actor_id: Option<i64>,
) -> Result<()> {
    let tx = conn.transaction()?;

    let old_state: Option<String> = tx
        .query_row(
            "SELECT supportos_state FROM conversations WHERE remote_id = ?1",
            params![conversation_remote_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    tx.execute(
        "UPDATE conversations SET supportos_state = ?1 WHERE remote_id = ?2",
        params![new_state, conversation_remote_id],
    )?;

    tx.execute(
        "INSERT INTO ticket_state_transitions
            (conversation_remote_id, from_state, to_state, actor_type, actor_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            conversation_remote_id,
            old_state,
            new_state,
            actor_type,
            actor_id
        ],
    )?;

    tx.commit()?;
    Ok(())
}

pub fn get_priority_history(
    conn: &Connection,
    conversation_remote_id: i64,
) -> Result<Vec<StateTransition>> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_remote_id, from_state, to_state, from_priority, to_priority,
                actor_type, actor_id, transitioned_at
         FROM ticket_state_transitions
         WHERE conversation_remote_id = ?1 AND (from_priority IS NOT NULL OR to_priority IS NOT NULL)
         ORDER BY transitioned_at ASC",
    )?;
    let rows = stmt
        .query_map(params![conversation_remote_id], |r| {
            Ok(StateTransition {
                id: Some(r.get(0)?),
                conversation_remote_id: r.get(1)?,
                from_state: r.get::<_, Option<String>>(2)?,
                to_state: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                from_priority: r.get::<_, Option<String>>(4)?,
                to_priority: r.get::<_, Option<String>>(5)?,
                actor_type: r.get(6)?,
                actor_id: r.get(7)?,
                transitioned_at: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn get_state_history(
    conn: &Connection,
    conversation_remote_id: i64,
) -> Result<Vec<StateTransition>> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_remote_id, from_state, to_state, from_priority, to_priority,
                actor_type, actor_id, transitioned_at
         FROM ticket_state_transitions
         WHERE conversation_remote_id = ?1 AND (from_state IS NOT NULL OR to_state IS NOT NULL)
         ORDER BY transitioned_at ASC",
    )?;
    let rows = stmt
        .query_map(params![conversation_remote_id], |r| {
            Ok(StateTransition {
                id: Some(r.get(0)?),
                conversation_remote_id: r.get(1)?,
                from_state: r.get::<_, Option<String>>(2)?,
                to_state: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                from_priority: r.get::<_, Option<String>>(4)?,
                to_priority: r.get::<_, Option<String>>(5)?,
                actor_type: r.get(6)?,
                actor_id: r.get(7)?,
                transitioned_at: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn compute_state_durations(
    conn: &Connection,
    conversation_remote_id: i64,
) -> Result<Vec<(String, f64)>> {
    let transitions = get_state_history(conn, conversation_remote_id)?;
    if transitions.is_empty() {
        return Ok(Vec::new());
    }

    let mut durations: Vec<(String, f64)> = Vec::new();
    for i in 0..transitions.len() {
        let from = transitions[i].to_state.as_str();
        let to_time = if i + 1 < transitions.len() {
            transitions[i + 1].transitioned_at.as_str()
        } else {
            "now"
        };
        let from_time = transitions[i].transitioned_at.as_str();

        let duration_secs: f64 = conn
            .query_row(
                "SELECT (julianday(?1) - julianday(?2)) * 86400.0",
                params![to_time, from_time],
                |r| r.get(0),
            )
            .unwrap_or(0.0);

        durations.push((from.to_string(), duration_secs));
    }
    Ok(durations)
}

pub fn apply_m032(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ticket_states (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            key         TEXT UNIQUE NOT NULL,
            name        TEXT NOT NULL,
            color       TEXT,
            sort_order  INTEGER NOT NULL DEFAULT 0,
            is_resolved INTEGER NOT NULL DEFAULT 0,
            built_in    INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );
        INSERT OR IGNORE INTO ticket_states (key, name, color, sort_order, is_resolved, built_in) VALUES
            ('new', 'New', '#3b82f6', 10, 0, 1),
            ('investigating', 'Investigating', '#f59e0b', 20, 0, 1),
            ('waiting-customer', 'Waiting on Customer', '#8b5cf6', 30, 0, 1),
            ('waiting-engineering', 'Waiting on Engineering', '#ef4444', 40, 0, 1),
            ('ready-verify', 'Ready to Verify', '#06b6d4', 50, 0, 1),
            ('resolved', 'Resolved', '#22c55e', 60, 1, 1);
        CREATE TABLE IF NOT EXISTS state_transitions (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id  INTEGER NOT NULL,
            previous_state_id INTEGER,
            new_state_id     INTEGER,
            actor_type       TEXT NOT NULL DEFAULT 'user',
            actor_local_id   INTEGER,
            reason           TEXT,
            occurred_at      TEXT NOT NULL,
            source           TEXT NOT NULL DEFAULT 'local',
            created_at       TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_state_transitions_conv
            ON state_transitions(conversation_id, occurred_at);",
    )?;
    let cols: Vec<String> = conn
        .prepare("PRAGMA table_info(conversations)")
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |r| r.get::<_, String>(1))
                .map(|rows| rows.filter_map(|x| x.ok()).collect())
                .ok()
        })
        .unwrap_or_default();
    if !cols.iter().any(|c| c == "supportos_state_id") {
        let _ = conn.execute(
            "ALTER TABLE conversations ADD COLUMN supportos_state_id INTEGER",
            [],
        );
    }
    let _ = conn.execute("UPDATE app_state SET schema_version = 32 WHERE id = 1", []);
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct TicketStateDef {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i64,
    pub is_resolved: i64,
    pub built_in: i64,
    pub created_at: String,
    pub updated_at: String,
}

fn row_to_def(r: &rusqlite::Row<'_>) -> rusqlite::Result<TicketStateDef> {
    Ok(TicketStateDef {
        id: r.get(0)?,
        key: r.get(1)?,
        name: r.get(2)?,
        color: r.get(3)?,
        sort_order: r.get(4)?,
        is_resolved: r.get(5)?,
        built_in: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
    })
}

const STATE_COLS: &str =
    "id, key, name, color, sort_order, is_resolved, built_in, created_at, updated_at";

pub fn list_states(conn: &Connection) -> Vec<TicketStateDef> {
    let Ok(mut stmt) = conn.prepare(&format!(
        "SELECT {STATE_COLS} FROM ticket_states ORDER BY sort_order, name"
    )) else {
        return Vec::new();
    };
    stmt.query_map([], row_to_def)
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

fn get_state(conn: &Connection, id: i64) -> Option<TicketStateDef> {
    conn.query_row(
        &format!("SELECT {STATE_COLS} FROM ticket_states WHERE id = ?1"),
        [id],
        row_to_def,
    )
    .ok()
}

fn get_state_by_key(conn: &Connection, key: &str) -> Option<TicketStateDef> {
    conn.query_row(
        &format!("SELECT {STATE_COLS} FROM ticket_states WHERE key = ?1"),
        [key],
        row_to_def,
    )
    .ok()
}

fn slugify_key(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut out = String::new();
    let mut dash = false;
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

pub fn create_state(
    conn: &Connection,
    name: &str,
    key: Option<&str>,
    color: Option<&str>,
    sort_order: Option<i64>,
    is_resolved: Option<bool>,
) -> std::result::Result<TicketStateDef, String> {
    let key = key.map(String::from).unwrap_or_else(|| slugify_key(name));
    let key = if key.is_empty() {
        format!("state-{}", chrono::Utc::now().timestamp_millis())
    } else {
        key
    };
    if get_state_by_key(conn, &key).is_some() {
        return Err(format!("A state with key '{key}' already exists."));
    }
    conn.execute(
        "INSERT INTO ticket_states (key, name, color, sort_order, is_resolved)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            key,
            name,
            color,
            sort_order.unwrap_or(500),
            i64::from(is_resolved.unwrap_or(false))
        ],
    )
    .map_err(|e| e.to_string())?;
    get_state(conn, conn.last_insert_rowid()).ok_or_else(|| "insert failed".into())
}

pub fn update_state(
    conn: &Connection,
    id: i64,
    patch: &serde_json::Value,
) -> std::result::Result<Option<TicketStateDef>, String> {
    let Some(state) = get_state(conn, id) else {
        return Ok(None);
    };
    if state.built_in == 1
        && let Some(want) = patch.get("is_resolved").and_then(|v| v.as_bool())
        && want != (state.is_resolved == 1)
    {
        return Err("The resolved semantics of built-in states cannot be changed.".into());
    }
    let name = patch
        .get("name")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or(state.name);
    let color = match patch.get("color") {
        Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        _ => state.color,
    };
    let sort_order = patch
        .get("sort_order")
        .and_then(|v| v.as_i64())
        .unwrap_or(state.sort_order);
    let is_resolved = match patch.get("is_resolved").and_then(|v| v.as_bool()) {
        Some(b) => i64::from(b),
        None => state.is_resolved,
    };
    conn.execute(
        "UPDATE ticket_states SET name = ?1, color = ?2, sort_order = ?3,
             is_resolved = ?4, updated_at = datetime('now') WHERE id = ?5",
        rusqlite::params![name, color, sort_order, is_resolved, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(get_state(conn, id))
}

pub fn delete_state(conn: &Connection, id: i64) -> (bool, String) {
    let Some(state) = get_state(conn, id) else {
        return (false, "State not found.".into());
    };
    if state.built_in == 1 {
        return (
            false,
            format!(
                "Built-in state '{}' cannot be deleted (it is part of the default workflow).",
                state.name
            ),
        );
    }
    let in_use: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations WHERE supportos_state_id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let _ = conn.execute(
        "UPDATE conversations SET supportos_state_id = NULL WHERE supportos_state_id = ?1",
        [id],
    );
    let _ = conn.execute(
        "UPDATE state_transitions SET previous_state_id = NULL WHERE previous_state_id = ?1",
        [id],
    );
    let _ = conn.execute(
        "DELETE FROM state_transitions WHERE new_state_id = ?1",
        [id],
    );
    let _ = conn.execute("DELETE FROM ticket_states WHERE id = ?1", [id]);
    if in_use > 0 {
        (
            true,
            format!("State deleted; {in_use} conversation(s) reset to no state."),
        )
    } else {
        (true, "State deleted.".into())
    }
}

fn parse_ts(raw: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(dt);
    }
    chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|naive| naive.and_utc().fixed_offset())
}

pub fn get_conversation_state(
    conn: &Connection,
    conversation_local_id: i64,
) -> Option<TicketStateDef> {
    conn.query_row(
        "SELECT ts.id, ts.key, ts.name, ts.color, ts.sort_order, ts.is_resolved,
                ts.built_in, ts.created_at, ts.updated_at
         FROM conversations c JOIN ticket_states ts ON ts.id = c.supportos_state_id
         WHERE c.id = ?1",
        [conversation_local_id],
        row_to_def,
    )
    .ok()
}

pub fn list_transitions(
    conn: &Connection,
    conversation_local_id: i64,
    limit: i64,
) -> Result<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.conversation_id, t.previous_state_id, t.new_state_id,
                t.actor_type, t.actor_local_id, t.reason, t.occurred_at, t.source,
                p.name AS previous_state_name,
                COALESCE(n.name, '(no state)') AS new_state_name,
                CASE WHEN t.actor_type = 'user' THEN
                    (SELECT TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, ''))
                       FROM users u WHERE u.id = t.actor_local_id)
                END AS actor_name
         FROM state_transitions t
         LEFT JOIN ticket_states p ON p.id = t.previous_state_id
         LEFT JOIN ticket_states n ON n.id = t.new_state_id
         WHERE t.conversation_id = ?1
         ORDER BY t.occurred_at DESC, t.id DESC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![conversation_local_id, limit], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, i64>(0)?,
                "conversation_id": r.get::<_, i64>(1)?,
                "previous_state_id": r.get::<_, Option<i64>>(2)?,
                "new_state_id": r.get::<_, Option<i64>>(3)?,
                "previous_state_name": r.get::<_, Option<String>>(9)?,
                "new_state_name": r.get::<_, String>(10)?,
                "actor_type": r.get::<_, String>(4)?,
                "actor_local_id": r.get::<_, Option<i64>>(5)?,
                "actor_name": r.get::<_, Option<String>>(11)?,
                "reason": r.get::<_, Option<String>>(6)?,
                "occurred_at": r.get::<_, String>(7)?,
                "source": r.get::<_, String>(8)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn state_lifecycle(conn: &Connection, conversation_local_id: i64) -> Result<serde_json::Value> {
    let mut transitions = list_transitions(conn, conversation_local_id, 1000)?;
    transitions.reverse();
    let current = get_conversation_state(conn, conversation_local_id);
    let now: chrono::DateTime<chrono::FixedOffset> = chrono::Utc::now().into();
    struct PerState {
        state_id: i64,
        state_name: String,
        entries: i64,
        total_minutes: Vec<f64>,
        last_entered: Option<String>,
    }
    let mut per_state: Vec<PerState> = Vec::new();
    for (i, t) in transitions.iter().enumerate() {
        let new_state_id = t["new_state_id"].as_i64().unwrap_or(0);
        let idx = per_state
            .iter()
            .position(|p| p.state_id == new_state_id)
            .unwrap_or_else(|| {
                per_state.push(PerState {
                    state_id: new_state_id,
                    state_name: t["new_state_name"].as_str().unwrap_or("").to_string(),
                    entries: 0,
                    total_minutes: Vec::new(),
                    last_entered: None,
                });
                per_state.len() - 1
            });
        let entry = &mut per_state[idx];
        entry.entries += 1;
        entry.last_entered = t["occurred_at"].as_str().map(String::from);
        let next_at = transitions
            .get(i + 1)
            .and_then(|n| n["occurred_at"].as_str());
        let is_current = current
            .as_ref()
            .map(|c| c.id == new_state_id)
            .unwrap_or(false);
        let end = next_at.or(if is_current { Some("") } else { None });
        if let (Some(occurred), Some(end_raw)) = (t["occurred_at"].as_str(), end) {
            let end_ts = if end_raw.is_empty() {
                Some(now)
            } else {
                parse_ts(end_raw)
            };
            if let (Some(start), Some(e)) = (parse_ts(occurred), end_ts) {
                let mins = (e - start).num_milliseconds() as f64 / 60_000.0;
                if mins.is_finite() && mins >= 0.0 {
                    entry.total_minutes.push(mins);
                }
            }
        }
    }
    let mut time_in_current: Option<i64> = None;
    if let (Some(current), Some(last)) = (current.as_ref(), transitions.last())
        && last["new_state_id"].as_i64() == Some(current.id)
        && let Some(occurred) = last["occurred_at"].as_str()
        && let Some(start) = parse_ts(occurred)
    {
        let mins = (now - start).num_milliseconds() as f64 / 60_000.0;
        if mins.is_finite() && mins >= 0.0 {
            time_in_current = Some(mins.round() as i64);
        }
    }
    let current_json = current
        .as_ref()
        .and_then(|c| serde_json::to_value(c).ok())
        .unwrap_or(serde_json::Value::Null);
    let per_state_json: Vec<serde_json::Value> = per_state
        .into_iter()
        .map(|e| {
            let total = if e.total_minutes.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!(e.total_minutes.iter().sum::<f64>().round() as i64)
            };
            let avg = if e.total_minutes.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!(
                    (e.total_minutes.iter().sum::<f64>() / e.total_minutes.len() as f64).round()
                        as i64
                )
            };
            serde_json::json!({
                "state_id": e.state_id,
                "state_name": e.state_name,
                "entries": e.entries,
                "total_minutes": total,
                "avg_minutes": avg,
                "last_entered": e.last_entered,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "current_state": current_json,
        "time_in_current_state_min": time_in_current,
        "transitions": transitions.len(),
        "per_state": per_state_json,
    }))
}

pub fn state_bottlenecks(conn: &Connection) -> Vec<serde_json::Value> {
    let spans: Vec<(i64, i64, String, Option<String>)> = conn
        .prepare(
            "SELECT t.conversation_id, t.new_state_id, t.occurred_at,
                    CASE
                      WHEN EXISTS (SELECT 1 FROM state_transitions t2
                                    WHERE t2.conversation_id = t.conversation_id
                                      AND (t2.occurred_at > t.occurred_at
                                           OR (t2.occurred_at = t.occurred_at AND t2.id > t.id)))
                        THEN (SELECT MIN(t2.occurred_at) FROM state_transitions t2
                               WHERE t2.conversation_id = t.conversation_id
                                 AND (t2.occurred_at > t.occurred_at
                                      OR (t2.occurred_at = t.occurred_at AND t2.id > t.id)))
                      WHEN (SELECT supportos_state_id FROM conversations WHERE id = t.conversation_id) = t.new_state_id
                        THEN 'now'
                      ELSE NULL
                    END AS span_end
               FROM state_transitions t
              ORDER BY t.conversation_id, t.occurred_at, t.id",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
            .ok()
        })
        .unwrap_or_default();
    let mut by_state: std::collections::HashMap<i64, Vec<f64>> = std::collections::HashMap::new();
    let mut convs: std::collections::HashMap<i64, std::collections::HashSet<i64>> =
        std::collections::HashMap::new();
    let now = chrono::Utc::now().to_rfc3339();
    for (conv, state, started, span_end) in spans {
        let Some(end) = span_end else { continue };
        let end = if end == "now" { now.clone() } else { end };
        let (Some(s), Some(e)) = (parse_ts(&started), parse_ts(&end)) else {
            continue;
        };
        let mins = (e - s).num_milliseconds() as f64 / 60_000.0;
        if !mins.is_finite() || mins < 0.0 {
            continue;
        }
        by_state.entry(state).or_default().push(mins);
        convs.entry(state).or_default().insert(conv);
    }
    let mut out: Vec<serde_json::Value> = by_state
        .into_iter()
        .map(|(state_id, minutes)| {
            let name: Option<String> = conn
                .query_row(
                    "SELECT name FROM ticket_states WHERE id = ?1",
                    [state_id],
                    |r| r.get(0),
                )
                .ok();
            let name = name.unwrap_or_else(|| format!("#{state_id}"));
            let conversations = convs.get(&state_id).map_or(0, |s| s.len());
            let avg = if minutes.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!(minutes.iter().sum::<f64>().round() as i64)
            };
            let max = if minutes.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!(minutes.iter().cloned().fold(f64::MIN, f64::max).round() as i64)
            };
            serde_json::json!({
                "state_id": state_id,
                "state_name": name,
                "conversations": conversations,
                "avg_minutes": avg,
                "max_minutes": max,
            })
        })
        .collect();
    out.sort_by_key(|v| {
        let avg = v["avg_minutes"].as_f64();
        std::cmp::Reverse(OrderedF64(avg.unwrap_or(0.0)))
    });
    out
}

struct OrderedF64(f64);
impl PartialEq for OrderedF64 {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for OrderedF64 {}
impl PartialOrd for OrderedF64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for OrderedF64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}
