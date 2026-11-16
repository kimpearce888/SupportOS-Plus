use eframe::egui;
use rusqlite::Connection;
use serde_json::json;
use sha2::{Digest, Sha256};
use spp_core::conversation_ops::NOT_CONNECTED;
use spp_core::conversation_ops::{create_outbound_job_keyed, set_outbound_status};
use spp_core::db::DbHandle;
use spp_core::event_bus::EventBus;
use spp_core::inbox::{ConversationDetail, ConversationListItem, InboxFilters};
use spp_core::sync_engine::SyncEngine;
use spp_core::textutil::cap_chars;
use spp_core::ticket_ops::{OperationResult, TicketOperation};

use crate::design::{self, BadgeTone, ButtonVariant, ComposerAct, ComposerState, MessageRole};
use crate::icons::Icon;
use crate::mention_popup::{
    Directory, apply_completion, caret_query, char_index_to_byte, filter_matches,
};
mod composer;
mod copilot;
mod list;

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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterBar {
    pub view: Option<ViewFilter>,
    pub mailbox: Option<i64>,
    pub channel: Option<String>,
    pub tag: Option<String>,
    pub query: Option<String>,
    pub number: Option<i64>,
    pub ops: Option<String>,
}

pub(crate) const INBOX_PAGE_SIZE: u32 = 50;

#[must_use]
pub(crate) fn parse_number_entry(text: &str) -> Option<i64> {
    text.trim()
        .trim_start_matches('#')
        .trim()
        .parse::<i64>()
        .ok()
}

