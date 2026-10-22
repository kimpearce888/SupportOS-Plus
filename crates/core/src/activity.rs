use rusqlite::{Connection, params};

use crate::catalog::ResponseState;
use crate::error::Result;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActivityEvent {
    pub id: Option<i64>,
    pub conversation_id: i64,
    pub event_type: String,
    pub actor_type: String,
    pub actor_id: Option<i64>,
    pub occurred_at: String,
    pub dedup_key: String,
}

pub const M003_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS activity_events (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        conversation_id INTEGER NOT NULL,
        event_type      TEXT NOT NULL,
        actor_type      TEXT NOT NULL,
        actor_id        INTEGER,
        occurred_at     TEXT NOT NULL,
        dedup_key        TEXT NOT NULL UNIQUE,
        created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );
    CREATE INDEX IF NOT EXISTS idx_activity_events_conv
        ON activity_events (conversation_id, occurred_at);
    CREATE INDEX IF NOT EXISTS idx_activity_events_type
        ON activity_events (event_type, occurred_at);

    -- Derived columns on conversations (updated by the activity engine).
    -- These are the "derived timestamps" the spec requires:
    -- first_customer_message_at, first_response_at, last_customer_reply_at,
    -- last_human_agent_response_at, last_activity_at, closed_at,
    -- customer_waiting_since, response_state.
    -- All added as nullable columns so existing rows don't break.
    ALTER TABLE conversations ADD COLUMN first_customer_message_at TEXT;
    ALTER TABLE conversations ADD COLUMN first_response_at TEXT;
    ALTER TABLE conversations ADD COLUMN last_customer_reply_at TEXT;
    ALTER TABLE conversations ADD COLUMN last_human_agent_response_at TEXT;
    ALTER TABLE conversations ADD COLUMN customer_waiting_since TEXT;
    ALTER TABLE conversations ADD COLUMN response_state TEXT NOT NULL DEFAULT 'needs_first_response';

    UPDATE app_state SET schema_version = 3 WHERE id = 1;
"#;

pub fn apply_m003(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS activity_events (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL,
            event_type      TEXT NOT NULL,
            actor_type      TEXT NOT NULL,
            actor_id        INTEGER,
            occurred_at     TEXT NOT NULL,
            dedup_key        TEXT NOT NULL UNIQUE,
            created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_activity_events_conv
            ON activity_events (conversation_id, occurred_at);
        CREATE INDEX IF NOT EXISTS idx_activity_events_type
            ON activity_events (event_type, occurred_at);",
    )?;

    for col in [
        "first_customer_message_at TEXT",
        "first_response_at TEXT",
        "last_customer_reply_at TEXT",
        "last_human_agent_response_at TEXT",
        "customer_waiting_since TEXT",
        "response_state TEXT NOT NULL DEFAULT 'needs_first_response'",
    ] {
        let sql = format!("ALTER TABLE conversations ADD COLUMN {col}");
        if let Err(e) = conn.execute(&sql, []) {
            let msg = e.to_string();
            if !msg.contains("duplicate column name") {
                return Err(crate::error::Error::Sqlite(e));
            }
        }
    }

    conn.execute("UPDATE app_state SET schema_version = 3 WHERE id = 1", [])?;

    Ok(())
}

pub fn record_event(conn: &Connection, event: &ActivityEvent) -> Result<bool> {
    let rows = conn.execute(
        "INSERT OR IGNORE INTO activity_events
            (conversation_id, event_type, actor_type, actor_id, occurred_at, dedup_key)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            event.conversation_id,
            event.event_type,
            event.actor_type,
            event.actor_id,
            event.occurred_at,
            event.dedup_key,
        ],
    )?;
    Ok(rows > 0)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FullActivityEvent {
    #[serde(flatten)]
    pub base: ActivityEvent,
    pub thread_local_id: Option<i64>,
    pub source: String,
    pub metadata: Option<String>,
}

pub fn record_full_event(conn: &Connection, event: &FullActivityEvent) -> Result<bool> {
    let has_columns = conn
        .prepare("SELECT metadata, source, thread_local_id FROM activity_events LIMIT 0")
        .is_ok();
    if !has_columns {
        return record_event(conn, &event.base);
    }
    let rows = conn.execute(
        "INSERT OR IGNORE INTO activity_events
            (conversation_id, event_type, actor_type, actor_id, occurred_at, dedup_key,
             thread_local_id, source, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            event.base.conversation_id,
            event.base.event_type,
            event.base.actor_type,
            event.base.actor_id,
            event.base.occurred_at,
            event.base.dedup_key,
            event.thread_local_id,
            event.source,
            event.metadata,
        ],
    )?;
    Ok(rows > 0)
}

#[must_use]
pub fn now_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct RebuildSummary {
    pub conversations: i64,
    pub events_written: i64,
    pub events_total: i64,
}

struct MirrorThread {
    id: i64,
    remote_id: Option<i64>,
    thread_type: String,
    state: Option<String>,
    body: Option<String>,
    from_type: Option<String>,
    created_by_user_id: Option<i64>,
    created_by_customer_id: Option<i64>,
    created_at: Option<String>,
}

