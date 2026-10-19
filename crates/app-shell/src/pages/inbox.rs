//! Inbox page (port of the web `pages/inbox.rs`).
//!
//! The list and detail read through the typed core functions
//! (`spp_core::inbox::list_conversations` / `get_conversation`) — the exact
//! same queries the `/api/conversations` routes serve — so the native page
//! shows precisely what the web page showed, without the HTTP hop.
//!
//! Mutations are **local-first through the durable outbound pipeline**:
//! the HTTP ops (`op_send_reply`, `op_add_note`, `op_change_status`,
//! `op_assign`) enqueue an `outbound_jobs` row and then call the live
//! provider. The native port reproduces the enqueue side exactly (same
//! kinds, payloads, and the `reply:{remote}:{send|draft}:{sha}` duplicate
//! key) and applies the local row changes through the typed
//! `spp_core::ticket_ops` executor, so the UI stays consistent while the
//! queued jobs are executed by the sync/worker infrastructure that lands
//! with the loopback-listener batch. Notes additionally create mention
//! notifications synchronously (the same `Mentioned` records the
//! notification sweep creates for synced notes).
//!
//! The composer keeps the web features: reply/internal-note tabs, cc/bcc,
//! Ctrl+Enter send, and the `@`-mention autocomplete popup.

use eframe::egui;
use rusqlite::Connection;
use serde_json::json;
use sha2::{Digest, Sha256};
use spp_core::conversation_ops::{create_outbound_job_keyed, set_outbound_status};
use spp_core::db::DbHandle;
use spp_core::eventing::EventBus;
use spp_core::inbox::{ConversationDetail, ConversationListItem, InboxFilters};
use spp_core::ticket_ops::{OperationResult, TicketOperation};

use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;
use crate::mention::{
    Directory, apply_completion, caret_query, char_index_to_byte, filter_matches,
};

/// Conversation status views offered by the filter bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewFilter {
    All,
    Active,
    Pending,
    Closed,
}

impl ViewFilter {
    const ALL: [Self; 4] = [Self::All, Self::Active, Self::Pending, Self::Closed];

    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Active => "Active",
            Self::Pending => "Pending",
            Self::Closed => "Closed",
        }
    }

    fn status(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Active => Some("active"),
            Self::Pending => Some("pending"),
            Self::Closed => Some("closed"),
        }
    }
}

/// Editable filter-bar state (the web page's URL query params).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterBar {
    pub view: Option<ViewFilter>,
    pub mailbox: Option<i64>,
    pub channel: Option<String>,
    pub tag: Option<String>,
    pub query: Option<String>,
    pub number: Option<i64>,
    /// Operations-tile drill-down (`unassigned`, `customer_waiting`, …).
    /// The web operations tiles linked `/inbox?view={tile}` which the route
    /// read as a status filter (a dead-end query); the port applies the
    /// tile's real conversation fragment instead — the same SQL the
    /// `/api/conversations?ops=` path compiles.
    pub ops: Option<String>,
}

/// Map the filter bar to the core `InboxFilters`.
///
/// The operations-tile fragment needs the waiting-threshold setting, so
/// the bar compiles against a connection (same as the HTTP route).
#[must_use]
pub fn filters_from_bar(bar: &FilterBar, conn: &rusqlite::Connection) -> InboxFilters {
    let mut filters = InboxFilters {
        status: bar
            .view
            .unwrap_or(ViewFilter::All)
            .status()
            .map(str::to_string),
        mailbox_id: bar.mailbox,
        assignee_id: None,
        customer_id: None,
        priority: None,
        channel: bar.channel.clone(),
        tag: bar.tag.clone(),
        query: bar.query.clone(),
        number: bar.number,
        limit: Some(50),
        offset: Some(0),
        extra_where: None,
        extra_params: Vec::new(),
    };
    if let Some(ops) = bar.ops.as_deref().filter(|s| !s.is_empty())
        && spp_core::operations::is_conversation_ops_tile(ops)
        && let Some((frag_sql, frag_params)) = spp_core::operations::tile_fragment(
            ops,
            spp_core::operations::waiting_threshold_minutes(conn),
        )
    {
        filters.extra_where = Some(frag_sql);
        filters.extra_params = frag_params;
    }
    filters
}

/// Composer mode (web: reply vs internal note tabs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerMode {
    Reply,
    Note,
}

/// Duplicate-send key: `reply:{remote_id}:{send|draft}:{sha256(text)[..24]}`
/// — identical to the HTTP `op_send_reply` key.
#[must_use]
pub fn reply_key(remote_id: i64, draft: bool, text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    format!(
        "reply:{}:{}:{}",
        remote_id,
        if draft { "draft" } else { "send" },
        &hex::encode(digest)[..24]
    )
}

/// Outcome of a composer/action submit, surfaced in the status line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionOutcome {
    Success(String),
    Rejected(String),
}

impl ActionOutcome {
    fn success_msg(self) -> Option<String> {
        match self {
            Self::Success(m) => Some(m),
            Self::Rejected(m) => Some(m),
        }
    }
}