#[must_use]
pub(crate) fn filters_from_bar(
    bar: &FilterBar,
    page: u32,
    conn: &rusqlite::Connection,
) -> InboxFilters {
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
        limit: Some(INBOX_PAGE_SIZE),
        offset: Some(page.saturating_mul(INBOX_PAGE_SIZE)),
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

pub mod inbox_geo {
    pub(super) const RAIL_W: f32 = 168.0;
    pub const COPILOT_W: f32 = 300.0;
    pub const READER_FLOOR: f32 = 480.0;
    pub(super) const RAIL_FOLD_W: f32 = 900.0;
    pub(super) const LIST_SHARE: f32 = 0.30;
    pub(super) const LIST_MIN_W: f32 = 280.0;
    pub const COMPOSER_MIN_H: f32 = 120.0;
    pub(super) const LIST_MAX_W: f32 = 560.0;
    pub(super) const LIST_RESIZE_MAX_W: f32 = 520.0;
    pub(super) const COMPOSER_DEFAULT_H: f32 = 348.0;
    pub const COPILOT_INSET: f32 = 12.0;
    pub const COMPOSER_TOP_INSET: f32 = 8.0;
    pub const COMPOSER_X_INSET: f32 = 24.0;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerMode {
    Reply,
    Note,
}

#[must_use]
pub(crate) fn reply_key(remote_id: i64, draft: bool, text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    format!(
        "reply:{}:{}:{}",
        remote_id,
        if draft { "draft" } else { "send" },
        &hex::encode(digest)[..24]
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActionOutcome {
    Success(String),
    Rejected(String),
}

impl ActionOutcome {
    #[must_use]
    pub(crate) fn is_rejected(&self) -> bool {
        matches!(self, Self::Rejected(_))
    }

    fn message(&self) -> &str {
        match self {
            Self::Success(m) | Self::Rejected(m) => m,
        }
    }
}

#[must_use]
pub(crate) fn assignable_users(conn: &Connection) -> Vec<(i64, String)> {
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

#[must_use]
pub(crate) fn tag_names(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT name FROM tags ORDER BY name")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| r.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

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

fn queue_job(
    conn: &Connection,
    kind: &str,
    payload: &serde_json::Value,
    conversation_id: Option<i64>,
    idempotency_key: Option<&str>,
) -> Result<i64, String> {
    let id = create_outbound_job_keyed(conn, kind, payload, conversation_id, idempotency_key)
        .map_err(|e| e.to_string())?;
    set_outbound_status(conn, id, "queued", None);
    Ok(id)
}

fn guard_eval_mode(conn: &Connection) -> Result<(), String> {
    if spp_core::conversation_ops::eval_mode_on(conn) {
        Err("AI evaluation mode is ON: no notes, replies or status changes are sent.".into())
    } else {
        Ok(())
    }
}

fn provider_connected(conn: &Connection) -> bool {
    spp_core::settings::get_string(conn, "provider_kind")
        .ok()
        .flatten()
        .map(|v| v != "empty")
        .unwrap_or(true)
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

pub struct InboxPage {
    pub bar: FilterBar,
    view_pick: ViewFilter,
    mailbox_pick: Option<i64>,
    channel_pick: Option<String>,
    tag_pick: Option<String>,
    search_edit: String,
    number_edit: String,
    list: Vec<ConversationListItem>,
    list_loaded: bool,
    total: u32,
    page: u32,
    selected: Option<i64>,
    detail: Option<ConversationDetail>,
    composer_mode: ComposerMode,
    composer_body: String,
    composer_cc: String,
    composer_bcc: String,
    mention_active: usize,
    mention_caret: usize,
    composer_focused: bool,
    users: Vec<(i64, String)>,
    mailboxes: Vec<(i64, String)>,
    refs_loaded: bool,
    tags: Vec<String>,
    directory: Directory,
    status: Option<String>,
    assign_pick: Option<i64>,
    tag_edit: String,
    detail_error: Option<String>,
    outbound_note: Option<(String, bool)>,
    status_message: Option<String>,
    status_is_error: bool,
    status_message_at: Option<std::time::Instant>,
    copilot_open: bool,
    composer_panel_h: Option<f32>,
    pub(crate) panel_rail_rect: Option<egui::Rect>,
    pub(crate) panel_list_rect: Option<egui::Rect>,
    pub(crate) panel_copilot_rect: Option<egui::Rect>,
    pub(crate) panel_composer_rect: Option<egui::Rect>,
    pub(crate) panel_reader_rect: Option<egui::Rect>,
    copilot: Option<spp_core::ai_local::CopilotReport>,
    copilot_error: Option<String>,
    triage: Vec<TriageChip>,
    briefing: Vec<spp_core::ai_prompts::MemoryEntry>,
    editing_chip: Option<spp_core::catalog::AiAttributeKey>,
    vector_index: Option<std::sync::Arc<spp_core::vectorstore_sqlitevec::VectorIndex>>,
    pending_ai: Option<PendingComposerAi>,
    coaching_review: Option<serde_json::Value>,
    qa_review: Option<serde_json::Value>,
    translate_lang: String,
    inserted_draft: Option<(i64, String)>,
    view_counts: [u32; 4],
    show_filters: bool,
    saved_replies: Vec<(String, String)>,
    draft_saved: bool,
    assistant_session: Option<i64>,
    assistant_messages: Vec<serde_json::Value>,
    assistant_input: String,
    assistant_pending: Option<i64>,
    assistant_error: Option<String>,
    drafts: std::collections::HashMap<i64, StashedDraft>,
}

#[derive(Debug, Clone)]
struct StashedDraft {
    mode: ComposerMode,
    body: String,
    cc: String,
    bcc: String,
    inserted_draft: Option<(i64, String)>,
}

#[derive(Debug, Clone)]
struct PendingComposerAi {
    job_id: i64,
    kind: &'static str,
    conversation_id: i64,
    origin_body: String,
}

#[derive(Clone)]
struct TriageChip {
    key: spp_core::catalog::AiAttributeKey,
    label: &'static str,
    value: String,
    confidence: String,
    source: String,
    editable: bool,
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
            list_loaded: false,
            total: 0,
            page: 0,
            selected: None,
            detail: None,
            composer_mode: ComposerMode::Reply,
            composer_body: String::new(),
            composer_cc: String::new(),
            composer_bcc: String::new(),
            composer_panel_h: None,
            panel_rail_rect: None,
            panel_list_rect: None,
            panel_copilot_rect: None,
            panel_composer_rect: None,
            panel_reader_rect: None,
            mention_active: 0,
            mention_caret: 0,
            composer_focused: false,
            users: Vec::new(),
            mailboxes: Vec::new(),
            refs_loaded: false,
            tags: Vec::new(),
            directory: Directory::default(),
            status: None,
            detail_error: None,
            assign_pick: None,
            tag_edit: String::new(),
            outbound_note: None,
            status_message: None,
            status_is_error: false,
            status_message_at: None,
            copilot_open: false,
            copilot: None,
            copilot_error: None,
            triage: Vec::new(),
            briefing: Vec::new(),
            editing_chip: None,
            vector_index: None,
            pending_ai: None,
            coaching_review: None,
            qa_review: None,
            translate_lang: "es".into(),
            view_counts: [0; 4],
            show_filters: false,
            saved_replies: Vec::new(),
            draft_saved: false,
            inserted_draft: None,
            assistant_session: None,
            assistant_messages: Vec::new(),
            assistant_input: String::new(),
            assistant_pending: None,
            assistant_error: None,
            drafts: std::collections::HashMap::new(),
        }
    }

    pub(crate) fn set_vector_index(
        &mut self,
        index: Option<std::sync::Arc<spp_core::vectorstore_sqlitevec::VectorIndex>>,
    ) {
        self.vector_index = index;
    }

    pub(crate) fn invalidate(&mut self) {
        self.list.clear();
        self.list_loaded = false;
        self.detail = None;
        self.users.clear();
        self.mailboxes.clear();
        self.refs_loaded = false;
        self.tags.clear();
        self.directory = Directory::default();
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if !self.refs_loaded {
            self.reload_refs(db);
        }
        if !self.list_loaded {
            self.reload_list(db);
        }
        if self.selected.is_some() && self.detail.is_none() && self.detail_error.is_none() {
            self.reload_detail(db);
        }
    }

    fn reload_refs(&mut self, db: &DbHandle) {
        let (users, mailboxes, tags, directory, saved_replies) = db.with_read(|conn| {
            let directory = Directory::from_core(
                &spp_core::mentions::build_mention_directory(conn).unwrap_or_default(),
            );
            let saved_replies = saved_reply_list(conn);
            (
                assignable_users(conn),
                crate::pages::dashboard::mailboxes(conn),
                tag_names(conn),
                directory,
                saved_replies,
            )
        });
        self.users = users;
        self.mailboxes = mailboxes;
        self.refs_loaded = true;
        self.tags = tags;
        self.directory = directory;
        self.saved_replies = saved_replies;
    }

    fn reload_list(&mut self, db: &DbHandle) {
        if !self.refs_loaded {
            self.reload_refs(db);
        }
        let result = db.with_read(|conn| {
            let filters = filters_from_bar(&self.bar, self.page, conn);
            spp_core::inbox::list_conversations(conn, &filters)
        });
        match result {
            Ok((items, total)) => {
                if items.is_empty() && self.page > 0 && total > 0 {
                    self.page = (total - 1) / INBOX_PAGE_SIZE;
                    return self.reload_list(db);
                }
                self.list = items;
                self.total = total;
                self.status = None;
            }
            Err(e) => self.status = Some(e.to_string()),
        }
        self.list_loaded = true;
        self.refresh_view_counts(db);
    }

    fn reload_detail(&mut self, db: &DbHandle) {
        let id = self.selected;
        let detail = id.map(|id| db.with_read(|conn| spp_core::inbox::get_conversation(conn, id)));
        match detail {
            Some(Ok(Some(loaded))) => {
                self.detail = Some(loaded);
                self.detail_error = None;
            }
            Some(Ok(None)) => {
                self.detail = None;
                self.detail_error = None;
                self.selected = None;
                self.assign_pick = None;
                self.set_status_message("Conversation not found — it may have been deleted.", true);
            }
            Some(Err(e)) => {
                self.detail = None;
                self.detail_error = Some(e.to_string());
            }
            None => {
                self.detail = None;
                self.detail_error = None;
            }
        }
        if let Some(id) = id {
            let (note, report) = db.with_read(|conn| {
                let note = spp_core::outbound::latest_for_conversation(conn, id).map(|o| {
                    let label = match o.kind.as_str() {
                        "create_reply" => "Reply",
                        "create_note" => "Note",
                        "update_status" => "Status change",
                        "assign" => "Assignment",
                        other => other,
                    };
                    match o.status.as_str() {
                        "queued" => (format!("{label}: queued for delivery…"), false),
                        "sending" => (format!("{label}: sending to Help Scout…"), false),
                        "confirmed" => (format!("{label} delivered to Help Scout ✓"), false),
                        "failed" => (
                            format!(
                                "{label} failed: {}",
                                o.error.as_deref().unwrap_or("unknown error")
                            ),
                            true,
                        ),
                        other => (format!("{label}: {other}"), false),
                    }
                });
                let report = if self.copilot_open {
                    Some(spp_core::ai_local::copilot_report(
                        conn,
                        self.vector_index.as_deref(),
                        id,
                    ))
                } else {
                    None
                };
                (note, report)
            });
            self.outbound_note = note;
            match report {
                Some(Ok(r)) => {
                    self.copilot = Some(r);
                    self.copilot_error = None;
                }
                Some(Err(e)) => {
                    self.copilot = None;
                    self.copilot_error = Some(e.to_string());
                }
                None => {}
            }
            let (coaching, qa, assistant) = db.with_read(|conn| {
                let coaching = spp_core::coaching::get(conn, id).ok().flatten();
                let status: Option<String> = conn
                    .query_row(
                        "SELECT status FROM conversations WHERE id = ?1",
                        rusqlite::params![id],
                        |r| r.get(0),
                    )
                    .ok();
                let qa = if status.as_deref() == Some("closed") {
                    let ai_ok = !matches!(
                        spp_core::ai_hub::backend_for_role(
                            conn,
                            spp_core::ai_hub::AiRole::Classify
                        ),
                        spp_core::ai_pipeline::AiBackend::Disabled,
                    );
                    spp_core::quality::get_qa(conn, id, ai_ok)
                } else {
                    None
                };
                let session: Option<i64> = conn
                    .query_row(
                        "SELECT id FROM copilot_sessions WHERE conversation_id = ?1
                          ORDER BY updated_at DESC LIMIT 1",
                        rusqlite::params![id],
                        |r| r.get(0),
                    )
                    .ok();
                let assistant = session
                    .map(|sid| spp_core::copilot::list_messages(conn, sid))
                    .unwrap_or_default();
                (coaching, qa, (session, assistant))
            });
            self.coaching_review = coaching;
            self.qa_review = qa;
            (self.assistant_session, self.assistant_messages) = assistant;
            self.reload_triage(db);
        }
    }

    fn reload_triage(&mut self, db: &DbHandle) {
        let Some(id) = self.selected else {
            self.triage.clear();
            self.briefing.clear();
            return;
        };
        let (rows, customer_id) = db.with_read(|conn| {
            (
                spp_core::ai_attributes::current_for_conversation(conn, id).unwrap_or_default(),
                conn.query_row(
                    "SELECT customer_local_id FROM conversations WHERE id = ?1",
                    rusqlite::params![id],
                    |r| r.get::<_, Option<i64>>(0),
                )
                .unwrap_or(None),
            )
        });
        let heuristic = self.copilot.as_ref().map(|c| &c.analysis);
        let value_of = |key: &str| -> Option<(String, String, String)> {
            rows.iter()
                .find(|r| r.attribute == key)
                .map(|r| (r.value.clone(), r.confidence.clone(), r.source.clone()))
        };
        let mut chips = Vec::new();
        let attribute_chip = |key: spp_core::catalog::AiAttributeKey,
                              value: String,
                              confidence: String,
                              source: String|
         -> TriageChip {
            TriageChip {
                key,
                label: key.label(),
                value,
                confidence,
                source,
                editable: true,
            }
        };
        if let Some((value, confidence, source)) = value_of("intent") {
            chips.push(attribute_chip(
                spp_core::catalog::AiAttributeKey::Intent,
                value,
                confidence,
                source,
            ));
        }
        if let Some((value, confidence, source)) = value_of("urgency") {
            chips.push(attribute_chip(
                spp_core::catalog::AiAttributeKey::Urgency,
                value,
                confidence,
                source,
            ));
        }
        if let Some((value, confidence, source)) = value_of("frustration_cues") {
            chips.push(attribute_chip(
                spp_core::catalog::AiAttributeKey::FrustrationCues,
                value,
                confidence,
                source,
            ));
        }
        if let Some((value, confidence, source)) = value_of("escalation_signal") {
            chips.push(attribute_chip(
                spp_core::catalog::AiAttributeKey::EscalationSignal,
                value,
                confidence,
                source,
            ));
        }
        if let Some((value, confidence, source)) = value_of("issue_cluster") {
            chips.push(attribute_chip(
                spp_core::catalog::AiAttributeKey::IssueCluster,
                value,
                confidence,
                source,
            ));
        }
        if value_of("sentiment").is_none()
            && let Some(sentiment) = heuristic
                .map(|h| h.sentiment.clone())
                .filter(|s| !s.is_empty())
        {
            chips.push(TriageChip {
                key: spp_core::catalog::AiAttributeKey::Intent,
                label: "sentiment",
                value: sentiment,
                confidence: "heuristic".into(),
                source: "local".into(),
                editable: false,
            });
        }
        self.triage = chips;
        self.briefing = customer_id
            .map(|cid| {
                db.with_read(|conn| spp_core::ai_evidence::customer_memory_facts(conn, cid, 3))
            })
            .unwrap_or_default();
    }

    fn override_chip_value(&mut self, db: &DbHandle, chip: &TriageChip, value: &str) {
        if value.is_empty() {
            let Some(id) = self.selected else { return };
            db.with_write(|conn| {
                let _ = conn.execute(
                    "UPDATE ai_attributes SET superseded_at = datetime('now')
                      WHERE conversation_id = ?1 AND attribute = ?2 AND superseded_at IS NULL",
                    rusqlite::params![id, chip.key.as_str()],
                );
            });
            self.reload_triage(db);
            return;
        }
        self.override_chip(db, chip, value);
    }

    fn override_chip(&mut self, db: &DbHandle, chip: &TriageChip, value: &str) {
        let Some(id) = self.selected else {
            return;
        };
        db.with_write(|conn| {
            let _ = spp_core::ai_attributes::upsert_single(
                conn,
                id,
                spp_core::ai_attributes::AttributeRecord {
                    key: chip.key,
                    value: value.to_string(),
                    confidence: "high",
                    source: "human",
                    evidence: Vec::new(),
                    run_id: None,
                },
            );
        });
        self.reload_triage(db);
    }

    pub fn select(&mut self, db: &DbHandle, id: i64) {
        let switching = self.selected != Some(id);
        if switching && let Some(prev) = self.selected {
            self.drafts.insert(
                prev,
                StashedDraft {
                    mode: self.composer_mode,
                    body: std::mem::take(&mut self.composer_body),
                    cc: std::mem::take(&mut self.composer_cc),
                    bcc: std::mem::take(&mut self.composer_bcc),
                    inserted_draft: self.inserted_draft.take(),
                },
            );
        }
        self.selected = Some(id);
        if switching {
            self.assign_pick = None;
            if let Some(stash) = self.drafts.get(&id).cloned() {
                self.composer_mode = stash.mode;
                self.composer_body = stash.body;
                self.composer_cc = stash.cc;
                self.composer_bcc = stash.bcc;
                self.inserted_draft = stash.inserted_draft;
            } else {
                self.composer_mode = ComposerMode::Reply;
                self.composer_body.clear();
                self.composer_cc.clear();
                self.composer_bcc.clear();
                self.inserted_draft = None;
            }
        }
        self.status_message = None;
        self.status_message_at = None;
        self.pending_ai = None;
        self.mention_caret = 0;
        self.mention_active = 0;
        self.coaching_review = None;
        self.qa_review = None;
        self.assistant_session = None;
        self.assistant_messages.clear();
        self.assistant_input.clear();
        self.assistant_pending = None;
        self.assistant_error = None;
        self.reload_detail(db);
    }

    pub(crate) fn composer_edited(&mut self, body: String) {
        if body.trim().is_empty() {
            self.inserted_draft = None;
        }
        self.composer_body = body;
    }

    pub(crate) fn submit_composer(&mut self, db: &DbHandle, bus: &EventBus) -> ActionOutcome {
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
        match &outcome {
            ActionOutcome::Success(msg) => {
                if let Some((draft_id, original)) = self.inserted_draft.take() {
                    let sent = body.clone();
                    let recorded = db.with_write(|conn| {
                        spp_core::ai_pipeline::record_feedback(
                            conn, draft_id, &original, &sent, true,
                        )
                        .and_then(|()| {
                            spp_core::ai_pipeline::set_draft_state(conn, draft_id, "accepted")
                        })
                    });
                    if let Err(e) = recorded {
                        tracing::warn!(error = %e, "draft feedback recording failed (non-fatal)");
                    }
                }
                self.set_status_message(msg, false);
                self.composer_body.clear();
                self.composer_cc.clear();
                self.composer_bcc.clear();
                if let Some(id) = self.selected {
                    self.drafts.remove(&id);
                }
            }
            ActionOutcome::Rejected(msg) => {
                self.set_status_message(msg, true);
            }
        }
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

    fn set_status_message(&mut self, message: impl Into<String>, is_error: bool) {
        self.status_message = Some(message.into());
        self.status_is_error = is_error;
        self.status_message_at = Some(std::time::Instant::now());
    }

    fn expire_status_message(&mut self) {
        const STATUS_MESSAGE_TTL: std::time::Duration = std::time::Duration::from_secs(45);
        if let Some(set_at) = self.status_message_at
            && set_at.elapsed() > STATUS_MESSAGE_TTL
        {
            self.status_message = None;
            self.status_message_at = None;
        }
    }

    pub(crate) fn change_status(
        &mut self,
        db: &DbHandle,
        bus: &EventBus,
        status: &str,
    ) -> ActionOutcome {
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
        self.set_status_message(outcome.message(), outcome.is_rejected());
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

    pub(crate) fn assign(
        &mut self,
        db: &DbHandle,
        bus: &EventBus,
        user_id: Option<i64>,
    ) -> ActionOutcome {
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
        self.set_status_message(outcome.message(), outcome.is_rejected());
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

    pub(crate) fn set_priority(
        &mut self,
        db: &DbHandle,
        bus: &EventBus,
        priority: &str,
    ) -> ActionOutcome {
        let Some(detail) = self.detail.clone() else {
            return ActionOutcome::Rejected("No conversation selected.".into());
        };
        let Some(new_priority) = spp_core::ticket_states::TicketPriority::parse(priority) else {
            return ActionOutcome::Rejected(format!("unknown priority: {priority}"));
        };
        let result: std::result::Result<OperationResult, String> = db.with_write(|conn| {
            guard_eval_mode(conn)?;
            let op = TicketOperation::SetPriority {
                conversation_remote_id: detail.remote_id,
                new_priority,
                actor_type: "user".into(),
                actor_id: None,
            };
            spp_core::ticket_ops::execute(conn, &op).map_err(|e| e.to_string())
        });
        let outcome = match result {
            Ok(OperationResult::Success { message }) => {
                emit_updated(bus, detail.id, detail.number);
                ActionOutcome::Success(message)
            }
            Ok(OperationResult::Rejected { reason }) => ActionOutcome::Rejected(reason),
            Err(reason) => ActionOutcome::Rejected(reason),
        };
        self.set_status_message(outcome.message(), outcome.is_rejected());
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

    pub(crate) fn set_workflow_state(
        &mut self,
        db: &DbHandle,
        bus: &EventBus,
        state: &str,
    ) -> ActionOutcome {
        let Some(detail) = self.detail.clone() else {
            return ActionOutcome::Rejected("No conversation selected.".into());
        };
        if !spp_core::ticket_states::workflow_state_is_valid(state) {
            return ActionOutcome::Rejected(format!("unknown workflow state: {state}"));
        }
        let result: std::result::Result<OperationResult, String> = db.with_write(|conn| {
            guard_eval_mode(conn)?;
            let op = TicketOperation::SetTicketState {
                conversation_remote_id: detail.remote_id,
                new_state: state.to_string(),
                actor_type: "user".into(),
                actor_id: None,
            };
            spp_core::ticket_ops::execute(conn, &op).map_err(|e| e.to_string())
        });
        let outcome = match result {
            Ok(OperationResult::Success { message }) => {
                emit_updated(bus, detail.id, detail.number);
                ActionOutcome::Success(message)
            }
            Ok(OperationResult::Rejected { reason }) => ActionOutcome::Rejected(reason),
            Err(reason) => ActionOutcome::Rejected(reason),
        };
        self.set_status_message(outcome.message(), outcome.is_rejected());
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

    pub(crate) fn set_tags(
        &mut self,
        db: &DbHandle,
        bus: &EventBus,
        tags: Vec<String>,
    ) -> ActionOutcome {
        let Some(detail) = self.detail.clone() else {
            return ActionOutcome::Rejected("No conversation selected.".into());
        };
        let result: std::result::Result<(), String> = db.with_write(|conn| {
            guard_eval_mode(conn)?;
            if !provider_connected(conn) {
                return Err(NOT_CONNECTED.to_string());
            }
            spp_core::conversation_ops::write_conversation_tags(conn, detail.id, &tags);
            queue_job(
                conn,
                "update_tags",
                &json!({ "conversationId": detail.id, "tags": tags }),
                Some(detail.id),
                None,
            )?;
            Ok(())
        });
        let outcome = match result {
            Ok(()) => {
                emit_updated(bus, detail.id, detail.number);
                ActionOutcome::Success("tags updated — sending to Help Scout".into())
            }
            Err(reason) => ActionOutcome::Rejected(reason),
        };
        self.set_status_message(outcome.message(), outcome.is_rejected());
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

    pub(crate) fn snooze(
        &mut self,
        db: &DbHandle,
        bus: &EventBus,
        snoozed_until: String,
    ) -> ActionOutcome {
        let Some(detail) = self.detail.clone() else {
            return ActionOutcome::Rejected("No conversation selected.".into());
        };
        let result: std::result::Result<(), String> = db.with_write(|conn| {
            guard_eval_mode(conn)?;
            if !provider_connected(conn) {
                return Err(NOT_CONNECTED.to_string());
            }
            conn.execute(
                "UPDATE conversations SET snoozed_until = ?1 WHERE id = ?2",
                rusqlite::params![snoozed_until, detail.id],
            )
            .map_err(|e| e.to_string())?;
            queue_job(
                conn,
                "snooze",
                &json!({
                    "conversationId": detail.id,
                    "snoozedUntil": snoozed_until,
                    "unsnoozeOnCustomerReply": true,
                }),
                Some(detail.id),
                None,
            )?;
            Ok(())
        });
        let outcome = match result {
            Ok(()) => {
                emit_updated(bus, detail.id, detail.number);
                ActionOutcome::Success(format!(
                    "snoozed until {snoozed_until} — sending to Help Scout"
                ))
            }
            Err(reason) => ActionOutcome::Rejected(reason),
        };
        self.set_status_message(outcome.message(), outcome.is_rejected());
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

    pub(crate) fn unsnooze(&mut self, db: &DbHandle, bus: &EventBus) -> ActionOutcome {
        let Some(detail) = self.detail.clone() else {
            return ActionOutcome::Rejected("No conversation selected.".into());
        };
        let result: std::result::Result<(), String> = db.with_write(|conn| {
            guard_eval_mode(conn)?;
            if !provider_connected(conn) {
                return Err(NOT_CONNECTED.to_string());
            }
            conn.execute(
                "UPDATE conversations SET snoozed_until = NULL WHERE id = ?1",
                rusqlite::params![detail.id],
            )
            .map_err(|e| e.to_string())?;
            queue_job(
                conn,
                "unsnooze",
                &json!({ "conversationId": detail.id }),
                Some(detail.id),
                None,
            )?;
            Ok(())
        });
        let outcome = match result {
            Ok(()) => {
                emit_updated(bus, detail.id, detail.number);
                ActionOutcome::Success("snooze removed — sending to Help Scout".into())
            }
            Err(reason) => ActionOutcome::Rejected(reason),
        };
        self.set_status_message(outcome.message(), outcome.is_rejected());
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        bus: &EventBus,
        engine: Option<&std::sync::Arc<SyncEngine>>,
    ) {
        self.ensure_loaded(db);
        self.poll_pending_ai(db);
        self.poll_assistant(db);

        let t = design::tokens(ui);
        if let Some(err) = &self.status {
            design::error_note(ui, err);
        }
        if !db.with_read(provider_connected) {
            design::hint_text(
                ui,
                "Help Scout is not connected — nothing can sync yet. Add your app credentials in \
                 Settings (Help Scout tab), press connect, and this inbox fills automatically.",
            );
        }
        if let Some(ops) = self.bar.ops.clone() {
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
                    self.page = 0;
                    self.reload_list(db);
                }
            });
            ui.add_space(design::scale::space::SP1);
        }

        let detail = self.detail.clone();

        let avail = ui.available_width();
        let rail_fits = avail >= inbox_geo::RAIL_FOLD_W;
        let mut rail_w = if rail_fits { inbox_geo::RAIL_W } else { 0.0 };
        let list_est = ((avail - rail_w) * inbox_geo::LIST_SHARE)
            .clamp(inbox_geo::LIST_MIN_W, inbox_geo::LIST_RESIZE_MAX_W);
        if self.copilot_open {
            let fits_with_rail =
                avail - rail_w - list_est >= inbox_geo::COPILOT_W + inbox_geo::READER_FLOOR;
            let fits_folded = avail - list_est >= inbox_geo::COPILOT_W + inbox_geo::READER_FLOOR;
            if !fits_with_rail && fits_folded {
                rail_w = 0.0;
            }
        }

        self.panel_rail_rect = None;
        self.panel_list_rect = None;
        self.panel_copilot_rect = None;
        self.panel_composer_rect = None;
        self.panel_reader_rect = None;

        if rail_w > 0.0 {
            egui::Panel::left("inbox-views")
                .exact_size(rail_w)
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(t.sidebar)
                        .inner_margin(egui::Margin::symmetric(
                            design::scale::space::SP3 as i8,
                            design::scale::space::SP3 as i8,
                        )),
                )
                .show(ui, |ui| {
                    self.panel_rail_rect = Some(ui.max_rect());
                    self.views_rail_ui(ui, db);
                });
        }
        egui::Panel::left("inbox-list")
            .default_size(
                (avail - rail_w)
                    .mul_add(inbox_geo::LIST_SHARE, 0.0)
                    .clamp(inbox_geo::LIST_MIN_W, inbox_geo::LIST_RESIZE_MAX_W),
            )
            .size_range(inbox_geo::LIST_MIN_W..=inbox_geo::LIST_MAX_W)
            .resizable(true)
            .show_separator_line(true)
            .frame(egui::Frame::new().fill(t.card).inner_margin(egui::Margin {
                left: design::scale::space::SP3 as i8,
                right: design::scale::space::SP3 as i8,
                top: design::scale::space::SP3 as i8,
                bottom: design::scale::space::SP2 as i8,
            }))
            .show(ui, |ui| {
                self.panel_list_rect = Some(ui.max_rect());
                self.list_ui(ui, db, engine, rail_w == 0.0);
            });
        let remaining = ui.available_width();
        let copilot_docked =
            self.copilot_open && remaining >= inbox_geo::COPILOT_W + inbox_geo::READER_FLOOR;

        if detail.is_some() && !(self.copilot_open && !copilot_docked) {
            let composer_h = self
                .composer_panel_h
                .unwrap_or(inbox_geo::COMPOSER_DEFAULT_H)
                .clamp(
                    inbox_geo::COMPOSER_MIN_H,
                    (ui.available_height() - design::scale::space::SP3)
                        .max(inbox_geo::COMPOSER_MIN_H),
                );
            egui::Panel::bottom("inbox-composer")
                .exact_size(composer_h)
                .frame(
                    egui::Frame::new()
                        .fill(t.background)
                        .inner_margin(egui::Margin {
                            left: design::scale::space::SP5 as i8,
                            right: design::scale::space::SP5 as i8,
                            top: design::scale::space::SP2 as i8,
                            bottom: design::scale::space::SP2 as i8,
                        }),
                )
                .show(ui, |ui| {
                    self.panel_composer_rect = Some(ui.max_rect());
                    if let Some(d) = detail.clone() {
                        let max_h = ui.available_height();
                        egui::ScrollArea::vertical()
                            .id_salt("inbox-composer-scroll")
                            .max_height(max_h)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                design::centered_column(ui, design::scale::READER_MEASURE, |ui| {
                                    self.composer_pane_ui(ui, db, bus, &d);
                                });
                            });
                    }
                });
        }
        if copilot_docked {
            egui::Panel::right("inbox-copilot")
                .exact_size(inbox_geo::COPILOT_W)
                .resizable(false)
                .show_separator_line(true)
                .frame(
                    egui::Frame::new()
                        .fill(t.sidebar)
                        .inner_margin(egui::Margin::symmetric(
                            design::scale::space::SP3 as i8,
                            design::scale::space::SP3 as i8,
                        )),
                )
                .show(ui, |ui| {
                    self.panel_copilot_rect = Some(ui.max_rect());
                    self.copilot_pane_ui(ui, db, detail.as_ref());
                });
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(egui::Margin {
                        left: design::scale::space::SP5 as i8,
                        right: design::scale::space::SP5 as i8,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show(ui, |ui| {
                self.panel_reader_rect = Some(ui.max_rect());
                if self.copilot_open && !copilot_docked {
                    self.copilot_pane_ui(ui, db, detail.as_ref());
                } else {
                    design::centered_column(ui, design::scale::READER_MEASURE, |ui| {
                        self.reader_ui(ui, db, bus, detail);
                    });
                }
            });
    }

    fn views_rail_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.label(
            egui::RichText::new("Views")
                .size(design::scale::text::META)
                .color(design::tokens(ui).muted_foreground),
        );
        ui.add_space(design::scale::space::SP1);
        let counts = self.view_counts;
        for (count, view) in counts.into_iter().zip(ViewFilter::ALL) {
            let selected = self.view_pick == view;
            if design::view_item(ui, view.label(), count, selected).clicked() {
                self.view_pick = view;
                self.bar.view = Some(view);
                self.page = 0;
                self.reload_list(db);
            }
        }
        ui.add_space(design::scale::space::SP1);
    }
}

impl InboxPage {
    fn coaching_review_ui(&mut self, ui: &mut egui::Ui, review: &serde_json::Value) {
        let summary = review.get("summary").cloned().unwrap_or_default();
        let flagged = summary.get("flagged").and_then(|v| v.as_i64()).unwrap_or(0);
        let checks_run = summary
            .get("checks_run")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let ai = review.get("ai").cloned().unwrap_or_default();
        let ai_available = ai
            .get("available")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        design::card_titled(
            ui,
            "Draft coaching",
            Some("advisory — never blocks sending"),
            |ui| {
                ui.horizontal(|ui| {
                    let tone = if flagged > 0 {
                        BadgeTone::Warning
                    } else {
                        BadgeTone::Success
                    };
                    design::badge(ui, &format!("{flagged} flagged"), tone);
                    design::hint_text(ui, &format!("{checks_run} checks ran"));
                    if ai_available {
                        design::badge(
                            ui,
                            &format!(
                                "AI layer: {}",
                                ai.get("model").and_then(|m| m.as_str()).unwrap_or("routed")
                            ),
                            BadgeTone::Ai,
                        );
                    } else {
                        design::badge(
                            ui,
                            "AI layer: none (deterministic only)",
                            BadgeTone::Neutral,
                        );
                    }
                    if let Some(note) = review.get("note").and_then(|n| n.as_str()) {
                        design::hint_text(ui, note);
                    }
                });
                if let Some(checks) = review.get("checks").and_then(|c| c.as_array()) {
                    egui::ScrollArea::vertical()
                        .id_salt("coaching-checks")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for check in checks {
                                let label = check
                                    .get("label")
                                    .and_then(|l| l.as_str())
                                    .unwrap_or("check");
                                let status = check
                                    .get("status")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("unavailable");
                                let detail = check
                                    .get("detail")
                                    .and_then(|d| d.as_str())
                                    .unwrap_or_default();
                                ui.horizontal(|ui| {
                                    let tone = match status {
                                        "flagged" => BadgeTone::Warning,
                                        "pass" => BadgeTone::Success,
                                        _ => BadgeTone::Neutral,
                                    };
                                    design::badge(ui, status, tone);
                                    ui.label(
                                        egui::RichText::new(label)
                                            .small()
                                            .color(design::tokens(ui).foreground),
                                    );
                                });
                                if !detail.is_empty() {
                                    design::hint_text(ui, &truncate_chars(detail, 220));
                                }
                                ui.add_space(2.0);
                            }
                        });
                }
            },
        );
    }

    fn state_history_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, detail: &ConversationDetail) {
        let rows = db.with_read(|conn| {
            spp_core::ticket_states::list_state_transitions(conn, detail.remote_id, 20)
        });
        if rows.is_empty() {
            return;
        }
        let t = design::tokens(ui);
        design::card_titled(
            ui,
            "State history",
            Some("local workflow transitions — not sent to Help Scout"),
            |ui| {
                for row in rows {
                    let change = if let Some(to) = row.to_state.as_deref() {
                        Some(format!(
                            "{} → {}",
                            row.from_state.as_deref().unwrap_or(design::MISSING_VALUE),
                            to
                        ))
                    } else if let Some(to) = row.to_priority.as_deref() {
                        Some(format!(
                            "priority {} → {}",
                            row.from_priority
                                .as_deref()
                                .unwrap_or(design::MISSING_VALUE),
                            to
                        ))
                    } else {
                        None
                    };
                    let Some(change) = change else { continue };
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(change)
                                .size(design::scale::text::SMALL)
                                .color(t.foreground),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "· {} · {}",
                                row.actor_type,
                                crate::pages::relative_time(&row.transitioned_at)
                            ))
                            .size(design::scale::text::SMALL)
                            .color(t.muted_foreground),
                        );
                    });
                }
            },
        );
    }

    fn qa_card_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        qa: &serde_json::Value,
        conversation_id: i64,
    ) {
        let deterministic = qa.get("deterministic").cloned().unwrap_or_default();
        let ai_available = qa
            .get("ai_available")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let has_ai = qa.get("ai").is_some();
        let busy = self
            .pending_ai
            .as_ref()
            .is_some_and(|p| p.kind == "qa_ai_layer");
        design::card_titled(
            ui,
            "Post-resolution QA",
            Some("computed locally from the mirrored thread"),
            |ui| {
                egui::Grid::new("qa-deterministic")
                    .num_columns(2)
                    .show(ui, |ui| {
                        for (key, label) in [
                            ("back_and_forth_count", "Back-and-forth messages"),
                            ("repeated_information_count", "Repeated information"),
                            ("messages_after_close", "Messages after close"),
                            ("handoff_count", "Handoffs"),
                            ("first_response_minutes", "First response (min)"),
                            ("resolution_minutes", "Resolution (min)"),
                        ] {
                            let value = deterministic
                                .get(key)
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| design::MISSING_VALUE.into());
                            design::field_label(ui, label);
                            ui.label(
                                egui::RichText::new(value.as_str())
                                    .small()
                                    .color(design::tokens(ui).foreground),
                            );
                            ui.end_row();
                        }
                    });
                if let Some(disclaimer) = deterministic
                    .get("computed_honestly")
                    .and_then(|d| d.as_str())
                {
                    design::hint_text(ui, disclaimer);
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if has_ai {
                        design::badge(ui, "AI review: present", BadgeTone::Ai);
                    } else if ai_available {
                        if busy {
                            design::hint_text(ui, "AI review running…");
                        } else if design::button(ui, "Run AI review", ButtonVariant::Outline)
                            .on_hover_text(
                                "the AI layer reads the transcript and answers: did the reply resolve it, is it evidence-grounded",
                            )
                            .clicked()
                        {
                            self.enqueue_composer_ai(
                                db,
                                "qa_ai_layer",
                                json!({ "conversationId": conversation_id }),
                                conversation_id,
                            );
                        }
                    } else {
                        design::hint_text(
                            ui,
                            "AI review needs a routed Classify role (AI Center > Model-role routing).",
                        );
                    }
                    if let Some(at) = qa.get("recomputed_at").and_then(|a| a.as_str()) {
                        design::hint_text(ui, &format!("AI layer recomputed {at}"));
                    }
                });
                if let Some(ai) = qa.get("ai") {
                    ui.add_space(4.0);
                    if let Some(answered) = ai.get("answered").and_then(|a| a.as_str()) {
                        design::hint_text(ui, &format!("answered: {answered}"));
                    }
                    if let Some(evidence) = ai.get("evidence_supported").and_then(|a| a.as_str()) {
                        design::hint_text(ui, &format!("evidence-supported: {evidence}"));
                    }
                    if let Some(reason) = ai.get("kb_reason").and_then(|a| a.as_str()) {
                        design::hint_text(ui, &format!("KB suggestion: {reason}"));
                    }
                }
            },
        );
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct InboxPanelGeometry {
    pub rail: Option<egui::Rect>,
    pub list: Option<egui::Rect>,
    pub copilot: Option<egui::Rect>,
    pub composer: Option<egui::Rect>,
    pub reader: Option<egui::Rect>,
}

