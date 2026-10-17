use rusqlite::Connection;

use crate::jobs;
use crate::webhook;

#[derive(Debug, PartialEq)]
pub enum WebhookProcessResult {
    Accepted { row_id: i64 },
    Duplicate { row_id: i64 },
    SignatureInvalid,
    BadRequest,
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

    if !secret.is_empty() {
        let valid = match provided_signature {
            Some(sig) => webhook::verify_signature(secret, body, sig).is_ok(),
            None => false,
        };
        if !valid {
            return WebhookProcessResult::SignatureInvalid;
        }
    }

    let raw = String::from_utf8_lossy(body).into_owned();
    let (row_id, duplicate) = match webhook::insert_webhook_event(conn, event_type, &raw, None) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "failed to persist webhook event");
            return WebhookProcessResult::BadRequest;
        }
    };
    if duplicate {
        let _ = webhook::set_webhook_event_state(conn, row_id, "duplicate", None);
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