fn derive_mirror_event(t: &MirrorThread) -> Option<(String, String, Option<i64>)> {
    if t.state.as_deref().is_some_and(|s| s != "published") {
        return None;
    }
    match t.thread_type.as_str() {
        "note" => Some(("internal_note".into(), "user".into(), t.created_by_user_id)),
        "lineitem" => {
            let text = t.body.as_deref().unwrap_or("").to_lowercase();
            let event_type = if text.contains("status") {
                "status_changed"
            } else if text.contains("assign") {
                "assignment_changed"
            } else if text.contains("moved") {
                "moved"
            } else if text.contains("tag") {
                "tag_added"
            } else {
                "lineitem_action"
            };
            Some((event_type.into(), "system_user".into(), None))
        }
        _ => match t.from_type.as_deref().unwrap_or("") {
            "customer" => Some((
                "customer_message".into(),
                "customer".into(),
                t.created_by_customer_id,
            )),
            "user" | "agent" => Some((
                "human_agent_message".into(),
                "user".into(),
                t.created_by_user_id,
            )),
            _ => Some(("customer_message".into(), "unknown".into(), None)),
        },
    }
}

/// Audit item 20: this is the ONLY derived-columns entry point.
/// `activity_events.conversation_id` is keyed on the LOCAL
/// `conversations.id` (see `rebuild_all`); the old remote-id-keyed
/// twin computed the response state from the wrong event set and
/// wiped the derived timestamps on every user action — removed. All
/// callers resolve the local id first (see ticket_ops).
pub fn update_derived_columns_by_local_id(conn: &Connection, conv_local: i64) -> Result<()> {
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM conversations WHERE id = ?1",
            params![conv_local],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    let state = if status.as_deref() == Some("closed") {
        ResponseState::Closed
    } else {
        let last_event: Option<String> = conn
            .query_row(
                "SELECT actor_type FROM activity_events
                 WHERE conversation_id = ?1
                 ORDER BY julianday(occurred_at) DESC, id DESC
                 LIMIT 1",
                params![conv_local],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        match last_event.as_deref() {
            Some("customer") => ResponseState::CustomerWaiting,
            Some("user" | "agent") => ResponseState::AgentWaiting,
            _ => ResponseState::NeedsFirstResponse,
        }
    };

    let first_customer_message_at: Option<String> = conn
        .query_row(
            "SELECT MIN(occurred_at) FROM activity_events
             WHERE conversation_id = ?1 AND actor_type = 'customer'",
            params![conv_local],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let first_response_at: Option<String> = conn
        .query_row(
            "SELECT MIN(occurred_at) FROM activity_events
             WHERE conversation_id = ?1 AND actor_type IN ('agent','user')
               AND event_type IN ('reply','human_agent_message')",
            params![conv_local],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let last_customer_reply_at: Option<String> = conn
        .query_row(
            "SELECT MAX(occurred_at) FROM activity_events
             WHERE conversation_id = ?1 AND actor_type = 'customer'",
            params![conv_local],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let last_human_agent_response_at: Option<String> = conn
        .query_row(
            "SELECT MAX(occurred_at) FROM activity_events
             WHERE conversation_id = ?1 AND actor_type IN ('agent','user')",
            params![conv_local],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let customer_waiting_since = match state {
        ResponseState::CustomerWaiting => last_customer_reply_at.clone(),
        _ => None,
    };

    conn.execute(
        "UPDATE conversations SET
            first_customer_message_at = ?1,
            first_response_at = ?2,
            last_customer_reply_at = ?3,
            last_human_agent_response_at = ?4,
            customer_waiting_since = ?5,
            response_state = ?6
         WHERE id = ?7",
        params![
            first_customer_message_at,
            first_response_at,
            last_customer_reply_at,
            last_human_agent_response_at,
            customer_waiting_since,
            state.as_str(),
            conv_local,
        ],
    )?;
    Ok(())
}

pub fn rebuild_all(conn: &Connection) -> Result<RebuildSummary> {
    let mut summary = RebuildSummary::default();

    let conversations: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM conversations")?;
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };
    summary.conversations = conversations.len() as i64;

    for conv_local in conversations {
        let threads: Vec<MirrorThread> = {
            let mut stmt = conn.prepare(
                "SELECT id, remote_id, type, state, body_text, from_type,
                        created_by_user_id, created_by_customer_id, created_at
                 FROM conversation_threads
                 WHERE conversation_id = ?1 AND deleted_at IS NULL",
            )?;
            let rows = stmt.query_map(params![conv_local], |r| {
                Ok(MirrorThread {
                    id: r.get(0)?,
                    remote_id: r.get(1)?,
                    thread_type: r.get(2)?,
                    state: r.get(3)?,
                    body: r.get(4)?,
                    from_type: r.get(5)?,
                    created_by_user_id: r.get(6)?,
                    created_by_customer_id: r.get(7)?,
                    created_at: r.get(8)?,
                })
            })?;
            rows.filter_map(|r| r.ok()).collect()
        };

        for t in &threads {
            let Some((event_type, actor_type, actor_id)) = derive_mirror_event(t) else {
                continue;
            };
            let dedup_key = match t.remote_id {
                Some(remote) => format!("thread:{remote}"),
                None => format!("rebuild:thread:{}", t.id),
            };
            let occurred_at = t
                .created_at
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(now_iso);
            let wrote = record_full_event(
                conn,
                &FullActivityEvent {
                    base: ActivityEvent {
                        id: None,
                        conversation_id: conv_local,
                        event_type,
                        actor_type,
                        actor_id,
                        occurred_at,
                        dedup_key,
                    },
                    thread_local_id: Some(t.id),
                    source: "rebuild".into(),
                    metadata: Some(
                        serde_json::json!({
                            "thread_remote_id": t.remote_id,
                            "thread_type": t.thread_type,
                        })
                        .to_string(),
                    ),
                },
            )?;
            if wrote {
                summary.events_written += 1;
            }
        }

        update_derived_columns_by_local_id(conn, conv_local)?;
    }

    summary.events_total = conn
        .query_row("SELECT COUNT(*) FROM activity_events", [], |r| r.get(0))
        .unwrap_or(0);
    Ok(summary)
}
