use rusqlite::Connection;

use crate::jobs;
use crate::webhook;

#[derive(Debug, PartialEq)]
pub enum WebhookProcessResult {
    Accepted {
        row_id: i64,
    },
    Duplicate {
        row_id: i64,
    },
    SignatureInvalid,
    BadRequest,
    /// No webhook secret is configured. Unsigned events are always
    /// hard-rejected (migration-map row 1): accepting them would let
    /// anyone with loopback access inject fake Help Scout events.
    SecretNotConfigured,
}

pub fn process_webhook(
    conn: &Connection,
    secret: &[u8],
    body: &[u8],
    provided_signature: Option<&str>,
    event_type: &str,
) -> WebhookProcessResult {
    if body.is_empty() {
        return WebhookProcessResult::BadRequest;
    }
    if serde_json::from_slice::<serde_json::Value>(body).is_err() {
        return WebhookProcessResult::BadRequest;
    }

    if secret.is_empty() {
        return WebhookProcessResult::SecretNotConfigured;
    }
    let valid = match provided_signature {
        Some(sig) => webhook::verify_signature(secret, body, sig).is_ok(),
        None => false,
    };
    if !valid {
        return WebhookProcessResult::SignatureInvalid;
    }

    let raw = String::from_utf8_lossy(body).into_owned();
    // Audit fix (M8): heal rows stranded in 'processing' by a crash
    // between the processing mark and the terminal write, before inserting
    // the new event.
    if let Err(e) = webhook::recover_stale_webhook_events(conn) {
        tracing::warn!(error = %e, "failed to recover stale webhook events");
    }
    let (row_id, duplicate) = match webhook::insert_webhook_event(conn, event_type, &raw, None) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "failed to persist webhook event");
            return WebhookProcessResult::BadRequest;
        }
    };
    if duplicate {
        // Audit fix (M8): the old code stamped the ORIGINAL row
        // 'duplicate' — overwriting 'pending' or 'failed'-with-retries-left
        // and declaring the event handled while Help Scout was still
        // retrying it. The original's processing state is left untouched;
        // the delivery simply answers as a duplicate.
        return WebhookProcessResult::Duplicate { row_id };
    }

    if let Err(e) = webhook::prune_webhook_events(conn, 5000) {
        tracing::warn!(error = %e, "failed to prune webhook_events");
    }

    process_event(conn, row_id, event_type, &raw);
    WebhookProcessResult::Accepted { row_id }
}

