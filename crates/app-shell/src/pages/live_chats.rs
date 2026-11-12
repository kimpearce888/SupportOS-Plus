use eframe::egui;
use spp_core::chats::{self, EndChatOutcome, MirrorChat, MirrorMessage, SendBlock};
use spp_core::db::DbHandle;

use super::{NavRequest, PageKey};
use crate::design::{self, BadgeTone, ButtonVariant, CalloutRole, MessageRole};
use crate::icons::Icon;

const LIST_COL_W: f32 = 340.0;
const SCROLL_H: f32 = 360.0;

/// AUD-H-class polish: chat timestamps render as compact wall times,
/// never raw ISO-8601 — the "2026-02-16T09:41:00.000Z" debug-register
/// strings read as internals, not product metadata.
fn compact_chat_time(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| {
            let mut out = iso.replacen('T', " ", 1);
            out.truncate(out.len().min(16));
            out
        })
}

#[derive(Clone)]
pub(crate) struct LiveChatsModel {
    chats: Vec<MirrorChat>,
    open: usize,
    ended: usize,
    me_remote_id: Option<i64>,
}

pub struct LiveChatsPage {
    model: Option<LiveChatsModel>,
    selected: Option<String>,
    show_ended: bool,
    draft: String,
    notice: Option<String>,
    detail_cache: Option<ChatDetailCache>,
}

