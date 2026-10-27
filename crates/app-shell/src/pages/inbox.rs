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
use spp_core::conversation_ops::NOT_CONNECTED;
use spp_core::conversation_ops::{create_outbound_job_keyed, set_outbound_status};
use spp_core::db::DbHandle;
use spp_core::eventing::EventBus;
use spp_core::inbox::{ConversationDetail, ConversationListItem, InboxFilters};
use spp_core::sync_engine::SyncEngine;
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

/// Rows per inbox page (clean-audit item 78: the list was hard-capped
/// at 50 with no pagination — conversation 51+ was unreachable; the cap
/// is now a page size with prev/next controls under the list).
pub const INBOX_PAGE_SIZE: u32 = 50;

/// Parse the `#` number-filter field ("#778", "778", " 778 " → 778;
/// anything non-numeric → None). Clean-audit item 143: the leading `#`
/// the field's own hint suggests used to fail the parse and the number
/// filter silently dropped.
#[must_use]
pub fn parse_number_entry(text: &str) -> Option<i64> {
    text.trim()
        .trim_start_matches('#')
        .trim()
        .parse::<i64>()
        .ok()
}

/// Map the filter bar to the core `InboxFilters`.
///
/// The operations-tile fragment needs the waiting-threshold setting, so
/// the bar compiles against a connection (same as the HTTP route).
/// `page` is the 0-based list page (item 78).
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
/// 1.3.0: `Rejected` no longer masquerades as success — the caller styles
/// it red and keeps the composer text instead of wiping it.
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
/// v1.4.0 (R4, D8): the enqueue is now a real `Result` — an INSERT
/// failure used to return job id `0` while the composer reported
/// success, so a reply could evaporate between the composer and the
/// dispatcher. The caller renders the error and keeps the typed text.
fn queue_job(
    conn: &Connection,
    kind: &str,
    payload: &serde_json::Value,
    conversation_id: Option<i64>,
    idempotency_key: Option<&str>,
) -> Result<i64, String> {
    let id = create_outbound_job_keyed(conn, kind, payload, conversation_id, idempotency_key)
        .map_err(|e| e.to_string())?;
    // Jobs are born "queued"; "sending" marks a provider call in flight,
    // which only the worker/loopback batch will set.
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

/// True when boot resolved real Help Scout credentials (boot writes the
/// `provider_kind` setting; tests default to real). When false, every
/// submit is rejected up front — before 1.3.0 the action silently
/// queued into a dispatcher that did not exist and reported success.
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
    /// Clean-audit item 78: the 0-based list page the pager is on
    /// (filters reset it to 0; the pager moves it).
    page: u32,
    selected: Option<i64>,
    detail: Option<ConversationDetail>,
    composer_mode: ComposerMode,
    composer_body: String,
    composer_cc: String,
    composer_bcc: String,
    mention_active: usize,
    mention_caret: usize,
    /// Clean-audit item 136: last frame's composer focus, read BEFORE
    /// the TextEdit renders so the open mention popup can consume the
    /// arrow keys ahead of it.
    composer_focused: bool,
    users: Vec<(i64, String)>,
    mailboxes: Vec<(i64, String)>,
    tags: Vec<String>,
    directory: Directory,
    status: Option<String>,
    assign_pick: Option<i64>,
    /// v1.4.0 (R5, D16): a failed detail load used to vanish via `.ok()`
    /// and render an empty pane; the exact error now shows in place of
    /// the thread so the user knows what broke.
    detail_error: Option<String>,
    /// Honest status of the user's last outbound action for this
    /// conversation, derived from the outbound queue on every detail
    /// reload: "Sending…", "Reply delivered", "Send failed: …".
    outbound_note: Option<(String, bool)>,
    status_message: Option<String>,
    status_is_error: bool,
    /// Audit fix (M22): wall-clock stamp captured whenever a transient
    /// status line is set, so a stale line (e.g. "Refreshing from Help
    /// Scout…" after the refresh finished, or an old validation error)
    /// expires on its own instead of sitting on screen until the user's
    /// next action replaces it.
    status_message_at: Option<std::time::Instant>,
    /// Copilot side panel (1.3.0): local analysis + similar issues +
    /// suggested reply + related articles + customer history.
    copilot_open: bool,
    copilot: Option<spp_core::ai_local::CopilotReport>,
    copilot_error: Option<String>,
    /// A2 triage-on-arrival chips (intent/urgency/sentiment/topic with
    /// confidence, editable): attribute-backed rows from ai_attributes,
    /// with the deterministic local analysis filling display-only gaps.
    triage: Vec<TriageChip>,
    /// A2 "know before you reply" briefing: the customer's durable facts.
    briefing: Vec<spp_core::ai_prompts::MemoryEntry>,
    /// The triage chip currently open for correction (A2).
    editing_chip: Option<spp_core::catalog::AiAttributeKey>,
    /// Shared vector index (A4): lets the copilot panel's similar-issues
    /// lane fuse FTS + sqlite-vec hits with real "semantic match" reasons.
    vector_index: Option<std::sync::Arc<spp_core::vectorstore_sqlitevec::VectorIndex>>,
    /// A3: the in-flight composer-AI job the page polls every frame
    /// (rewrite / translate / coaching review / QA AI layer). Cleared
    /// once the job reaches a terminal state.
    pending_ai: Option<PendingComposerAi>,
    /// A3: the persisted coaching review for the selected conversation
    /// (`coaching_reviews` via `coaching::get`) — advisory, never blocks
    /// a send.
    coaching_review: Option<serde_json::Value>,
    /// A3: post-resolution QA for the selected conversation (the
    /// deterministic layer + the on-demand AI layer when run).
    qa_review: Option<serde_json::Value>,
    /// A3: the translate-target picker (language code).
    translate_lang: String,
    /// A6: provenance of the AI draft currently sitting in the composer
    /// — (ai_drafts id, original text) — so accept, edit-distance and
    /// reject feedback record against the real row.
    inserted_draft: Option<(i64, String)>,
    // ---- A5 inbox assistant (copilot chat scoped to this conversation) ----
    /// The conversation's newest copilot session id.
    assistant_session: Option<i64>,
    /// The assistant message log for the selected conversation.
    assistant_messages: Vec<serde_json::Value>,
    /// The question being typed into the assistant.
    assistant_input: String,
    /// The in-flight copilot_chat job id (polled each frame).
    assistant_pending: Option<i64>,
    /// Last assistant error (job failure text, verbatim).
    assistant_error: Option<String>,
    /// Clean-audit item 79: per-conversation composer drafts. Clicking
    /// another row used to wipe the typed reply silently — the composer
    /// state (body/cc/bcc/mode + AI-draft provenance) now travels with
    /// its conversation and is restored on reselect. A successful submit
    /// drops the stash (the text was sent); the stash never outlives the
    /// session.
    drafts: std::collections::HashMap<i64, StashedDraft>,
}