pub fn process_event(conn: &Connection, row_id: i64, event_type: &str, raw: &str) {
    if let Err(e) = webhook::set_webhook_event_state(conn, row_id, "processing", None) {
        tracing::error!(error = %e, "failed to mark webhook event processing");
        return;
    }

    let payload: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => {
            let _ = webhook::set_webhook_event_state(
                conn,
                row_id,
                "failed",
                Some("Unparseable payload"),
            );
            return;
        }
    };

    let conversation_id = positive_number(payload.get("objectID"))
        .or_else(|| positive_number(payload.get("id")))
        .or_else(|| positive_number(payload.get("conversationId")));

    let mut enqueued: Option<(&str, serde_json::Value)> = None;
    match event_type {
        "convo.created"
        | "convo.updated"
        | "convo.assigned"
        | "convo.status"
        | "convo.tags"
        | "convo.custom-fields"
        | "convo.moved"
        | "convo.customer.reply.created"
        | "convo.agent.reply.created"
        | "convo.note.created"
        | "convo.ai-answers.created" => {
            if let Some(id) = conversation_id {
                enqueued = Some((
                    "sync_conversation",
                    serde_json::json!({"remoteId": id, "source": "webhook"}),
                ));
            }
        }
        "convo.merged" => {
            if let Some(id) = conversation_id {
                enqueued = Some((
                    "sync_conversation_merge",
                    serde_json::json!({"remoteId": id}),
                ));
            }
        }
        "convo.deleted" => {
            if let Some(id) = conversation_id {
                enqueued = Some(("delete_conversation", serde_json::json!({"remoteId": id})));
            }
        }
        "customer.created" | "customer.updated" => {
            if let Some(id) = conversation_id {
                enqueued = Some(("sync_customer", serde_json::json!({"remoteId": id})));
            }
        }
        "customer.deleted" => {
            if let Some(id) = conversation_id {
                enqueued = Some(("delete_customer", serde_json::json!({"remoteId": id})));
            }
        }
        "organization.created" | "organization.updated" | "organization.deleted" => {
            enqueued = Some(("sync_organizations", serde_json::json!({})));
        }
        "satisfaction.ratings" => {
            let rating_id = positive_number(payload.get("ratingId"))
                .or_else(|| positive_number(payload.get("rating_id")))
                .or_else(|| positive_number(payload.get("id")));
            enqueued = Some((
                "sync_conversation_ratings",
                serde_json::json!({"conversationId": conversation_id, "ratingId": rating_id}),
            ));
        }
        "tag.created" | "tag.updated" | "tag.deleted" => {
            enqueued = Some(("sync_tags", serde_json::json!({})));
        }
        "user.status.changed" => {
            enqueued = Some(("sync_user_statuses", serde_json::json!({})));
        }
        // P2: the beacon.chat.* events are the only push path for live
        // chats — Help Scout exposes no chat list endpoint — so they
        // ingest the chat payload straight into the mirror (m059). A chat
        // is not a conversation: the mirror keeps even ended chats,
        // because the platform stops serving them once they collapse into
        // a conversation. Since 2026-10 the platform only delivers
        // created and customer.replied (agent.replied / assigned / ended
        // were retired from the enum); the arms stay so a manual
        // subscription still ingests.
        "beacon.chat.created"
        | "beacon.chat.customer.replied"
        | "beacon.chat.agent.replied"
        | "beacon.chat.assigned"
        | "beacon.chat.ended" => {
            let chat_id = payload
                .get("chat")
                .and_then(|c| c.get("id"))
                .or_else(|| payload.get("id"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            match crate::chats::ingest_event(conn, event_type, &payload) {
                Ok(()) => tracing::info!(event_type, chat_id, "beacon chat ingested"),
                Err(e) => {
                    tracing::error!(error = %e, event_type, "failed to ingest beacon chat");
                    let _ = webhook::set_webhook_event_state(
                        conn,
                        row_id,
                        "failed",
                        Some(&e.to_string()),
                    );
                    return;
                }
            }
        }
        _ => {}
    }

    if let Some((kind, payload)) = enqueued
        && let Err(e) = jobs::enqueue(conn, kind, &payload.to_string())
    {
        tracing::error!(error = %e, kind, "failed to enqueue webhook job");
        let _ = webhook::set_webhook_event_state(conn, row_id, "failed", Some(&e.to_string()));
        return;
    }

    let _ = webhook::set_webhook_event_state(conn, row_id, "processed", None);
}

pub fn drain_pending(conn: &Connection) -> usize {
    match webhook::get_pending_webhook_events(conn, 50) {
        Ok(pending) => {
            let count = pending.len();
            for ev in pending {
                process_event(conn, ev.id, &ev.event_type, &ev.payload);
            }
            count
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to query pending webhook events");
            0
        }
    }
}

fn positive_number(v: Option<&serde_json::Value>) -> Option<i64> {
    let n = v?.as_f64()?;
    if n.is_finite() && n > 0.0 && n.fract() == 0.0 {
        Some(n as i64)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        // The data path no longer self-ensures schema (row 6): tests open the
        // database the way the app does — through the full migration runner.
        let mut conn = Connection::open_in_memory().expect("in-memory connection");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn signed(secret: &[u8], body: &[u8]) -> String {
        crate::webhook::compute_signature(secret, body)
    }

    #[test]
    fn accepts_valid_signature_and_persists_processed_event() {
        let conn = fresh_conn();
        let secret = b"top-secret";
        let body = br#"{"objectID":123}"#;
        let sig = signed(secret, body);

        let row_id = match process_webhook(&conn, secret, body, Some(&sig), "convo.created") {
            WebhookProcessResult::Accepted { row_id } => row_id,
            other => panic!("expected Accepted, got {other:?}"),
        };

        let state: String = conn
            .query_row(
                "SELECT processing_state FROM webhook_events WHERE id = ?1",
                rusqlite::params![row_id],
                |r| r.get(0),
            )
            .expect("event row");
        assert_eq!(state, "processed");

        // convo.created with objectID enqueues a sync job
        let jobs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM jobs WHERE type = 'sync_conversation'",
                [],
                |r| r.get(0),
            )
            .expect("jobs table readable");
        assert_eq!(jobs, 1);
    }

    #[test]
    fn rejects_empty_body() {
        let conn = fresh_conn();
        let result = process_webhook(&conn, b"secret", b"", None, "convo.created");
        assert!(matches!(result, WebhookProcessResult::BadRequest));
    }

    #[test]
    fn rejects_non_json_body() {
        let conn = fresh_conn();
        let body = b"not-json";
        let sig = signed(b"secret", body);
        let result = process_webhook(&conn, b"secret", body, Some(&sig), "convo.created");
        assert!(matches!(result, WebhookProcessResult::BadRequest));
    }

    #[test]
    fn rejects_wrong_signature() {
        let conn = fresh_conn();
        let result = process_webhook(
            &conn,
            b"secret",
            br#"{"id":1}"#,
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="),
            "convo.created",
        );
        assert!(matches!(result, WebhookProcessResult::SignatureInvalid));
    }

    #[test]
    fn rejects_missing_signature_when_secret_set() {
        let conn = fresh_conn();
        let result = process_webhook(&conn, b"secret", br#"{"id":1}"#, None, "convo.created");
        assert!(matches!(result, WebhookProcessResult::SignatureInvalid));
    }

    #[test]
    fn empty_secret_hard_rejects_without_a_secret() {
        let conn = fresh_conn();
        let result = process_webhook(&conn, b"", br#"{"id":1}"#, None, "convo.created");
        assert!(matches!(result, WebhookProcessResult::SecretNotConfigured));

        // A provided (unverifiable) signature does not rescue an unconfigured
        // secret: there is nothing to verify it against.
        let result = process_webhook(
            &conn,
            b"",
            br#"{"id":1}"#,
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="),
            "convo.created",
        );
        assert!(matches!(result, WebhookProcessResult::SecretNotConfigured));
    }

    #[test]
    fn duplicate_event_returns_same_row() {
        let conn = fresh_conn();
        let secret = b"secret";
        let body = br#"{"objectID":7}"#;
        let sig = signed(secret, body);

        let first_id = match process_webhook(&conn, secret, body, Some(&sig), "convo.created") {
            WebhookProcessResult::Accepted { row_id } => row_id,
            other => panic!("expected first Accepted, got {other:?}"),
        };
        let second = process_webhook(&conn, secret, body, Some(&sig), "convo.created");
        assert!(matches!(
            second,
            WebhookProcessResult::Duplicate { row_id } if row_id == first_id
        ));
    }

    #[test]
    fn drain_pending_processes_stored_events() {
        let conn = fresh_conn();
        for id in 1..=2 {
            let payload = format!(r#"{{"objectID":{id}}}"#);
            crate::webhook::insert_webhook_event(&conn, "convo.created", &payload, None)
                .expect("insert pending event");
        }
        assert_eq!(
            crate::webhook::pending_event_count(&conn).expect("pending count"),
            2
        );
        assert_eq!(drain_pending(&conn), 2);
        assert_eq!(
            crate::webhook::pending_event_count(&conn).expect("pending count"),
            0
        );
    }

    #[test]
    fn beacon_chat_events_ingest_into_the_mirror() {
        let conn = fresh_conn();
        let secret = b"chat-secret";
        let body = br#"{"chat": {"id": "chat-1", "mailboxId": 3, "state": "open",
            "customer": {"id": 7, "first": "Kim", "last": "One"},
            "events": [{"id": "m-1", "type": "customer.message.created", "text": "hi",
                        "createdAt": "2026-10-21T10:00:00Z"}]}}"#;
        let sig = signed(secret, body);

        let row_id = match process_webhook(&conn, secret, body, Some(&sig), "beacon.chat.created") {
            WebhookProcessResult::Accepted { row_id } => row_id,
            other => panic!("expected Accepted, got {other:?}"),
        };

        let (state, chats): (String, i64) = conn
            .query_row(
                "SELECT processing_state,
                        (SELECT COUNT(*) FROM beacon_chats WHERE chat_id = 'chat-1')
                 FROM webhook_events WHERE id = ?1",
                rusqlite::params![row_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("event row");
        assert_eq!(state, "processed");
        assert_eq!(chats, 1, "the chat payload must land in the m059 mirror");
    }

    #[test]
    fn beacon_chat_without_a_chat_object_marks_the_event_failed() {
        let conn = fresh_conn();
        let secret = b"chat-secret";
        let body = br#"{"id": 123}"#;
        let sig = signed(secret, body);
        let row_id = match process_webhook(&conn, secret, body, Some(&sig), "beacon.chat.assigned")
        {
            WebhookProcessResult::Accepted { row_id } => row_id,
            other => panic!("expected Accepted, got {other:?}"),
        };
        let state: String = conn
            .query_row(
                "SELECT processing_state FROM webhook_events WHERE id = ?1",
                rusqlite::params![row_id],
                |r| r.get(0),
            )
            .expect("event row");
        assert_eq!(state, "failed");
        // Nothing was mirrored from an id-less payload.
        let mirrored: i64 = conn
            .query_row("SELECT COUNT(*) FROM beacon_chats", [], |r| r.get(0))
            .expect("mirror readable");
        assert_eq!(mirrored, 0);
    }
}