/// Users available for assignment: `(id, display name)`.
#[must_use]
pub fn assignable_users(conn: &Connection) -> Vec<(i64, String)> {
    conn.prepare("SELECT id, first_name || ' ' || last_name FROM users ORDER BY id")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                let id: i64 = r.get(0)?;
                let name: String = r.get(1).unwrap_or_default();
                Ok((id, name.trim().to_string()))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

/// Tags offered by the tag filter.
#[must_use]
pub fn tag_names(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT name FROM tags ORDER BY name")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| r.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

/// Insert a local note thread row (the row sync would bring back).
fn insert_local_note(
    conn: &Connection,
    conversation_id: i64,
    body: &str,
    actor_id: Option<i64>,
) -> spp_core::Result<i64> {
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, body_text, from_type, state,
             created_by_user_id, created_at)
         VALUES (?1, 'note', ?2, 'user', 'published', ?3,
                 strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        rusqlite::params![conversation_id, body, actor_id],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Queue an outbound job, returning its id (mirrors the HTTP ops).
fn queue_job(
    conn: &Connection,
    kind: &str,
    payload: &serde_json::Value,
    conversation_id: Option<i64>,
    idempotency_key: Option<&str>,
) -> i64 {
    let id = create_outbound_job_keyed(conn, kind, payload, conversation_id, idempotency_key);
    // Jobs are born "queued"; "sending" marks a provider call in flight,
    // which only the worker/loopback batch will set.
    set_outbound_status(conn, id, "queued", None);
    id
}

fn guard_eval_mode(conn: &Connection) -> Result<(), String> {
    if spp_core::conversation_ops::eval_mode_on(conn) {
        Err("AI evaluation mode is ON: no notes, replies or status changes are sent.".into())
    } else {
        Ok(())
    }
}

fn emit_updated(bus: &EventBus, conversation_id: i64, number: i64) {
    bus.emit(&spp_core::events::ServerEvent::conversation_updated(
        Some(conversation_id),
        Some(number),
        None,
        None,
        "local_action",
    ));
}

/// The interactive inbox page state.
pub struct InboxPage {
    pub bar: FilterBar,
    view_pick: ViewFilter,
    mailbox_pick: Option<i64>,
    channel_pick: Option<String>,
    tag_pick: Option<String>,
    search_edit: String,
    number_edit: String,
    list: Vec<ConversationListItem>,
    total: u32,
    selected: Option<i64>,
    detail: Option<ConversationDetail>,
    composer_mode: ComposerMode,
    composer_body: String,
    composer_cc: String,
    composer_bcc: String,
    mention_active: usize,
    mention_caret: usize,
    users: Vec<(i64, String)>,
    mailboxes: Vec<(i64, String)>,
    tags: Vec<String>,
    directory: Directory,
    status: Option<String>,
    assign_pick: Option<i64>,
    status_message: Option<String>,
}

impl InboxPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bar: FilterBar::default(),
            view_pick: ViewFilter::All,
            mailbox_pick: None,
            channel_pick: None,
            tag_pick: None,
            search_edit: String::new(),
            number_edit: String::new(),
            list: Vec::new(),
            total: 0,
            selected: None,
            detail: None,
            composer_mode: ComposerMode::Reply,
            composer_body: String::new(),
            composer_cc: String::new(),
            composer_bcc: String::new(),
            mention_active: 0,
            mention_caret: 0,
            users: Vec::new(),
            mailboxes: Vec::new(),
            tags: Vec::new(),
            directory: Directory::default(),
            status: None,
            assign_pick: None,
            status_message: None,
        }
    }

    /// Live-update invalidation: reload the list (and the open detail).
    pub fn invalidate(&mut self) {
        self.list.clear();
        self.detail = None;
    }

    fn reload_refs(&mut self, db: &DbHandle) {
        let (users, mailboxes, tags, directory) = db.with_read(|conn| {
            let directory = Directory::from_core(
                &spp_core::mentions::build_mention_directory(conn).unwrap_or_default(),
            );
            (
                assignable_users(conn),
                crate::pages::dashboard::mailboxes(conn),
                tag_names(conn),
                directory,
            )
        });
        self.users = users;
        self.mailboxes = mailboxes;
        self.tags = tags;
        self.directory = directory;
    }

    fn reload_list(&mut self, db: &DbHandle) {
        if self.mailboxes.is_empty() {
            self.reload_refs(db);
        }
        let result = db.with_read(|conn| {
            let filters = filters_from_bar(&self.bar, conn);
            spp_core::inbox::list_conversations(conn, &filters)
        });
        match result {
            Ok((items, total)) => {
                self.list = items;
                self.total = total;
                self.status = None;
            }
            Err(e) => self.status = Some(e.to_string()),
        }
    }

    fn reload_detail(&mut self, db: &DbHandle) {
        let id = self.selected;
        let detail = id.map(|id| db.with_read(|conn| spp_core::inbox::get_conversation(conn, id)));
        self.detail = detail.and_then(|result| result.ok()).flatten();
    }

    /// Select a conversation and load its detail.
    pub fn select(&mut self, db: &DbHandle, id: i64) {
        self.selected = Some(id);
        self.composer_body.clear();
        self.composer_cc.clear();
        self.composer_bcc.clear();
        self.status_message = None;
        self.reload_detail(db);
    }

    /// Submit the composer (reply or note) — see the module docs for the
    /// local-first outbound semantics.
    pub fn submit_composer(&mut self, db: &DbHandle, bus: &EventBus) -> ActionOutcome {
        let body = self.composer_body.trim().to_string();
        if body.is_empty() {
            return ActionOutcome::Rejected("Composer is empty.".into());
        }
        let Some(detail) = self.detail.clone() else {
            return ActionOutcome::Rejected("No conversation selected.".into());
        };
        let outcome = match self.composer_mode {
            ComposerMode::Reply => submit_reply(
                db,
                bus,
                &detail,
                &body,
                &self.composer_cc,
                &self.composer_bcc,
            ),
            ComposerMode::Note => submit_note(db, bus, &detail, &body),
        };
        if let Some(msg) = outcome.clone().success_msg() {
            self.status_message = Some(msg);
            self.composer_body.clear();
            self.composer_cc.clear();
            self.composer_bcc.clear();
        }
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

    /// Change the conversation status (local write + queued remote job).
    pub fn change_status(&mut self, db: &DbHandle, bus: &EventBus, status: &str) -> ActionOutcome {
        let Some(detail) = self.detail.clone() else {
            return ActionOutcome::Rejected("No conversation selected.".into());
        };
        let outcome = mutate(
            db,
            bus,
            &detail,
            TicketOperation::ChangeStatus {
                conversation_remote_id: detail.remote_id,
                new_status: status.to_string(),
                actor_type: "user".into(),
                actor_id: None,
            },
            "update_status",
            &json!({ "conversationId": detail.id, "status": status }),
        );
        if let Some(msg) = outcome.clone().success_msg() {
            self.status_message = Some(msg);
        }
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

    /// Assign the conversation (local write + queued remote job).
    pub fn assign(&mut self, db: &DbHandle, bus: &EventBus, user_id: Option<i64>) -> ActionOutcome {
        let Some(detail) = self.detail.clone() else {
            return ActionOutcome::Rejected("No conversation selected.".into());
        };
        let outcome = mutate(
            db,
            bus,
            &detail,
            TicketOperation::Assign {
                conversation_remote_id: detail.remote_id,
                assignee_local_id: user_id,
                actor_type: "user".into(),
                actor_id: None,
            },
            "assign",
            &json!({ "conversationId": detail.id, "userId": user_id }),
        );
        if let Some(msg) = outcome.clone().success_msg() {
            self.status_message = Some(msg);
        }
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, bus: &EventBus) {
        if self.mailboxes.is_empty() {
            self.reload_refs(db);
        }
        if self.list.is_empty() {
            self.reload_list(db);
        }

        let subtitle = format!("{} of {} conversations", self.list.len(), self.total);
        design::page_header(ui, "Inbox", Some(&subtitle), |ui| {
            if design::button(ui, "reset filters", ButtonVariant::Ghost)
                .on_hover_text("clear every filter and reload")
                .clicked()
            {
                self.bar = FilterBar::default();
                self.view_pick = ViewFilter::All;
                self.mailbox_pick = None;
                self.channel_pick = None;
                self.tag_pick = None;
                self.search_edit.clear();
                self.number_edit.clear();
                self.reload_list(db);
            }
        });

        if let Some(ops) = self.bar.ops.clone() {
            let t = design::tokens(ui);
            ui.horizontal(|ui| {
                let (id, rect) = ui.allocate_space(egui::vec2(16.0, 16.0));
                Icon::Filter.paint_at(ui.painter(), rect, t.info_foreground);
                ui.advance_cursor_after_rect(rect);
                let _ = id;
                ui.label(
                    egui::RichText::new(format!("Operations drill-down: {ops}"))
                        .small()
                        .color(t.info_foreground),
                );
                if design::icon_button(
                    ui,
                    Icon::X,
                    ButtonVariant::Ghost,
                    "clear the operations filter",
                )
                .clicked()
                {
                    self.bar.ops = None;
                    self.reload_list(db);
                }
            });
            ui.add_space(4.0);
        }

        // View chips.
        ui.horizontal(|ui| {
            for view in ViewFilter::ALL {
                if design::chip(ui, self.view_pick == view, view.label()).clicked() {
                    self.view_pick = view;
                    self.bar.view = Some(view);
                    self.reload_list(db);
                }
            }
        });
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("inbox-mailbox")
                .width(170.0)
                .selected_text(
                    self.mailbox_pick
                        .and_then(|id| {
                            self.mailboxes
                                .iter()
                                .find(|(mid, _)| *mid == id)
                                .map(|(_, name)| name.clone())
                        })
                        .unwrap_or_else(|| "All mailboxes".into()),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.mailbox_pick, None, "All mailboxes");
                    for (id, name) in self.mailboxes.clone() {
                        ui.selectable_value(&mut self.mailbox_pick, Some(id), name);
                    }
                });
            egui::ComboBox::from_id_salt("inbox-channel")
                .width(130.0)
                .selected_text(
                    self.channel_pick
                        .clone()
                        .map(|c| match c.as_str() {
                            "email" => "Email".to_string(),
                            "chat" => "Chat".to_string(),
                            other => other.to_string(),
                        })
                        .unwrap_or_else(|| "All channels".into()),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.channel_pick, None, "All channels");
                    ui.selectable_value(&mut self.channel_pick, Some("email".into()), "Email");
                    ui.selectable_value(&mut self.channel_pick, Some("chat".into()), "Chat");
                });
            egui::ComboBox::from_id_salt("inbox-tag")
                .width(150.0)
                .selected_text(self.tag_pick.clone().unwrap_or_else(|| "Any tag".into()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.tag_pick, None, "Any tag");
                    for tag in self.tags.clone() {
                        ui.selectable_value(&mut self.tag_pick, Some(tag.clone()), tag);
                    }
                });
            if design::button(ui, "apply", ButtonVariant::Secondary)
                .on_hover_text("apply the mailbox, channel and tag picks")
                .clicked()
            {
                self.bar.mailbox = self.mailbox_pick;
                self.bar.channel = self.channel_pick.clone();
                self.bar.tag = self.tag_pick.clone();
                self.reload_list(db);
            }
        });
        ui.horizontal(|ui| {
            let mut search = self.search_edit.clone();
            let response = ui.add_sized(
                [280.0, 32.0],
                egui::TextEdit::singleline(&mut search).hint_text("search conversations…"),
            );
            if response.changed() {
                self.search_edit = search.clone();
            }
            let mut number = self.number_edit.clone();
            ui.add_sized(
                [90.0, 32.0],
                egui::TextEdit::singleline(&mut number).hint_text("#"),
            );
            self.number_edit = number;
            if design::button_with_icon(ui, Icon::Search, "search", ButtonVariant::Primary)
                .clicked()
            {
                let trimmed = self.search_edit.trim();
                self.bar.query = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                self.bar.number = self.number_edit.trim().parse::<i64>().ok();
                self.reload_list(db);
            }
        });
        ui.add_space(6.0);

        if let Some(err) = &self.status {
            design::error_text(ui, &format!("error: {err}"));
        }

        // Conversation list (left) + detail (remaining space).
        egui::Panel::left("inbox-list")
            .exact_size(360.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(design::tokens(ui).sidebar)
                    .inner_margin(egui::Margin {
                        left: 0,
                        right: 12,
                        top: 4,
                        bottom: 4,
                    }),
            )
            .show(ui, |ui| {
                self.list_ui(ui, db);
            });
        ui.add_space(12.0);
        self.detail_ui(ui, db, bus);
    }

    fn list_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let t = design::tokens(ui);
        let mut clicked: Option<i64> = None;
        egui::ScrollArea::vertical()
            .id_salt("inbox-list-scroll")
            .show(ui, |ui| {
                for item in self.list.clone() {
                    let selected = self.selected == Some(item.id);
                    let (auto_id, rect) = ui.allocate_space(egui::vec2(ui.available_width(), 66.0));
                    let response = ui.interact(
                        rect,
                        auto_id.with(("inbox-item", item.id)),
                        egui::Sense::click(),
                    );
                    let fill = if selected {
                        t.selection
                    } else if response.hovered() {
                        t.accent
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    if fill != egui::Color32::TRANSPARENT {
                        ui.painter()
                            .rect_filled(rect, egui::CornerRadius::same(8), fill);
                    }
                    let pad = 10.0;
                    let inner = egui::Rect::from_min_max(
                        egui::pos2(rect.min.x + pad, rect.min.y + 7.0),
                        egui::pos2(rect.max.x - pad, rect.max.y - 7.0),
                    );
                    // Row 1: subject (truncated) + status badge.
                    let (tone, label) = design::status_tone(&item.status);
                    let badge_galley = badge_galley(ui, label, tone);
                    let badge_w = badge_galley.size().x + 18.0;
                    let subject_width = inner.width() - badge_w - 8.0;
                    let subject = format!(
                        "#{}  {}",
                        item.number,
                        item.subject.as_deref().unwrap_or("(no subject)"),
                    );
                    let subject_galley = egui::WidgetText::from(
                        egui::RichText::new(subject)
                            .size(13.5)
                            .strong()
                            .color(t.foreground),
                    )
                    .into_galley(
                        ui,
                        Some(egui::TextWrapMode::Truncate),
                        subject_width,
                        egui::TextStyle::Button,
                    );
                    ui.painter().galley(
                        egui::pos2(inner.min.x, inner.min.y),
                        subject_galley,
                        t.foreground,
                    );
                    paint_badge(
                        ui,
                        badge_galley,
                        tone,
                        egui::pos2(inner.max.x - badge_w, inner.min.y - 3.0),
                    );
                    // Row 2: customer · mailbox · assignee.
                    let meta = format!(
                        "{} · {}",
                        item.customer_name.as_deref().unwrap_or("(no customer)"),
                        item.mailbox_name.as_deref().unwrap_or("-"),
                    );
                    let meta_galley = egui::WidgetText::from(
                        egui::RichText::new(meta).small().color(t.muted_foreground),
                    )
                    .into_galley(
                        ui,
                        Some(egui::TextWrapMode::Truncate),
                        inner.width(),
                        egui::TextStyle::Small,
                    );
                    ui.painter().galley(
                        egui::pos2(inner.min.x, inner.min.y + 20.0),
                        meta_galley,
                        t.muted_foreground,
                    );
                    // Row 3: preview + updated time.
                    let preview = item.preview.clone().unwrap_or_default();
                    let time = item
                        .updated_at
                        .as_deref()
                        .map(compact_time)
                        .unwrap_or_default();
                    let preview_width = inner.width() - 124.0;
                    let preview_galley = egui::WidgetText::from(
                        egui::RichText::new(preview)
                            .small()
                            .color(t.muted_foreground),
                    )
                    .into_galley(
                        ui,
                        Some(egui::TextWrapMode::Truncate),
                        preview_width,
                        egui::TextStyle::Small,
                    );
                    ui.painter().galley(
                        egui::pos2(inner.min.x, inner.min.y + 38.0),
                        preview_galley,
                        t.muted_foreground,
                    );
                    let time_galley = egui::WidgetText::from(
                        egui::RichText::new(time).small().color(t.muted_foreground),
                    )
                    .into_galley(
                        ui,
                        Some(egui::TextWrapMode::Truncate),
                        112.0,
                        egui::TextStyle::Small,
                    );
                    ui.painter().galley(
                        egui::pos2(inner.max.x - 112.0, inner.min.y + 38.0),
                        time_galley,
                        t.muted_foreground,
                    );
                    if response.clicked() {
                        clicked = Some(item.id);
                    }
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::Role::ListItem,
                            ui.is_enabled(),
                            selected,
                            item.subject
                                .clone()
                                .unwrap_or_else(|| "conversation".into()),
                        )
                    });
                }
            });
        if let Some(id) = clicked {
            self.select(db, id);
        }
    }

    fn detail_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, bus: &EventBus) {
        let Some(detail) = self.detail.clone() else {
            design::empty_state(
                ui,
                Icon::Inbox,
                "Select a conversation",
                "Pick a conversation from the list to read the thread and reply.",
            );
            return;
        };

        // The whole detail pane scrolls; the composer stays reachable at
        // any window height.
        egui::ScrollArea::vertical()
            .id_salt("inbox-detail-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.detail_body_ui(ui, db, bus, detail);
            });
    }

    fn detail_body_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        bus: &EventBus,
        detail: ConversationDetail,
    ) {
        let t = design::tokens(ui);
        // Subject + status badge.
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "#{}  {}",
                    detail.number,
                    detail.subject.as_deref().unwrap_or("(no subject)")
                ))
                .size(15.5)
                .strong()
                .color(t.foreground),
            );
            let (tone, label) = design::status_tone(&detail.status);
            design::badge(ui, label, tone);
        });
        design::hint_text(
            ui,
            &format!(
                "{} <{}> · mailbox: {} · priority: {}",
                detail.customer_name.as_deref().unwrap_or("-"),
                detail.customer_email.as_deref().unwrap_or("-"),
                detail.mailbox_name.as_deref().unwrap_or("-"),
                detail.priority.as_deref().unwrap_or("none"),
            ),
        );
        ui.add_space(8.0);

        // Action bar: status, assignee (local change + queued remote job).
        design::card(ui, |ui| {
            ui.horizontal(|ui| {
                design::field_label(ui, "status:");
                for status in ["active", "pending", "closed", "spam"] {
                    if design::chip(ui, detail.status == status, status)
                        .on_hover_text("local change + queued remote job")
                        .clicked()
                    {
                        self.change_status(db, bus, status);
                    }
                }
            });
            ui.horizontal(|ui| {
                design::field_label(ui, "assign:");
                egui::ComboBox::from_id_salt("inbox-assign")
                    .width(200.0)
                    .selected_text(
                        detail
                            .assignee_name
                            .clone()
                            .unwrap_or_else(|| "unassigned".into()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.assign_pick, None, "unassigned");
                        for (id, name) in self.users.clone() {
                            ui.selectable_value(&mut self.assign_pick, Some(id), name);
                        }
                    });
                if design::button(ui, "assign", ButtonVariant::Secondary).clicked() {
                    self.assign(db, bus, self.assign_pick);
                }
            });
            if let Some(msg) = &self.status_message {
                design::success_text(ui, msg);
            }
        });
        ui.add_space(8.0);

        // Thread timeline.
        design::card_titled(ui, "Thread", None, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("inbox-thread")
                .max_height(300.0)
                .show(ui, |ui| {
                    for entry in &detail.thread {
                        thread_entry_ui(ui, entry);
                    }
                });
        });
        ui.add_space(8.0);

        // Composer with mention autocomplete.
        design::card_titled(
            ui,
            if self.composer_mode == ComposerMode::Reply {
                "Reply to customer"
            } else {
                "Internal note"
            },
            None,
            |ui| {
                ui.horizontal(|ui| {
                    if design::chip_with_icon(
                        ui,
                        self.composer_mode == ComposerMode::Reply,
                        Icon::Send,
                        "Reply",
                    )
                    .clicked()
                    {
                        self.composer_mode = ComposerMode::Reply;
                    }
                    if design::chip_with_icon(
                        ui,
                        self.composer_mode == ComposerMode::Note,
                        Icon::Pencil,
                        "Internal note",
                    )
                    .clicked()
                    {
                        self.composer_mode = ComposerMode::Note;
                    }
                });
                let hint = match self.composer_mode {
                    ComposerMode::Reply => "Type your reply to the customer… (Ctrl+Enter sends)",
                    ComposerMode::Note => {
                        "Type an internal note (visible only to your team) — @ to mention (Ctrl+Enter sends)"
                    }
                };
                let mut body = self.composer_body.clone();
                let output = egui::TextEdit::multiline(&mut body)
                    .hint_text(hint)
                    .desired_rows(5)
                    .desired_width(ui.available_width())
                    .lock_focus(false)
                    .show(ui);
                let mut changed = false;
                if output.response.changed() {
                    self.composer_body = body.clone();
                    changed = true;
                }
                if let Some(range) = output.cursor_range {
                    let caret = char_index_to_byte(&body, range.primary.index.0);
                    self.mention_caret = caret;
                    if changed {
                        self.mention_active = 0;
                    }
                }
                // Ctrl+Enter sends (matches the web widget).
                if output.response.has_focus()
                    && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter))
                {
                    self.submit_composer(db, bus);
                }

                // Mention popup.
                if let Some((query, start)) = caret_query(&body, self.mention_caret) {
                    let matches = filter_matches(&self.directory, &query);
                    if !matches.is_empty() {
                        design::section_title(ui, "@ mentions");
                        egui::Grid::new("mention-popup")
                            .num_columns(1)
                            .show(ui, |ui| {
                                for (i, (token, display, _)) in matches.iter().enumerate() {
                                    let picked = design::chip(
                                        ui,
                                        self.mention_active == i,
                                        display.as_str(),
                                    )
                                    .clicked();
                                    if picked {
                                        let completed = apply_completion(
                                            &body,
                                            start,
                                            token,
                                            self.mention_caret,
                                        );
                                        self.composer_body = completed;
                                    }
                                }
                            });
                        design::hint_text(
                            ui,
                            "Enter/Tab completes the highlighted entry when typed; click to pick",
                        );
                        if ui.input(|i| {
                            i.key_pressed(egui::Key::Tab) || i.key_pressed(egui::Key::Enter)
                        }) && output.response.has_focus()
                            && let Some((token, _, _)) =
                                matches.get(self.mention_active.min(matches.len() - 1))
                        {
                            let completed =
                                apply_completion(&body, start, token, self.mention_caret);
                            self.composer_body = completed;
                        }
                    }
                }

                if self.composer_mode == ComposerMode::Reply {
                    ui.horizontal(|ui| {
                        let mut cc = self.composer_cc.clone();
                        ui.add_sized(
                            [ui.available_width() / 2.0 - 4.0, 28.0],
                            egui::TextEdit::singleline(&mut cc)
                                .hint_text("Cc (comma-separated emails)"),
                        );
                        self.composer_cc = cc;
                    });
                    ui.horizontal(|ui| {
                        let mut bcc = self.composer_bcc.clone();
                        ui.add_sized(
                            [ui.available_width() / 2.0 - 4.0, 28.0],
                            egui::TextEdit::singleline(&mut bcc)
                                .hint_text("Bcc (comma-separated emails)"),
                        );
                        self.composer_bcc = bcc;
                    });
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    design::hint_text(ui, "Ctrl+Enter sends");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if design::button_with_icon(ui, Icon::Send, "send", ButtonVariant::Primary)
                            .clicked()
                        {
                            self.submit_composer(db, bus);
                        }
                    });
                });
            },
        );
    }
}