impl InboxPage {
    pub fn panel_geometry(&self) -> InboxPanelGeometry {
        InboxPanelGeometry {
            rail: self.panel_rail_rect,
            list: self.panel_list_rect,
            copilot: self.panel_copilot_rect,
            composer: self.panel_composer_rect,
            reader: self.panel_reader_rect,
        }
    }
}

impl Default for InboxPage {
    fn default() -> Self {
        Self::new()
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = cap_chars(s, max);
        format!("{cut}…")
    }
}

fn recipient_label(ui: &mut egui::Ui, text: &str) {
    ui.vertical(|ui| {
        ui.set_min_width(34.0);
        ui.label(
            egui::RichText::new(text)
                .size(design::scale::text::SMALL)
                .color(design::tokens(ui).muted_foreground),
        );
    });
}

fn recipient_row_static(ui: &mut egui::Ui, label: &str, value: &str) {
    let t = design::tokens(ui);
    ui.horizontal(|ui| {
        recipient_label(ui, label);
        let w = ui.available_width();
        ui.allocate_ui(egui::vec2(w, design::scale::CONTROL_H), |ui| {
            egui::Frame::new()
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::new(1.0, t.border))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(
                    design::scale::space::CLUSTER as i8,
                    0,
                ))
                .show(ui, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(value)
                                    .size(design::scale::text::SMALL)
                                    .color(t.foreground),
                            )
                            .truncate(),
                        );
                    });
                });
        });
    });
}

