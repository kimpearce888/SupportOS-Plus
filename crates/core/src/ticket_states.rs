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

pub(crate) fn apply_m004(conn: &Connection) -> Result<()> {
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

pub(crate) fn set_ticket_state(
    conn: &mut Connection,
    conversation_remote_id: i64,
    new_state: &str,
    actor_type: &str,
    actor_id: Option<i64>,
) -> Result<()> {
    if !workflow_state_is_valid(new_state) {
        return Err(crate::error::Error::Validation(format!(
            "unknown workflow state {new_state:?} — expected one of {}",
            TICKET_WORKFLOW_STATES.join(", ")
        )));
    }
    let tx = conn.transaction()?;

    let old_state: Option<String> = tx
        .query_row(
            "SELECT supportos_state FROM conversations WHERE remote_id = ?1",
            params![conversation_remote_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    let state_row_id: Option<i64> = tx
        .query_row(
            "SELECT id FROM ticket_states WHERE key = ?1",
            params![new_state],
            |r| r.get(0),
        )
        .ok();

    tx.execute(
        "UPDATE conversations SET supportos_state = ?1, supportos_state_id = ?2
          WHERE remote_id = ?3",
        params![new_state, state_row_id, conversation_remote_id],
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

pub const TICKET_WORKFLOW_STATES: [&str; 6] = [
    "new",
    "investigating",
    "waiting-customer",
    "waiting-engineering",
    "ready-verify",
    "resolved",
];

#[must_use]
pub fn workflow_state_is_valid(key: &str) -> bool {
    TICKET_WORKFLOW_STATES.contains(&key)
}

#[derive(Debug, Clone, Serialize)]
pub struct StateTransitionRow {
    pub from_state: Option<String>,
    pub to_state: Option<String>,
    pub from_priority: Option<String>,
    pub to_priority: Option<String>,
    pub actor_type: String,
    pub actor_id: Option<i64>,
    pub transitioned_at: String,
}

pub fn list_state_transitions(
    conn: &Connection,
    conversation_remote_id: i64,
    limit: u32,
) -> Vec<StateTransitionRow> {
    crate::db::query_rows(
        conn,
        "SELECT from_state, to_state, from_priority, to_priority,
                actor_type, actor_id, transitioned_at
           FROM ticket_state_transitions
          WHERE conversation_remote_id = ?1
          ORDER BY transitioned_at DESC, id DESC
          LIMIT ?2",
        params![conversation_remote_id, i64::from(limit.clamp(1, 100))],
        |r| {
            Ok(StateTransitionRow {
                from_state: r.get(0)?,
                to_state: r.get(1)?,
                from_priority: r.get(2)?,
                to_priority: r.get(3)?,
                actor_type: r.get(4)?,
                actor_id: r.get(5)?,
                transitioned_at: r.get(6)?,
            })
        },
    )
    .unwrap_or_default()
}

pub(crate) fn apply_m032(conn: &Connection) -> Result<()> {
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
    let cols: Vec<String> =
        crate::db::query_rows(conn, "PRAGMA table_info(conversations)", [], |r| {
            r.get::<_, String>(1)
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

#[cfg(test)]
mod audp2_tests {
    use super::*;

    fn conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn seed_conversation(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (1001, 51, 'B4 subject', 'p', 'active')",
            [],
        )
        .expect("conversation");
        1001
    }

    #[test]
    fn audp2_workflow_state_writes_state_id_and_transition() {
        let mut c = conn();
        let remote = seed_conversation(&c);

        set_ticket_state(&mut c, remote, "investigating", "user", Some(5)).expect("set state");

        let (state, state_id): (Option<String>, Option<i64>) = c
            .query_row(
                "SELECT supportos_state, supportos_state_id FROM conversations WHERE remote_id = ?1",
                params![remote],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("row");
        assert_eq!(state.as_deref(), Some("investigating"));
        let resolved_id: i64 = c
            .query_row(
                "SELECT id FROM ticket_states WHERE key = 'investigating'",
                [],
                |r| r.get(0),
            )
            .expect("lookup row");
        assert_eq!(state_id, Some(resolved_id));

        set_ticket_state(&mut c, remote, "resolved", "user", None).expect("second set");
        let rows: Vec<(Option<String>, Option<String>)> = {
            let mut stmt = c
                .prepare(
                    "SELECT from_state, to_state FROM ticket_state_transitions
                      WHERE conversation_remote_id = ?1 ORDER BY id",
                )
                .expect("stmt");
            stmt.query_map(params![remote], |r| Ok((r.get(0)?, r.get(1)?)))
                .expect("map")
                .filter_map(|x| x.ok())
                .collect()
        };
        assert_eq!(
            rows,
            vec![
                (None, Some("investigating".into())),
                (Some("investigating".into()), Some("resolved".into()))
            ],
            "both transitions recorded with the from-state chain"
        );
    }

    #[test]
    fn audp2_junk_workflow_states_are_rejected() {
        let mut c = conn();
        let remote = seed_conversation(&c);
        let err = set_ticket_state(&mut c, remote, "investigating-typo", "user", None)
            .expect_err("a typo must not encode a bogus state");
        assert!(
            err.to_string().contains("unknown workflow state"),
            "the error names the problem: {err}"
        );
        let state: Option<String> = c
            .query_row(
                "SELECT supportos_state FROM conversations WHERE remote_id = ?1",
                params![remote],
                |r| r.get(0),
            )
            .expect("row");
        assert_eq!(state, None, "nothing was written");
        let transitions: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM ticket_state_transitions WHERE conversation_remote_id = ?1",
                params![remote],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(transitions, 0);
    }

    #[test]
    fn audp2_state_history_lists_both_lanes_newest_first() {
        let mut c = conn();
        let remote = seed_conversation(&c);
        set_ticket_state(&mut c, remote, "new", "user", None).expect("state");
        set_priority(&mut c, remote, TicketPriority::High, "user", Some(5)).expect("priority");
        set_ticket_state(&mut c, remote, "investigating", "automation", None).expect("state 2");

        let rows = list_state_transitions(&c, remote, 20);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].to_state.as_deref(), Some("investigating"));
        assert_eq!(rows[0].actor_type, "automation");
        assert_eq!(rows[1].to_priority.as_deref(), Some("high"));
        assert!(rows[1].to_state.is_none());
        assert_eq!(rows[2].to_state.as_deref(), Some("new"));
        assert_eq!(rows[2].from_state, None);
        assert_eq!(list_state_transitions(&c, remote, 1).len(), 1);
    }
}
