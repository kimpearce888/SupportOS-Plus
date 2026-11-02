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
use spp_core::ticket_ops::{OperationResult, TicketOperation};

use crate::design::{
    self, AvatarSize, BadgeTone, ButtonVariant, ComposerAct, ComposerState, MessageRole,
};
use crate::icons::Icon;
use crate::mention_popup::{
    Directory, apply_completion, caret_query, char_index_to_byte, filter_matches,
};

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

pub const INBOX_PAGE_SIZE: u32 = 50;

#[must_use]
pub fn parse_number_entry(text: &str) -> Option<i64> {
    text.trim()
        .trim_start_matches('#')
        .trim()
        .parse::<i64>()
        .ok()
}

#[must_use]
pub fn filters_from_bar(bar: &FilterBar, page: u32, conn: &rusqlite::Connection) -> InboxFilters {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerMode {
    Reply,
    Note,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionOutcome {
    Success(String),
    Rejected(String),
}

impl ActionOutcome {
    #[must_use]
    pub fn is_rejected(&self) -> bool {
        matches!(self, Self::Rejected(_))
    }

    fn message(&self) -> &str {
        match self {
            Self::Success(m) | Self::Rejected(m) => m,
        }
    }
}

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

#[must_use]
pub fn tag_names(conn: &Connection) -> Vec<String> {
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
    detail_error: Option<String>,
    outbound_note: Option<(String, bool)>,
    status_message: Option<String>,
    status_is_error: bool,
    status_message_at: Option<std::time::Instant>,
    copilot_open: bool,
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

    pub fn set_vector_index(
        &mut self,
        index: Option<std::sync::Arc<spp_core::vectorstore_sqlitevec::VectorIndex>>,
    ) {
        self.vector_index = index;
    }

    pub fn invalidate(&mut self) {
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
        if self.selected.is_some() && self.detail.is_none() {
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
        self.coaching_review = None;
        self.qa_review = None;
        self.assistant_session = None;
        self.assistant_messages.clear();
        self.assistant_input.clear();
        self.assistant_pending = None;
        self.assistant_error = None;
        self.reload_detail(db);
    }

    pub fn composer_edited(&mut self, body: String) {
        if body.trim().is_empty() {
            self.inserted_draft = None;
        }
        self.composer_body = body;
    }

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
        self.set_status_message(outcome.message(), outcome.is_rejected());
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

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
            design::error_text(ui, &format!("error: {err}"));
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

        egui::Panel::left("inbox-views")
            .exact_size(172.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(t.sidebar)
                    .inner_margin(egui::Margin {
                        left: 10,
                        right: 10,
                        top: 12,
                        bottom: 12,
                    }),
            )
            .show(ui, |ui| {
                self.views_rail_ui(ui, db);
            });
        egui::Panel::left("inbox-list")
            .exact_size(330.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(t.card).inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 12,
                bottom: 8,
            }))
            .show(ui, |ui| {
                self.list_ui(ui, db, engine);
            });
        if self.copilot_open {
            egui::Panel::right("inbox-copilot")
                .exact_size(320.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(t.sidebar)
                        .inner_margin(egui::Margin {
                            left: 12,
                            right: 12,
                            top: 12,
                            bottom: 12,
                        }),
                )
                .show(ui, |ui| {
                    self.copilot_pane_ui(ui, db, detail.as_ref());
                });
        }
        if detail.is_some() {
            egui::Panel::bottom("inbox-composer")
                .frame(
                    egui::Frame::new()
                        .fill(t.background)
                        .inner_margin(egui::Margin {
                            left: 22,
                            right: 22,
                            top: 8,
                            bottom: 10,
                        }),
                )
                .show(ui, |ui| {
                    if let Some(d) = detail.clone() {
                        self.composer_pane_ui(ui, db, bus, &d);
                    }
                });
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(egui::Margin {
                        left: 22,
                        right: 22,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show(ui, |ui| {
                self.reader_ui(ui, db, bus, detail);
            });
    }

    fn views_rail_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.label(
            egui::RichText::new("INBOX VIEWS")
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

    #[allow(clippy::too_many_lines)]
    fn list_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        engine: Option<&std::sync::Arc<SyncEngine>>,
    ) {
        let t = design::tokens(ui);
        ui.label(
            egui::RichText::new(self.view_pick.label())
                .size(design::scale::text::SECTION)
                .strong()
                .color(t.foreground),
        );
        let subtitle = if self.total > INBOX_PAGE_SIZE {
            let from = self.page * INBOX_PAGE_SIZE + 1;
            let to = from - 1 + self.list.len() as u32;
            format!("{from}-{to} of {} conversations", self.total)
        } else {
            format!("{} of {} conversations", self.list.len(), self.total)
        };
        design::hint_text(ui, &subtitle);
        ui.add_space(design::scale::space::SP1);

        let mut apply_entered = false;
        let apply = design::filter_bar(
            ui,
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    let mut search = self.search_edit.clone();
                    let response = ui.add_sized(
                        [140.0, 28.0],
                        egui::TextEdit::singleline(&mut search).hint_text("search conversations…"),
                    );
                    if response.changed() {
                        self.search_edit = search;
                    }
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        apply_entered = true;
                    }
                    let mut number = self.number_edit.clone();
                    let number_response = ui.add_sized(
                        [56.0, 28.0],
                        egui::TextEdit::singleline(&mut number).hint_text("#"),
                    );
                    if number_response.changed() {
                        self.number_edit = number;
                    }
                    if number_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        apply_entered = true;
                    }
                    if self.show_filters {
                        egui::ComboBox::from_id_salt("inbox-mailbox")
                            .width(150.0)
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
                            .width(120.0)
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
                                ui.selectable_value(
                                    &mut self.channel_pick,
                                    Some("email".into()),
                                    "Email",
                                );
                                ui.selectable_value(
                                    &mut self.channel_pick,
                                    Some("chat".into()),
                                    "Chat",
                                );
                            });
                        egui::ComboBox::from_id_salt("inbox-tag")
                            .width(130.0)
                            .selected_text(
                                self.tag_pick.clone().unwrap_or_else(|| "Any tag".into()),
                            )
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.tag_pick, None, "Any tag");
                                for tag in self.tags.clone() {
                                    ui.selectable_value(&mut self.tag_pick, Some(tag.clone()), tag);
                                }
                            });
                    }
                });
            },
            "apply",
        );

        ui.horizontal(|ui| {
            if design::button(
                ui,
                if self.show_filters {
                    "hide filters"
                } else {
                    "filters"
                },
                ButtonVariant::Text,
            )
            .clicked()
            {
                self.show_filters = !self.show_filters;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let syncing = engine.is_some_and(|e| e.is_running());
                if design::button_with_icon(
                    ui,
                    Icon::Refresh,
                    if syncing { "syncing…" } else { "Sync now" },
                    ButtonVariant::Outline,
                )
                .on_hover_text(
                    "pull new and updated conversations from Help Scout right now \
                     (also happens automatically: 30 s while busy, backing off \
                     when quiet)",
                )
                .clicked()
                    && let Some(engine) = engine
                {
                    if !crate::pages::sync_health::spawn_sync_action(
                        db,
                        engine,
                        crate::pages::sync_health::SyncAction::Incremental,
                    ) {
                        self.set_status_message(
                            "A sync is already running — watch the ring in the header.",
                            false,
                        );
                    } else {
                        self.set_status_message("Syncing…", false);
                    }
                }
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
                    self.page = 0;
                    self.reload_list(db);
                }
            });
        });
        if apply.clicked() || apply_entered {
            self.apply_filters(db);
        }
        ui.add_space(design::scale::space::SP2);

        let mut clicked: Option<i64> = None;
        egui::ScrollArea::vertical()
            .id_salt("inbox-list-scroll")
            .show(ui, |ui| {
                if self.list.is_empty() {
                    design::empty_state_slim(
                        ui,
                        Icon::Inbox,
                        "No conversations here",
                        "Adjust the filters or run a sync — conversations arrive from \
                         Help Scout.",
                    );
                }
                for item in self.list.clone() {
                    let selected = self.selected == Some(item.id);
                    let row_id = egui::Id::unique(("inbox-row", item.id));
                    let hovered = ui.ctx().read_response(row_id).is_some_and(|r| r.hovered());
                    let fill = if selected {
                        t.selection
                    } else if hovered {
                        t.accent
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    let subject = format!(
                        "#{}  {}",
                        item.number,
                        item.subject.as_deref().unwrap_or("(no subject)"),
                    );
                    let response = egui::Frame::new()
                        .fill(fill)
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::same(8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(
                                            item.customer_name
                                                .as_deref()
                                                .unwrap_or("(no customer)"),
                                        )
                                        .size(design::scale::text::CONTROL)
                                        .strong(),
                                    )
                                    .truncate(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(
                                                item.updated_at
                                                    .as_deref()
                                                    .map(compact_time)
                                                    .unwrap_or_default(),
                                            )
                                            .size(design::scale::text::META)
                                            .color(t.muted_foreground),
                                        );
                                    },
                                );
                            });
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&subject)
                                        .size(design::scale::text::SMALL)
                                        .color(t.foreground),
                                )
                                .truncate(),
                            );
                            ui.horizontal(|ui| {
                                let assignee_label = item.assignee_name.clone().or_else(|| {
                                    item.assigned_team_name
                                        .as_deref()
                                        .map(|team| format!("team: {team}"))
                                });
                                design::assignee(ui, assignee_label.as_deref());
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let (tone, label) = design::status_tone(&item.status);
                                        design::badge(ui, label, tone);
                                    },
                                );
                            });
                        })
                        .response;
                    let response = ui
                        .interact(response.rect, row_id, egui::Sense::click())
                        .on_hover_text(item.preview.clone().unwrap_or_else(|| subject.clone()));
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::Role::ListItem,
                            ui.is_enabled(),
                            selected,
                            subject.clone(),
                        )
                    });
                    if response.clicked() {
                        clicked = Some(item.id);
                    }
                }
            });
        if let Some(id) = clicked {
            self.select(db, id);
        }
        let pages = self.total.div_ceil(INBOX_PAGE_SIZE);
        let delta = design::pager(ui, self.page, pages);
        if delta != 0
            && let Some(next) = self.page.checked_add_signed(delta)
            && next < pages
        {
            self.page = next;
            self.reload_list(db);
        }
    }

    fn apply_filters(&mut self, db: &DbHandle) {
        let trimmed = self.search_edit.trim();
        self.bar.query = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
        self.bar.number = parse_number_entry(&self.number_edit);
        self.bar.mailbox = self.mailbox_pick;
        self.bar.channel = self.channel_pick.clone();
        self.bar.tag = self.tag_pick.clone();
        self.page = 0;
        self.reload_list(db);
    }

    fn refresh_view_counts(&mut self, db: &DbHandle) {
        for (slot, view) in self.view_counts.iter_mut().zip(ViewFilter::ALL) {
            let mut bar = self.bar.clone();
            bar.view = Some(view);
            let filters = db.with_read(|conn| {
                let mut f = filters_from_bar(&bar, 0, conn);
                f.limit = Some(1);
                f
            });
            *slot = db
                .with_read(|conn| {
                    spp_core::inbox::list_conversations(conn, &filters).map(|(_, total)| total)
                })
                .unwrap_or(0);
        }
    }

    fn reader_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        bus: &EventBus,
        detail: Option<ConversationDetail>,
    ) {
        let Some(detail) = detail else {
            if let Some(err) = &self.detail_error {
                design::empty_state(
                    ui,
                    Icon::AlertCircle,
                    "This conversation could not be loaded",
                    err,
                );
                return;
            }
            design::empty_state(
                ui,
                Icon::Inbox,
                "Select a conversation",
                "Pick a conversation from the list to read the thread and reply.",
            );
            return;
        };
        self.reader_head_ui(ui, db, &detail);
        ui.add_space(design::scale::space::SP2);
        egui::ScrollArea::vertical()
            .id_salt("inbox-reader-scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                self.action_bar_ui(ui, db, bus, &detail);
                ui.add_space(design::scale::space::SP2);
                self.triage_ui(ui, db);
                if !self.briefing.is_empty() {
                    self.briefing_ui(ui);
                    ui.add_space(design::scale::space::SP2);
                }
                self.thread_ui(ui, &detail);
                ui.add_space(design::scale::space::SP2);
                if let Some(qa) = self.qa_review.clone() {
                    self.qa_card_ui(ui, db, &qa, detail.id);
                    ui.add_space(design::scale::space::SP2);
                }
                if let Some(review) = self.coaching_review.clone() {
                    self.coaching_review_ui(ui, &review);
                    ui.add_space(design::scale::space::SP2);
                }
            });
    }

    fn reader_head_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, detail: &ConversationDetail) {
        let name = detail
            .customer_name
            .clone()
            .unwrap_or_else(|| "customer".into());
        let initials: String = name
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect();
        ui.horizontal(|ui| {
            design::avatar(
                ui,
                AvatarSize::Lg,
                &initials.to_uppercase(),
                BadgeTone::Neutral,
            );
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "#{}  {}",
                        detail.number,
                        detail.subject.as_deref().unwrap_or("(no subject)")
                    ))
                    .size(design::scale::text::SECTION)
                    .strong(),
                );
                let who = match detail.customer_email.as_deref() {
                    Some(email) if !email.trim().is_empty() => {
                        format!(
                            "{} <{}>",
                            detail.customer_name.as_deref().unwrap_or("-"),
                            email
                        )
                    }
                    _ => detail.customer_name.as_deref().unwrap_or("-").to_string(),
                };
                design::hint_text(
                    ui,
                    &format!(
                        "{} · mailbox: {} · priority: {}",
                        who,
                        detail.mailbox_name.as_deref().unwrap_or("-"),
                        detail.priority.as_deref().unwrap_or("none"),
                    ),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (tone, label) = design::status_tone(&detail.status);
                design::badge(ui, label, tone);
                if design::chip_with_icon(ui, self.copilot_open, Icon::Sparkles, "Copilot")
                    .on_hover_text("Copilot: local summary, similar past issues, suggested reply")
                    .clicked()
                {
                    self.copilot_open = !self.copilot_open;
                    self.copilot = None;
                    self.copilot_error = None;
                    self.reload_detail(db);
                }
                if design::icon_button(
                    ui,
                    Icon::Refresh,
                    ButtonVariant::Ghost,
                    "refresh this conversation from Help Scout",
                )
                .clicked()
                {
                    db.with_write(|conn| {
                        let _ = spp_core::jobs::enqueue(
                            conn,
                            "sync_conversation",
                            &json!({ "remoteId": detail.remote_id }).to_string(),
                        );
                    });
                    self.set_status_message("Refreshing from Help Scout…", false);
                }
            });
        });
    }

    fn action_bar_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        bus: &EventBus,
        detail: &ConversationDetail,
    ) {
        design::card(ui, |ui| {
            ui.horizontal(|ui| {
                design::field_label(ui, "status:");
                for status in ["active", "pending", "closed", "spam"] {
                    if design::chip(ui, detail.status == status, status)
                        .on_hover_text("applied locally and sent to Help Scout")
                        .clicked()
                    {
                        self.change_status(db, bus, status);
                    }
                }
            });
            ui.horizontal(|ui| {
                design::field_label(ui, "assign:");
                let assign_label: String = detail
                    .assignee_name
                    .clone()
                    .or_else(|| {
                        detail
                            .assigned_team_name
                            .as_deref()
                            .map(|team| format!("team: {team}"))
                    })
                    .unwrap_or_else(|| "unassigned".into());
                egui::ComboBox::from_id_salt("inbox-assign")
                    .width(200.0)
                    .selected_text(assign_label.clone())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.assign_pick, None, "unassigned");
                        for (id, name) in self.users.clone() {
                            ui.selectable_value(&mut self.assign_pick, Some(id), name);
                        }
                    })
                    .response
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            ui.is_enabled(),
                            format!("assign: {assign_label}"),
                        )
                    });
                if design::button(ui, "assign", ButtonVariant::Secondary).clicked() {
                    self.assign(db, bus, self.assign_pick);
                }
            });
            self.expire_status_message();
            if let Some(msg) = &self.status_message {
                if self.status_is_error {
                    design::error_text(ui, msg);
                } else {
                    design::success_text(ui, msg);
                }
            }
            if let Some((note, is_error)) = &self.outbound_note {
                if *is_error {
                    design::error_text(ui, note);
                } else {
                    design::hint_text(ui, note);
                }
            }
        });
    }

    fn triage_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.triage.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for chip in &self.triage {
                let tone = match (chip.label, chip.value.as_str()) {
                    ("urgency", "high" | "critical") => BadgeTone::Destructive,
                    ("urgency", "moderate") => BadgeTone::Warning,
                    ("frustration cues", "strong" | "moderate") => BadgeTone::Warning,
                    ("escalation signal", "true") => BadgeTone::Destructive,
                    (_, _) => BadgeTone::Info,
                };
                let text = if chip.confidence == "heuristic" {
                    format!("{}: {}", chip.label, chip.value)
                } else if chip.source == "human" {
                    format!("{}: {} ✓", chip.label, chip.value)
                } else {
                    format!("{}: {} · {}", chip.label, chip.value, chip.confidence)
                };
                let response = design::badge(ui, &text, tone);
                let hint = format!(
                    "source: {} · confidence: {}. {}",
                    chip.source,
                    chip.confidence,
                    if chip.editable {
                        "click to correct"
                    } else {
                        "deterministic estimate — not stored"
                    },
                );
                let mut response = response.on_hover_text(hint);
                if chip.editable {
                    response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
                }
                if chip.editable && response.clicked() {
                    self.editing_chip = Some(chip.key);
                }
            }
        });
        if let Some(key) = self.editing_chip {
            let chip = self.triage.iter().find(|c| c.key == key).cloned();
            if let Some(chip) = chip {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(format!("set {}:", chip.label.to_lowercase()));
                    if ui.button("unknown (clear)").clicked() {
                        self.override_chip_value(db, &chip, "");
                        self.editing_chip = None;
                    }
                    for value in key.values() {
                        if ui.button(value.to_string()).clicked() {
                            self.override_chip_value(db, &chip, value);
                            self.editing_chip = None;
                        }
                    }
                    if ui.button("cancel").clicked() {
                        self.editing_chip = None;
                    }
                });
            } else {
                self.editing_chip = None;
            }
        }
        ui.add_space(design::scale::space::CLUSTER);
    }

    fn briefing_ui(&self, ui: &mut egui::Ui) {
        let t = design::tokens(ui);
        design::card_titled(
            ui,
            "Know before you reply",
            Some("durable customer facts extracted from past conversations"),
            |ui| {
                ui.horizontal(|ui| {
                    Icon::Sparkles.show(ui, 14.0, t.muted_foreground);
                    design::badge(ui, "AI", BadgeTone::Ai);
                });
                for fact in &self.briefing {
                    ui.label(
                        egui::RichText::new(format!("· {}: {}", fact.key, fact.value))
                            .small()
                            .color(t.foreground),
                    );
                }
            },
        );
    }

    #[allow(clippy::too_many_lines)]
    fn copilot_pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        detail: Option<&ConversationDetail>,
    ) {
        ui.horizontal(|ui| {
            Icon::Sparkles.show(ui, 14.0, design::tokens(ui).ai);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Copilot")
                        .size(design::scale::text::CONTROL)
                        .strong(),
                );
                design::hint_text(ui, "local · no configuration needed");
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if design::icon_button(ui, Icon::X, ButtonVariant::Ghost, "close the copilot panel")
                    .clicked()
                {
                    self.copilot_open = false;
                }
            });
        });
        design::divider(ui);
        let Some(detail) = detail else {
            design::hint_text(
                ui,
                "Select a conversation — the copilot reads the local mirror for it.",
            );
            return;
        };
        egui::ScrollArea::vertical()
            .id_salt("inbox-copilot-scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if let Some(err) = &self.copilot_error {
                    design::error_text(ui, &format!("copilot: {err}"));
                    return;
                }
                let Some(report) = self.copilot.clone() else {
                    design::hint_text(ui, "Analyzing the conversation…");
                    return;
                };
                let a = &report.analysis;

                design::card_titled(ui, "Signals", None, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        design::badge(ui, &a.intent, design::BadgeTone::Info);
                        let urgency_tone = match a.urgency.as_str() {
                            "high" => design::BadgeTone::Destructive,
                            "medium" => design::BadgeTone::Warning,
                            _ => design::BadgeTone::Neutral,
                        };
                        design::badge(ui, &format!("urgency: {}", a.urgency), urgency_tone);
                        let sentiment_tone = match a.sentiment.as_str() {
                            "negative" => design::BadgeTone::Warning,
                            "positive" => design::BadgeTone::Success,
                            _ => design::BadgeTone::Neutral,
                        };
                        design::badge(
                            ui,
                            &format!("sentiment: {}", a.sentiment),
                            sentiment_tone,
                        );
                        if a.escalation {
                            design::badge(
                                ui,
                                "escalation risk",
                                design::BadgeTone::Destructive,
                            );
                        }
                    });
                });
                ui.add_space(design::scale::space::SP2);

                if !a.summary.is_empty() || !a.questions.is_empty() {
                    design::card_titled(ui, "Summary", None, |ui| {
                        for line in &a.summary {
                            ui.label(
                                egui::RichText::new(format!(
                                    "• {}",
                                    spp_core::richtext::break_long_tokens(line, 40)
                                ))
                                .small()
                                .color(design::tokens(ui).foreground),
                            );
                        }
                        for q in &a.questions {
                            ui.label(
                                egui::RichText::new(format!(
                                    "? {}",
                                    spp_core::richtext::break_long_tokens(q, 40)
                                ))
                                .small()
                                .color(design::tokens(ui).muted_foreground),
                            );
                        }
                    });
                    ui.add_space(design::scale::space::SP2);
                }

                design::hint_text(
                    ui,
                    &format!(
                        "History: {} conversation(s) on file{}",
                        report.history.previous_conversations,
                        report
                            .history
                            .last_contact
                            .as_deref()
                            .map(|t| format!(" · last contact {t}"))
                            .unwrap_or_default()
                    ),
                );
                ui.add_space(design::scale::space::SP1);

                if report.similar.is_empty() {
                    design::hint_text(ui, "No similar past conversations found yet.");
                } else {
                    design::card_titled(ui, "Similar past issues", None, |ui| {
                        for s in report.similar.iter().take(3) {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!(
                                            "#{} {}",
                                            s.number, s.subject
                                        ))
                                        .small()
                                        .strong()
                                        .color(design::tokens(ui).foreground),
                                    )
                                    .truncate(),
                                );
                                design::badge(
                                    ui,
                                    &format!("{}%", (s.score * 100.0) as u32),
                                    design::BadgeTone::Ai,
                                );
                            });
                            ui.label(
                                egui::RichText::new(format!(
                                    "resolved with: {}",
                                    truncate_chars(&s.resolution, 160)
                                ))
                                .small()
                                .color(design::tokens(ui).muted_foreground),
                            );
                            if !s.why.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "why: {}",
                                        s.why.join(", ")
                                    ))
                                    .small()
                                    .weak(),
                                );
                            }
                            ui.add_space(design::scale::space::SP1);
                        }
                    });
                    ui.add_space(design::scale::space::SP2);
                }

                if !report.articles.is_empty() {
                    design::card_titled(ui, "Relevant help articles", None, |ui| {
                        for art in report.articles.iter().take(3) {
                            ui.label(
                                egui::RichText::new(format!(
                                    "• {} — {}",
                                    art.name,
                                    truncate_chars(&art.snippet, 120)
                                ))
                                .small()
                                .color(design::tokens(ui).muted_foreground),
                            );
                        }
                    });
                    ui.add_space(design::scale::space::SP2);
                }

                if design::cop_suggest(
                    ui,
                    "Suggested reply",
                    std::slice::from_ref(&report.suggested_reply),
                    &["Insert into reply"],
                )
                .is_some()
                {
                    self.composer_mode = ComposerMode::Reply;
                    self.inserted_draft = None;
                    self.composer_body = report.suggested_reply.clone();
                    self.draft_saved = false;
                }
                ui.add_space(design::scale::space::SP2);
                if let Some((draft_id, draft, model)) =
                    db.with_read(|conn| spp_core::ai_local::latest_ai_draft_row(conn, detail.id))
                {
                    match design::cop_suggest(
                        ui,
                        &format!("LM Studio draft ({model})"),
                        std::slice::from_ref(&draft),
                        &["Insert LM Studio draft", "Discard draft"],
                    ) {
                        Some(0) => {
                            self.composer_mode = ComposerMode::Reply;
                            self.inserted_draft = Some((draft_id, draft.clone()));
                            self.composer_body = draft.clone();
                            self.draft_saved = false;
                        }
                        Some(1) => {
                            let rejected = db.with_write(|conn| {
                                spp_core::ai_pipeline::record_feedback(
                                    conn, draft_id, &draft, &draft, false,
                                )
                                .and_then(|()| {
                                    spp_core::ai_pipeline::set_draft_state(
                                        conn, draft_id, "rejected",
                                    )
                                })
                            });
                            match rejected {
                                Ok(()) => self.set_status_message(
                                    "Draft marked rejected — feedback recorded.",
                                    false,
                                ),
                                Err(e) => {
                                    self.set_status_message(format!("Reject failed: {e}"), true)
                                }
                            }
                        }
                        _ => {}
                    }
                    if self
                        .inserted_draft
                        .as_ref()
                        .is_some_and(|(id, _)| *id == draft_id)
                    {
                        design::badge(ui, "in composer", BadgeTone::Info);
                    }
                }
                ui.add_space(design::scale::space::SP2);
                design::divider(ui);
                design::section_title(ui, "Ask the assistant");
                let (feature_on, agent_ok) = db.with_read(|conn| {
                    (
                        spp_core::ai_hub::ai_feature_enabled(conn, "copilot"),
                        !matches!(
                            spp_core::ai_hub::backend_for_role(
                                conn,
                                spp_core::ai_hub::AiRole::Agent,
                            ),
                            spp_core::ai_pipeline::AiBackend::Disabled,
                        ),
                    )
                });
                if !feature_on {
                    design::hint_text(ui, "Copilot is switched off (AI Center > Features).");
                } else if !agent_ok {
                    design::hint_text(
                        ui,
                        "Route the Agent role to an endpoint to ask questions (AI Center > Model-role routing).",
                    );
                } else {
                    if let Some(err) = self.assistant_error.clone() {
                        design::error_text(ui, &format!("assistant: {err}"));
                    }
                    if !self.assistant_messages.is_empty() {
                        egui::ScrollArea::vertical()
                            .id_salt("assistant-messages")
                            .max_height(180.0)
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                for message in &self.assistant_messages {
                                    let role = message
                                        .get("role")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("?");
                                    if role == "tool" {
                                        continue;
                                    }
                                    let content = message
                                        .get("content")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    design::cop_chat(ui, role == "assistant", content);
                                }
                            });
                    }
                    let busy = self.assistant_pending.is_some();
                    ui.horizontal(|ui| {
                        let mut question = self.assistant_input.clone();
                        let response = ui.add_sized(
                            [ui.available_width() - 64.0, 26.0],
                            egui::TextEdit::singleline(&mut question)
                                .hint_text("ask about this conversation…"),
                        );
                        if response.changed() {
                            self.assistant_input = question;
                        }
                        if (design::button(
                            ui,
                            if busy { "thinking…" } else { "ask" },
                            ButtonVariant::Ai,
                        )
                        .clicked()
                            && !busy)
                            || (ui.input(|i| i.key_pressed(egui::Key::Enter))
                                && response.lost_focus()
                                && !busy)
                        {
                            let question = self.assistant_input.trim().to_string();
                            if !question.is_empty() {
                                let payload = json!({
                                    "question": question,
                                    "conversationId": detail.id,
                                    "sessionId": self.assistant_session,
                                });
                                let job = db.with_write(|conn| {
                                    spp_core::jobs::enqueue_on(
                                        conn,
                                        "ai",
                                        "copilot_chat",
                                        &payload.to_string(),
                                        3,
                                        2,
                                    )
                                });
                                match job {
                                    Ok(job_id) => {
                                        self.assistant_pending = Some(job_id);
                                        self.assistant_input.clear();
                                    }
                                    Err(e) => self.assistant_error = Some(e.to_string()),
                                }
                            }
                        }
                    });
                    let starters = db
                        .with_read(|conn| spp_core::copilot::starter_questions(conn, detail.id));
                    if !starters.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            for (question, hint) in starters.iter().take(4) {
                                if design::chip(ui, false, &truncate_chars(question, 34))
                                    .on_hover_text(hint)
                                    .clicked()
                                    && self.assistant_pending.is_none()
                                {
                                    self.assistant_input = question.clone();
                                }
                            }
                        });
                    }
                }
            });
    }

    fn insert_ai_draft(&mut self, db: &DbHandle, conversation_id: i64) {
        self.composer_mode = ComposerMode::Reply;
        if let Some((draft_id, draft, _model)) =
            db.with_read(|conn| spp_core::ai_local::latest_ai_draft_row(conn, conversation_id))
        {
            self.inserted_draft = Some((draft_id, draft.clone()));
            self.composer_body = draft;
            self.set_status_message("Latest AI draft inserted — review before sending.", false);
        } else if let Some(text) = self.copilot.as_ref().map(|r| r.suggested_reply.clone()) {
            if text.trim().is_empty() {
                self.set_status_message("The copilot has no suggested reply yet.", false);
                return;
            }
            self.inserted_draft = None;
            self.composer_body = text;
            self.set_status_message("Suggested reply inserted — review before sending.", false);
        } else {
            self.set_status_message(
                "No AI draft yet — open Copilot to analyze the conversation first.",
                false,
            );
        }
        self.draft_saved = false;
    }

    fn enqueue_composer_ai(
        &mut self,
        db: &DbHandle,
        kind: &'static str,
        payload: serde_json::Value,
        conversation_id: i64,
    ) {
        let job = db.with_write(|conn| {
            spp_core::jobs::enqueue_on(conn, "ai", kind, &payload.to_string(), 3, 2)
        });
        match job {
            Ok(job_id) => {
                self.pending_ai = Some(PendingComposerAi {
                    job_id,
                    kind,
                    conversation_id,
                    origin_body: self.composer_body.clone(),
                });
                self.set_status_message(
                    format!("{kind} queued — the result lands here when it finishes."),
                    false,
                );
            }
            Err(e) => {
                self.set_status_message(format!("Could not queue {kind}: {e}"), true);
            }
        }
    }

    fn poll_pending_ai(&mut self, db: &DbHandle) {
        let Some(p) = self.pending_ai.clone() else {
            return;
        };
        let (status, error, result): (String, Option<String>, Option<String>) =
            db.with_read(|conn| {
                conn.query_row(
                    "SELECT status, error, result FROM jobs WHERE id = ?1",
                    rusqlite::params![p.job_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap_or(("missing".into(), None, None))
            });
        match status.as_str() {
            "queued" | "running" => {}
            "completed" => {
                self.pending_ai = None;
                match p.kind {
                    "rewrite_composer" | "translate_composer" => {
                        if self.composer_body != p.origin_body {
                            self.set_status_message(
                                "Transform finished, but you edited the draft while it ran — \
                                 your text was kept.",
                                false,
                            );
                            return;
                        }
                        match result {
                            Some(text) if !text.trim().is_empty() => {
                                self.composer_body = text;
                                self.set_status_message(
                                    "Composer updated by the AI transform.",
                                    false,
                                );
                            }
                            _ => {
                                self.set_status_message(
                                    "The AI transform produced no result — the feature is off or no Draft role endpoint is routed.",
                                    true,
                                );
                            }
                        }
                    }
                    "review_draft" => {
                        self.coaching_review = db.with_read(|conn| {
                            spp_core::coaching::get(conn, p.conversation_id)
                                .ok()
                                .flatten()
                        });
                        self.set_status_message(
                            "Draft review finished — see the coaching card.",
                            false,
                        );
                    }
                    "qa_ai_layer" => {
                        self.qa_review = db.with_read(|conn| {
                            spp_core::quality::get_qa(conn, p.conversation_id, true)
                        });
                        self.set_status_message("Post-resolution QA AI review finished.", false);
                    }
                    _ => {}
                }
            }
            _ => {
                self.pending_ai = None;
                self.set_status_message(
                    format!(
                        "AI job failed: {}",
                        error.unwrap_or_else(|| "unknown error".into())
                    ),
                    true,
                );
            }
        }
    }

    fn poll_assistant(&mut self, db: &DbHandle) {
        let Some(job_id) = self.assistant_pending else {
            return;
        };
        let (status, error): (String, Option<String>) = db.with_read(|conn| {
            conn.query_row(
                "SELECT status, error FROM jobs WHERE id = ?1",
                rusqlite::params![job_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or(("missing".into(), None))
        });
        let Some(conv_id) = self.selected else {
            self.assistant_pending = None;
            return;
        };
        match status.as_str() {
            "queued" | "running" => {}
            "completed" => {
                self.assistant_pending = None;
                self.assistant_error = None;
                let (session, messages) = db.with_read(|conn| {
                    let session: Option<i64> = conn
                        .query_row(
                            "SELECT id FROM copilot_sessions WHERE conversation_id = ?1
                              ORDER BY updated_at DESC LIMIT 1",
                            rusqlite::params![conv_id],
                            |r| r.get(0),
                        )
                        .ok();
                    let messages = session
                        .map(|sid| spp_core::copilot::list_messages(conn, sid))
                        .unwrap_or_default();
                    (session, messages)
                });
                self.assistant_session = session;
                self.assistant_messages = messages;
            }
            _ => {
                self.assistant_pending = None;
                self.assistant_error = Some(
                    error.unwrap_or_else(|| "the assistant job failed without an error".into()),
                );
            }
        }
    }

    fn thread_ui(&mut self, ui: &mut egui::Ui, detail: &ConversationDetail) {
        if detail.thread.is_empty() {
            design::empty_state_slim(
                ui,
                Icon::Inbox,
                "No messages yet",
                "The thread has no entries in the local mirror yet.",
            );
            return;
        }
        let mut last_day: Option<String> = None;
        for entry in &detail.thread {
            let day: String = entry.created_at.chars().take(10).collect();
            if last_day.as_deref() != Some(day.as_str()) {
                design::day_divider(ui, &day);
                last_day = Some(day);
            }
            let (role, role_label) = message_role_of(&entry.thread_type);
            let actor = entry
                .actor_name
                .clone()
                .unwrap_or_else(|| entry.actor_type.clone());
            let time = compact_time(&entry.created_at);
            let spans = spp_core::richtext::html_to_spans(entry.body.as_deref().unwrap_or(""));
            design::message_card_with(
                ui,
                role,
                &actor,
                role_label,
                &time,
                move |ui| render_rich_spans(ui, &spans),
                |_ui| {},
            );
            ui.add_space(design::scale::space::SP2);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn composer_pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        bus: &EventBus,
        detail: &ConversationDetail,
    ) {
        let conv_id = detail.id;
        let busy = self.pending_ai.is_some();
        ui.horizontal_wrapped(|ui| {
            if design::chip_with_icon(
                ui,
                false,
                Icon::Sparkles,
                if busy {
                    "AI working…"
                } else {
                    "Review draft"
                },
            )
            .on_hover_text(
                "run the 9 deterministic coaching checks (plus the AI layer when a model is \
                 routed) — advisory only",
            )
            .clicked()
                && !busy
                && !self.composer_body.trim().is_empty()
            {
                self.enqueue_composer_ai(
                    db,
                    "review_draft",
                    json!({
                        "conversationId": conv_id,
                        "draft": self.composer_body,
                    }),
                    conv_id,
                );
            }
            let gen_ok = db.with_read(spp_core::ai_local::generative_backend_available);
            if gen_ok && !busy && !self.composer_body.trim().is_empty() {
                for (instruction, label) in [
                    ("shorten", "Shorten"),
                    ("expand", "Expand"),
                    ("warmer", "Warmer"),
                ] {
                    if design::chip(ui, false, label)
                        .on_hover_text("rewrite the composer text with the Draft role model")
                        .clicked()
                    {
                        self.enqueue_composer_ai(
                            db,
                            "rewrite_composer",
                            json!({
                                "conversationId": conv_id,
                                "text": self.composer_body,
                                "instruction": instruction,
                            }),
                            conv_id,
                        );
                    }
                }
                egui::ComboBox::from_id_salt("composer-translate-lang")
                    .selected_text(self.translate_lang.as_str())
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        for (code, name) in spp_core::translation::SUPPORTED_LANGUAGES {
                            ui.selectable_value(
                                &mut self.translate_lang,
                                code.to_string(),
                                format!("{code} — {name}"),
                            );
                        }
                    });
                if design::chip(ui, false, "Translate")
                    .on_hover_text(
                        "translate the composer text to the picked language (cached locally)",
                    )
                    .clicked()
                {
                    let text: String = self.composer_body.chars().take(8000).collect();
                    self.enqueue_composer_ai(
                        db,
                        "translate_composer",
                        json!({
                            "conversationId": conv_id,
                            "text": text,
                            "targetLang": self.translate_lang,
                        }),
                        conv_id,
                    );
                }
            }
        });
        ui.add_space(design::scale::space::SP1);

        let mention_len = caret_query(&self.composer_body, self.mention_caret)
            .map_or(0, |(q, _)| filter_matches(&self.directory, &q).len());
        if mention_len > 0 && self.composer_focused {
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
                self.mention_active =
                    crate::palette::move_selection(self.mention_active, mention_len, 1);
            }
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
                self.mention_active =
                    crate::palette::move_selection(self.mention_active, mention_len, -1);
            }
        }

        let saved_reply_names: Vec<String> = self
            .saved_replies
            .iter()
            .map(|(name, _)| name.clone())
            .collect();
        let mut state = ComposerState {
            mode: match self.composer_mode {
                ComposerMode::Reply => design::ComposerMode::Reply,
                ComposerMode::Note => design::ComposerMode::Note,
            },
            draft: self.composer_body.clone(),
            draft_saved: self.draft_saved,
            caret: None,
            focused: false,
        };
        let act = design::composer(ui, "inbox", &mut state, &saved_reply_names);
        self.composer_focused = state.focused;
        if let Some(caret) = state.caret {
            self.mention_caret = char_index_to_byte(&state.draft, caret);
        }
        match act {
            Some(ComposerAct::Send(mode, text)) => {
                self.composer_mode = match mode {
                    design::ComposerMode::Reply => ComposerMode::Reply,
                    design::ComposerMode::Note => ComposerMode::Note,
                };
                self.composer_body = text;
                self.draft_saved = false;
                self.submit_composer(db, bus);
            }
            Some(ComposerAct::SavedReply(index)) => {
                if let Some((_, text)) = self.saved_replies.get(index) {
                    let text = text.clone();
                    self.composer_edited(text);
                    self.draft_saved = true;
                }
            }
            Some(ComposerAct::AiInsert) => self.insert_ai_draft(db, conv_id),
            None => {
                if state.draft != self.composer_body {
                    self.mention_active = 0;
                    let body = state.draft.clone();
                    self.composer_edited(body);
                    self.draft_saved = true;
                }
            }
        }

        if self.composer_mode == ComposerMode::Reply {
            ui.horizontal(|ui| {
                let mut cc = self.composer_cc.clone();
                let response = ui.add_sized(
                    [ui.available_width(), 28.0],
                    egui::TextEdit::singleline(&mut cc).hint_text("Cc (comma-separated emails)"),
                );
                let _ = response;
                self.composer_cc = cc;
            });
            ui.horizontal(|ui| {
                let mut bcc = self.composer_bcc.clone();
                ui.add_sized(
                    [ui.available_width(), 28.0],
                    egui::TextEdit::singleline(&mut bcc).hint_text("Bcc (comma-separated emails)"),
                );
                self.composer_bcc = bcc;
            });
        }

        let body = self.composer_body.clone();
        if let Some((query, start)) = caret_query(&body, self.mention_caret) {
            let matches = filter_matches(&self.directory, &query);
            if !matches.is_empty() {
                design::section_title(ui, "@ mentions");
                egui::Grid::new("mention-popup")
                    .num_columns(1)
                    .show(ui, |ui| {
                        for (i, (token, display, _)) in matches.iter().enumerate() {
                            let picked =
                                design::chip(ui, self.mention_active == i, display.as_str())
                                    .clicked();
                            if picked {
                                let completed =
                                    apply_completion(&body, start, token, self.mention_caret);
                                self.composer_body = completed;
                            }
                        }
                    });
                design::hint_text(
                    ui,
                    "Enter/Tab completes the highlighted entry when typed; click to pick",
                );
                if ui.input(|i| i.key_pressed(egui::Key::Tab) || i.key_pressed(egui::Key::Enter))
                    && self.composer_focused
                    && let Some((token, _, _)) =
                        matches.get(self.mention_active.min(matches.len() - 1))
                {
                    let completed = apply_completion(&body, start, token, self.mention_caret);
                    self.composer_body = completed;
                }
            }
        }
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
                                .unwrap_or_else(|| "—".into());
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

impl Default for InboxPage {
    fn default() -> Self {
        Self::new()
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
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

pub fn submit_note(
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
mod tests {
    use super::*;
    use crate::testsupport::fixture;
    use spp_core::event_bus::EventBus;

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
        let filters = db.with_read(|conn| filters_from_bar(&bar, 0, conn));
        assert_eq!(filters.status.as_deref(), Some("active"));
        assert_eq!(filters.mailbox_id, Some(2));
        assert_eq!(filters.channel.as_deref(), Some("email"));
        assert_eq!(filters.tag.as_deref(), Some("bug"));
        assert_eq!(filters.query.as_deref(), Some("export"));
        assert_eq!(filters.number, Some(42));
        assert_eq!(filters.limit, Some(INBOX_PAGE_SIZE));
        assert_eq!(filters.offset, Some(0));
        assert_eq!(ViewFilter::All.status(), None);
    }

    #[test]
    fn ops_tile_filter_drills_into_unassigned_conversations() {
        let db = fixture(vec![seeded().0]);
        db.with_write(|conn| {
            crate::testsupport::seed_conversation(conn, 2, "Orphan", "active", "email", 1, 0, 10);
        });
        let bar = FilterBar {
            ops: Some("unassigned".into()),
            ..FilterBar::default()
        };
        let (items, total) = db
            .with_read(|conn| {
                let filters = filters_from_bar(&bar, 0, conn);
                spp_core::inbox::list_conversations(conn, &filters)
            })
            .expect("ops list");
        assert_eq!(total, 1, "only the unassigned conversation matches");
        assert_eq!(items[0].id, 2);

        let bad = FilterBar {
            ops: Some("' OR 1=1 --".into()),
            ..FilterBar::default()
        };
        let filters = db.with_read(|conn| filters_from_bar(&bad, 0, conn));
        assert!(filters.extra_where.is_none());
    }

    #[test]
    fn reply_key_matches_op_send_reply_format() {
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
        let message = match outcome {
            ActionOutcome::Success(m) => m,
            other => panic!("expected success, got {other:?}"),
        };
        assert!(
            message.contains("delivering to Help Scout"),
            "honest delivery wording expected, got: {message}"
        );

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

        let second = submit_reply(&db, &bus, &detail, "here is the fix", "", "");
        assert!(matches!(second, ActionOutcome::Rejected(m) if m.contains("duplicate")));

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
            let detail = spp_core::inbox::get_conversation(conn, 1)
                .expect("detail")
                .expect("conversation");
            assert_eq!(detail.thread.len(), 1);
            assert_eq!(detail.thread[0].thread_type, "note");
            assert_eq!(detail.thread[0].body.as_deref(), Some("fyi @ada please check"));

            let (kind,): (String,) = conn
                .query_row(
                    "SELECT kind FROM outbound_jobs ORDER BY id DESC LIMIT 1",
                    [],
                    |r| Ok((r.get(0)?,)),
                )
                .expect("job");
            assert_eq!(kind, "create_note");

            let mentions: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM notifications WHERE type = 'mentioned' AND target_user_id = 5",
                    [],
                    |r| r.get(0),
                )
                .expect("notifications");
            assert_eq!(mentions, 1);
        });

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

    #[test]
    fn r4_d9_reply_carries_the_configured_status_after() {
        let db = fixture(vec![seeded().0]);
        let bus = EventBus::new(8);
        let detail = db
            .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
            .expect("detail")
            .expect("conversation 1 exists");

        let outcome = submit_reply(&db, &bus, &detail, "status after test", "", "");
        assert!(matches!(outcome, ActionOutcome::Success(_)));
        let payload: String = db.with_read(|conn| {
            conn.query_row(
                "SELECT payload FROM outbound_jobs ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .expect("job row")
        });
        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("payload json");
        assert_eq!(
            parsed["statusAfter"], "pending",
            "D9: the default reply statusAfter is pending"
        );

        db.with_write(|conn| spp_core::settings::set_string(conn, "reply_status_after", "keep"))
            .expect("set keep");
        let outcome = submit_reply(&db, &bus, &detail, "keep setting text", "", "");
        assert!(matches!(outcome, ActionOutcome::Success(_)));
        let payload: String = db.with_read(|conn| {
            conn.query_row(
                "SELECT payload FROM outbound_jobs ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .expect("job row")
        });
        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("payload json");
        assert!(
            parsed["statusAfter"].is_null(),
            "D9: keep means no statusAfter on the wire"
        );
    }

    #[test]
    fn r4_d8_a_failed_enqueue_is_rejected_instead_of_faking_success() {
        let db = fixture(vec![seeded().0]);
        let bus = EventBus::new(8);
        let detail = db
            .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
            .expect("detail")
            .expect("conversation 1 exists");

        db.with_write(|conn| conn.execute_batch("DROP TABLE outbound_jobs"))
            .expect("drop the queue");

        let outcome = submit_reply(&db, &bus, &detail, "must be rejected", "", "");
        match outcome {
            ActionOutcome::Rejected(reason) => {
                assert!(
                    !reason.is_empty(),
                    "the failure must carry the database's reason"
                );
            }
            other => panic!("expected rejection, got {other:?}"),
        }
    }

    #[test]
    fn audit24_poll_fills_the_composer_from_the_jobs_own_result() {
        let db = fixture(vec![seeded().0]);
        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO ai_runs (type, conversation_id, status, model, prompt_version,
                                       input_hash, input_refs, created_at, started_at,
                                       response_json)
                 VALUES ('draft_rewrite', NULL, 'completed', 'some-model', 'draft_rewrite_v1',
                         'bait', '[]', datetime('now'), datetime('now'),
                         '{\"output\":\"FOREIGN HISTORICAL TEXT\",\"instruction\":\"warmer\"}')",
                [],
            )
            .expect("foreign run");
        });
        let job_id = db.with_write(|conn| {
            spp_core::jobs::enqueue_on(conn, "ai", "rewrite_composer", "{}", 3, 2).expect("enqueue")
        });
        db.with_write(|conn| {
            conn.execute(
                "UPDATE jobs SET status = 'completed', completed_at = datetime('now'),
                                 result = 'THE JOB RESULT' WHERE id = ?1",
                rusqlite::params![job_id],
            )
            .expect("complete with result")
        });

        let mut page = InboxPage {
            pending_ai: Some(PendingComposerAi {
                job_id,
                kind: "rewrite_composer",
                conversation_id: 1,
                origin_body: String::new(),
            }),
            ..Default::default()
        };
        page.poll_pending_ai(&db);
        assert_eq!(page.composer_body, "THE JOB RESULT");
        assert!(page.pending_ai.is_none(), "the poll resolves the job");
        assert!(!page.status_is_error, "a real result is a success");

        let job_id2 = db.with_write(|conn| {
            spp_core::jobs::enqueue_on(conn, "ai", "translate_composer", "{}", 3, 2)
                .expect("enqueue")
        });
        db.with_write(|conn| {
            conn.execute(
                "UPDATE jobs SET status = 'completed', completed_at = datetime('now') WHERE id = ?1",
                rusqlite::params![job_id2],
            )
            .expect("complete without result")
        });
        page.composer_body.clear();
        page.pending_ai = Some(PendingComposerAi {
            job_id: job_id2,
            kind: "translate_composer",
            conversation_id: 1,
            origin_body: String::new(),
        });
        page.poll_pending_ai(&db);
        assert!(
            page.composer_body.is_empty(),
            "no foreign text may leak into the composer"
        );
        assert!(page.status_is_error, "the honest no-result message shows");
    }

    #[test]
    fn audit73_transform_completion_never_overwrites_in_flight_typing() {
        let db = fixture(vec![seeded().0]);
        let job_id = db.with_write(|conn| {
            spp_core::jobs::enqueue_on(conn, "ai", "rewrite_composer", "{}", 3, 2).expect("enqueue")
        });
        db.with_write(|conn| {
            conn.execute(
                "UPDATE jobs SET status = 'completed', completed_at = datetime('now'),
                                 result = 'REWRITTEN RESULT' WHERE id = ?1",
                rusqlite::params![job_id],
            )
            .expect("complete with result")
        });

        let mut page = InboxPage {
            pending_ai: Some(PendingComposerAi {
                job_id,
                kind: "rewrite_composer",
                conversation_id: 1,
                origin_body: "original draft".into(),
            }),
            composer_body: "the user typed this in the meantime".into(),
            ..Default::default()
        };
        page.poll_pending_ai(&db);
        assert_eq!(
            page.composer_body, "the user typed this in the meantime",
            "in-flight typing wins over the finishing transform"
        );
        assert!(page.pending_ai.is_none(), "the poll still resolves the job");
        assert!(
            page.status_message
                .as_deref()
                .is_some_and(|m| m.contains("your text was kept")),
            "the kept-edit decision is said out loud: {:?}",
            page.status_message
        );
        assert!(!page.status_is_error, "keeping the edit is not an error");

        let job_id2 = db.with_write(|conn| {
            spp_core::jobs::enqueue_on(conn, "ai", "rewrite_composer", "{}", 3, 2).expect("enqueue")
        });
        db.with_write(|conn| {
            conn.execute(
                "UPDATE jobs SET status = 'completed', completed_at = datetime('now'),
                                 result = 'REWRITTEN RESULT' WHERE id = ?1",
                rusqlite::params![job_id2],
            )
            .expect("complete with result")
        });
        page.pending_ai = Some(PendingComposerAi {
            job_id: job_id2,
            kind: "rewrite_composer",
            conversation_id: 1,
            origin_body: "original draft".into(),
        });
        page.composer_body = "original draft".into();
        page.poll_pending_ai(&db);
        assert_eq!(
            page.composer_body, "REWRITTEN RESULT",
            "an unedited composer still receives the transform"
        );
    }

    #[test]
    fn audit25_open_is_the_await_agent_status_and_legacy_waiting_maps_to_it() {
        let db = fixture(vec![seeded().0]);
        let bus = EventBus::new(8);
        let detail = db
            .with_read(|conn| spp_core::inbox::get_conversation(conn, 1))
            .expect("detail")
            .expect("conversation 1 exists");

        let status_after_for = |text: &str| -> serde_json::Value {
            let outcome = submit_reply(&db, &bus, &detail, text, "", "");
            assert!(matches!(outcome, ActionOutcome::Success(_)));
            let payload: String = db.with_read(|conn| {
                conn.query_row(
                    "SELECT payload FROM outbound_jobs ORDER BY id DESC LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .expect("job row")
            });
            serde_json::from_str(&payload).expect("payload json")
        };

        db.with_write(|conn| spp_core::settings::set_string(conn, "reply_status_after", "open"))
            .expect("set open");
        let parsed = status_after_for("open setting text");
        assert_eq!(
            parsed["statusAfter"], "open",
            "the await-agent status must be the platform's own `open`"
        );

        db.with_write(|conn| spp_core::settings::set_string(conn, "reply_status_after", "waiting"))
            .expect("set legacy waiting");
        let parsed = status_after_for("legacy waiting text");
        assert_eq!(
            parsed["statusAfter"], "open",
            "the legacy phantom status maps onto the real one"
        );
    }

    #[test]
    fn ensure_loaded_latches_and_invalidate_refetches() {
        let db = crate::testsupport::empty_db();
        let mut page = InboxPage::new();
        assert!(!page.refs_loaded);
        assert!(!page.list_loaded);
        page.ensure_loaded(&db);

        assert!(page.refs_loaded);
        assert!(page.list_loaded);
        assert!(page.mailboxes.is_empty(), "no mailboxes seeded yet");
        assert!(page.list.is_empty(), "a legitimately empty mirror");

        page.status = Some("sentinel".into());
        page.ensure_loaded(&db);
        assert_eq!(
            page.status.as_deref(),
            Some("sentinel"),
            "both gates latch: no reload ran (pre-fix reload_refs + reload_list re-ran every frame on an empty mirror)"
        );

        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO mailboxes (remote_id, name, email) VALUES (778, 'Latched', 'latched@example.test')",
                [],
            )
        })
        .expect("mailbox");
        page.invalidate();
        assert!(!page.refs_loaded);
        assert!(!page.list_loaded);
        page.ensure_loaded(&db);
        assert!(page.refs_loaded);
        assert!(page.list_loaded);
        assert_eq!(page.mailboxes.len(), 1, "invalidate refetches the refs");
    }
}