fn recipient_row_input(ui: &mut egui::Ui, label: &str, buf: &mut String) {
    ui.horizontal(|ui| {
        recipient_label(ui, label);
        let w = ui.available_width();
        ui.add_sized(
            [w, design::scale::CONTROL_H],
            egui::TextEdit::singleline(buf).hint_text("comma-separated emails"),
        );
    });
}

fn compact_time(iso: &str) -> String {
    let mut out = iso.replacen('T', " ", 1);
    out.truncate(out.len().min(16));
    out
}

fn render_rich_spans(ui: &mut egui::Ui, spans: &[spp_core::richtext::Span]) {
    for span in spans {
        let mut rich = egui::RichText::new(spp_core::richtext::break_long_tokens(&span.text, 40));
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
            let size = match level {
                1 => design::scale::text::LEAD,
                2 => design::scale::text::CONTROL,
                _ => design::scale::text::SMALL,
            };
            rich = rich.size(size).strong();
        }
        if let Some(href) = &style.link {
            ui.hyperlink_to(rich, href.clone());
        } else {
            ui.label(rich);
        }
    }
}

fn message_role_of(thread_type: &str) -> (MessageRole, Option<&'static str>) {
    match thread_type {
        "customer" | "customer_message" => (MessageRole::Customer, None),
        "message" | "reply" => (MessageRole::Agent, None),
        "note" => (MessageRole::Note, None),
        "chat" => (MessageRole::Customer, Some("Chat")),
        "phone" => (MessageRole::Agent, Some("Phone")),
        kind if kind.starts_with("ai") => (MessageRole::Ai, Some("AI")),
        _ => (MessageRole::Agent, Some("Other")),
    }
}