/// One stashed composer draft (clean-audit item 79).
#[derive(Debug, Clone)]
struct StashedDraft {
    mode: ComposerMode,
    body: String,
    cc: String,
    bcc: String,
    inserted_draft: Option<(i64, String)>,
}

/// One in-flight composer-AI job the inbox polls (A3/A6). Jobs carry
/// their results in durable rows (`ai_runs` response JSON or
/// `coaching_reviews` / `post_resolution_qa`), so the poll only needs the
/// job row's terminal state — never a fabricated interim value.
///
/// `origin_body` is the composer text at enqueue time (clean-audit item
/// 73): a rewrite/translate result may only replace the composer when
/// the user typed nothing while the job ran — in-flight edits are never
/// clobbered by a finishing transform.
#[derive(Debug, Clone)]
struct PendingComposerAi {
    job_id: i64,
    kind: &'static str,
    conversation_id: i64,
    origin_body: String,
}

/// One editable triage chip (A2). Attribute-backed chips open a vocab
/// row on click and write a human-sourced ai_attributes row.
#[derive(Clone)]
struct TriageChip {
    key: spp_core::catalog::AiAttributeKey,
    label: &'static str,
    value: String,
    confidence: String,
    source: String,
    /// Heuristic-only chips (no attribute backing) are display-only.
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
            inserted_draft: None,
            assistant_session: None,
            assistant_messages: Vec::new(),
            assistant_input: String::new(),
            assistant_pending: None,
            assistant_error: None,
            drafts: std::collections::HashMap::new(),
        }
    }

    /// A4: receive the shared vector index (the same store the workers
    /// embed into) so the copilot panel can fuse FTS + sqlite-vec.
    pub fn set_vector_index(
        &mut self,
        index: Option<std::sync::Arc<spp_core::vectorstore_sqlitevec::VectorIndex>>,
    ) {
        self.vector_index = index;
    }

    /// Live-update invalidation: clear the cached list; the detail is
    /// reloaded (selection preserved) on the next frame. Before 1.3.0
    /// this nulled the open conversation on every sync event, so the
    /// pane flashed to "Select a conversation" right after sending.
    /// Clean-audit item 137: the filter/assign refs (users, mailboxes,
    /// tags, the mention directory) loaded once per session — new
    /// teammates, mailboxes and tags a sync brought in never reached the
    /// dropdowns until restart. Invalidation clears the refs caches too;
    /// the next frame's empty-gate reloads them.
    pub fn invalidate(&mut self) {
        self.list.clear();
        self.detail = None;
        self.users.clear();
        self.mailboxes.clear();
        self.tags.clear();
        self.directory = Directory::default();
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
            let filters = filters_from_bar(&self.bar, self.page, conn);
            spp_core::inbox::list_conversations(conn, &filters)
        });
        match result {
            Ok((items, total)) => {
                // Clean-audit item 78: a page past the last one (the data
                // shrank underneath — deletions, a narrowing sync) falls
                // back to the last page that still holds rows instead of
                // rendering an empty dead end. One recursive step: the
                // clamped page always has rows.
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
    }

    fn reload_detail(&mut self, db: &DbHandle) {
        let id = self.selected;
        let detail = id.map(|id| db.with_read(|conn| spp_core::inbox::get_conversation(conn, id)));
        // v1.4.0 (R5, D16): `.and_then(|result| result.ok()).flatten()`
        // swallowed every database error here, so a failed thread load
        // looked exactly like "no conversation selected".
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
            // A3: coaching review + post-resolution QA ride the same reload
            // (both are persisted rows keyed by conversation; QA computes
            // its deterministic layer lazily on first read of a closed
            // conversation).
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
                // A5: the conversation's newest copilot session + log.
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

    /// A2: load the triage chips and the "know before you reply" briefing
    /// for the selected conversation. ai_attributes rows lead (AI or
    /// human-sourced, confidence attached); the deterministic local
    /// analysis fills display-only gaps.
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
        // Attribute-backed chips (editable): intent, urgency, frustration,
        // escalation, issue cluster.
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
        // Display-only heuristic chips where no attribute exists yet:
        // sentiment comes from the deterministic local analysis.
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

    /// A2: apply a human correction from the vocab popup.
    fn override_chip_value(&mut self, db: &DbHandle, chip: &TriageChip, value: &str) {
        if value.is_empty() {
            // "unknown": supersede the current row with the catalog's
            // unknown marker so the chip disappears from the header until
            // the next extraction repopulates it.
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

    /// A2: apply a human correction to a triage chip — supersedes the
    /// current ai_attributes row with a human-sourced one (confidence
    /// high, source human; automation rules read the same table).
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

    /// Select a conversation and load its detail.
    pub fn select(&mut self, db: &DbHandle, id: i64) {
        // Clean-audit item 79: the outgoing conversation's typed draft is
        // stashed, not wiped — peeking at another row loses nothing, and
        // reselecting restores the text exactly (body/cc/bcc/mode + the
        // AI-draft provenance, so accept/edit-distance feedback still
        // records against the real row). Clicking the row that is
        // ALREADY open touches the composer not at all — typing
        // continues.
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
                // No stash: the incoming conversation starts from the
                // default composer (reply mode, clean fields).
                self.composer_mode = ComposerMode::Reply;
                self.composer_body.clear();
                self.composer_cc.clear();
                self.composer_bcc.clear();
                self.inserted_draft = None;
            }
        }
        self.status_message = None;
        self.status_message_at = None;
        // A3/A6: per-conversation AI state resets with the selection —
        // pending jobs and cards belong to the previous conversation,
        // never the new one.
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

    /// Clean-audit item 135: the AI-draft provenance dies with the draft
    /// — any composer content that did not come from the inserted draft
    /// (or an edit of it) must not be recorded as an acceptance of that
    /// draft row.
    ///
    /// The copilot suggested reply is not an `ai_drafts` row, so it
    /// carries no provenance of its own — and it REPLACES whatever sat in
    /// the composer, killing any inserted draft's claim on the text. That
    /// insert site applies the rule with field writes (`report` borrows
    /// `self.copilot` across the render closure, so a `&mut self` helper
    /// cannot be called there); the note-mode and wipe rules live in the
    /// helpers below.
    /// Clean-audit item 135: an internal note is not the customer reply
    /// the draft was generated for, and `submit_composer` records the
    /// acceptance in whichever mode is active — switching to note mode
    /// therefore spends the provenance.
    pub fn set_composer_mode(&mut self, mode: ComposerMode) {
        if mode == ComposerMode::Note {
            self.inserted_draft = None;
        }
        self.composer_mode = mode;
    }

    /// Clean-audit item 135: an edit that WIPES the composer abandons
    /// the inserted draft — keeping the provenance would record a
    /// phantom acceptance for whatever the user types next. Edits that
    /// keep text keep the provenance (the edit distance is the metric).
    pub fn composer_edited(&mut self, body: String) {
        if body.trim().is_empty() {
            self.inserted_draft = None;
        }
        self.composer_body = body;
    }

    /// Submit the composer (reply or note). The job is dispatched to
    /// Help Scout by the outbound worker within ~2 seconds; the status
    /// line then reflects the real delivery result.
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
                // A6 feedback loop: sending with an inserted AI draft records
                // accept + edit-distance against the real ai_drafts row —
                // the edit-rate in the AI usage dashboard becomes real.
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
                // Clean-audit item 79: the sent conversation's stash is
                // spent — reselecting starts from a clean composer
                // instead of resurrecting the reply that already went
                // out.
                if let Some(id) = self.selected {
                    self.drafts.remove(&id);
                }
            }
            // Rejections keep the typed text (the user fixes the problem
            // and re-sends; wiping the composer destroyed their work).
            ActionOutcome::Rejected(msg) => {
                self.set_status_message(msg, true);
            }
        }
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

    /// Audit fix (M22): every transient status line carries its set-time
    /// so rendering can expire it (see [`Self::expire_status_message`]).
    fn set_status_message(&mut self, message: impl Into<String>, is_error: bool) {
        self.status_message = Some(message.into());
        self.status_is_error = is_error;
        self.status_message_at = Some(std::time::Instant::now());
    }

    /// Audit fix (M22): a status line older than the TTL is dropped
    /// before rendering — completed one-shot notes never outlive their
    /// moment.
    fn expire_status_message(&mut self) {
        const STATUS_MESSAGE_TTL: std::time::Duration = std::time::Duration::from_secs(45);
        if let Some(set_at) = self.status_message_at
            && set_at.elapsed() > STATUS_MESSAGE_TTL
        {
            self.status_message = None;
            self.status_message_at = None;
        }
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
        self.set_status_message(outcome.message(), outcome.is_rejected());
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
        self.set_status_message(outcome.message(), outcome.is_rejected());
        self.reload_detail(db);
        self.reload_list(db);
        outcome
    }

    /// Render the page. `engine` powers the "Sync now" header action
    /// (a real incremental pull from Help Scout — before 1.3.0 no page
    /// except Sync Health could touch the network).
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        bus: &EventBus,
        engine: Option<&std::sync::Arc<SyncEngine>>,
    ) {
        if self.mailboxes.is_empty() {
            self.reload_refs(db);
        }
        if self.list.is_empty() {
            self.reload_list(db);
        }
        // Invalidation cleared the detail: reload it (selection kept) so
        // the open conversation survives sync events instead of
        // vanishing behind "Select a conversation".
        if self.selected.is_some() && self.detail.is_none() {
            self.reload_detail(db);
        }
        // A3: resolve in-flight composer-AI jobs (rewrite / translate /
        // coaching / QA) — results land in durable rows, the poll just
        // moves them into the page state.
        self.poll_pending_ai(db);
        // A5: the conversation-scoped assistant poll.
        self.poll_assistant(db);

        // Clean-audit item 78: the subtitle reports the visible RANGE once
        // pagination is in play ("51-100 of 1234"), not a misleading
        // "50 of 1234".
        let subtitle = if self.total > INBOX_PAGE_SIZE {
            let from = self.page * INBOX_PAGE_SIZE + 1;
            let to = from - 1 + self.list.len() as u32;
            format!("{from}-{to} of {} conversations", self.total)
        } else {
            format!("{} of {} conversations", self.list.len(), self.total)
        };
        let syncing = engine.is_some_and(|e| e.is_running());
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
                self.page = 0;
                self.reload_list(db);
            }
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
                // R8 (D6): a click during a running sync is answered, not
                // swallowed — the toast reports the completion outcome.
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
                    self.page = 0;
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
                    self.page = 0;
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
                self.page = 0;
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
                self.bar.number = parse_number_entry(&self.number_edit);
                self.page = 0;
                self.reload_list(db);
            }
        });
        ui.add_space(6.0);

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
        // Clean-audit item 78: prev/next under the list — the 50-row cap
        // was a dead end before (conversation 51+ was unreachable, the
        // subtitle just said "50 of 1234").
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

    fn detail_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, bus: &EventBus) {
        let Some(detail) = self.detail.clone() else {
            // R5 (D16): a load failure is not "nothing selected" — say
            // what broke so the user can act on it.
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

        // The whole detail pane scrolls; the composer stays reachable at
        // any window height.
        egui::ScrollArea::vertical()
            .id_salt("inbox-detail-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.detail_body_ui(ui, db, bus, detail);
            });
    }

    /// The Copilot side panel: everything the local engine can say about
    /// this conversation, zero configuration required. LM Studio passes
    /// are offered on top when a provider is configured.
    fn copilot_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, detail: &ConversationDetail) {
        design::card_titled(
            ui,
            "Copilot",
            Some("local · no configuration needed"),
            |ui| {
                if let Some(err) = &self.copilot_error {
                    design::error_text(ui, &format!("copilot: {err}"));
                    return;
                }
                let Some(report) = &self.copilot else {
                    design::hint_text(ui, "Analyzing the conversation…");
                    return;
                };
                let a = &report.analysis;

                // Signal chips.
                ui.horizontal(|ui| {
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
                    design::badge(ui, &format!("sentiment: {}", a.sentiment), sentiment_tone);
                    if a.escalation {
                        design::badge(ui, "escalation risk", design::BadgeTone::Destructive);
                    }
                });
                ui.add_space(6.0);

                // Summary.
                if !a.summary.is_empty() {
                    design::field_label(ui, "What the customer needs");
                    for line in &a.summary {
                        ui.label(
                            egui::RichText::new(format!("• {line}"))
                                .small()
                                .color(design::tokens(ui).foreground),
                        );
                    }
                    ui.add_space(4.0);
                }

                // Open questions.
                if !a.questions.is_empty() {
                    design::field_label(ui, "Open questions");
                    for q in &a.questions {
                        ui.label(
                            egui::RichText::new(format!("? {q}"))
                                .small()
                                .color(design::tokens(ui).muted_foreground),
                        );
                    }
                    ui.add_space(4.0);
                }

                // Customer history.
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
                ui.add_space(6.0);

                // Similar past issues ("have we seen this before?").
                if report.similar.is_empty() {
                    design::hint_text(ui, "No similar past conversations found yet.");
                } else {
                    design::field_label(ui, "Similar past issues");
                    for s in report.similar.iter().take(3) {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("#{} {}", s.number, s.subject))
                                    .small()
                                    .strong()
                                    .color(design::tokens(ui).foreground),
                            );
                            design::badge(
                                ui,
                                &format!("{}%", (s.score * 100.0) as u32),
                                design::BadgeTone::Info,
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
                                egui::RichText::new(format!("why: {}", s.why.join(", ")))
                                    .small()
                                    .weak(),
                            );
                        }
                        ui.add_space(3.0);
                    }
                    ui.add_space(4.0);
                }

                // Related Help Center / knowledge articles.
                if !report.articles.is_empty() {
                    design::field_label(ui, "Relevant help articles");
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
                    ui.add_space(6.0);
                }

                // Suggested reply (evidence-grounded, editable in composer).
                design::field_label(ui, "Suggested reply");
                // Audit fix (M22): the queued-draft status line goes through
                // the TTL'd status mechanism — set via a flag AFTER the
                // closure because `report` borrows `self.copilot` across it
                // (a `&mut self` method call inside would conflict).
                let mut lm_draft_queued = false;
                ui.push_id("copilot-suggested", |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("copilot-suggested-scroll")
                        .max_height(160.0)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(&report.suggested_reply)
                                    .small()
                                    .color(design::tokens(ui).foreground),
                            );
                        });
                });
                ui.horizontal(|ui| {
                if design::button_with_icon(
                    ui,
                    Icon::Send,
                    "Insert into reply",
                    ButtonVariant::Primary,
                )
                .on_hover_text("put the draft in the composer for editing")
                .clicked()
                {
                    // Clean-audit item 135: the suggested reply is not a
                    // drafts row — replacing the composer content with it
                    // kills any inserted draft's provenance (field writes:
                    // `report` borrows `self.copilot` across this closure,
                    // so a `&mut self` method call would conflict).
                    self.composer_mode = ComposerMode::Reply;
                    self.inserted_draft = None;
                    self.composer_body = report.suggested_reply.clone();
                }
                let (gen_ok, has_draft) = db.with_read(|conn| {
                    (
                        spp_core::ai_local::generative_backend_available(conn),
                        spp_core::ai_local::latest_ai_draft(conn, detail.id).is_some(),
                    )
                });
                if gen_ok {
                    if design::button(ui, "Draft with LM Studio", ButtonVariant::Outline)
                        .on_hover_text("generative draft grounded in the same evidence")
                        .clicked()
                    {
                        db.with_write(|conn| {
                            let _ = spp_core::jobs::enqueue_on(
                                conn,
                                "ai",
                                "generate_draft",
                                &json!({ "conversationId": detail.id }).to_string(),
                                3,
                                2,
                            );
                        });
                        lm_draft_queued = true;
                    }
                } else {
                    ui.label(
                        egui::RichText::new(
                            "Tip: install LM Studio and set a chat model in Settings for generative drafts.",
                        )
                        .small()
                        .weak(),
                    );
                }
                if has_draft {
                    ui.label(
                        egui::RichText::new("an LM Studio draft exists — sent to composer")
                            .small()
                            .weak(),
                    );
                }
            });
                if lm_draft_queued {
                    self.set_status_message("LM Studio draft generation queued…", false);
                }
                // Surface the latest LM Studio draft (if any) below the local one.
                if let Some((draft_id, draft, model)) =
                    db.with_read(|conn| spp_core::ai_local::latest_ai_draft_row(conn, detail.id))
                {
                    ui.add_space(4.0);
                    design::field_label(ui, &format!("LM Studio draft ({model})"));
                    ui.push_id("copilot-lm-draft", |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("copilot-lm-draft-scroll")
                            .max_height(140.0)
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(&draft)
                                        .small()
                                        .color(design::tokens(ui).foreground),
                                );
                            });
                    });
                    ui.horizontal(|ui| {
                        if design::button(ui, "Insert LM Studio draft", ButtonVariant::Secondary)
                            .clicked()
                        {
                            self.composer_mode = ComposerMode::Reply;
                            // A6: track provenance so the send path records
                            // accept + edit distance against this row.
                            self.inserted_draft = Some((draft_id, draft.clone()));
                            self.composer_body = draft.clone();
                        }
                        // A6: an explicit reject closes the loop for drafts
                        // the agent never sends (state + zero-edit feedback).
                        if design::button(ui, "Discard draft", ButtonVariant::Ghost)
                            .on_hover_text("mark this AI draft as rejected (records feedback)")
                            .clicked()
                        {
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
                                Ok(()) => {
                                    self.set_status_message(
                                        "Draft marked rejected — feedback recorded.",
                                        false,
                                    );
                                }
                                Err(e) => {
                                    self.set_status_message(format!("Reject failed: {e}"), true);
                                }
                            }
                        }
                        if self
                            .inserted_draft
                            .as_ref()
                            .is_some_and(|(id, _)| *id == draft_id)
                        {
                            design::badge(ui, "in composer", BadgeTone::Info);
                        }
                    });
                }
                // A5: the inbox assistant — the same 22-tool copilot agent
                // scoped to this conversation. Turns run as worker jobs;
                // answers cite the mirror rows the tools returned.
                ui.add_space(10.0);
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
                                    let role =
                                        message.get("role").and_then(|v| v.as_str()).unwrap_or("?");
                                    if role == "tool" {
                                        continue;
                                    }
                                    let content = message
                                        .get("content")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    ui.horizontal(|ui| {
                                        design::badge(
                                            ui,
                                            role,
                                            if role == "assistant" {
                                                BadgeTone::Info
                                            } else {
                                                BadgeTone::Neutral
                                            },
                                        );
                                        ui.label(egui::RichText::new(content).small());
                                    });
                                }
                            });
                    }
                    let busy = self.assistant_pending.is_some();
                    ui.horizontal(|ui| {
                        let mut question = self.assistant_input.clone();
                        let response = ui.add_sized(
                            [ui.available_width() - 110.0, 26.0],
                            egui::TextEdit::singleline(&mut question)
                                .hint_text("ask about this conversation…"),
                        );
                        if response.changed() {
                            self.assistant_input = question;
                        }
                        if (design::button(
                            ui,
                            if busy { "thinking…" } else { "ask" },
                            ButtonVariant::Primary,
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
                    // Starter questions grounded in this conversation.
                    let starters =
                        db.with_read(|conn| spp_core::copilot::starter_questions(conn, detail.id));
                    if !starters.is_empty() {
                        ui.horizontal(|ui| {
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
            },
        );
    }

    /// Queue a composer-AI job and start polling it (A3). The payload is
    /// built by the caller; the worker's feature/role re-checks stay
    /// authoritative.
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
                    // Clean-audit item 73: what the composer held when the
                    // transform was queued — the result may only land if
                    // the user typed nothing since.
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

    /// Poll the in-flight composer-AI job (A3): completed jobs move their
    /// durable results into page state; failed jobs surface their exact
    /// error; running/queued jobs keep the working note.
    ///
    /// Clean-audit item 24: the rewrite/translate branch reads the job's
    /// OWN `jobs.result` row. The old query scraped the newest matching
    /// `ai_runs` row (`ORDER BY id DESC LIMIT 1`, any conversation via
    /// `conversation_id IS NULL`) — a stale or foreign historical run
    /// filled the composer, cache-hit translations (which write no run
    /// row at all) picked an older run's text, and feature-off paths
    /// claimed "Composer updated" with somebody else's output.
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
                        // This job's own output — never a neighbouring or
                        // historical run's (audit item 24)...
                        // and never over typing made in flight (audit item
                        // 73): the result only replaces the composer when
                        // the text still equals what the transform was
                        // queued from.
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

    /// A5: poll the conversation-scoped assistant chat job.
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

    fn detail_body_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        bus: &EventBus,
        detail: ConversationDetail,
    ) {
        let t = design::tokens(ui);
        // Subject + status badge + per-conversation actions.
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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if design::icon_button(
                    ui,
                    Icon::Sparkles,
                    ButtonVariant::Ghost,
                    "Copilot: local summary, similar past issues, suggested reply",
                )
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
        // Audit fix (M25): with no customer email the old format printed
        // a stray "<>" after the name ("Priya Sharma <> · mailbox: …");
        // the <email> part is now only rendered when an email exists.
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
        ui.add_space(8.0);

        // A2 triage-on-arrival: intent/urgency/sentiment/topic chips with
        // confidence, editable where attribute-backed (click to correct —
        // the override lands in ai_attributes with source "human" and the
        // automation rules read the same rows).
        if !self.triage.is_empty() {
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
            // The inline vocab row for the chip being corrected.
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
            ui.add_space(6.0);
        }
        // A2 "know before you reply": the customer's durable facts, the
        // same quarantine-filtered rows the evidence pack feeds on.
        if !self.briefing.is_empty() {
            design::card_titled(
                ui,
                "Know before you reply",
                Some("durable customer facts extracted from past conversations"),
                |ui| {
                    ui.horizontal(|ui| {
                        Icon::Sparkles.show(ui, 14.0, t.muted_foreground);
                        design::badge(ui, "AI", BadgeTone::Info);
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
            ui.add_space(6.0);
        }

        // Action bar: status, assignee — applied locally for instant
        // feedback AND dispatched to Help Scout by the outbound worker.
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
                egui::ComboBox::from_id_salt("inbox-assign")
                    .width(200.0)
                    // Clean-audit item 33: a team assignment is its own
                    // state — "team: Billing" — never "unassigned", and
                    // never a user name that happens to share the team's
                    // platform id.
                    .selected_text(
                        detail
                            .assignee_name
                            .clone()
                            .or_else(|| {
                                detail
                                    .assigned_team_name
                                    .as_deref()
                                    .map(|t| format!("team: {t}"))
                            })
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
            // Honest outcome lines: the local action result, then the
            // real delivery state from the outbound queue (sending →
            // delivered / failed) — never a green "queued" that goes
            // nowhere (the 1.2.0 lie).
            // Audit fix (M22): drop a status line past its TTL before
            // rendering so finished notes don't stick around forever.
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
        ui.add_space(8.0);

        // Copilot side panel (zero-config local intelligence; LM Studio
        // optional on top).
        if self.copilot_open {
            self.copilot_ui(ui, db, &detail);
            ui.add_space(8.0);
        }

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

        // A3: post-resolution QA for closed conversations — the
        // deterministic checks surfaced with their honest disclaimers, plus
        // the on-demand AI layer.
        if let Some(qa) = self.qa_review.clone() {
            self.qa_card_ui(ui, db, &qa, detail.id);
            ui.add_space(8.0);
        }

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
                        self.set_composer_mode(ComposerMode::Reply);
                    }
                    if design::chip_with_icon(
                        ui,
                        self.composer_mode == ComposerMode::Note,
                        Icon::Pencil,
                        "Internal note",
                    )
                    .clicked()
                    {
                        self.set_composer_mode(ComposerMode::Note);
                    }
                });
                let hint = match self.composer_mode {
                    ComposerMode::Reply => "Type your reply to the customer… (Ctrl+Enter sends)",
                    ComposerMode::Note => {
                        "Type an internal note (visible only to your team) — @ to mention (Ctrl+Enter sends)"
                    }
                };
                // Clean-audit item 136: while the mention popup is open it
                // owns the arrow keys — the highlight moves. The keys are
                // consumed BEFORE the TextEdit renders: an un-consumed
                // ArrowDown would move the caret, change the @-query and
                // close the popup on the very first press (and the popup
                // code runs after the field, too late to undo that).
                let mention_len = caret_query(&self.composer_body, self.mention_caret)
                    .map_or(0, |(q, _)| filter_matches(&self.directory, &q).len());
                if mention_len > 0 && self.composer_focused {
                    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown))
                    {
                        self.mention_active =
                            crate::palette::move_selection(self.mention_active, mention_len, 1);
                    }
                    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
                        self.mention_active =
                            crate::palette::move_selection(self.mention_active, mention_len, -1);
                    }
                }
                let mut body = self.composer_body.clone();
                let output = egui::TextEdit::multiline(&mut body)
                    .hint_text(hint)
                    .desired_rows(5)
                    .desired_width(ui.available_width())
                    .lock_focus(false)
                    .show(ui);
                let mut changed = false;
                if output.response.changed() {
                    // Clean-audit item 135: routed through the helper so a
                    // wiped composer abandons the draft provenance.
                    self.composer_edited(body.clone());
                    changed = true;
                }
                self.composer_focused = output.response.has_focus();
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

                // A3: the composer AI menu — pre-send review (deterministic +
                // AI layer), rewrites and translation. All transforms replace
                // the composer text only after the user sees the result;
                // nothing is sent automatically.
                let conv_id = detail.id;
                let busy = self.pending_ai.is_some();
                ui.horizontal(|ui| {
                    if design::chip_with_icon(
                        ui,
                        false,
                        Icon::Sparkles,
                        if busy { "AI working…" } else { "Review draft" },
                    )
                    .on_hover_text(
                        "run the 9 deterministic coaching checks (plus the AI layer when a model is routed) — advisory only",
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
                    let gen_ok =
                        db.with_read(spp_core::ai_local::generative_backend_available);
                    if gen_ok && !busy && !self.composer_body.trim().is_empty() {
                        for (instruction, label) in
                            [("shorten", "Shorten"), ("expand", "Expand"), ("warmer", "Warmer")]
                        {
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
                                for (code, name) in
                                    spp_core::translation::SUPPORTED_LANGUAGES
                                {
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
                            let text: String = self
                                .composer_body
                                .chars()
                                .take(8000)
                                .collect();
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
                // A3: the coaching card — the persisted review of the draft,
                // advisory only, refreshed by "Review draft".
                if let Some(review) = self.coaching_review.clone() {
                    self.coaching_review_ui(ui, &review);
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

impl InboxPage {
    /// A3: the coaching review card — the 9 deterministic checks + AI layer
    /// of the persisted `coaching_reviews` row. Advisory only: it never
    /// blocks a send and says so.
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
                            BadgeTone::Info,
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

    /// A3: the post-resolution QA card for closed conversations —
    /// deterministic checks with their honest disclaimers, the AI layer
    /// when it has run, and the on-demand "Run AI review" button.
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
                        design::badge(ui, "AI review: present", BadgeTone::Success);
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
    // Audit fix (H6): real Help Scout threads carry `customer` (customer
    // message), `message` (agent reply), `note`, `chat`, `phone` and
    // `lineitem` (meta) — the old mock-era mapping only knew `reply` and
    // `customer_message`, so every real entry rendered as "Other".
    // Legacy mock types (`reply`, `customer_message`) keep their chips.
    let (tone, label) = match entry.thread_type.as_str() {
        "customer" | "customer_message" => (BadgeTone::Success, "Customer"),
        "message" | "reply" => (BadgeTone::Info, "Agent"),
        "note" => (BadgeTone::Warning, "Note"),
        "chat" => (BadgeTone::Info, "Chat"),
        "phone" => (BadgeTone::Info, "Phone"),
        kind if kind.starts_with("ai") => (BadgeTone::Info, "AI"),
        // `lineitem` and anything unknown stay neutral/meta.
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
            // Audit fix (M25): the thread rows printed the raw ISO stamp
            // ("2026-10-26T16:24:26Z"); reuse the list-row compact helper.
            ui.label(
                egui::RichText::new(compact_time(&entry.created_at))
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
        // v1.4.0 (R4, D9): a sent reply moves the conversation to the
        // configured status (default `pending`, decision D-b). Before
        // this, replies never carried statusAfter and the conversation
        // stayed in whatever status it was in — the "status never
        // changes" complaint.
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
            // Audit item 25: Help Scout's own status vocabulary is
            // open/pending/closed — `waiting` never existed there, so a
            // reply configured with it was rejected on the wire. `open`
            // is the real "await agent" status; a stored legacy `waiting`
            // maps onto it.
            "open" | "waiting" => json!("open"),
            "keep" => serde_json::Value::Null,
            // default: pending (await customer) — Help Scout semantics
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
        if !provider_connected(conn) {
            return Err(NOT_CONNECTED.to_string());
        }
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
                let filters = filters_from_bar(&bar, 0, conn);
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
        let filters = db.with_read(|conn| filters_from_bar(&bad, 0, conn));
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

    #[test]
    fn r4_d9_reply_carries_the_configured_status_after() {
        // Default (decision D-b): pending — a sent reply moves the
        // conversation to "awaiting the customer" like Help Scout itself.
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

        // The "keep" setting restores the pre-1.4.0 behavior: no status
        // transition on send.
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
        // Break the queue so the INSERT fails; the composer must say so
        // and the typed text must survive (pre-1.4.0: job id 0 + a green
        // "accepted" while the reply evaporated).
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

    /// Clean-audit item 24: the composer poll reads THIS job's `jobs.result`
    /// row. The old query scraped the newest matching `ai_runs` row (any
    /// conversation, any session) — a foreign historical run filled the
    /// composer and claimed "Composer updated by the AI transform."
    #[test]
    fn audit24_poll_fills_the_composer_from_the_jobs_own_result() {
        let db = fixture(vec![seeded().0]);
        // The bait: a newer, foreign, GLOBAL draft_rewrite run that the old
        // newest-run scrape would have picked.
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
        // This job's own pinned result.
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

        // A completed job with NO result (feature off / cache hit that wrote
        // nothing) says so honestly instead of falling back to ai_runs.
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

    /// Clean-audit item 73: a finishing rewrite/translate must not
    /// clobber typing made while the job ran. The result lands only when
    /// the composer still holds the text the transform was queued from;
    /// an in-flight edit keeps the user's text and says so.
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

        // The user typed while the transform was running.
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

        // The untouched path still lands the result (audit item 24 intact).
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

    /// Clean-audit item 25: `open` is the platform's own await-agent status
    /// and rides the wire; the legacy phantom `waiting` maps onto it —
    /// never onto a status Help Scout would reject.
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

        // A stored legacy "waiting" must never reach the wire.
        db.with_write(|conn| spp_core::settings::set_string(conn, "reply_status_after", "waiting"))
            .expect("set legacy waiting");
        let parsed = status_after_for("legacy waiting text");
        assert_eq!(
            parsed["statusAfter"], "open",
            "the legacy phantom status maps onto the real one"
        );
    }
}

/// Clean-audit item 78: the inbox list was hard-capped at 50 rows with
/// no pagination — conversation 51+ was unreachable. The cap is now a
/// page size with offset paging (and a clamp when the data shrinks
/// underneath the current page).
#[cfg(test)]
mod batch27_audit78_tests {
    use super::*;
    use crate::testsupport::{fixture, seed_conversation};

    fn seed_many(conn: &mut rusqlite::Connection) {
        // 55 conversations: page 0 holds 50, page 1 holds 5.
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

        // The pager advances: page 1 holds the remaining 5 rows.
        page.page = 1;
        page.reload_list(&db);
        assert_eq!(page.list.len(), 5, "page 1 holds the rest");
        assert_eq!(page.total, 55, "the total is page-independent");
        // The two pages together cover every conversation exactly once.
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

        // The data shrinks to 60 rows... then to 12 while the user sits
        // on page 1 (offset 50 > 12 rows): the reload clamps to page 0.
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

/// Clean-audit item 79: clicking another row used to wipe the typed
/// reply draft silently — the composer state now travels with its
/// conversation and is restored on reselect.
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

        // The user peeks at the other row.
        page.select(&db, 2);
        assert_eq!(page.composer_body, "", "the incoming row starts clean");
        assert_eq!(
            page.composer_mode,
            ComposerMode::Reply,
            "the incoming row starts in reply mode"
        );

        // Coming back: the draft is exactly where they left it.
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

/// Clean-audit item 137: the filter/assign refs (users, mailboxes, tags,
/// the mention directory) loaded once per session — a sync bringing new
/// tags never reached the dropdowns until restart. Invalidation clears
/// the refs caches and the next frame's empty-gate reloads them.
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

        // A sync lands a new tag...
        db.with_write(|conn| crate::testsupport::seed_tag(conn, 1, "billing"));

        // ...before the fix, only a restart surfaced it (the empty-gate
        // never fired again). Invalidation must clear the caches and the
        // reload must pick the new tag up.
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

/// Clean-audit item 143: the `#` field's own hint suggests "#778", but
/// the leading hash used to fail the parse and the number filter
/// silently dropped.
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
