//! Live chats page (P2, batch 15): the Beacon chat surface.
//!
//! Platform reality first: Help Scout has **no chat list endpoint** —
//! chats arrive through the five `beacon.chat.*` webhook events into the
//! m059 mirror (`spp_core::chats`), and the webhook IS the data path. The
//! page answers visitors while the chat is still open and points ended
//! chats at the conversation they became (`GET /v2/chats/{id}` answers
//! 404 once a chat collapses into a conversation, so the mirror's last
//! word is all there is).
//!
//! Sending uses the documented `PUT /v2/chats/{id}/messages/{uuid}`
//! contract through the `send_chat_message` job: the message id is
//! generated here, at enqueue time, so a retry after a timeout can never
//! double-send (the platform's 422 "id value is already in use" answer
//! is treated as delivered). The send box enforces the platform's
//! assigned-agent rule in plain language — unassigned chats note that
//! sending accepts them, chats held by another agent are blocked with
//! the 400 reason, ended chats point at their conversation.

use eframe::egui;
use spp_core::chats::{self, MirrorChat, MirrorMessage, SendBlock};
use spp_core::db::DbHandle;

use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

/// The cached page model (re-read on invalidation).
#[derive(Clone)]
pub struct LiveChatsModel {
    chats: Vec<MirrorChat>,
    open: usize,
    ended: usize,
    /// The connected user (`accounts` row), for the assigned-agent rule.
    me_remote_id: Option<i64>,
}

/// The interactive live chats page state.
pub struct LiveChatsPage {
    model: Option<LiveChatsModel>,
    selected: Option<String>,
    show_ended: bool,
    draft: String,
    notice: Option<String>,
    /// Audit fix (M21): cached detail-pane data — the send rule and the
    /// whole message history used to be re-read from the mirror on every
    /// rendered frame.
    detail_cache: Option<ChatDetailCache>,
}

/// Audit fix (M21): one detail pane's mirror reads, keyed by chat and
/// timestamped so the pane re-queries at most once per second (plus
/// immediately on selection, send and invalidation).
struct ChatDetailCache {
    chat_id: String,
    at: std::time::Instant,
    guard: SendBlock,
    messages: Vec<MirrorMessage>,
}

/// How long a detail-pane cache entry is reused before the next read.
const DETAIL_TTL: std::time::Duration = std::time::Duration::from_secs(1);

