use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::helpscout::{HsChat, HsChatEvent, HsChatParticipant};

pub(crate) fn apply_m059_beacon_chats(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS beacon_chats (
            chat_id            TEXT PRIMARY KEY,
            beacon_id          TEXT,
            mailbox_remote_id  INTEGER NOT NULL DEFAULT 0,
            customer_remote_id INTEGER,
            customer_name      TEXT,
            assignee_remote_id INTEGER,
            assignee_name      TEXT,
            preview            TEXT,
            state              TEXT NOT NULL DEFAULT 'open',
            created_at         TEXT,
            ended_at           TEXT,
            last_event_at      TEXT,
            raw_json           TEXT,
            conversation_remote_id INTEGER,
            conversation_number    INTEGER
        );
        CREATE TABLE IF NOT EXISTS beacon_chat_messages (
            chat_id     TEXT NOT NULL,
            message_id  TEXT NOT NULL,
            direction   TEXT NOT NULL DEFAULT 'customer',
            sender_name TEXT,
            text        TEXT,
            created_at  TEXT,
            PRIMARY KEY (chat_id, message_id)
        );
        CREATE INDEX IF NOT EXISTS idx_beacon_chats_state
            ON beacon_chats (state, last_event_at DESC);",
    )?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirrorChat {
    pub chat_id: String,
    pub beacon_id: Option<String>,
    pub mailbox_remote_id: i64,
    pub customer_remote_id: Option<i64>,
    pub customer_name: Option<String>,
    pub assignee_remote_id: Option<i64>,
    pub assignee_name: Option<String>,
    pub preview: Option<String>,
    pub state: String,
    pub created_at: Option<String>,
    pub ended_at: Option<String>,
    pub last_event_at: Option<String>,
    pub conversation_remote_id: Option<i64>,
    pub conversation_number: Option<i64>,
}