fn saved_reply_list(conn: &rusqlite::Connection) -> Vec<(String, String)> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT name, COALESCE(NULLIF(text, ''), NULLIF(chat_text, ''), preview, name)
          FROM saved_replies WHERE deleted_at IS NULL ORDER BY name LIMIT 25",
    ) else {
        return Vec::new();
    };
    let rows = stmt.query_map([], |r| {
        let name: String = r.get(0)?;
        let text: Option<String> = r.get(1)?;
        Ok((name.clone(), text.unwrap_or(name)))
    });
    match rows {
        Ok(rows) => rows.filter_map(Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

pub(crate) fn submit_reply(
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
        if !provider_connected(conn) {
            return Err(NOT_CONNECTED.to_string());
        }
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
        let status_after = conn
            .query_row(
                "SELECT value FROM application_settings WHERE key = 'reply_status_after'",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok()
            .and_then(|raw| serde_json::from_str::<String>(&raw).ok().or(Some(raw)))
            .unwrap_or_else(|| "\"pending\"".to_string())
            .trim_matches('"')
            .to_string();
        let status_after = match status_after.as_str() {
            "open" | "waiting" => json!("open"),
            "keep" => serde_json::Value::Null,
            _ => json!("pending"),
        };
        let job_id = queue_job(
            conn,
            "create_reply",
            &json!({
                "conversationId": detail.id,
                "text": body,
                "draft": false,
                "cc": cc,
                "bcc": bcc,
                "statusAfter": status_after,
                "assignTo": serde_json::Value::Null,
            }),
            Some(detail.id),
            Some(&key),
        )?;
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
        Ok(_job_id) => {
            emit_updated(bus, detail.id, detail.number);
            ActionOutcome::Success(
                "Reply accepted — delivering to Help Scout (watch the status line).".into(),
            )
        }
        Err(reason) => ActionOutcome::Rejected(reason),
    }
}

pub(crate) fn submit_note(
    db: &DbHandle,
    bus: &EventBus,
    detail: &ConversationDetail,
    body: &str,
) -> ActionOutcome {
    let result: Result<i64, String> = db.with_write(|conn| {
        guard_eval_mode(conn)?;
        if !provider_connected(conn) {
            return Err(NOT_CONNECTED.to_string());
        }
        let thread_id =
            insert_local_note(conn, detail.id, body, None).map_err(|e| e.to_string())?;
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
                        body: Some(cap_chars(body, 200)),
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
        )?;
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
        Ok(_job_id) => {
            emit_updated(bus, detail.id, detail.number);
            ActionOutcome::Success(
                "Internal note added — posting to Help Scout (watch the status line).".into(),
            )
        }
        Err(reason) => ActionOutcome::Rejected(reason),
    }
}

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
        if !provider_connected(conn) {
            return Err(NOT_CONNECTED.to_string());
        }
        let outcome = spp_core::ticket_ops::execute(conn, &op).map_err(|e| e.to_string())?;
        if matches!(outcome, OperationResult::Success { .. }) {
            queue_job(conn, job_kind, payload, Some(detail.id), None)?;
        }
        Ok(outcome)
    });
    match result {
        Ok(OperationResult::Success { message }) => {
            emit_updated(bus, detail.id, detail.number);
            ActionOutcome::Success(format!("{message} — sending to Help Scout"))
        }
        Ok(OperationResult::Rejected { reason }) => ActionOutcome::Rejected(reason),
        Err(reason) => ActionOutcome::Rejected(reason),
    }
}

#[cfg(test)]
mod audp0_tests;
#[cfg(test)]
mod batch27_audit78_tests;
#[cfg(test)]
mod batch27_audit79_tests;
#[cfg(test)]
mod batch33_audit137_tests;
#[cfg(test)]
mod batch33_audit143_tests;
#[cfg(test)]
mod tests;