impl Default for InboxPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Compact an ISO timestamp (`2026-10-19T22:26:10.207Z`) to
/// `2026-10-19 22:26` for the narrow list rows.
fn compact_time(iso: &str) -> String {
    let mut out = iso.replacen('T', " ", 1);
    out.truncate(out.len().min(16));
    out
}

/// Build the galley for a small badge label (used by the custom list-row
/// painter).
fn badge_galley(ui: &mut egui::Ui, label: &str, tone: BadgeTone) -> std::sync::Arc<egui::Galley> {
    let t = design::tokens(ui);
    let fg = match tone {
        BadgeTone::Neutral => t.secondary_foreground,
        BadgeTone::Success => t.success_foreground,
        BadgeTone::Warning => t.warning_foreground,
        BadgeTone::Destructive => t.destructive_text,
        BadgeTone::Info => t.info_foreground,
    };
    egui::WidgetText::from(egui::RichText::new(label).size(11.0).strong().color(fg)).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Small,
    )
}

/// Paint a badge from a prebuilt galley at a top-left position (the list
/// rows need the same soft-fill pill look as [`design::badge`] without
/// allocating layout space).
fn paint_badge(
    ui: &mut egui::Ui,
    galley: std::sync::Arc<egui::Galley>,
    tone: BadgeTone,
    pos: egui::Pos2,
) {
    let t = design::tokens(ui);
    let bg = match tone {
        BadgeTone::Neutral => t.muted,
        BadgeTone::Success => t.success_soft,
        BadgeTone::Warning => t.warning_soft,
        BadgeTone::Destructive => t.destructive_soft,
        BadgeTone::Info => t.info_soft,
    };
    let fg = match tone {
        BadgeTone::Neutral => t.secondary_foreground,
        BadgeTone::Success => t.success_foreground,
        BadgeTone::Warning => t.warning_foreground,
        BadgeTone::Destructive => t.destructive_text,
        BadgeTone::Info => t.info_foreground,
    };
    let size = egui::vec2(galley.size().x + 18.0, galley.size().y + 6.0);
    let rect = egui::Rect::from_min_size(pos, size);
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(6), bg);
    ui.painter().galley(
        egui::pos2(rect.min.x + 9.0, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
}

/// One thread entry: kind badge, actor, timestamp, and the rich body.
fn thread_entry_ui(ui: &mut egui::Ui, entry: &spp_core::inbox::ThreadEntry) {
    let t = design::tokens(ui);
    let actor = entry
        .actor_name
        .clone()
        .unwrap_or_else(|| entry.actor_type.clone());
    let (tone, label) = match entry.thread_type.as_str() {
        "reply" => (BadgeTone::Info, "Reply"),
        "note" => (BadgeTone::Warning, "Note"),
        "customer_message" => (BadgeTone::Success, "Customer"),
        kind if kind.starts_with("ai") => (BadgeTone::Info, "AI"),
        _ => (BadgeTone::Neutral, "Other"),
    };
    ui.horizontal(|ui| {
        design::badge(ui, label, tone);
        ui.label(
            egui::RichText::new(actor)
                .size(13.0)
                .strong()
                .color(t.foreground),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(entry.created_at.as_str())
                    .small()
                    .color(t.muted_foreground),
            );
        });
    });
    let body = entry.body.as_deref().unwrap_or("");
    for span in spp_core::richtext::html_to_spans(body) {
        let mut rich = egui::RichText::new(span.text.clone());
        let style = &span.style;
        if style.bold {
            rich = rich.strong();
        }
        if style.italic {
            rich = rich.italics();
        }
        if style.underline {
            rich = rich.underline();
        }
        if style.strikethrough {
            rich = rich.strikethrough();
        }
        if style.code {
            rich = rich.monospace();
        }
        if style.quote {
            rich = rich.weak();
        }
        if let Some(level) = style.heading {
            let size = 22.0 - f32::from(level) * 2.0;
            rich = rich.size(size.max(12.0)).strong();
        }
        if let Some(href) = &style.link {
            ui.hyperlink_to(rich, href.clone());
        } else {
            ui.label(rich);
        }
    }
    design::divider(ui);
}