impl MirrorChat {
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state != "ended"
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirrorMessage {
    pub chat_id: String,
    pub message_id: String,
    pub direction: String,
    pub sender_name: Option<String>,
    pub text: Option<String>,
    pub created_at: Option<String>,
}

fn opt_str(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(str::to_string)
}

fn parse_participant(v: &Value) -> Option<HsChatParticipant> {
    let remote_id = v.get("id").and_then(|x| x.as_i64())?;
    Some(HsChatParticipant {
        remote_id,
        first: opt_str(v, "first").or_else(|| opt_str(v, "firstName")),
        last: opt_str(v, "last").or_else(|| opt_str(v, "lastName")),
        email: opt_str(v, "email"),
    })
}

#[must_use]
pub(crate) fn parse_chat_object(v: &Value) -> Option<HsChat> {
    let id = v.get("id").and_then(|x| x.as_str())?.to_string();
    if id.is_empty() {
        return None;
    }
    let events = ["events", "threads"]
        .iter()
        .find_map(|key| v.get(*key).and_then(|e| e.as_array()).cloned())
        .unwrap_or_default();
    Some(HsChat {
        id,
        beacon_id: opt_str(v, "beaconId"),
        mailbox_id: v.get("mailboxId").and_then(|x| x.as_i64()).unwrap_or(0),
        customer: v.get("customer").and_then(parse_participant),
        assignee: v.get("assignee").and_then(parse_participant),
        state: v
            .get("state")
            .and_then(|x| x.as_str())
            .unwrap_or("open")
            .to_string(),
        preview: opt_str(v, "preview"),
        created_at: opt_str(v, "createdAt"),
        ended_at: opt_str(v, "endedAt"),
        conversation_remote_id: v
            .get("conversationId")
            .or_else(|| v.get("conversation").and_then(|c| c.get("id")))
            .and_then(|x| x.as_i64()),
        events,
    })
}

#[must_use]
pub(crate) fn parse_chat_event(event_type: &str, payload: &Value) -> Option<HsChatEvent> {
    let chat_value = payload
        .get("chat")
        .filter(|c| c.get("id").is_some_and(|i| i.as_str().is_some()))
        .unwrap_or(payload);
    parse_chat_object(chat_value).map(|chat| HsChatEvent {
        event_type: event_type.to_string(),
        chat,
    })
}

pub fn ingest_event(conn: &Connection, event_type: &str, payload: &Value) -> Result<()> {
    let Some(event) = parse_chat_event(event_type, payload) else {
        return Err(Error::Validation(format!(
            "{event_type}: payload carries no chat object (string id)"
        )));
    };
    upsert_chat(conn, &event)?;
    record_event_messages(conn, &event.chat)?;
    Ok(())
}

fn upsert_chat(conn: &Connection, event: &HsChatEvent) -> Result<()> {
    let chat = &event.chat;
    let ended = event.event_type == "beacon.chat.ended" || chat.state == "ended";
    let state = if ended { "ended" } else { "open" };
    let preview = chat
        .preview
        .clone()
        .or_else(|| last_message_text(chat))
        .or_else(|| chat.customer.as_ref().map(|c| c.name()));
    let last_event_at = chat
        .events
        .iter()
        .rev()
        .find_map(|e| opt_str(e, "createdAt"))
        .or_else(|| chat.ended_at.clone())
        .or_else(|| chat.created_at.clone())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    conn.execute(
        "INSERT INTO beacon_chats (
            chat_id, beacon_id, mailbox_remote_id, customer_remote_id, customer_name,
            assignee_remote_id, assignee_name, preview, state, created_at, ended_at,
            last_event_at, raw_json, conversation_remote_id, conversation_number
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
        ON CONFLICT(chat_id) DO UPDATE SET
            beacon_id = excluded.beacon_id,
            mailbox_remote_id = excluded.mailbox_remote_id,
            customer_remote_id = excluded.customer_remote_id,
            customer_name = excluded.customer_name,
            assignee_remote_id = excluded.assignee_remote_id,
            assignee_name = excluded.assignee_name,
            preview = excluded.preview,
            state = CASE WHEN excluded.state = 'ended' THEN 'ended'
                         ELSE beacon_chats.state END,
            ended_at = COALESCE(excluded.ended_at, beacon_chats.ended_at),
            last_event_at = excluded.last_event_at,
            raw_json = excluded.raw_json,
            conversation_remote_id =
                COALESCE(excluded.conversation_remote_id, beacon_chats.conversation_remote_id),
            conversation_number =
                COALESCE(excluded.conversation_number, beacon_chats.conversation_number)",
        params![
            chat.id,
            chat.beacon_id,
            chat.mailbox_id,
            chat.customer.as_ref().map(|c| c.remote_id),
            chat.customer.as_ref().map(|c| c.name()),
            chat.assignee.as_ref().map(|a| a.remote_id),
            chat.assignee.as_ref().map(|a| a.name()),
            preview,
            state,
            chat.created_at,
            chat.ended_at,
            last_event_at,
            serde_json::to_string(chat).unwrap_or_default(),
            chat.conversation_remote_id,
            Option::<i64>::None,
        ],
    )?;
    Ok(())
}

fn last_message_text(chat: &HsChat) -> Option<String> {
    chat.events
        .iter()
        .rev()
        .find_map(|e| opt_str(e, "text").filter(|t| !t.is_empty()))
}

fn event_message_id(event: &Value) -> Option<String> {
    opt_str(event, "id")
        .or_else(|| {
            event
                .get("id")
                .and_then(|x| x.as_i64())
                .map(|n| n.to_string())
        })
        .filter(|s| !s.is_empty())
}

fn event_direction(event: &Value) -> &'static str {
    let kind = opt_str(event, "type").unwrap_or_default();
    if kind.contains("customer") {
        "customer"
    } else if kind.contains("agent") {
        "agent"
    } else {
        "system"
    }
}

fn record_event_messages(conn: &Connection, chat: &HsChat) -> Result<()> {
    for event in &chat.events {
        let Some(message_id) = event_message_id(event) else {
            continue;
        };
        conn.execute(
            "INSERT OR IGNORE INTO beacon_chat_messages
                (chat_id, message_id, direction, sender_name, text, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                chat.id,
                message_id,
                event_direction(event),
                opt_str(event, "author"),
                opt_str(event, "text"),
                opt_str(event, "createdAt"),
            ],
        )?;
    }
    Ok(())
}

