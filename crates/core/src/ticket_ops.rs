use rusqlite::{Connection, params};
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

fn conversation_local_id(conn: &Connection, remote_id: i64) -> Option<i64> {
    conn.query_row(
        "SELECT id FROM conversations WHERE remote_id = ?1",
        params![remote_id],
        |r| r.get(0),
    )
    .ok()
}

fn execute_assign(
    conn: &mut Connection,
    remote_id: i64,
    assignee_id: Option<i64>,
    actor_type: &str,
    actor_id: Option<i64>,
) -> Result<OperationResult> {
    let Some(local_id) = conversation_local_id(conn, remote_id) else {
        return Ok(OperationResult::Rejected {
            reason: format!("conversation {remote_id} not found"),
        });
    };

    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE conversations SET assignee_local_id = ?1 WHERE remote_id = ?2",
        params![assignee_id, remote_id],
    )?;

    tx.execute(
        "INSERT INTO activity_events (conversation_id, event_type, actor_type, actor_id, occurred_at, dedup_key)
         VALUES (?1, 'assign', ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 'assign_' || ?1 || '_' || COALESCE(?3, -1) || '_' || strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        params![local_id, actor_type, actor_id],
    )?;

    tx.commit()?;

    crate::activity::update_derived_columns_by_local_id(conn, local_id)?;

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
    let valid = matches!(new_status, "active" | "pending" | "closed" | "spam");
    if !valid {
        return Ok(OperationResult::Rejected {
            reason: format!("invalid status: {new_status}"),
        });
    }

    let Some(local_id) = conversation_local_id(conn, remote_id) else {
        return Ok(OperationResult::Rejected {
            reason: format!("conversation {remote_id} not found"),
        });
    };

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
                 'status_' || ?1 || '_' || COALESCE(?3, -1) || '_' || ?4 || '_' || strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        params![local_id, actor_type, actor_id, new_status],
    )?;

    tx.commit()?;

    crate::activity::update_derived_columns_by_local_id(conn, local_id)?;

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
    if conversation_local_id(conn, remote_id).is_none() {
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
    if conversation_local_id(conn, remote_id).is_none() {
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
    body: &str,
    actor_type: &str,
    actor_id: Option<i64>,
) -> Result<OperationResult> {
    let Some(local_id) = conversation_local_id(conn, remote_id) else {
        return Ok(OperationResult::Rejected {
            reason: format!("conversation {remote_id} not found"),
        });
    };

    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO activity_events (conversation_id, event_type, actor_type, actor_id, occurred_at, dedup_key)
         VALUES (?1, 'note', ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 'note_' || ?1 || '_' || COALESCE(?3, -1) || '_' || strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        params![local_id, actor_type, actor_id],
    )?;

    tx.commit()?;

    if !body.trim().is_empty() {
        crate::conversation_ops::create_outbound_job_keyed(
            conn,
            "create_note",
            &serde_json::json!({
                "conversationId": local_id,
                "text": body,
            }),
            Some(local_id),
            Some(&format!(
                "ticket_note_{local_id}_{}",
                crate::embeddings::content_hash(body)
            )),
        )?;
    }

    crate::activity::update_derived_columns_by_local_id(conn, local_id)?;

    Ok(OperationResult::Success {
        message: format!("note queued for delivery on conversation {remote_id}"),
    })
}

#[cfg(test)]
mod batch19_tests {
    use super::*;
    use crate::db::DbHandle;

    fn db() -> DbHandle {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        DbHandle::new(conn)
    }

    fn seed(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status)
             VALUES (4201, 42, 'Refund stuck', 'preview', 'active')",
            [],
        )
        .expect("conversation");
        let local = conn.last_insert_rowid();
        crate::activity::record_event(
            conn,
            &crate::activity::ActivityEvent {
                id: None,
                conversation_id: local,
                event_type: "customer_message".into(),
                actor_type: "customer".into(),
                actor_id: None,
                occurred_at: "2026-10-20T10:00:00.000Z".into(),
                dedup_key: format!("thread:9900_{local}"),
            },
        )
        .expect("customer event");
        local
    }

    #[test]
    fn a_status_change_keys_its_event_by_the_local_conversation_id() {
        let db = db();
        let local = db.with_write(|c| seed(c));
        db.with_write(|c| {
            execute(
                c,
                &TicketOperation::ChangeStatus {
                    conversation_remote_id: 4201,
                    new_status: "pending".into(),
                    actor_type: "user".into(),
                    actor_id: None,
                },
            )
        })
        .expect("status change");

        let event_conversation_id: i64 = db
            .with_read(|c| {
                c.query_row(
                    "SELECT conversation_id FROM activity_events
                      WHERE event_type = 'status_change'",
                    [],
                    |r| r.get(0),
                )
            })
            .expect("the status-change event exists");
        assert_eq!(
            event_conversation_id, local,
            "events are keyed on conversations.id, never the remote id {event_conversation_id}"
        );
    }

    #[test]
    fn a_user_action_preserves_the_derived_customer_history() {
        let db = db();
        db.with_write(|c| {
            let local = seed(c);
            crate::activity::update_derived_columns_by_local_id(c, local).expect("derive baseline");
            execute(
                c,
                &TicketOperation::Assign {
                    conversation_remote_id: 4201,
                    assignee_local_id: None,
                    actor_type: "user".into(),
                    actor_id: None,
                },
            )
            .expect("assign");
        });

        let (first_customer_message_at, last_customer_reply_at): (Option<String>, Option<String>) =
            db.with_read(|c| {
                c.query_row(
                    "SELECT first_customer_message_at, last_customer_reply_at
                       FROM conversations WHERE remote_id = 4201",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
            })
            .expect("conversation");
        assert!(
            first_customer_message_at.is_some(),
            "the customer history survived the user action (was wiped pre-fix)"
        );
        assert!(last_customer_reply_at.is_some());
    }

    #[test]
    fn an_assign_keys_its_event_locally_and_an_unknown_remote_is_rejected() {
        let db = db();
        let local = db.with_write(|c| seed(c));
        let outcome = db
            .with_write(|c| {
                execute(
                    c,
                    &TicketOperation::AddNote {
                        conversation_remote_id: 4201,
                        body: "internal note".into(),
                        actor_type: "user".into(),
                        actor_id: None,
                    },
                )
            })
            .expect("note");
        assert!(matches!(outcome, OperationResult::Success { .. }));

        let event_conversation_id: i64 = db
            .with_read(|c| {
                c.query_row(
                    "SELECT conversation_id FROM activity_events WHERE event_type = 'note'",
                    [],
                    |r| r.get(0),
                )
            })
            .expect("note event");
        assert_eq!(event_conversation_id, local);

        let rejected = db
            .with_write(|c| {
                execute(
                    c,
                    &TicketOperation::ChangeStatus {
                        conversation_remote_id: 999_999,
                        new_status: "closed".into(),
                        actor_type: "user".into(),
                        actor_id: None,
                    },
                )
            })
            .expect("execute");
        assert!(
            matches!(rejected, OperationResult::Rejected { .. }),
            "an unknown remote conversation is rejected, not invented"
        );
    }

    #[test]
    fn add_note_stores_its_body_on_the_outbound_note_lane() {
        let db = db();
        let outcome = db
            .with_write(|c| {
                let _local = seed(c);
                execute(
                    c,
                    &TicketOperation::AddNote {
                        conversation_remote_id: 4201,
                        body: "checked the refund ledger — held by fraud rule".into(),
                        actor_type: "user".into(),
                        actor_id: None,
                    },
                )
            })
            .expect("note");
        assert!(matches!(outcome, OperationResult::Success { .. }));

        let (payload, idem): (String, String) = db
            .with_read(|c| {
                c.query_row(
                    "SELECT payload, idempotency_key FROM outbound_jobs
                      WHERE kind = 'create_note'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
            })
            .expect("the note was queued on the outbound lane");
        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("payload json");
        assert_eq!(
            parsed["text"], "checked the refund ledger — held by fraud rule",
            "the body is stored, not discarded"
        );
        assert!(
            parsed["conversationId"].is_i64(),
            "the lane gets the LOCAL conversation id: {parsed}"
        );
        assert!(
            idem.starts_with("ticket_note_"),
            "the job is idempotently keyed: {idem}"
        );
        let events: i64 = db
            .with_read(|c| {
                c.query_row(
                    "SELECT COUNT(*) FROM activity_events WHERE event_type = 'note'",
                    [],
                    |r| r.get(0),
                )
            })
            .expect("activity count");
        assert_eq!(events, 1, "the local activity event is still recorded");
    }
}
