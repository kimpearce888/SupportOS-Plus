use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::ticket_states::TicketPriority;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TicketOperation {
    Assign {
        conversation_remote_id: i64,
        assignee_local_id: Option<i64>,
        actor_type: String,
        actor_id: Option<i64>,
    },
    ChangeStatus {
        conversation_remote_id: i64,
        new_status: String,
        actor_type: String,
        actor_id: Option<i64>,
    },
    SetPriority {
        conversation_remote_id: i64,
        new_priority: TicketPriority,
        actor_type: String,
        actor_id: Option<i64>,
    },
    SetTicketState {
        conversation_remote_id: i64,
        new_state: String,
        actor_type: String,
        actor_id: Option<i64>,
    },
    AddNote {
        conversation_remote_id: i64,
        body: String,
        actor_type: String,
        actor_id: Option<i64>,
    },
}

#[derive(Debug, Clone)]
pub enum OperationResult {
    Success { message: String },
    Rejected { reason: String },
}

pub fn execute(conn: &mut Connection, op: &TicketOperation) -> Result<OperationResult> {
    match op {
        TicketOperation::Assign {
            conversation_remote_id,
            assignee_local_id,
            actor_type,
            actor_id,
        } => execute_assign(
            conn,
            *conversation_remote_id,
            *assignee_local_id,
            actor_type,
            *actor_id,
        ),
        TicketOperation::ChangeStatus {
            conversation_remote_id,
            new_status,
            actor_type,
            actor_id,
        } => execute_change_status(
            conn,
            *conversation_remote_id,
            new_status,
            actor_type,
            *actor_id,
        ),
        TicketOperation::SetPriority {
            conversation_remote_id,
            new_priority,
            actor_type,
            actor_id,
        } => execute_set_priority(
            conn,
            *conversation_remote_id,
            *new_priority,
            actor_type,
            *actor_id,
        ),
        TicketOperation::SetTicketState {
            conversation_remote_id,
            new_state,
            actor_type,
            actor_id,
        } => execute_set_ticket_state(
            conn,
            *conversation_remote_id,
            new_state,
            actor_type,
            *actor_id,
        ),
        TicketOperation::AddNote {
            conversation_remote_id,
            body,
            actor_type,
            actor_id,
        } => execute_add_note(conn, *conversation_remote_id, body, actor_type, *actor_id),
    }
}

fn conversation_exists(conn: &Connection, remote_id: i64) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM conversations WHERE remote_id = ?1)",
        params![remote_id],
        |r| r.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

fn execute_assign(
    conn: &mut Connection,
    remote_id: i64,
    assignee_id: Option<i64>,
    actor_type: &str,
    actor_id: Option<i64>,
) -> Result<OperationResult> {
    if !conversation_exists(conn, remote_id) {
        return Ok(OperationResult::Rejected {
            reason: format!("conversation {remote_id} not found"),
        });
    }

    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE conversations SET assignee_local_id = ?1 WHERE remote_id = ?2",
        params![assignee_id, remote_id],
    )?;

    tx.execute(
        "INSERT INTO activity_events (conversation_id, event_type, actor_type, actor_id, occurred_at, dedup_key)
         VALUES (?1, 'assign', ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 'assign_' || ?1 || '_' || ?3 || '_' || strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        params![remote_id, actor_type, actor_id],
    )?;

    tx.commit()?;

    crate::activity::update_derived_columns(conn, remote_id)?;

    Ok(OperationResult::Success {
        message: format!("conversation {remote_id} assigned"),
    })
}

fn execute_change_status(
    conn: &mut Connection,
    remote_id: i64,
    new_status: &str,
    actor_type: &str,
    actor_id: Option<i64>,
) -> Result<OperationResult> {
    if !conversation_exists(conn, remote_id) {
        return Ok(OperationResult::Rejected {
            reason: format!("conversation {remote_id} not found"),
        });
    }

    let valid = matches!(new_status, "active" | "pending" | "closed" | "spam");
    if !valid {
        return Ok(OperationResult::Rejected {
            reason: format!("invalid status: {new_status}"),
        });
    }

    let tx = conn.transaction()?;

    let closed_at = if new_status == "closed" {
        Some("strftime('%Y-%m-%dT%H:%M:%fZ','now')")
    } else {
        None
    };

    match closed_at {
        Some(_) => {
            tx.execute(
                "UPDATE conversations SET status = ?1, closed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                 WHERE remote_id = ?2",
                params![new_status, remote_id],
            )?;
        }
        None => {
            tx.execute(
                "UPDATE conversations SET status = ?1, closed_at = NULL WHERE remote_id = ?2",
                params![new_status, remote_id],
            )?;
        }
    }

    tx.execute(
        "INSERT OR IGNORE INTO activity_events (conversation_id, event_type, actor_type, actor_id, occurred_at, dedup_key)
         VALUES (?1, 'status_change', ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 'status_' || ?1 || '_' || ?3 || '_' || ?4 || '_' || strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        params![remote_id, actor_type, actor_id, new_status],
    )?;

    tx.commit()?;

    crate::activity::update_derived_columns(conn, remote_id)?;

    Ok(OperationResult::Success {
        message: format!("conversation {remote_id} status changed to {new_status}"),
    })
}

fn execute_set_priority(
    conn: &mut Connection,
    remote_id: i64,
    priority: TicketPriority,
    actor_type: &str,
    actor_id: Option<i64>,
) -> Result<OperationResult> {
    if !conversation_exists(conn, remote_id) {
        return Ok(OperationResult::Rejected {
            reason: format!("conversation {remote_id} not found"),
        });
    }

    crate::ticket_states::set_priority(conn, remote_id, priority, actor_type, actor_id)?;

    Ok(OperationResult::Success {
        message: format!(
            "conversation {remote_id} priority set to {}",
            priority.as_str()
        ),
    })
}

fn execute_set_ticket_state(
    conn: &mut Connection,
    remote_id: i64,
    new_state: &str,
    actor_type: &str,
    actor_id: Option<i64>,
) -> Result<OperationResult> {
    if !conversation_exists(conn, remote_id) {
        return Ok(OperationResult::Rejected {
            reason: format!("conversation {remote_id} not found"),
        });
    }

    crate::ticket_states::set_ticket_state(conn, remote_id, new_state, actor_type, actor_id)?;

    Ok(OperationResult::Success {
        message: format!("conversation {remote_id} state set to {new_state}"),
    })
}

fn execute_add_note(
    conn: &mut Connection,
    remote_id: i64,
    _body: &str,
    actor_type: &str,
    actor_id: Option<i64>,
) -> Result<OperationResult> {
    if !conversation_exists(conn, remote_id) {
        return Ok(OperationResult::Rejected {
            reason: format!("conversation {remote_id} not found"),
        });
    }

    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO activity_events (conversation_id, event_type, actor_type, actor_id, occurred_at, dedup_key)
         VALUES (?1, 'note', ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 'note_' || ?1 || '_' || ?3 || '_' || strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        params![remote_id, actor_type, actor_id],
    )?;

    tx.commit()?;

    Ok(OperationResult::Success {
        message: format!("note added to conversation {remote_id}"),
    })
}