struct ChatDetailCache {
    chat_id: String,
    at: std::time::Instant,
    guard: SendBlock,
    messages: Vec<MirrorMessage>,
}

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

    pub(crate) fn invalidate(&mut self) {
        self.model = None;
        self.detail_cache = None;
    }

    pub fn select_chat(&mut self, chat_id: &str) {
        self.selected = Some(chat_id.to_owned());
        self.draft.clear();
        self.notice = None;
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

    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        self.ensure_loaded(db);

        // UX-16: the subtitle explains the feature in user terms — the old
        // copy leaked webhook event names and API-shape trivia.
        design::page_header(
            ui,
            "Live chats",
            Some(
                "Real-time Beacon chats from your Help Scout widget arrive here \
                 automatically.",
            ),
            |ui| {
                let model = self.model.as_ref().expect("loaded above");
                design::page_counter(ui, &format!("{} open · {} ended", model.open, model.ended));
            },
        );

        ui.horizontal(|ui| {
            design::field_label(ui, "View:");
            if design::chip(ui, !self.show_ended, "Open chats").clicked() {
                self.show_ended = false;
            }
            if design::chip(ui, self.show_ended, "Ended chats").clicked() {
                self.show_ended = true;
            }
        });
        if let Some(notice) = &self.notice {
            design::status_line(ui, !notice.starts_with("could not"), notice);
        }
        ui.add_space(design::scale::space::SP2);

        let Some(model) = self.model.clone() else {
            ui.spinner();
            return None;
        };
        let visible: Vec<MirrorChat> = model
            .chats
            .iter()
            .filter(|c| self.show_ended || c.is_open())
            .cloned()
            .collect();

        if visible.is_empty() {
            // UX-16: the empty state carries the next step as a real button
            // instead of naming Settings in dead prose.
            if self.show_ended {
                design::empty_state(
                    ui,
                    Icon::Send,
                    "No ended chats",
                    "Chats land here after the visitor closes the conversation.",
                );
            } else if design::empty_state_cta_with(
                ui,
                Icon::Send,
                "No live chats yet",
                "Chats appear here in real time once the webhook relay is connected \
                 in Settings → Help Scout.",
                Icon::Sliders,
                "Open Settings",
            ) {
                return Some(NavRequest::Page(PageKey::Settings));
            }
            return None;
        }

        if ui.available_width() > 860.0 {
            // AUD-P2-18 (C14): the detail card's drill-downs (the customer
            // and the conversation the chat became) return the frame's
            // first navigation request.
            let opened = ui
                .horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(LIST_COL_W);
                        self.chat_list_ui(ui, &visible);
                    });
                    ui.add_space(design::scale::space::PAD_CARD);
                    ui.vertical(|ui| self.chat_detail_or_hint_ui(ui, db, &model))
                        .inner
                })
                .inner;
            if let Some(request) = opened {
                return Some(request);
            }
        } else {
            ui.vertical(|ui| {
                self.chat_list_ui(ui, &visible);
                ui.add_space(design::scale::space::SP2);
            });
            if let Some(request) = ui
                .vertical(|ui| self.chat_detail_or_hint_ui(ui, db, &model))
                .inner
            {
                return Some(request);
            }
        }
        None
    }

    fn chat_list_ui(&mut self, ui: &mut egui::Ui, visible: &[MirrorChat]) {
        design::section_title(ui, "Chats");
        let t = design::tokens(ui);
        egui::Frame::new()
            .fill(t.card)
            .stroke(egui::Stroke::new(1.0, t.border))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::same(design::scale::space::CLUSTER as i8))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("spp-live-chats-list")
                    .max_height(SCROLL_H)
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        for chat in visible {
                            self.chat_row_ui(ui, chat);
                        }
                    });
            });
    }

    fn chat_row_ui(&mut self, ui: &mut egui::Ui, chat: &MirrorChat) {
        let selected = self.selected.as_deref() == Some(chat.chat_id.as_str());
        let title = chat
            .customer_name
            .clone()
            .unwrap_or_else(|| "visitor".to_string());
        let preview = chat
            .preview
            .clone()
            .unwrap_or_else(|| "(no message yet)".to_string());
        let head = format!(
            "{} · {}",
            title,
            chat.last_event_at
                .as_deref()
                .map(compact_chat_time)
                .unwrap_or_else(|| "—".into())
        );
        let a11y = format!("chat with {title} — {}", chat.state);
        let t = design::tokens(ui);

        let row_w = ui.available_width();
        let text_w = (row_w - 2.0 * design::scale::space::PAD_TOOL).max(40.0);
        let head_galley = egui::WidgetText::from(
            egui::RichText::new(&head)
                .size(design::scale::text::BODY)
                .strong(),
        )
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            (text_w - 14.0).max(40.0),
            egui::TextStyle::Body,
        );
        let preview_galley = egui::WidgetText::from(
            egui::RichText::new(&preview)
                .size(design::scale::text::SMALL)
                .color(t.muted_foreground),
        )
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            text_w,
            egui::TextStyle::Small,
        );
        let height = head_galley.size().y + 2.0 + preview_galley.size().y + 2.0 * 8.0;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(row_w, height), egui::Sense::click());
        resp.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::Role::ListItem,
                ui.is_enabled(),
                selected,
                a11y.clone(),
            )
        });

        if selected {
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(6), t.selection);
            ui.painter().rect_filled(
                egui::Rect::from_min_size(rect.min, egui::vec2(2.0, rect.height())),
                egui::CornerRadius::ZERO,
                t.primary,
            );
        } else if resp.hovered() || resp.has_focus() {
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(6), t.accent);
        }

        let head_top = rect.min.y + 8.0;
        let head_pos = egui::pos2(rect.min.x + design::scale::space::PAD_TOOL + 14.0, head_top);
        let preview_pos = egui::pos2(
            rect.min.x + design::scale::space::PAD_TOOL,
            head_top + head_galley.size().y + 2.0,
        );
        ui.painter().circle_filled(
            egui::pos2(
                rect.min.x + design::scale::space::PAD_TOOL + 4.0,
                head_top + head_galley.size().y / 2.0,
            ),
            4.0,
            design::tone_solid(
                t,
                if chat.is_open() {
                    BadgeTone::Success
                } else {
                    BadgeTone::Neutral
                },
            ),
        );
        ui.painter().galley(head_pos, head_galley, t.foreground);
        ui.painter()
            .galley(preview_pos, preview_galley, t.muted_foreground);

        if resp.clicked() {
            self.select_chat(&chat.chat_id);
        }
    }

    fn chat_detail_or_hint_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        model: &LiveChatsModel,
    ) -> Option<NavRequest> {
        let Some(chat_id) = self.selected.clone() else {
            design::hint_text(ui, "select a chat to answer it");
            return None;
        };
        let Some(chat) = model.chats.iter().find(|c| c.chat_id == chat_id).cloned() else {
            design::hint_text(ui, "select a chat to answer it");
            return None;
        };
        self.chat_detail_ui(ui, db, chat)
    }

    fn chat_detail_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        chat: MirrorChat,
    ) -> Option<NavRequest> {
        let (guard, messages) = if self
            .detail_cache
            .as_ref()
            .is_some_and(|c| c.chat_id == chat.chat_id && c.at.elapsed() < DETAIL_TTL)
        {
            let cache = self.detail_cache.as_ref().expect("checked above");
            (cache.guard.clone(), cache.messages.clone())
        } else {
            let guard = db.with_read(|conn| chats::send_block(conn, &chat.chat_id, None));
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

        // AUD-P2-18 (C14): resolve the chat's cross-links to local mirror
        // rows before the card — the conversation an ended chat became,
        // and the customer behind it (both route the clicks).
        let conversation = chat
            .conversation_remote_id
            .and_then(|remote| conversation_link(db, remote));
        let customer_id = chat
            .customer_remote_id
            .and_then(|remote| customer_local_id(db, remote));

        // AUD-P2-18 (C14): the drill-down targets collect here — the card
        // body closure can't return, so the page returns them after.
        let mut nav: Option<NavRequest> = None;
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
                chat.last_event_at
                    .as_deref()
                    .map(compact_chat_time)
                    .unwrap_or_else(|| "—".into())
            )),
            |ui| {
                // AUD-P2-18 (C14): the subtitle's cross-links — the chat's
                // customer, and (once it ends) the conversation it became.
                // card_titled's hint slot renders plain text, so the links
                // live on their own row directly beneath it. Unresolved
                // remotes get the honest "not in the mirror yet" note —
                // the webhook knows them, the local mirror doesn't.
                ui.horizontal(|ui| {
                    let t = design::tokens(ui);
                    if let Some(customer_id) = customer_id {
                        let name = chat
                            .customer_name
                            .clone()
                            .unwrap_or_else(|| format!("customer {customer_id}"));
                        let response = ui
                            .add(
                                egui::Label::new(
                                    egui::RichText::new(format!("customer: {name}"))
                                        .size(design::scale::text::SMALL)
                                        .color(t.primary),
                                )
                                .sense(egui::Sense::click()),
                            )
                            .on_hover_text("open the customer's profile");
                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if response.clicked() && nav.is_none() {
                            nav = Some(NavRequest::OpenCustomer(customer_id));
                        }
                    } else if let Some(remote) = chat.customer_remote_id {
                        design::hint_text(ui, &format!("customer {remote} not in the mirror yet"));
                    }
                    if let Some((local_id, number)) = conversation {
                        let response = ui
                            .add(
                                egui::Label::new(
                                    egui::RichText::new(format!("became conversation #{number}"))
                                        .size(design::scale::text::SMALL)
                                        .color(t.primary),
                                )
                                .sense(egui::Sense::click()),
                            )
                            .on_hover_text("open the conversation this chat became in the Inbox");
                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if response.clicked() && nav.is_none() {
                            nav = Some(NavRequest::OpenConversation(local_id));
                        }
                    } else if let Some(remote) = chat.conversation_remote_id {
                        design::hint_text(
                            ui,
                            &format!("conversation {remote} not in the mirror yet"),
                        );
                    }
                });

                if messages.is_empty() {
                    design::hint_text(ui, "no messages stored for this chat yet");
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("spp-live-chat-messages")
                        .max_height(SCROLL_H)
                        .auto_shrink([false; 2])
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                        )
                        .show(ui, |ui| {
                            for message in &messages {
                                message_card_ui(ui, message);
                                ui.add_space(10.0);
                            }
                        });
                }

                // AUD-L5 polish: the state badge, the send-guard note,
                // and the end-chat action form ONE grouped card under
                // the transcript — a floating badge + stray callout +
                // loose button read as debug residue.
                ui.add_space(design::scale::space::SP2);
                design::card_titled(ui, "Conversation", None, |ui| {
                    design::badge(
                        ui,
                        &chat.state,
                        if chat.is_open() {
                            BadgeTone::Success
                        } else {
                            BadgeTone::Neutral
                        },
                    );
                    if !guard.hint.is_empty() {
                        ui.add_space(design::scale::space::SP2);
                        if guard.can_send {
                            design::callout(ui, CalloutRole::Neutral, &guard.hint);
                        } else {
                            design::callout(ui, CalloutRole::Err, &guard.hint);
                        }
                    }
                });

                // AUD-P2-18 (C14): the agent's end-chat action — placed with
                // the reply controls (the same place the send rule lives).
                // It stays available for open chats even when the reply box
                // is guarded (held by another agent): the outcome's own
                // refusal explains, instead of the button vanishing.
                if chat.is_open() {
                    ui.add_space(design::scale::space::SP2);
                    if design::button(ui, "End chat", ButtonVariant::Destructive)
                        .on_hover_text(
                            "ends the chat on this side — it moves to the Ended \
                             chats view; the platform keeps its own record",
                        )
                        .clicked()
                    {
                        self.end_chat(db, &chat.chat_id);
                    }
                }

                if guard.can_send {
                    ui.add_space(design::scale::space::SP2);
                    ui.add(
                        egui::TextEdit::multiline(&mut self.draft)
                            .desired_width(f32::INFINITY)
                            .hint_text("reply to the visitor…"),
                    );
                    ui.horizontal(|ui| {
                        design::hint_text(
                            ui,
                            "the message id is generated here — a retry can never double-send",
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !self.draft.trim().is_empty() {
                                if design::button(ui, "Send", ButtonVariant::Primary).clicked() {
                                    self.send(db, &chat.chat_id);
                                }
                            } else {
                                design::hint_text(ui, "type a reply to send");
                            }
                        });
                    });
                }
            },
        );
        nav
    }

    /// AUD-P2-18 (C14): the agent's end-chat action — routed through the
    /// core lane so the assignee guard (Help Scout refuses a foreign
    /// agent's writes) and the sticky ended upsert stay in one place. A
    /// chat the loaded view already knows as ended is a quiet no-op (the
    /// button never offers on one — this is the stale-click path).
    fn end_chat(&mut self, db: &DbHandle, chat_id: &str) {
        let me = self.model.as_ref().and_then(|m| m.me_remote_id);
        let known_ended = self
            .model
            .as_ref()
            .is_some_and(|m| m.chats.iter().any(|c| c.chat_id == chat_id && !c.is_open()));
        if known_ended {
            self.reload(db);
            self.detail_cache = None;
            return;
        }
        match db.with_write(|conn| chats::end_chat(conn, chat_id, me)) {
            Ok(EndChatOutcome::Ended) => {
                self.notice = Some("chat ended — it moved to the Ended chats view".into());
                self.reload(db);
                self.detail_cache = None;
            }
            Ok(EndChatOutcome::AlreadyEnded) => {
                // no-op: the mirror already says ended — just refresh the view.
                self.reload(db);
                self.detail_cache = None;
            }
            Ok(EndChatOutcome::Missing) => {
                self.notice = Some("this chat is not in the mirror (yet)".into());
            }
            Ok(EndChatOutcome::HeldByOther(hint)) => {
                self.notice = Some(format!("could not end the chat — {hint}"));
            }
            Err(e) => self.notice = Some(format!("could not end the chat: {e}")),
        }
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

fn message_card_ui(ui: &mut egui::Ui, message: &MirrorMessage) {
    let (role, role_label, fallback): (MessageRole, Option<String>, &str) =
        match message.direction.as_str() {
            "agent" => (MessageRole::Agent, None, "you"),
            "customer" => (MessageRole::Customer, None, "visitor"),
            other => (MessageRole::Customer, Some(other.to_string()), other),
        };
    let name = message
        .sender_name
        .clone()
        .unwrap_or_else(|| fallback.to_string());
    let time = message
        .created_at
        .as_deref()
        .map(compact_chat_time)
        .unwrap_or_else(|| "—".into());
    let time = time.as_str();
    let body = message.text.as_deref().unwrap_or("(no text)");
    design::message_card_with(
        ui,
        role,
        &name,
        role_label.as_deref(),
        time,
        |ui| {
            // AUD-L5: the body ALWAYS wraps (explicit wrap mode + long
            // token breaking, the inbox thread's treatment) — the old
            // plain label could render a clipped sliver that read as a
            // broken transcript.
            ui.add(
                egui::Label::new(
                    egui::RichText::new(spp_core::richtext::break_long_tokens(body, 40))
                        .size(design::scale::text::LEAD)
                        .color(design::tokens(ui).foreground),
                )
                .wrap(),
            );
        },
        |_ui| {},
    );
}

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

/// AUD-P2-18 (C14): resolve the conversation an ended chat became — the
/// chat stores the remote conversation id; the mirror's local row carries
/// the id the Inbox deep link routes to and the number the user cites.
/// `None` means the webhook referenced a conversation the mirror has not
/// synced yet (the honest fallback text renders instead of a link).
fn conversation_link(db: &DbHandle, remote_id: i64) -> Option<(i64, i64)> {
    db.with_read(|conn| {
        conn.query_row(
            "SELECT id, number FROM conversations
              WHERE remote_id = ?1 AND deleted_at IS NULL",
            rusqlite::params![remote_id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .ok()
    })
}

/// AUD-P2-18 (C14): resolve the chat's customer to the local mirror row
/// (the profile deep link routes to the local id). `None` = the mirror
/// has no such customer yet — the honest fallback text renders instead.
fn customer_local_id(db: &DbHandle, remote_id: i64) -> Option<i64> {
    db.with_read(|conn| {
        conn.query_row(
            "SELECT id FROM customers
              WHERE remote_id = ?1 AND deleted_at IS NULL",
            rusqlite::params![remote_id],
            |r| r.get::<_, i64>(0),
        )
        .ok()
    })
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

        let held = db
            .with_read(|conn| chats::send_block(conn, "chat-1", Some(9)))
            .expect("guard");
        assert!(!held.can_send && held.held_by_other);

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

    #[test]
    fn select_chat_is_the_row_click_state_transition() {
        let mut page = LiveChatsPage::new();
        page.draft = "draft text".into();
        page.notice = Some("stale notice".into());
        page.select_chat("chat-1");
        assert_eq!(page.selected.as_deref(), Some("chat-1"));
        assert!(page.draft.is_empty(), "the draft clears on selection");
        assert!(page.notice.is_none(), "a stale notice clears on selection");
    }

    /// AUD-P2-18 (C14): the conversation link resolves the chat's remote
    /// conversation id to the mirror's local row — the click routes to the
    /// Inbox reader; an unsynced remote resolves to nothing (the honest
    /// fallback renders instead of a dead link).
    #[test]
    fn the_conversation_link_resolves_and_misses_honestly() {
        // the fixture's seed_conversation: local id 1, remote 1001, number 501
        let db = fixture(vec![(
            "probe",
            1,
            "Export broken",
            "closed",
            "email",
            1,
            5,
            9,
        )]);
        db.with_write(|conn| {
            ingest(
                conn,
                serde_json::json!({
                    "id": "chat-1", "mailboxId": 3, "state": "ended",
                    "conversationId": 1001, "endedAt": "2026-10-21T10:05:00Z"
                }),
                "beacon.chat.ended",
            );
        });

        let link = conversation_link(&db, 1001);
        assert_eq!(
            link,
            Some((1, 501)),
            "the remote id resolves to the local row"
        );
        assert_eq!(
            link.map(|(local_id, _)| NavRequest::OpenConversation(local_id)),
            Some(NavRequest::OpenConversation(1)),
            "the click opens the conversation in the Inbox"
        );
        assert_eq!(
            conversation_link(&db, 424_242),
            None,
            "an unsynced remote resolves to no link — the honest fallback renders"
        );
    }

    /// AUD-P2-18 (C14): the customer link resolves the chat's remote
    /// customer id to the local profile row; soft-deleted customers and
    /// unsynced remotes resolve to nothing (the honest fallback renders).
    #[test]
    fn the_customer_link_resolves_and_misses_honestly() {
        // the fixture seeds customers 9 (remote 909) and 10 (remote 910)
        let db = fixture(vec![]);
        assert_eq!(customer_local_id(&db, 909), Some(9));
        assert_eq!(
            customer_local_id(&db, 909).map(NavRequest::OpenCustomer),
            Some(NavRequest::OpenCustomer(9)),
            "the click opens the customer's profile"
        );
        assert_eq!(
            customer_local_id(&db, 424_242),
            None,
            "an unsynced remote resolves to no link"
        );

        db.with_write(|conn| {
            conn.execute(
                "UPDATE customers SET deleted_at = datetime('now') WHERE remote_id = 910",
                [],
            )
            .expect("soft delete");
        });
        assert_eq!(
            customer_local_id(&db, 910),
            None,
            "a soft-deleted customer no longer resolves"
        );
    }

    /// AUD-P2-18 (C14): the end-chat guards — the connected user can end
    /// an open chat (the view reloads with it ended), another agent's chat
    /// refuses with the held-by hint, an already-ended chat is a no-op,
    /// and a chat outside the mirror says so.
    #[test]
    fn end_chat_guards_ending_refusal_and_the_no_op() {
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
                    "customer": {"id": 9, "first": "Kim", "last": "One"}
                }),
                "beacon.chat.created",
            );
            ingest(
                conn,
                serde_json::json!({
                    "id": "chat-2", "mailboxId": 3, "state": "open",
                    "assignee": {"id": 5, "first": "Ada"}
                }),
                "beacon.chat.assigned",
            );
        });
        let mut page = LiveChatsPage::new();
        page.reload(&db);

        // held by another agent: refused with the hint, the chat stays open
        page.end_chat(&db, "chat-2");
        assert!(
            page.notice
                .as_deref()
                .is_some_and(|n| n.starts_with("could not end the chat — held by")),
            "the refusal explains who holds the chat: {:?}",
            page.notice
        );
        let state: String = db
            .with_read(|conn| {
                conn.query_row(
                    "SELECT state FROM beacon_chats WHERE chat_id = 'chat-2'",
                    [],
                    |r| r.get(0),
                )
            })
            .expect("chat-2 row");
        assert_eq!(state, "open", "a held chat is not ended");

        // the connected user ends the unassigned chat: it lands as ended
        page.notice = None;
        page.end_chat(&db, "chat-1");
        assert_eq!(
            page.notice.as_deref(),
            Some("chat ended — it moved to the Ended chats view")
        );
        let state: String = db
            .with_read(|conn| {
                conn.query_row(
                    "SELECT state FROM beacon_chats WHERE chat_id = 'chat-1'",
                    [],
                    |r| r.get(0),
                )
            })
            .expect("chat-1 row");
        assert_eq!(state, "ended");
        assert_eq!(
            page.model.as_ref().expect("reloaded").ended,
            1,
            "the view reloads with the chat counted as ended"
        );
        assert!(page.detail_cache.is_none(), "the stale guard cache drops");

        // an already-ended chat: a no-op — no new notice
        page.notice = None;
        page.end_chat(&db, "chat-1");
        assert!(page.notice.is_none(), "AlreadyEnded stays silent");

        // a chat outside the mirror: said so, nothing written
        page.notice = None;
        page.end_chat(&db, "chat-999");
        assert_eq!(
            page.notice.as_deref(),
            Some("this chat is not in the mirror (yet)")
        );
    }
}