#[cfg(test)]
mod batch27_audit78_tests {
    use super::*;
    use crate::testsupport::{fixture, seed_conversation};

    fn seed_many(conn: &mut rusqlite::Connection) {
        for i in 1..=55i64 {
            seed_conversation(conn, i, &format!("conv {i}"), "active", "email", 1, 5, 9);
        }
    }

    #[test]
    fn page_one_reaches_conversations_past_the_cap() {
        let db = fixture(Vec::new());
        db.with_write(seed_many);

        let mut page = InboxPage::new();
        page.reload_list(&db);
        assert_eq!(page.total, 55, "the total counts every match");
        assert_eq!(page.list.len(), 50, "page 0 holds the page size");
        let page_zero_ids: std::collections::HashSet<i64> =
            page.list.iter().map(|i| i.id).collect();
        assert_eq!(page_zero_ids.len(), 50, "no duplicates within page 0");

        page.page = 1;
        page.reload_list(&db);
        assert_eq!(page.list.len(), 5, "page 1 holds the rest");
        assert_eq!(page.total, 55, "the total is page-independent");
        let mut seen = page_zero_ids;
        for item in &page.list {
            assert!(seen.insert(item.id), "no row appears on two pages");
        }
        assert_eq!(seen.len(), 55, "every conversation is reachable");
    }

