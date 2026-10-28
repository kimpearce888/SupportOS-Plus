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
//!
//! RS-17 (wave A) rebuild on **Design Contract v1.0 archetype DS-A-04** —
//! the settings-page recipe (docs/audit/design-contract-v1.md §4):
//! PageHeader carrying the DS-C-01 counter pill (register A-6 — counts
//! beside the header) → the lab's view toolbar (a `view` field label plus
//! the two DS-C-06 chips — the conformed lab defines **no tab row** for
//! this page, the view switch IS the chip pair) → the two-col
//! master-detail composition (the lab's `DS.twoCol(…, wide-left)`: a
//! 340px chat-list column and the detail card, collapsing to one column
//! under 860px of content width) with messages as DS-C-20 role-typed
//! message cards and the send-rule hint rendered as a proper DS-C-15
//! callout (the DC register's live-chats row). The conformed lab
//! reference is `mockups/design-lab/pages-ds.js`
//! (`DS.definePage("live-chats", …)`).
//!
//! Behavior, queries, and data flow are unchanged (decisions.md D6:
//! presentation only). The list still reads `spp_core::chats::list_chats`
//! (open first, newest event first), the detail pane still re-queries the
//! mirror through the timestamped cache (audit fix M21: at most once per
//! second plus on selection/send/invalidation), the send guard still
//! resolves `chats::send_block` against the connected `accounts` row, and
//! send still enqueues the same `send_chat_message` job with the
//! caller-generated message id riding in the payload.

use eframe::egui;
use spp_core::chats::{self, MirrorChat, MirrorMessage, SendBlock};
use spp_core::db::DbHandle;

use crate::design::{self, BadgeTone, ButtonVariant, CalloutRole, MessageRole};
use crate::icons::Icon;