/// Submit a customer reply: durable outbound job with duplicate-send
/// protection (see module docs).
pub fn submit_reply(
    db: &DbHandle,
    bus: &EventBus,
    detail: &ConversationDetail,
    body: &str,
    cc: &str,
    bcc: &str,
) -> ActionOutcome {
    let cc: Vec<String> = cc
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let bcc: Vec<String> = bcc
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let key = reply_key(detail.remote_id, false, body);
    let result: Result<i64, String> = db.with_write(|conn| {
        guard_eval_mode(conn)?;
        let existing: Option<String> = conn
            .query_row(
                "SELECT status FROM outbound_jobs WHERE idempotency_key = ?1",
                rusqlite::params![&key],
                |r| r.get(0),
            )
            .ok();
        if existing.is_some_and(|status| status != "failed") {
            return Err(
                "This exact reply was already sent (duplicate-send protection).".to_string(),
            );
        }
        let job_id = queue_job(
            conn,
            "create_reply",
            &json!({
                "conversationId": detail.id,
                "text": body,
                "draft": false,
                "cc": cc,
                "bcc": bcc,
                "statusAfter": serde_json::Value::Null,
                "assignTo": serde_json::Value::Null,
            }),
            Some(detail.id),
            Some(&key),
        );
        let _ = spp_core::jobs::audit_entry(
            conn,
            "user",
            "reply_queued",
            Some(detail.id),
            Some(&json!({ "status": detail.status }).to_string()),
            None,
            None,
            None,
            false,
            Some(job_id),
            None,
        );
        Ok(job_id)
    });
    match result {
        Ok(job_id) => {
            emit_updated(bus, detail.id, detail.number);
            ActionOutcome::Success(format!("Reply queued as job #{job_id}."))
        }
        Err(reason) => ActionOutcome::Rejected(reason),
    }
}