    #[test]
    fn offset_compiles_from_the_page_index() {
        let db = fixture(Vec::new());
        let bar = FilterBar::default();
        let filters = db.with_read(|conn| filters_from_bar(&bar, 2, conn));
        assert_eq!(filters.limit, Some(INBOX_PAGE_SIZE));
        assert_eq!(filters.offset, Some(2 * INBOX_PAGE_SIZE));
    }

    #[test]
    fn a_page_past_the_end_falls_back_to_the_last_page() {
        let db = fixture(Vec::new());
        db.with_write(seed_many);

        let mut page = InboxPage::new();
        page.reload_list(&db);
        assert_eq!(page.list.len(), 50);

        db.with_write(|conn| {
            conn.execute("DELETE FROM conversations WHERE id > 12", [])
                .expect("delete");
        });
        page.reload_list(&db);
        assert_eq!(page.page, 0, "the past-the-end page clamped");
        assert_eq!(page.list.len(), 12, "the last-page rows render");
        assert!(!page.list.is_empty());
    }
}

#[cfg(test)]
mod batch27_audit79_tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn row_click_stashes_and_restores_the_typed_draft() {
        let db = fixture(vec![("conv", 1, "First", "active", "email", 1, 5, 9)]);
        db.with_write(|conn| {
            crate::testsupport::seed_conversation(conn, 2, "Second", "active", "email", 1, 5, 10);
        });