pub fn list_chats(conn: &Connection, include_ended: bool) -> Result<Vec<MirrorChat>> {
    let filter = if include_ended {
        String::new()
    } else {
        "WHERE state != 'ended'".to_string()
    };
    let sql = format!(
        "SELECT chat_id, beacon_id, mailbox_remote_id, customer_remote_id, customer_name,
                assignee_remote_id, assignee_name, preview, state, created_at, ended_at,
                last_event_at, conversation_remote_id, conversation_number
           FROM beacon_chats {filter}
          ORDER BY (state = 'ended'), last_event_at DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(MirrorChat {
                chat_id: r.get(0)?,
                beacon_id: r.get(1)?,
                mailbox_remote_id: r.get(2)?,
                customer_remote_id: r.get(3)?,
                customer_name: r.get(4)?,
                assignee_remote_id: r.get(5)?,
                assignee_name: r.get(6)?,
                preview: r.get(7)?,
                state: r.get(8)?,
                created_at: r.get(9)?,
                ended_at: r.get(10)?,
                last_event_at: r.get(11)?,
                conversation_remote_id: r.get(12)?,
                conversation_number: r.get(13)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub(crate) fn chat_by_id(conn: &Connection, chat_id: &str) -> Result<Option<MirrorChat>> {
    let chat = conn
        .query_row(
            "SELECT chat_id, beacon_id, mailbox_remote_id, customer_remote_id, customer_name,
                    assignee_remote_id, assignee_name, preview, state, created_at, ended_at,
                    last_event_at, conversation_remote_id, conversation_number
               FROM beacon_chats WHERE chat_id = ?1",
            params![chat_id],
            |r| {
                Ok(MirrorChat {
                    chat_id: r.get(0)?,
                    beacon_id: r.get(1)?,
                    mailbox_remote_id: r.get(2)?,
                    customer_remote_id: r.get(3)?,
                    customer_name: r.get(4)?,
                    assignee_remote_id: r.get(5)?,
                    assignee_name: r.get(6)?,
                    preview: r.get(7)?,
                    state: r.get(8)?,
                    created_at: r.get(9)?,
                    ended_at: r.get(10)?,
                    last_event_at: r.get(11)?,
                    conversation_remote_id: r.get(12)?,
                    conversation_number: r.get(13)?,
                })
            },
        )
        .optional()?;
    Ok(chat)
}

pub fn messages_for(conn: &Connection, chat_id: &str) -> Result<Vec<MirrorMessage>> {
    let mut stmt = conn.prepare(
        "SELECT chat_id, message_id, direction, sender_name, text, created_at
           FROM beacon_chat_messages WHERE chat_id = ?1
          ORDER BY created_at, rowid",
    )?;
    let rows = stmt
        .query_map(params![chat_id], |r| {
            Ok(MirrorMessage {
                chat_id: r.get(0)?,
                message_id: r.get(1)?,
                direction: r.get(2)?,
                sender_name: r.get(3)?,
                text: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

static MESSAGE_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[must_use]
pub fn new_message_id() -> String {
    let nanos = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_default()
        .unsigned_abs();
    let pid = u64::from(std::process::id());
    let seq = MESSAGE_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let raw = (nanos.rotate_left(13) ^ (pid << 40) ^ seq.rotate_left(7)) | 0x8000_0000_0000_0000;
    format!(
        "{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}",
        (raw >> 32) as u32,
        ((raw >> 16) & 0xffff) as u16,
        ((raw >> 4) & 0xfff) as u16,
        (raw & 0xfff) as u16,
        seq
    )
}

pub(crate) fn record_outgoing_message(
    conn: &Connection,
    chat_id: &str,
    message_id: &str,
    text: &str,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO beacon_chat_messages
            (chat_id, message_id, direction, sender_name, text, created_at)
         VALUES (?1, ?2, 'agent', NULL, ?3, ?4)",
        params![chat_id, message_id, text, chrono::Utc::now().to_rfc3339()],
    )?;
    conn.execute(
        "UPDATE beacon_chats SET preview = ?2, last_event_at = ?3 WHERE chat_id = ?1",
        params![chat_id, text, chrono::Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct SendBlock {
    pub can_send: bool,
    pub chat_open: bool,
    pub held_by_other: bool,
    pub hint: String,
}

pub fn send_block(
    conn: &Connection,
    chat_id: &str,
    me_remote_id: Option<i64>,
) -> Result<SendBlock> {
    let Some(chat) = chat_by_id(conn, chat_id)? else {
        return Ok(SendBlock {
            can_send: false,
            chat_open: false,
            held_by_other: false,
            hint: "this chat is not in the mirror (yet)".to_string(),
        });
    };
    if chat.state == "ended" {
        let link = match (chat.conversation_number, chat.conversation_remote_id) {
            (Some(number), _) => format!("it became conversation #{number}"),
            (None, Some(remote)) => format!("it became conversation {remote}"),
            (None, None) => {
                "the conversation it became has not arrived in the mirror yet".to_string()
            }
        };
        return Ok(SendBlock {
            can_send: false,
            chat_open: false,
            held_by_other: false,
            hint: format!("this chat ended — {link}"),
        });
    }
    match (chat.assignee_remote_id, me_remote_id) {
        (Some(assignee), Some(me)) if assignee != me => Ok(SendBlock {
            can_send: false,
            chat_open: true,
            held_by_other: true,
            hint: format!(
                "held by {} — only the assigned agent can message; Help Scout answers 400",
                chat.assignee_name.as_deref().unwrap_or("another agent")
            ),
        }),
        (Some(_), None) => Ok(SendBlock {
            can_send: false,
            chat_open: true,
            held_by_other: true,
            hint: format!(
                "held by {} — only the assigned agent can message; Help Scout answers 400",
                chat.assignee_name.as_deref().unwrap_or("another agent")
            ),
        }),
        _ => {
            let hint = if chat.assignee_remote_id.is_some() {
                String::new()
            } else {
                "no agent is assigned — sending accepts this chat automatically".to_string()
            };
            Ok(SendBlock {
                can_send: true,
                chat_open: true,
                held_by_other: false,
                hint,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory connection");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn chat_payload(chat_id: &str) -> Value {
        serde_json::json!({
            "id": chat_id,
            "beaconId": "b-1",
            "mailboxId": 3,
            "state": "open",
            "createdAt": "2026-10-21T10:00:00Z",
            "customer": {"id": 77, "first": "Kim", "last": "One", "email": "kim@example.com"},
        })
    }

    fn replied_payload(chat_id: &str, message_id: &str, text: &str) -> Value {
        serde_json::json!({
            "id": chat_id,
            "mailboxId": 3,
            "state": "open",
            "events": [{
                "id": message_id,
                "type": "customer.message.created",
                "text": text,
                "createdAt": "2026-10-21T10:01:00Z",
                "author": "Kim One"
            }]
        })
    }

    #[test]
    fn m059_is_idempotent_and_creates_both_tables() {
        let conn = fresh_conn();
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM beacon_chats", [], |r| r.get(0))
            .expect("beacon_chats exists");
        assert_eq!(before, 0);
        apply_m059_beacon_chats(&conn).expect("re-apply m059");
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM beacon_chat_messages", [], |r| {
                r.get(0)
            })
            .expect("beacon_chat_messages exists");
        assert_eq!(n, 0);
    }

    #[test]
    fn created_event_upserts_the_mirror() {
        let conn = fresh_conn();
        ingest_event(&conn, "beacon.chat.created", &chat_payload("c-1")).expect("ingest created");
        let chat = chat_by_id(&conn, "c-1").expect("read").expect("row");
        assert_eq!(chat.state, "open");
        assert_eq!(chat.customer_remote_id, Some(77));
        assert_eq!(chat.customer_name.as_deref(), Some("Kim One"));
        assert_eq!(chat.mailbox_remote_id, 3);
    }

    #[test]
    fn reply_event_stores_the_message_and_dedupes() {
        let conn = fresh_conn();
        ingest_event(&conn, "beacon.chat.created", &chat_payload("c-1")).expect("created");
        let payload = replied_payload("c-1", "m-1", "hello there");
        ingest_event(&conn, "beacon.chat.customer.replied", &payload).expect("replied");
        ingest_event(&conn, "beacon.chat.customer.replied", &payload).expect("replayed");

        let messages = messages_for(&conn, "c-1").expect("messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id, "m-1");
        assert_eq!(messages[0].direction, "customer");
        assert_eq!(messages[0].text.as_deref(), Some("hello there"));

        let chat = chat_by_id(&conn, "c-1").expect("read").expect("row");
        assert_eq!(chat.preview.as_deref(), Some("hello there"));
    }

    #[test]
    fn assigned_event_moves_the_assignee() {
        let conn = fresh_conn();
        ingest_event(&conn, "beacon.chat.created", &chat_payload("c-1")).expect("created");
        let assigned = serde_json::json!({
            "id": "c-1",
            "mailboxId": 3,
            "state": "open",
            "assignee": {"id": 9, "first": "Ada", "last": "Byron"}
        });
        ingest_event(&conn, "beacon.chat.assigned", &assigned).expect("assigned");
        let chat = chat_by_id(&conn, "c-1").expect("read").expect("row");
        assert_eq!(chat.assignee_remote_id, Some(9));
        assert_eq!(chat.assignee_name.as_deref(), Some("Ada Byron"));
    }

    #[test]
    fn ended_event_closes_the_chat_and_keeps_it() {
        let conn = fresh_conn();
        ingest_event(&conn, "beacon.chat.created", &chat_payload("c-1")).expect("created");
        let ended = serde_json::json!({
            "id": "c-1",
            "mailboxId": 3,
            "state": "ended",
            "endedAt": "2026-10-21T10:05:00Z",
            "conversationId": 12345
        });
        ingest_event(&conn, "beacon.chat.ended", &ended).expect("ended");

        let chat = chat_by_id(&conn, "c-1").expect("read").expect("row");
        assert_eq!(chat.state, "ended");
        assert_eq!(chat.conversation_remote_id, Some(12345));

        let late = serde_json::json!({"id": "c-1", "mailboxId": 3, "state": "open"});
        ingest_event(&conn, "beacon.chat.customer.replied", &late).expect("late event");
        let chat = chat_by_id(&conn, "c-1").expect("read").expect("row");
        assert_eq!(chat.state, "ended");
    }

    #[test]
    fn list_orders_open_chats_first() {
        let conn = fresh_conn();
        ingest_event(&conn, "beacon.chat.created", &chat_payload("c-open")).expect("open");
        let ended = serde_json::json!({
            "id": "c-done", "mailboxId": 3, "state": "ended",
            "endedAt": "2026-10-21T10:05:00Z"
        });
        ingest_event(&conn, "beacon.chat.ended", &ended).expect("ended");

        let all = list_chats(&conn, true).expect("list all");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].chat_id, "c-open");

        let open_only = list_chats(&conn, false).expect("list open");
        assert_eq!(open_only.len(), 1);
        assert_eq!(open_only[0].chat_id, "c-open");
    }

    #[test]
    fn payload_without_a_chat_object_is_a_validation_error() {
        let conn = fresh_conn();
        let err = ingest_event(&conn, "beacon.chat.created", &serde_json::json!({"id": 1}))
            .expect_err("numeric id is not a chat uuid object");
        assert!(err.to_string().contains("no chat object"), "{err}");
        assert!(chat_by_id(&conn, "1").expect("read").is_none());
    }

    #[test]
    fn send_block_enforces_the_assigned_agent_rule() {
        let conn = fresh_conn();
        ingest_event(&conn, "beacon.chat.created", &chat_payload("c-1")).expect("created");
        let assigned = serde_json::json!({
            "id": "c-1", "mailboxId": 3, "state": "open",
            "assignee": {"id": 9, "first": "Ada"}
        });
        ingest_event(&conn, "beacon.chat.assigned", &assigned).expect("assigned");

        let held = send_block(&conn, "c-1", Some(42)).expect("block");
        assert!(!held.can_send);
        assert!(held.held_by_other);
        assert!(held.hint.contains("400"), "{}", held.hint);

        let mine = send_block(&conn, "c-1", Some(9)).expect("block");
        assert!(mine.can_send);
        assert!(mine.hint.is_empty());

        ingest_event(&conn, "beacon.chat.created", &chat_payload("c-2")).expect("second");
        let free = send_block(&conn, "c-2", Some(9)).expect("block");
        assert!(free.can_send);
        assert!(free.hint.contains("accepts"), "{}", free.hint);

        let ended = serde_json::json!({
            "id": "c-2", "mailboxId": 3, "state": "ended",
            "conversationId": 777
        });
        ingest_event(&conn, "beacon.chat.ended", &ended).expect("ended");
        let closed = send_block(&conn, "c-2", Some(9)).expect("block");
        assert!(!closed.can_send);
        assert!(closed.hint.contains("conversation 777"), "{}", closed.hint);
    }

    #[test]
    fn outgoing_messages_share_the_webhook_echo_key() {
        let conn = fresh_conn();
        ingest_event(&conn, "beacon.chat.created", &chat_payload("c-1")).expect("created");
        let mid = new_message_id();
        record_outgoing_message(&conn, "c-1", &mid, "we are on it").expect("record");

        let echo = serde_json::json!({
            "id": "c-1", "mailboxId": 3, "state": "open",
            "events": [{"id": mid, "type": "agent.message.created",
                        "text": "we are on it", "createdAt": "2026-10-21T10:02:00Z"}]
        });
        ingest_event(&conn, "beacon.chat.agent.replied", &echo).expect("echo");
        let messages = messages_for(&conn, "c-1").expect("messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].direction, "agent");
    }

    #[test]
    fn message_ids_are_unique_and_uuid_shaped() {
        let a = new_message_id();
        let b = new_message_id();
        assert_ne!(a, b);
        for id in [a, b] {
            let groups: Vec<usize> = id.split('-').map(str::len).collect();
            assert_eq!(groups, vec![8, 4, 4, 4, 12], "id {id} is not uuid-shaped");
        }
    }

    #[test]
    fn a_chat_missing_from_the_mirror_answers_none_and_guards_honestly() {
        let conn = fresh_conn();
        assert!(chat_by_id(&conn, "ghost-chat").expect("read").is_none());
        assert!(
            messages_for(&conn, "ghost-chat")
                .expect("messages")
                .is_empty()
        );
        let guard = send_block(&conn, "ghost-chat", Some(9)).expect("guard");
        assert!(!guard.can_send);
        assert!(guard.hint.contains("not in the mirror"), "{}", guard.hint);
    }
}