impl LiveChatsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            model: None,
            selected: None,
            show_ended: false,
            draft: String::new(),
            notice: None,
            detail_cache: None,
        }
    }

    /// Live-update invalidation (webhook events mark the page dirty
    /// through `pages::invalidated_pages`).
    pub fn invalidate(&mut self) {
        self.model = None;
        // Audit fix (M21): fresh mirror data may change the send rule.
        self.detail_cache = None;
    }

    fn reload(&mut self, db: &DbHandle) {
        let (chats, me_remote_id) = db.with_read(|conn| {
            let chats = chats::list_chats(conn, true).unwrap_or_default();
            let me_remote_id = conn
                .query_row(
                    "SELECT remote_id FROM accounts ORDER BY last_seen_at DESC LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .ok();
            (chats, me_remote_id)
        });
        let open = chats.iter().filter(|c| c.is_open()).count();
        let ended = chats.len() - open;
        self.model = Some(LiveChatsModel {
            chats,
            open,
            ended,
            me_remote_id,
        });
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.model.is_none() {
            self.reload(db);
        }
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        design::page_header(
            ui,
            "Live chats",
            Some(
                "Beacon chats arrive through the five beacon.chat.* webhook events — \
                 Help Scout has no list endpoint, the webhook IS the data path.",
            ),
            |ui| {
                let model = self.model.as_ref().expect("loaded above");
                ui.label(format!("{} open · {} ended", model.open, model.ended));
            },
        );

        ui.horizontal(|ui| {
            if design::chip(ui, !self.show_ended, "open chats").clicked() {
                self.show_ended = false;
            }
            if design::chip(ui, self.show_ended, "show ended chats").clicked() {
                self.show_ended = true;
            }
        });
        if let Some(notice) = &self.notice {
            design::hint_text(ui, notice);
        }
        design::divider(ui);

        let Some(model) = self.model.clone() else {
            ui.spinner();
            return;
        };
        let visible: Vec<MirrorChat> = model
            .chats
            .iter()
            .filter(|c| self.show_ended || c.is_open())
            .cloned()
            .collect();

        if visible.is_empty() {
            design::empty_state(
                ui,
                Icon::Send,
                if self.show_ended {
                    "No ended chats"
                } else {
                    "No live chats yet"
                },
                "Chats appear here the moment the beacon.chat.created webhook lands — \
                 connect the webhook relay (Settings) and they will arrive on their own.",
            );
            return;
        }

        ui.horizontal(|ui| {
            // The chat list.
            ui.vertical(|ui| {
                design::section_title(ui, "Chats");
                egui::ScrollArea::vertical()
                    .id_salt("spp-live-chats-list")
                    .max_height(480.0)
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        for chat in &visible {
                            let selected = self.selected.as_deref() == Some(chat.chat_id.as_str());
                            let title = chat
                                .customer_name
                                .clone()
                                .unwrap_or_else(|| "visitor".to_string());
                            let preview = chat
                                .preview
                                .clone()
                                .unwrap_or_else(|| "(no message yet)".to_string());
                            let line = format!(
                                "{} · {}\n{}",
                                title,
                                chat.last_event_at.as_deref().unwrap_or("—"),
                                preview
                            );
                            let row = ui.selectable_label(selected, line);
                            if row.clicked() {
                                self.selected = Some(chat.chat_id.clone());
                                self.draft.clear();
                                self.notice = None;
                            }
                        }
                    });
            });

            ui.separator();

            // The selected chat: message history + the send box.
            ui.vertical(|ui| {
                let Some(chat_id) = self.selected.clone() else {
                    design::hint_text(ui, "select a chat to answer it");
                    return;
                };
                let Some(chat) = model.chats.iter().find(|c| c.chat_id == chat_id).cloned() else {
                    design::hint_text(ui, "select a chat to answer it");
                    return;
                };
                self.chat_detail_ui(ui, db, chat);
            });
        });
    }

    fn chat_detail_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, chat: MirrorChat) {
        // Audit fix (M21): serve the send rule + message history from the
        // timestamped cache when it is fresh; re-read the mirror at most
        // once per second (and on selection/send/invalidation, which
        // drop the cache) instead of on every rendered frame.
        let (guard, messages) = if self
            .detail_cache
            .as_ref()
            .is_some_and(|c| c.chat_id == chat.chat_id && c.at.elapsed() < DETAIL_TTL)
        {
            let cache = self.detail_cache.as_ref().expect("checked above");
            (cache.guard.clone(), cache.messages.clone())
        } else {
            let guard = db.with_read(|conn| chats::send_block(conn, &chat.chat_id, None));
            // The rule resolves against the connected user, not a guess.
            let guard = match (guard, self.model.as_ref().and_then(|m| m.me_remote_id)) {
                (Ok(g), me) => {
                    if g.held_by_other && me.is_some() {
                        db.with_read(|conn| chats::send_block(conn, &chat.chat_id, me).unwrap_or(g))
                    } else {
                        g
                    }
                }
                (Err(_), _) => SendBlock {
                    can_send: false,
                    chat_open: chat.is_open(),
                    held_by_other: false,
                    hint: "the send rule could not be read from the mirror".to_string(),
                },
            };
            let messages = db
                .with_read(|conn| chats::messages_for(conn, &chat.chat_id))
                .unwrap_or_default();
            self.detail_cache = Some(ChatDetailCache {
                chat_id: chat.chat_id.clone(),
                at: std::time::Instant::now(),
                guard: guard.clone(),
                messages: messages.clone(),
            });
            (guard, messages)
        };

        design::card_titled(
            ui,
            &format!(
                "Chat with {}",
                chat.customer_name.as_deref().unwrap_or("a visitor")
            ),
            Some(&format!(
                "state: {} · mailbox #{} · last event {}",
                chat.state,
                chat.mailbox_remote_id,
                chat.last_event_at.as_deref().unwrap_or("—")
            )),
            |ui| {
                // Audit fix (M21): the message history comes from the
                // timestamped cache resolved above — re-reading the mirror
                // here on every rendered frame was the hot path.
                if messages.is_empty() {
                    design::hint_text(ui, "no messages stored for this chat yet");
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("spp-live-chat-messages")
                        .max_height(240.0)
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            for message in &messages {
                                message_row_ui(ui, message);
                            }
                        });
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    design::badge(
                        ui,
                        &chat.state,
                        if chat.is_open() {
                            BadgeTone::Success
                        } else {
                            BadgeTone::Neutral
                        },
                    );
                });
                if !guard.hint.is_empty() {
                    if guard.can_send {
                        design::hint_text(ui, &guard.hint);
                    } else {
                        design::error_text(ui, &guard.hint);
                    }
                }

                if guard.can_send {
                    ui.add_space(4.0);
                    ui.add(
                        egui::TextEdit::multiline(&mut self.draft)
                            .desired_width(f32::INFINITY)
                            .hint_text("reply to the visitor…"),
                    );
                    ui.horizontal(|ui| {
                        if !self.draft.trim().is_empty() {
                            if design::button(ui, "send", ButtonVariant::Primary).clicked() {
                                self.send(db, &chat.chat_id);
                            }
                        } else {
                            design::hint_text(ui, "type a reply to send");
                        }
                        design::hint_text(
                            ui,
                            "the message id is generated here — a retry can never double-send",
                        );
                    });
                }
            },
        );
    }

    fn send(&mut self, db: &DbHandle, chat_id: &str) {
        let text = self.draft.trim().to_string();
        if text.is_empty() {
            return;
        }
        let (payload, message_id) = send_payload(chat_id, &text);
        match db.with_write(|conn| {
            spp_core::jobs::enqueue_on(conn, "sync", "send_chat_message", &payload, 1, 2)
        }) {
            Ok(_) => {
                self.draft.clear();
                // Audit fix (M21): the sent message reaches the mirror via
                // the platform echo — drop the cache so the pane re-reads
                // instead of showing the stale history for a second.
                self.detail_cache = None;
                self.notice = Some(format!(
                    "message queued for delivery (id {message_id}) — the platform's \
                     agent.replied echo lands it in the mirror"
                ));
            }
            Err(e) => {
                self.notice = Some(format!("could not queue the message: {e}"));
            }
        }
    }
}