        let mut page = InboxPage::new();
        page.select(&db, 1);
        page.composer_mode = ComposerMode::Note;
        page.composer_body = "typed but unsent reply".into();
        page.composer_cc = "cc@example.com".into();

        page.select(&db, 2);
        assert_eq!(page.composer_body, "", "the incoming row starts clean");
        assert_eq!(
            page.composer_mode,
            ComposerMode::Reply,
            "the incoming row starts in reply mode"
        );

        page.select(&db, 1);
        assert_eq!(page.composer_body, "typed but unsent reply");
        assert_eq!(page.composer_cc, "cc@example.com");
        assert_eq!(page.composer_mode, ComposerMode::Note);
    }

    #[test]
    fn reselecting_the_same_row_keeps_the_typing_untouched() {
        let db = fixture(vec![("conv", 1, "First", "active", "email", 1, 5, 9)]);
        let mut page = InboxPage::new();
        page.select(&db, 1);
        page.composer_body = "mid-thought".into();

        page.select(&db, 1);
        assert_eq!(
            page.composer_body, "mid-thought",
            "clicking the open row changes nothing"
        );
    }
}

#[cfg(test)]
mod batch33_audit137_tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn invalidation_refreshes_the_filter_refs() {
        let db = fixture(vec![("conv", 1, "First", "active", "email", 1, 5, 9)]);
        let mut page = InboxPage::new();
        page.select(&db, 1);
        page.reload_list(&db);
        assert!(!page.mailboxes.is_empty(), "refs load with the page");
        assert!(page.tags.is_empty(), "no tags seeded yet");

        db.with_write(|conn| crate::testsupport::seed_tag(conn, 1, "billing"));

        page.invalidate();
        assert!(
            page.mailboxes.is_empty(),
            "invalidation clears the refs caches"
        );
        page.reload_list(&db);
        assert!(
            page.tags.iter().any(|t| t == "billing"),
            "the synced tag reaches the tag filter: {:?}",
            page.tags
        );
    }
}

#[cfg(test)]
mod batch33_audit143_tests {
    use super::*;

    #[test]
    fn hash_prefixed_number_entries_parse() {
        assert_eq!(parse_number_entry("#778"), Some(778));
        assert_eq!(parse_number_entry("778"), Some(778));
        assert_eq!(parse_number_entry("  # 778  "), Some(778));
        assert_eq!(parse_number_entry("# 778"), Some(778));
        assert_eq!(parse_number_entry(""), None);
        assert_eq!(parse_number_entry("#abc"), None);
        assert_eq!(
            parse_number_entry("##42"),
            Some(42),
            "leading hashes all strip"
        );
    }
}
