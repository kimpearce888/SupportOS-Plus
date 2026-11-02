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