/// Submit an internal note: local thread row + mention notifications +
/// durable outbound job (see module docs).
pub fn submit_note(
    db: &DbHandle,
    bus: &EventBus,
    detail: &ConversationDetail,
    body: &str,
) -> ActionOutcome {
    let result: Result<i64, String> = db.with_write(|conn| {
        guard_eval_mode(conn)?;
        let thread_id =
            insert_local_note(conn, detail.id, body, None).map_err(|e| e.to_string())?;
        // Mention notifications (same records the sweep creates for synced
        // notes), deduplicated by thread id + target.
        if let Ok(directory) = spp_core::mentions::build_mention_directory(conn)
            && let Ok(mentions) = spp_core::mentions::parse_mentions(body, &directory)
        {
            for m in mentions {
                let Some(user) = m.user_local_id else {
                    continue;
                };
                let _ = spp_core::notifications::record_notification(
                    conn,
                    Some(bus),
                    &spp_core::notifications::NotificationInput {
                        notification_type: spp_core::catalog::NotificationType::Mentioned,
                        title: format!("mentioned you on #{}", detail.number),
                        body: Some(body.chars().take(200).collect()),
                        target_user_local_id: Some(user),
                        actor_user_local_id: None,
                        conversation_id: Some(detail.id),
                        conversation_number: Some(detail.number),
                        dedup_key: format!("n:mention:{thread_id}:{user}"),
                        ..Default::default()
                    },
                );
            }
        }
        let job_id = queue_job(
            conn,
            "create_note",
            &json!({
                "conversationId": detail.id,
                "text": body,
                "aiGenerated": false,
            }),
            Some(detail.id),
            None,
        );
        let _ = spp_core::jobs::audit_entry(
            conn,
            "user",
            "note_added",
            Some(detail.id),
            None,
            None,
            None,
            None,
            false,
            Some(job_id),
            None,
        );
        Ok(job_id)
    });
    match result {
        Ok(job_id) => {
            emit_updated(bus, detail.id, detail.number);
            ActionOutcome::Success(format!("Internal note added (job #{job_id} queued)."))
        }
        Err(reason) => ActionOutcome::Rejected(reason),
    }
}