/// One message row: who sent it, when, and the text — nothing invented.
fn message_row_ui(ui: &mut egui::Ui, message: &MirrorMessage) {
    let who = match message.direction.as_str() {
        "agent" => format!(
            "agent · {}",
            message.sender_name.as_deref().unwrap_or("you")
        ),
        "customer" => format!(
            "customer · {}",
            message.sender_name.as_deref().unwrap_or("visitor")
        ),
        other => other.to_string(),
    };
    let line = format!(
        "{} — {}\n{}",
        who,
        message.created_at.as_deref().unwrap_or("—"),
        message.text.as_deref().unwrap_or("(no text)")
    );
    let strong = egui::RichText::new(line).weak();
    ui.label(strong);
    ui.add_space(2.0);
}

/// Build the `send_chat_message` job payload: the message id is
/// generated once, here, so every retry of the job reuses it (the
/// platform's 422 "id value is already in use" answer is the
/// no-double-send guarantee working, not an error).
#[must_use]
fn send_payload(chat_id: &str, text: &str) -> (String, String) {
    let message_id = chats::new_message_id();
    let payload = serde_json::json!({
        "chatId": chat_id,
        "messageId": message_id,
        "text": text,
    })
    .to_string();
    (payload, message_id)
}

impl Default for LiveChatsPage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    fn ingest(conn: &rusqlite::Connection, payload: serde_json::Value, event: &str) {
        spp_core::chats::ingest_event(conn, event, &payload).expect("ingest chat event");
    }

    #[test]
    fn reload_loads_ingested_chats_and_the_connected_user() {
        let db = fixture(vec![]);
        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO accounts (remote_id, plan, raw_json, last_seen_at)
                 VALUES (9, 'owner', '{}', datetime('now'))",
                [],
            )
            .expect("seed the connected user");
            ingest(
                conn,
                serde_json::json!({
                    "id": "chat-1", "mailboxId": 3, "state": "open",
                    "customer": {"id": 7, "first": "Kim", "last": "One"}
                }),
                "beacon.chat.created",
            );
            ingest(
                conn,
                serde_json::json!({
                    "id": "chat-2", "mailboxId": 3, "state": "ended",
                    "conversationId": 42, "endedAt": "2026-10-21T10:05:00Z"
                }),
                "beacon.chat.ended",
            );
        });

        let mut page = LiveChatsPage::new();
        page.reload(&db);
        let model = page.model.expect("model loaded");
        assert_eq!(model.chats.len(), 2);
        assert_eq!(model.open, 1);
        assert_eq!(model.ended, 1);
        assert_eq!(model.me_remote_id, Some(9));
    }

    #[test]
    fn send_payload_generates_the_message_id_once() {
        let (payload, message_id) = send_payload("chat-1", "on it");
        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("json payload");
        assert_eq!(parsed["chatId"], "chat-1");
        assert_eq!(parsed["text"], "on it");
        assert_eq!(parsed["messageId"], message_id.as_str());
        // The id rides IN the payload: a retried job reuses it, which is
        // the no-double-send guarantee.
        let groups: Vec<usize> = message_id.split('-').map(str::len).collect();
        assert_eq!(groups, vec![8, 4, 4, 4, 12], "uuid-shaped: {message_id}");
    }

    #[test]
    fn the_send_guard_is_resolved_from_the_mirror() {
        let db = fixture(vec![]);
        db.with_write(|conn| {
            ingest(
                conn,
                serde_json::json!({
                    "id": "chat-1", "mailboxId": 3, "state": "open",
                    "assignee": {"id": 5, "first": "Ada"}
                }),
                "beacon.chat.assigned",
            );
        });

        // Another agent's chat is blocked with the platform's reason.
        let held = db
            .with_read(|conn| chats::send_block(conn, "chat-1", Some(9)))
            .expect("guard");
        assert!(!held.can_send && held.held_by_other);

        // Our own chat is sendable with no rule note.
        let mine = db
            .with_read(|conn| chats::send_block(conn, "chat-1", Some(5)))
            .expect("guard");
        assert!(mine.can_send && mine.hint.is_empty());
    }

    #[test]
    fn invalidation_forces_a_reload() {
        let db = fixture(vec![]);
        let mut page = LiveChatsPage::new();
        page.reload(&db);
        assert!(page.model.is_some());
        page.invalidate();
        assert!(page.model.is_none());
        page.reload(&db);
        assert!(page.model.is_some());
    }
}