/// The lab's two-col split (`DS.twoCol(…, wide-left)`): the chat-list
/// column is 340px and the detail card takes the rest.
const LIST_COL_W: f32 = 340.0;
/// The lab's `.ds-scroll-360` cap — shared by the chat list and the
/// message history.
const SCROLL_H: f32 = 360.0;

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

    /// The chat-row act: select the chat, clear the draft and any stale
    /// notice — the exact state transition a list-row click has always
    /// performed. Public so harnesses can pre-select a chat like the
    /// lab's default state (`sel: "chat-1"`); the row click path calls
    /// the same transition.
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

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        // DS-C-01 PageHeader + counter pill (register A-6): the counts live
        // beside the header, exactly the lab's `[counter]` cluster — the
        // page has no other header action, so the action set is unchanged.
        design::page_header(
            ui,
            "Live chats",
            Some(
                "Beacon chats arrive through the five beacon.chat.* webhook events — \
                 Help Scout has no list endpoint, the webhook IS the data path.",
            ),
            |ui| {
                let model = self.model.as_ref().expect("loaded above");
                design::page_counter(ui, &format!("{} open · {} ended", model.open, model.ended));
            },
        );

        // The lab's view toolbar: the `view` field label + the two DS-C-06
        // chips. Same instant switch as before (no apply step) — the
        // conformed lab defines no tab row for this page.
        ui.horizontal(|ui| {
            design::field_label(ui, "view");
            if design::chip(ui, !self.show_ended, "open chats").clicked() {
                self.show_ended = false;
            }
            if design::chip(ui, self.show_ended, "show ended chats").clicked() {
                self.show_ended = true;
            }
        });
        // The send outcome: DS-C-15 status-line — the action-result role
        // (register B-1), same text the page has always shown.
        if let Some(notice) = &self.notice {
            design::status_line(ui, !notice.starts_with("could not"), notice);
        }
        ui.add_space(design::scale::space::SP2);

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
            // DS-C-14: why + next step. (The lab adds an "Open Settings"
            // CTA here; the shell keeps the hint-only form because the page
            // cannot navigate — no new workflow, decisions.md D6.)
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

        // The lab's `DS.twoCol(list, detailCard, wide-left)`: 340px list
        // column + detail card, collapsing to one column under the lab's
        // 860px content-width breakpoint.
        if ui.available_width() > 860.0 {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(LIST_COL_W);
                    self.chat_list_ui(ui, &visible);
                });
                ui.add_space(design::scale::space::PAD_CARD); // the lab's 14px grid gap
                ui.vertical(|ui| {
                    self.chat_detail_or_hint_ui(ui, db, &model);
                });
            });
        } else {
            ui.vertical(|ui| {
                self.chat_list_ui(ui, &visible);
                ui.add_space(design::scale::space::SP2);
                self.chat_detail_or_hint_ui(ui, db, &model);
            });
        }
    }

    /// The left column: the "Chats" section title over the lab's
    /// `.ds-sel-list` (1px border, r-lg, surface, 5px pad) scrolling rows.
    fn chat_list_ui(&mut self, ui: &mut egui::Ui, visible: &[MirrorChat]) {
        design::section_title(ui, "Chats");
        let t = design::tokens(ui);
        egui::Frame::new()
            .fill(t.card)
            .stroke(egui::Stroke::new(1.0, t.border))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::same(5))
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

    /// One conversation row (the lab's `DS.selRow` anatomy): two lines —
    /// state dot + `name · last event` (strong) over the preview (muted) —
    /// selected = neutral fill + the DC §2.2 2px primary left inset. The
    /// raw UTC last-event stamp is kept (webhook data, register note).
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
            chat.last_event_at.as_deref().unwrap_or("—")
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
            // Neutral fill + the selected-row inset (DC §2.2: --ds-primary).
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

        // Head line: the DS-C-06 state dot (same tone the row's badge would
        // render: open → success, ended → neutral) + the strong head text.
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

    /// The right column: the selected chat's detail card, or the
    /// select-a-chat hint when nothing is selected.
    fn chat_detail_or_hint_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, model: &LiveChatsModel) {
        let Some(chat_id) = self.selected.clone() else {
            design::hint_text(ui, "select a chat to answer it");
            return;
        };
        let Some(chat) = model.chats.iter().find(|c| c.chat_id == chat_id).cloned() else {
            design::hint_text(ui, "select a chat to answer it");
            return;
        };
        self.chat_detail_ui(ui, db, chat);
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
                        .max_height(SCROLL_H)
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            for message in &messages {
                                message_card_ui(ui, message);
                                ui.add_space(10.0); // the lab's .ds-msg margin
                            }
                        });
                }

                ui.add_space(design::scale::space::PAD_TOOL);
                design::badge(
                    ui,
                    &chat.state,
                    if chat.is_open() {
                        BadgeTone::Success
                    } else {
                        BadgeTone::Neutral
                    },
                );
                // The send rule in plain language, rendered as a proper
                // DS-C-15 callout (the DC register's live-chats row):
                // sendable = neutral context, blocked = err.
                if !guard.hint.is_empty() {
                    ui.add_space(design::scale::space::SP2);
                    if guard.can_send {
                        design::callout(ui, CalloutRole::Neutral, &guard.hint);
                    } else {
                        design::callout(ui, CalloutRole::Err, &guard.hint);
                    }
                }

                if guard.can_send {
                    ui.add_space(design::scale::space::SP2);
                    ui.add(
                        egui::TextEdit::multiline(&mut self.draft)
                            .desired_width(f32::INFINITY)
                            .hint_text("reply to the visitor…"),
                    );
                    // The lab's split tool-row: the no-double-send hint
                    // left, the send action right. The button still only
                    // exists while the draft is non-empty (same gating).
                    ui.horizontal(|ui| {
                        design::hint_text(
                            ui,
                            "the message id is generated here — a retry can never double-send",
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !self.draft.trim().is_empty() {
                                if design::button(ui, "send", ButtonVariant::Primary).clicked() {
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

/// One message as a DS-C-20 role-typed card (the lab renders mirror
/// messages through `DS.messageCard`): customer = surface card, agent =
/// the agent surface; an unknown direction keeps the raw direction as the
/// role label — nothing invented. Sender fallbacks are the page's existing
/// plain-language ones ("you" / "visitor").
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
    let time = message.created_at.as_deref().unwrap_or("—");
    let body = message.text.as_deref().unwrap_or("(no text)");
    design::message_card_with(
        ui,
        role,
        &name,
        role_label.as_deref(),
        time,
        |ui| {
            ui.label(
                egui::RichText::new(body)
                    .size(design::scale::text::LEAD)
                    .color(design::tokens(ui).foreground),
            );
        },
        |_ui| {},
    );
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
}