/// Shared mutation path: typed local write + queued remote job + audit +
/// live-update event.
fn mutate(
    db: &DbHandle,
    bus: &EventBus,
    detail: &ConversationDetail,
    op: TicketOperation,
    job_kind: &str,
    payload: &serde_json::Value,
) -> ActionOutcome {
    let result: Result<OperationResult, String> = db.with_write(|conn| {
        guard_eval_mode(conn)?;
        let outcome = spp_core::ticket_ops::execute(conn, &op).map_err(|e| e.to_string())?;
        if matches!(outcome, OperationResult::Success { .. }) {
            queue_job(conn, job_kind, payload, Some(detail.id), None);
        }
        Ok(outcome)
    });
    match result {
        Ok(OperationResult::Success { message }) => {
            emit_updated(bus, detail.id, detail.number);
            ActionOutcome::Success(format!("{message} (remote job queued)"))
        }
        Ok(OperationResult::Rejected { reason }) => ActionOutcome::Rejected(reason),
        Err(reason) => ActionOutcome::Rejected(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;
    use spp_core::eventing::EventBus;

    fn seeded() -> (crate::testsupport::ConvSeed, i64, i64, i64, i64) {
        (
            ("conv", 1, "Export broken", "active", "email", 1, 5, 9),
            1,
            5,
            9,
            1001,
        )
    }

    #[test]
    fn filters_from_bar_maps_every_field() {
        let db = fixture(Vec::new());
        let bar = FilterBar {
            view: Some(ViewFilter::Active),
            mailbox: Some(2),
            channel: Some("email".into()),
            tag: Some("bug".into()),
            query: Some("export".into()),
            number: Some(42),
            ops: None,
        };
        let filters = db.with_read(|conn| filters_from_bar(&bar, conn));
        assert_eq!(filters.status.as_deref(), Some("active"));
        assert_eq!(filters.mailbox_id, Some(2));
        assert_eq!(filters.channel.as_deref(), Some("email"));
        assert_eq!(filters.tag.as_deref(), Some("bug"));
        assert_eq!(filters.query.as_deref(), Some("export"));
        assert_eq!(filters.number, Some(42));
        assert_eq!(filters.limit, Some(50));
        assert_eq!(ViewFilter::All.status(), None);
    }

    #[test]
    fn ops_tile_filter_drills_into_unassigned_conversations() {
        let db = fixture(vec![seeded().0]);
        // Seed an unassigned conversation for the drill to find.
        db.with_write(|conn| {
            crate::testsupport::seed_conversation(conn, 2, "Orphan", "active", "email", 1, 0, 10);
        });
        let bar = FilterBar {
            ops: Some("unassigned".into()),
            ..FilterBar::default()
        };
        let (items, total) = db
            .with_read(|conn| {
                let filters = filters_from_bar(&bar, conn);
                spp_core::inbox::list_conversations(conn, &filters)
            })
            .expect("ops list");
        assert_eq!(total, 1, "only the unassigned conversation matches");
        assert_eq!(items[0].id, 2);

        // Unknown tile keys must not smuggle SQL into the where clause.
        let bad = FilterBar {
            ops: Some("' OR 1=1 --".into()),
            ..FilterBar::default()
        };
        let filters = db.with_read(|conn| filters_from_bar(&bad, conn));
        assert!(filters.extra_where.is_none());
    }

    #[test]
    fn reply_key_matches_op_send_reply_format() {
        // Format contract: reply:{remote}:{send|draft}:{24 hex chars}.
        let key = reply_key(1234, false, "hello");
        assert!(key.starts_with("reply:1234:send:"));
        assert_eq!(key.len(), "reply:1234:send:".len() + 24);
        let draft = reply_key(1234, true, "hello");
        assert!(draft.starts_with("reply:1234:draft:"));
        assert_ne!(key, draft);
    }

    #[test]
    fn submit_reply_creates_durable_job_and_blocks_duplicates() {
        let db = fixture(vec![seeded().0]);
        let bus = EventBus::new(8);
        let detail = db
            .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
            .expect("detail")
            .expect("conversation 1 exists");

        let outcome = submit_reply(&db, &bus, &detail, "here is the fix", "", "");
        let job_id = match outcome {
            ActionOutcome::Success(m) => m,
            other => panic!("expected success, got {other:?}"),
        };
        assert!(job_id.contains("job #"));

        let (kind, status, key, payload): (String, String, String, String) = db.with_read(|conn| {
            conn.query_row(
                "SELECT kind, status, idempotency_key, payload FROM outbound_jobs WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("job row")
        });
        assert_eq!(kind, "create_reply");
        assert_eq!(status, "queued");
        let expected_key = reply_key(detail.remote_id, false, "here is the fix");
        assert_eq!(key, expected_key);
        assert!(payload.contains("here is the fix"));

        // Same text again → duplicate-send protection.
        let second = submit_reply(&db, &bus, &detail, "here is the fix", "", "");
        assert!(matches!(second, ActionOutcome::Rejected(m) if m.contains("duplicate")));

        // Different text → new job.
        let third = submit_reply(&db, &bus, &detail, "different text", "", "");
        assert!(matches!(third, ActionOutcome::Success(_)));
    }

    #[test]
    fn submit_note_adds_thread_row_jobs_and_mention_notifications() {
        let db = fixture(vec![seeded().0]);
        let bus = EventBus::new(8);
        let mut rx = bus.subscribe();
        let detail = db
            .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
            .expect("detail")
            .expect("conversation 1 exists");

        let outcome = submit_note(&db, &bus, &detail, "fyi @ada please check");
        assert!(matches!(outcome, ActionOutcome::Success(_)));

        db.with_read(|conn| {
            // Local thread row exists and is visible in the detail.
            let detail = spp_core::inbox::get_conversation(conn, 1)
                .expect("detail")
                .expect("conversation");
            assert_eq!(detail.thread.len(), 1);
            assert_eq!(detail.thread[0].thread_type, "note");
            assert_eq!(detail.thread[0].body.as_deref(), Some("fyi @ada please check"));

            // Outbound job queued.
            let (kind,): (String,) = conn
                .query_row(
                    "SELECT kind FROM outbound_jobs ORDER BY id DESC LIMIT 1",
                    [],
                    |r| Ok((r.get(0)?,)),
                )
                .expect("job");
            assert_eq!(kind, "create_note");

            // Mention notification for Ada (user 5), deduped by thread.
            let mentions: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM notifications WHERE type = 'mentioned' AND target_user_id = 5",
                    [],
                    |r| r.get(0),
                )
                .expect("notifications");
            assert_eq!(mentions, 1);
        });

        // The bus got a ConversationUpdated event for the live update
        // (rx subscribed before the action above).
        let mut saw_update = false;
        while let Ok(event) = rx.try_recv() {
            if matches!(event, spp_core::events::ServerEvent::ConversationUpdated(_)) {
                saw_update = true;
            }
        }
        assert!(saw_update);
    }

    #[test]
    fn change_status_writes_locally_and_queues_remote_job() {
        let db = fixture(vec![seeded().0]);
        let bus = EventBus::new(8);
        let detail = db
            .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
            .expect("detail")
            .expect("conversation 1 exists");

        let outcome = mutate(
            &db,
            &bus,
            &detail,
            TicketOperation::ChangeStatus {
                conversation_remote_id: detail.remote_id,
                new_status: "closed".into(),
                actor_type: "user".into(),
                actor_id: None,
            },
            "update_status",
            &json!({ "conversationId": detail.id, "status": "closed" }),
        );
        assert!(matches!(outcome, ActionOutcome::Success(_)));

        db.with_read(|conn| {
            let status: String = conn
                .query_row("SELECT status FROM conversations WHERE id = 1", [], |r| {
                    r.get(0)
                })
                .expect("status");
            assert_eq!(status, "closed");
            let (kind,): (String,) = conn
                .query_row(
                    "SELECT kind FROM outbound_jobs ORDER BY id DESC LIMIT 1",
                    [],
                    |r| Ok((r.get(0)?,)),
                )
                .expect("job");
            assert_eq!(kind, "update_status");
        });
    }

    #[test]
    fn eval_mode_blocks_all_mutations() {
        let db = fixture(vec![seeded().0]);
        db.with_write(|conn| {
            spp_core::settings::set_string(conn, "ai_evaluation_mode", "true")
                .expect("set eval mode");
        });
        let bus = EventBus::new(8);
        let detail = db
            .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
            .expect("detail")
            .expect("conversation 1 exists");

        assert!(matches!(
            submit_note(&db, &bus, &detail, "note"),
            ActionOutcome::Rejected(_)
        ));
        assert!(matches!(
            submit_reply(&db, &bus, &detail, "reply", "", ""),
            ActionOutcome::Rejected(_)
        ));
        let jobs: i64 = db.with_read(|conn| {
            conn.query_row("SELECT COUNT(*) FROM outbound_jobs", [], |r| r.get(0))
                .expect("jobs")
        });
        assert_eq!(jobs, 0, "eval mode must not create jobs");
    }

    #[test]
    fn assignable_users_and_tags_list() {
        let db = fixture(vec![("conv", 1, "Tagged", "active", "email", 1, 5, 9)]);
        db.with_write(|conn| {
            crate::testsupport::seed_tag(conn, 1, "billing");
        });
        let users = db.with_read(assignable_users);
        assert!(users.contains(&(5, "Ada Lovelace".into())));
        let tags = db.with_read(tag_names);
        assert_eq!(tags, vec!["billing".to_string()]);
    }
}
