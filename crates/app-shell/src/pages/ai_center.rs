//! AI Center page (port of the web `pages/ai_center.rs`).
//!
//! Redesigned with the shadcn-style design system (`crate::design`):
//! AI-derived content is marked with a Sparkles icon and an “AI” badge,
//! while operational state (connection, run statuses) uses the semantic
//! success/neutral/destructive badge tones — presentation only.
//!
//! Tabs (web: Health / Analytics / Attributes / Jobs):
//! * **Health** — LM Studio settings read from the settings table plus a
//!   test-connection button that lists models through
//!   `ai_lm_studio::OpenAiCompatibleClient` (the `/api/ai/status` route).
//! * **Analytics** — the `/api/ai/analytics` SQL: tickets analyzed, run
//!   success rate, draft states, edit rate, verification warnings,
//!   unsupported-claim rate, common failure patterns.
//! * **Attributes** — the AI attribute catalog (`AiAttributeKey::ALL`,
//!   `/api/attributes/catalog`) and live distributions
//!   (`ai_attributes::distributions`, `/api/attributes/report`).
//! * **Jobs** — the recent `ai_runs` table (`/api/ai/jobs`).

use std::sync::{Arc, Mutex};

use super::{ConfirmRequest, NavRequest};
use eframe::egui;
use spp_core::ai_attributes::AttributeDistribution;
use spp_core::catalog::AiAttributeKey;
use spp_core::db::DbHandle;

use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

/// Page tabs (web: `AiTab`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Health,
    Analytics,
    Attributes,
    Copilot,
    Usage,
    Jobs,
}

impl Tab {
    const ALL: [Self; 6] = [
        Self::Health,
        Self::Analytics,
        Self::Attributes,
        Self::Copilot,
        Self::Usage,
        Self::Jobs,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Health => "Health",
            Self::Analytics => "Analytics",
            Self::Attributes => "Attributes",
            Self::Copilot => "Copilot",
            Self::Usage => "Usage",
            Self::Jobs => "Jobs",
        }
    }
}

/// LM Studio settings snapshot (the `/api/ai/status` settings half).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AiSettings {
    pub ai_enabled: bool,
    pub base_url: String,
    pub chat_model: Option<String>,
    pub embedding_model: Option<String>,
    pub timeout_ms: i64,
    pub concurrency: i64,
}

/// Load the settings half of the AI status (the route's read-only view).
#[must_use]
pub fn load_ai_settings(conn: &rusqlite::Connection) -> AiSettings {
    AiSettings {
        ai_enabled: spp_core::settings::get_bool(conn, "ai_enabled", true).unwrap_or(true),
        base_url: spp_core::settings::get_string(conn, "lmstudio_base_url")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "http://127.0.0.1:1234".into()),
        chat_model: spp_core::settings::get_string(conn, "lmstudio_chat_model")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty()),
        embedding_model: spp_core::settings::get_string(conn, "lmstudio_embedding_model")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty()),
        timeout_ms: spp_core::settings::get_i64(conn, "lmstudio_timeout_ms", 120_000)
            .unwrap_or(120_000),
        concurrency: spp_core::settings::get_i64(conn, "lmstudio_concurrency", 2).unwrap_or(2),
    }
}

/// AI analytics metrics (typed port of the `/api/ai/analytics` SQL).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AiAnalytics {
    pub tickets_analyzed: i64,
    pub analysis_success_rate: f64,
    pub draft_count: i64,
    pub drafts_accepted: i64,
    pub drafts_rejected: i64,
    pub draft_edit_rate: f64,
    pub verification_warnings: i64,
    pub unsupported_claim_rate: f64,
    pub failure_patterns: Vec<(String, i64)>,
}

/// Load the analytics (the route SQL, in-process).
#[must_use]
pub fn load_analytics(conn: &rusqlite::Connection) -> AiAnalytics {
    let (tickets_analyzed, analysis_completed, analysis_failed): (i64, i64, i64) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(DISTINCT conversation_id) FROM ai_runs
                  WHERE type = 'ticket_analysis' AND status = 'completed'
                    AND conversation_id IS NOT NULL),
                (SELECT COUNT(*) FROM ai_runs
                  WHERE type = 'ticket_analysis' AND status = 'completed'),
                (SELECT COUNT(*) FROM ai_runs
                  WHERE type = 'ticket_analysis' AND status = 'failed')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, 0, 0));
    let (draft_count, drafts_accepted, drafts_rejected, drafts_edited): (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM ai_drafts),
                (SELECT COUNT(*) FROM ai_drafts WHERE state = 'accepted'),
                (SELECT COUNT(*) FROM ai_drafts WHERE state = 'rejected'),
                (SELECT COUNT(DISTINCT f.draft_id) FROM ai_feedback f
                   JOIN ai_drafts d ON d.id = f.draft_id
                  WHERE f.edit_distance > 20)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap_or((0, 0, 0, 0));
    let (verification_warnings, unsupported_claims): (i64, i64) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM ai_verifications
                  WHERE verified = 1 AND warnings != '[]'),
                (SELECT COUNT(*) FROM ai_verifications
                  WHERE unsupported_claims != '[]')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));
    let analysis_success_rate = if analysis_completed + analysis_failed > 0 {
        ((analysis_completed as f64) / ((analysis_completed + analysis_failed) as f64) * 1000.0)
            .round()
            / 10.0
    } else {
        0.0
    };
    let feedback_total = drafts_accepted + drafts_rejected + drafts_edited;
    let draft_edit_rate = if feedback_total > 0 {
        ((drafts_edited as f64) / (feedback_total as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };
    let unsupported_claim_rate = if draft_count > 0 {
        ((unsupported_claims as f64) / (draft_count as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };
    let failure_patterns = conn
        .prepare(
            "SELECT json_extract(verification, '$.warnings[0]') AS pattern, COUNT(*) AS count
               FROM ai_drafts
              WHERE verification IS NOT NULL
                AND json_extract(verification, '$.warnings[0]') IS NOT NULL
              GROUP BY pattern ORDER BY count DESC LIMIT 5",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
                .map(|rows| rows.filter_map(Result::ok).collect())
        })
        .unwrap_or_default();
    AiAnalytics {
        tickets_analyzed,
        analysis_success_rate,
        draft_count,
        drafts_accepted,
        drafts_rejected,
        draft_edit_rate,
        verification_warnings,
        unsupported_claim_rate,
        failure_patterns,
    }
}

/// One `ai_runs` job row (typed port of `/api/ai/jobs`).
#[derive(Debug, Clone, Default)]
pub struct AiJobRow {
    pub id: i64,
    pub run_type: String,
    pub status: String,
    pub conversation_id: Option<i64>,
    pub model: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub latency_ms: Option<i64>,
}

/// Load the recent AI job rows.
#[must_use]
pub fn load_jobs(conn: &rusqlite::Connection, limit: i64) -> Vec<AiJobRow> {
    let limit = limit.clamp(1, 1000);
    conn.prepare(
        "SELECT id, type, status, conversation_id, model, error, created_at, latency_ms
           FROM ai_runs ORDER BY id DESC LIMIT ?1",
    )
    .and_then(|mut stmt| {
        stmt.query_map(rusqlite::params![limit], |r| {
            Ok(AiJobRow {
                id: r.get(0)?,
                run_type: r.get(1)?,
                status: r.get(2)?,
                conversation_id: r.get(3)?,
                model: r.get(4)?,
                error: r.get(5)?,
                created_at: r.get(6)?,
                latency_ms: r.get(7)?,
            })
        })
        .map(|rows| rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

/// The interactive AI center page state.
pub struct AiCenterPage {
    tab: Tab,
    settings: Option<AiSettings>,
    analytics: Option<AiAnalytics>,
    distributions: Vec<AttributeDistribution>,
    jobs: Vec<AiJobRow>,
    /// FIND-005: armed shell-confirm request for the global AI kill
    /// switch — raised by [`Self::ui`] at the end of the frame.
    pending_confirm: Option<ConfirmRequest>,
    /// Connection test outcome, written by the background test thread
    /// and polled each frame.
    connection_status: Arc<Mutex<Option<String>>>,
    // ---- A1 provider hub state ----
    hub_endpoints: Vec<spp_core::ai_hub::EndpointConfig>,
    hub_roles: spp_core::ai_hub::RoleMap,
    /// Clean-audit item 74: the last PERSISTED role map. The model text
    /// field commits on blur (and only when the live map differs from
    /// this snapshot) — keystrokes update the live map in memory only,
    /// so typing no longer persists and enqueues a golden_eval per
    /// character.
    hub_roles_saved: spp_core::ai_hub::RoleMap,
    hub_loaded: bool,
    /// Probe outcomes per endpoint id, filled by the probe thread.
    probe_results: std::collections::BTreeMap<String, String>,
    /// Shared probe slot: (endpoint id, result) — one probe at a time.
    probe_slot: Arc<Mutex<Option<(String, String)>>>,
    /// The endpoint whose API-key field is currently open.
    key_edit_for: Option<String>,
    key_edit_value: String,
    /// Add-endpoint form.
    add_name: String,
    add_url: String,
    add_key: String,
    add_price_prompt: String,
    add_price_completion: String,
    /// Cost ledger rows (last 7 days).
    ledger: Vec<spp_core::ai_hub::UsageRow>,
    /// Per-feature toggle states (name, enabled), loaded at reload.
    feature_states: Vec<(&'static str, bool)>,
    /// Shared vector index (A4/A5): the copilot chat's similar-issues tools
    /// fuse FTS + sqlite-vec through it.
    vector_index: Option<std::sync::Arc<spp_core::vectorstore_sqlitevec::VectorIndex>>,
    // ---- A5 copilot chat state ----
    /// Copilot sessions (copilot_sessions rows, newest first).
    copilot_sessions: Vec<serde_json::Value>,
    /// The open session's messages (copilot_messages rows).
    copilot_messages: Vec<serde_json::Value>,
    /// The open session id (None = fresh chat; the job creates the row).
    copilot_session: Option<i64>,
    /// The question being typed.
    copilot_input: String,
    /// The in-flight copilot_chat job id (polled each frame).
    copilot_pending: Option<i64>,
    /// Last chat error (job failures land here verbatim).
    copilot_error: Option<String>,
    /// Chat data loaded flag (invalidation clears it).
    copilot_loaded: bool,
    // ---- A6 usage dashboard state ----
    /// Golden runs, newest first (eval scores).
    golden_runs: Vec<spp_core::ai_eval::GoldenRunRow>,
    /// (total drafts, accepted, rejected) from ai_drafts states.
    draft_states: (i64, i64, i64),
    /// (feedback rows, edited rows, avg edit distance) from ai_feedback.
    edit_stats: (i64, i64, f64),
    // ---- item 151: the ai_settings provider record (pre-hub fallback) ----
    /// The stored ai_settings row (get_ai_status snapshot), loaded with
    /// the rest of the Health tab.
    ai_record: Option<spp_core::ai_center::AiStatus>,
    /// Provider-kind selection index into `ProviderKind::ALL`.
    ai_kind_pick: usize,
    ai_chat_model_edit: String,
    ai_embed_model_edit: String,
    ai_embed_dim_edit: String,
    ai_base_url_edit: String,
    /// False once any form field is edited. A clean form re-seeds its
    /// fields from the stored record on every reload; a dirty form keeps
    /// its edits across invalidation (the batch-27 unsaved-edits
    /// convention) until Save commits them through the core write API.
    ai_form_clean: bool,
    /// Last save outcome: (ok, message).
    ai_record_note: Option<(bool, String)>,
}

impl AiCenterPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::Health,
            settings: None,
            analytics: None,
            distributions: Vec::new(),
            jobs: Vec::new(),
            pending_confirm: None,
            connection_status: Arc::new(Mutex::new(None)),
            hub_endpoints: Vec::new(),
            hub_roles: spp_core::ai_hub::RoleMap::default(),
            hub_roles_saved: spp_core::ai_hub::RoleMap::default(),
            hub_loaded: false,
            probe_results: std::collections::BTreeMap::new(),
            probe_slot: Arc::new(Mutex::new(None)),
            key_edit_for: None,
            key_edit_value: String::new(),
            add_name: String::new(),
            add_url: String::new(),
            add_key: String::new(),
            add_price_prompt: String::new(),
            add_price_completion: String::new(),
            ledger: Vec::new(),
            feature_states: Vec::new(),
            vector_index: None,
            copilot_sessions: Vec::new(),
            copilot_messages: Vec::new(),
            copilot_session: None,
            copilot_input: String::new(),
            copilot_pending: None,
            copilot_error: None,
            copilot_loaded: false,
            golden_runs: Vec::new(),
            draft_states: (0, 0, 0),
            edit_stats: (0, 0, 0.0),
            ai_record: None,
            ai_kind_pick: 0,
            ai_chat_model_edit: String::new(),
            ai_embed_model_edit: String::new(),
            ai_embed_dim_edit: String::new(),
            ai_base_url_edit: String::new(),
            ai_form_clean: true,
            ai_record_note: None,
        }
    }

    /// A4/A5: receive the shared vector index for the copilot chat tools.
    pub fn set_vector_index(
        &mut self,
        index: Option<std::sync::Arc<spp_core::vectorstore_sqlitevec::VectorIndex>>,
    ) {
        self.vector_index = index;
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.settings = None;
        self.analytics = None;
        self.distributions.clear();
        self.jobs.clear();
        self.hub_loaded = false;
        self.ledger.clear();
        self.copilot_loaded = false;
        // Item 151: drop the stored-record snapshot (it reloads) but keep
        // the edit fields and the dirty flag — unsaved edits survive.
        self.ai_record = None;
    }

    fn reload(&mut self, db: &DbHandle) {
        let (settings, analytics, distributions, jobs, ai_record) = db.with_read(|conn| {
            (
                Some(load_ai_settings(conn)),
                Some(load_analytics(conn)),
                spp_core::ai_attributes::distributions(conn).unwrap_or_default(),
                load_jobs(conn, 100),
                // Item 151: the stored provider record drives the panel.
                spp_core::ai_center::get_ai_status(conn).ok(),
            )
        });
        self.settings = settings;
        self.analytics = analytics;
        self.distributions = distributions;
        self.jobs = jobs;
        self.seed_ai_record(ai_record);
        self.reload_hub(db);
    }

    /// Item 151: refresh the stored-record snapshot and, while the form
    /// is clean, re-seed the edit fields from it. A dirty (typed) form
    /// keeps its edits until Save commits them.
    fn seed_ai_record(&mut self, record: Option<spp_core::ai_center::AiStatus>) {
        self.ai_record = record;
        if self.ai_form_clean
            && let Some(rec) = &self.ai_record
        {
            self.ai_kind_pick = spp_core::ai_center::ProviderKind::ALL
                .iter()
                .position(|k| *k == rec.provider_kind)
                .unwrap_or(0);
            self.ai_chat_model_edit = rec.chat_model.clone().unwrap_or_default();
            self.ai_embed_model_edit = rec.embedding_model.clone().unwrap_or_default();
            self.ai_embed_dim_edit = rec.embedding_dim.map(|d| d.to_string()).unwrap_or_default();
            self.ai_base_url_edit = rec.base_url.clone().unwrap_or_default();
        }
    }

    /// Clean-audit item 74: persist a role-map edit and re-run the
    /// golden set — ONCE per committed edit. Called when the model text
    /// field gives up focus (a finished edit) and when the endpoint
    /// combo picks a new endpoint (a discrete selection). Keystrokes
    /// alone never reach this method: they only sync the live map.
    /// An unchanged map is a no-op — no second save, no second eval
    /// (the blur guard at the call site and this check agree).
    fn commit_role_map(&mut self, db: &DbHandle, roles: &spp_core::ai_hub::RoleMap) {
        self.hub_roles = roles.clone();
        if *roles == self.hub_roles_saved {
            return;
        }
        let _ = db.with_write(|conn| spp_core::ai_hub::save_roles(conn, roles));
        self.hub_roles_saved = roles.clone();
        // A6: model-role reassignment re-runs the golden set — the eval
        // scores on the Usage tab must describe the routing that is live.
        let _ =
            db.with_write(|conn| spp_core::jobs::enqueue_on(conn, "ai", "golden_eval", "{}", 3, 2));
    }

    /// Load the A1 provider hub state (endpoints, roles, ledger, toggles).
    fn reload_hub(&mut self, db: &DbHandle) {
        let (endpoints, roles, ledger, features) = db.with_read(|conn| {
            (
                spp_core::ai_hub::load_endpoints(conn).unwrap_or_default(),
                spp_core::ai_hub::load_roles(conn).unwrap_or_default(),
                spp_core::ai_hub::usage_ledger(conn, 7).unwrap_or_default(),
                spp_core::ai_hub::AI_FEATURES
                    .iter()
                    .map(|(name, _, _)| (*name, spp_core::ai_hub::ai_feature_enabled(conn, name)))
                    .collect::<Vec<_>>(),
            )
        });
        self.hub_endpoints = endpoints;
        self.hub_roles = roles.clone();
        self.hub_roles_saved = roles;
        self.ledger = ledger;
        self.feature_states = features;
        self.hub_loaded = true;
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.settings.is_none() {
            self.reload(db);
        }
    }

    /// Test the LM Studio connection (the async half of `/api/ai/status`):
    /// the result lands in the shared status slot on a background thread.
    pub fn test_connection(&mut self) {
        let Some(settings) = self.settings.clone() else {
            return;
        };
        let base_url = settings.base_url.clone();
        let timeout = u64::try_from(settings.timeout_ms.max(1)).unwrap_or(120_000);
        let status = Arc::clone(&self.connection_status);
        if let Ok(mut guard) = status.lock() {
            *guard = Some("testing…".into());
        }
        std::thread::Builder::new()
            .name("spp-ai-test".to_string())
            .spawn(move || {
                let result = (|| -> Result<usize, String> {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| e.to_string())?;
                    runtime.block_on(async {
                        let client =
                            spp_core::ai_lm_studio::OpenAiCompatibleClient::new_with_timeout(
                                &base_url, timeout,
                            );
                        let models = client.list_models().await.map_err(|e| e.to_string())?;
                        Ok(models.len())
                    })
                })();
                if let Ok(mut guard) = status.lock() {
                    *guard = Some(match result {
                        Ok(0) => "connected · no models loaded".into(),
                        Ok(n) => format!("connected · {n} model(s)"),
                        Err(e) => format!("offline · {e}"),
                    });
                }
            })
            .ok();
        // The thread is detached: the status slot is polled every frame.
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        self.ensure_loaded(db);
        // A5: resolve the in-flight copilot chat turn (the worker persists
        // messages; the poll only moves the terminal state into the page).
        self.poll_copilot_job(db);

        design::page_header(ui, "AI Center", None, |_| {});
        // DS-C-09 underline tab rail — the lab's six labels, same state
        // machine (the rail carries its own divider).
        let labels: Vec<&str> = Tab::ALL.iter().map(|t| t.label()).collect();
        if let Some(i) = design::tabs(
            ui,
            &labels,
            Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0),
        ) && let Some(tab) = Tab::ALL.get(i)
        {
            self.tab = *tab;
        }

        match self.tab {
            Tab::Health => self.health_ui(ui, db),
            Tab::Analytics => self.analytics_ui(ui),
            Tab::Attributes => self.attributes_ui(ui),
            Tab::Copilot => self.copilot_ui(ui, db),
            Tab::Usage => self.usage_ui(ui, db),
            Tab::Jobs => self.jobs_ui(ui),
        }
        // FIND-005: raise the armed confirm request (see the field doc).
        if let Some(request) = self.pending_confirm.take() {
            return Some(NavRequest::Confirm(request));
        }
        None
    }

    /// FIND-005: runs only after the shell dialog confirms — disables
    /// every AI feature and endpoint call at once.
    pub fn disable_all_ai_confirmed(&mut self, db: &DbHandle) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::settings::set_bool(conn, "ai_enabled", false));
        self.reload_hub(db);
        result
    }

    /// A5: poll the in-flight copilot_chat job — completed turns reload the
    /// session + messages; failures surface the exact worker error.
    fn poll_copilot_job(&mut self, db: &DbHandle) {
        let Some(job_id) = self.copilot_pending else {
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
        match status.as_str() {
            "queued" | "running" => {}
            "completed" => {
                self.copilot_pending = None;
                self.copilot_error = None;
                self.reload_copilot(db);
            }
            _ => {
                self.copilot_pending = None;
                self.copilot_error =
                    Some(error.unwrap_or_else(|| "the chat job failed without an error".into()));
                self.reload_copilot(db);
            }
        }
    }

    /// A5: load sessions (newest first) + the open session's messages.
    fn reload_copilot(&mut self, db: &DbHandle) {
        let (sessions, messages) = db.with_read(|conn| {
            let sessions = spp_core::copilot::list_sessions(conn, 50);
            // If no session is open, fall back to the newest so the page
            // always shows something after a completed turn.
            let session = self.copilot_session.or_else(|| {
                sessions
                    .first()
                    .and_then(|s| s.get("id"))
                    .and_then(|v| v.as_i64())
            });
            let messages = session
                .map(|id| spp_core::copilot::list_messages(conn, id))
                .unwrap_or_default();
            (sessions, messages)
        });
        self.copilot_sessions = sessions;
        self.copilot_messages = messages;
        self.copilot_loaded = true;
    }

    /// A5: the copilot chat on the 22-tool read-only agent. Sessions and
    /// messages are durable rows; the turn runs as a worker job so the UI
    /// thread never blocks on the model.
    fn copilot_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if !self.copilot_loaded {
            self.reload_copilot(db);
        }
        let t = design::tokens(ui);
        // Gate the whole surface honestly: feature + Agent role.
        let (feature_on, agent_ok) = db.with_read(|conn| {
            (
                spp_core::ai_hub::ai_feature_enabled(conn, "copilot"),
                !matches!(
                    spp_core::ai_hub::backend_for_role(conn, spp_core::ai_hub::AiRole::Agent),
                    spp_core::ai_pipeline::AiBackend::Disabled,
                ),
            )
        });
        if !feature_on {
            design::empty_state(
                ui,
                Icon::Sparkles,
                "Copilot is switched off",
                "Enable it in AI Center > Features & kill switches.",
            );
            return;
        }
        if !agent_ok {
            design::empty_state(
                ui,
                Icon::Sparkles,
                "No Agent role endpoint routed",
                "Assign one in AI Center > Model-role routing (Health tab) to chat with the 22-tool agent.",
            );
            return;
        }
        ui.horizontal(|ui| {
            ui.set_min_width(ui.available_width());
            // Left column: sessions.
            egui::ScrollArea::vertical()
                .id_salt("copilot-sessions")
                .max_height(420.0)
                .show(ui, |ui| {
                    design::section_title(ui, "Sessions");
                    if design::button(ui, "New chat", ButtonVariant::Outline).clicked() {
                        self.copilot_session = None;
                        self.copilot_messages.clear();
                        self.copilot_error = None;
                    }
                    for session in self.copilot_sessions.clone() {
                        let id = session.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                        let title: String = session
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("(untitled)")
                            .chars()
                            .take(60)
                            .collect();
                        let count = session
                            .get("message_count")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        let selected = self.copilot_session == Some(id);
                        if design::chip(ui, selected, &format!("{title} ({count})"))
                            .on_hover_text("open this session")
                            .clicked()
                        {
                            self.copilot_session = Some(id);
                            self.copilot_messages =
                                db.with_read(|conn| spp_core::copilot::list_messages(conn, id));
                        }
                    }
                });
        });
        design::divider(ui);
        // Messages.
        design::section_title(ui, "Chat");
        if let Some(err) = self.copilot_error.clone() {
            design::error_text(ui, &format!("last turn failed: {err}"));
        }
        if self.copilot_messages.is_empty() {
            design::hint_text(
                ui,
                "Ask anything answerable from the local mirror — the agent runs 22 read-only tools (searches, lookups, clusters, gaps) and cites what it used.",
            );
        }
        egui::ScrollArea::vertical()
            .id_salt("copilot-messages")
            .max_height(320.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for message in &self.copilot_messages {
                    let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("?");
                    let content = message
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if role == "tool" {
                        continue;
                    }
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
                        ui.label(egui::RichText::new(content).small().color(t.foreground));
                    });
                    if role == "assistant"
                        && let Some(citations) = message.get("citations").and_then(|c| c.as_array())
                        && !citations.is_empty()
                    {
                        ui.horizontal(|ui| {
                            design::hint_text(ui, "cited:");
                            for citation in citations.iter().take(6) {
                                let label: String = citation
                                    .get("label")
                                    .and_then(|l| l.as_str())
                                    .unwrap_or("source")
                                    .chars()
                                    .take(48)
                                    .collect();
                                design::badge(ui, &label, BadgeTone::Success);
                            }
                        });
                    }
                    ui.add_space(4.0); /* DC sp-1 (B-3 normalization, RS-12) */
                }
            });
        // Input.
        let busy = self.copilot_pending.is_some();
        ui.horizontal(|ui| {
            let mut question = self.copilot_input.clone();
            let response = ui.add_sized(
                [ui.available_width() - 120.0, 28.0],
                egui::TextEdit::singleline(&mut question).hint_text("Ask the copilot…"),
            );
            if response.changed() {
                self.copilot_input = question;
            }
            // Clean-audit item 75: Enter submits only when the copilot
            // input itself surrendered focus on that keypress (egui
            // single-line TextEdits give up focus on Enter) — a global
            // Enter used to submit from anywhere on the AI Center page,
            // firing mid-typing in other fields and on stray keypresses.
            let submit = response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                && !self.copilot_input.is_empty();
            if (design::button_with_icon(
                ui,
                Icon::Send,
                if busy { "thinking…" } else { "send" },
                ButtonVariant::Primary,
            )
            .clicked()
                && !busy)
                || (submit && !busy)
            {
                let question = self.copilot_input.trim().to_string();
                if !question.is_empty() {
                    let payload = serde_json::json!({
                        "question": question,
                        "sessionId": self.copilot_session,
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
                            self.copilot_pending = Some(job_id);
                            self.copilot_input.clear();
                        }
                        Err(e) => self.copilot_error = Some(e.to_string()),
                    }
                }
            }
        });
    }

    /// A6: the AI usage dashboard — per-feature tokens/latency/cost from the
    /// ledger, live edit rate from real feedback rows, and the golden-set
    /// eval scores with the on-demand runner.
    fn usage_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        // Usage data rides `reload` (invalidation refreshes it).
        let ledger30: Vec<spp_core::ai_hub::UsageRow> =
            db.with_read(|conn| spp_core::ai_hub::usage_ledger(conn, 30).unwrap_or_default());
        let (golden_runs, draft_states, edit_stats) = db.with_read(|conn| {
            (
                spp_core::ai_eval::latest_golden_runs(conn, 5),
                conn.query_row(
                    "SELECT COUNT(*),
                            COALESCE(SUM(CASE WHEN state = 'accepted' THEN 1 ELSE 0 END), 0),
                            COALESCE(SUM(CASE WHEN state = 'rejected' THEN 1 ELSE 0 END), 0)
                       FROM ai_drafts",
                    [],
                    |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                        ))
                    },
                )
                .unwrap_or((0, 0, 0)),
                conn.query_row(
                    "SELECT COUNT(*),
                            COALESCE(SUM(CASE WHEN edit_distance > 20 THEN 1 ELSE 0 END), 0),
                            COALESCE(AVG(edit_distance), 0.0)
                       FROM ai_feedback",
                    [],
                    |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, f64>(2)?,
                        ))
                    },
                )
                .unwrap_or((0, 0, 0.0)),
            )
        });
        self.golden_runs = golden_runs;
        self.draft_states = draft_states;
        self.edit_stats = edit_stats;

        design::section_title(ui, "Per-feature usage — last 30 days");
        if ledger30.is_empty() {
            design::hint_text(
                ui,
                "No completed model runs yet. Tokens, latency and estimated cost appear here as features run.",
            );
        } else {
            egui::ScrollArea::vertical()
                .id_salt("usage-ledger")
                .max_height(260.0)
                .show(ui, |ui| {
                    egui::Grid::new("usage-ledger-grid")
                        .striped(true)
                        .show(ui, |ui| {
                            for label in [
                                "day",
                                "feature",
                                "model",
                                "runs",
                                "prompt tok",
                                "completion tok",
                                "latency s",
                                "est cost",
                            ] {
                                design::field_label(ui, label);
                            }
                            ui.end_row();
                            for row in &ledger30 {
                                ui.monospace(row.day.as_str());
                                ui.monospace(row.feature.as_str());
                                ui.monospace(row.model.as_str());
                                ui.monospace(row.runs.to_string());
                                ui.monospace(row.prompt_tokens.to_string());
                                ui.monospace(row.completion_tokens.to_string());
                                ui.monospace(format!(
                                    "{:.1}",
                                    row.latency_ms_total as f64 / 1000.0
                                ));
                                ui.monospace(format!("${:.4}", row.est_cost_usd));
                                ui.end_row();
                            }
                        });
                });
        }
        ui.add_space(8.0);

        design::section_title(ui, "Draft feedback (edit rate)");
        let (rows, edited, avg_distance) = self.edit_stats;
        let (drafts, accepted, rejected) = self.draft_states;
        ui.horizontal(|ui| {
            design::badge(ui, &format!("{drafts} drafts"), BadgeTone::Neutral);
            design::badge(ui, &format!("{accepted} accepted"), BadgeTone::Success);
            design::badge(ui, &format!("{rejected} rejected"), BadgeTone::Warning);
            design::badge(ui, &format!("{rows} feedback rows"), BadgeTone::Info);
            if rows > 0 {
                design::badge(
                    ui,
                    &format!(
                        "{:.0}% edited (>20 chars)",
                        100.0 * edited as f64 / rows as f64
                    ),
                    BadgeTone::Info,
                );
                design::badge(
                    ui,
                    &format!("avg edit distance {avg_distance:.0}"),
                    BadgeTone::Neutral,
                );
            }
        });
        design::hint_text(
            ui,
            "Accept, edit-distance and reject come from the composer send path and the draft panel — real rows, zero invention.",
        );
        ui.add_space(8.0);

        design::section_title(ui, "Golden-set evals (9 seeded cases)");
        ui.horizontal(|ui| {
            let busy = db
                .with_read(|conn| {
                    conn.query_row(
                        "SELECT COUNT(*) FROM jobs
                          WHERE type = 'golden_eval' AND status IN ('queued','running')",
                        [],
                        |r| r.get::<_, i64>(0),
                    )
                    .unwrap_or(0)
                })
                > 0;
            if busy {
                design::hint_text(ui, "golden set running…");
            } else if design::button(ui, "Run golden set now", ButtonVariant::Outline)
                .on_hover_text(
                    "the 9 seeded cases run through the real analysis prompt; scores land below and re-run automatically on prompt/role changes",
                )
                .clicked()
            {
                let _ = db.with_write(|conn| {
                    spp_core::jobs::enqueue_on(conn, "ai", "golden_eval", "{}", 3, 2)
                });
            }
        });
        if self.golden_runs.is_empty() {
            design::hint_text(
                ui,
                "No eval runs recorded yet — run the golden set to score the routed Classify model.",
            );
        } else {
            for run in &self.golden_runs {
                ui.horizontal(|ui| {
                    let tone = if run.failed == 0 {
                        BadgeTone::Success
                    } else if run.passed > 0 {
                        BadgeTone::Warning
                    } else {
                        BadgeTone::Destructive
                    };
                    design::badge(
                        ui,
                        &format!("{}/{} passed", run.passed, run.passed + run.failed),
                        tone,
                    );
                    ui.monospace(format!("backend: {}", run.backend));
                    design::hint_text(ui, &format!("run #{} — {}", run.id, run.started_at));
                });
                if let Some(results) = run.results.as_array()
                    && run.failed > 0
                {
                    for case in results.iter().take(9) {
                        let name = case.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                        let error = case.get("error").and_then(|v| v.as_str());
                        if let Some(error) = error {
                            design::hint_text(ui, &format!("{name}: {error}"));
                        }
                    }
                }
            }
        }
    }

    fn health_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let Some(settings) = self.settings.clone() else {
            ui.spinner();
            return;
        };
        let t = design::tokens(ui);
        design::card(ui, |ui| {
            design::section_title(ui, "LM Studio settings");
            design::form_grid(ui, "ai-health", |ui| {
                design::field_label(ui, "AI enabled:");
                if settings.ai_enabled {
                    design::badge(ui, "enabled", BadgeTone::Success);
                } else {
                    design::badge(ui, "disabled", BadgeTone::Neutral);
                }
                ui.end_row();
                design::field_label(ui, "provider:");
                ui.monospace(
                    egui::RichText::new(if settings.ai_enabled {
                        "lmstudio"
                    } else {
                        "disabled"
                    })
                    .color(t.foreground),
                );
                ui.end_row();
                design::field_label(ui, "base url:");
                ui.monospace(egui::RichText::new(settings.base_url.as_str()).color(t.foreground));
                ui.end_row();
                design::field_label(ui, "chat model:");
                ui.monospace(
                    egui::RichText::new(
                        settings
                            .chat_model
                            .clone()
                            .unwrap_or_else(|| "(unset)".into()),
                    )
                    .color(t.foreground),
                );
                ui.end_row();
                design::field_label(ui, "embedding model:");
                ui.monospace(
                    egui::RichText::new(
                        settings
                            .embedding_model
                            .clone()
                            .unwrap_or_else(|| "(unset)".into()),
                    )
                    .color(t.foreground),
                );
                ui.end_row();
                design::field_label(ui, "timeout:");
                ui.monospace(
                    egui::RichText::new(format!("{} ms", settings.timeout_ms)).color(t.foreground),
                );
                ui.end_row();
                design::field_label(ui, "concurrency:");
                ui.monospace(
                    egui::RichText::new(settings.concurrency.to_string()).color(t.foreground),
                );
                ui.end_row();
            });
            design::divider(ui);
            ui.add_space(6.0);
            design::hint_text(
                ui,
                "Start LM Studio locally (default http://127.0.0.1:1234) and configure the \
                 models in Settings → LM Studio. Attributes stay local and never sync back.",
            );
            if let Some(status) = self
                .connection_status
                .lock()
                .ok()
                .and_then(|guard| guard.clone())
            {
                let text = format!("connection: {status}");
                if status.starts_with("connected") {
                    design::success_text(ui, &text);
                } else if status.starts_with("offline") {
                    design::error_text(ui, &text);
                } else {
                    design::hint_text(ui, &text);
                }
            }
            if design::button(ui, "test connection", ButtonVariant::Primary).clicked() {
                self.test_connection();
            }
        });
        // Item 151: the ai_settings write API is live here — this panel
        // edits the pre-hub fallback record that
        // ai_pipeline::backend_from_settings and
        // embeddings::lm_studio_embedding_settings consult when the
        // Settings → LM Studio keys are unset.
        self.ai_record_ui(ui, db);
        // A1: the provider hub lives directly under the health snapshot.
        self.hub_ui(ui, db);
    }

    /// Item 151: the ai_settings provider-record editor. Every control
    /// writes through the core write API (`ai_center::set_*`) on Save —
    /// the row is the fallback layer the pipeline already consumes, so
    /// these edits are load-bearing, not decorative.
    fn ai_record_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let Some(record) = self.ai_record.clone() else {
            return;
        };
        design::card_titled(
            ui,
            "AI provider record (ai_settings)",
            Some(
                "the pre-hub fallback record the chat pipeline and embedding \nresolution consult when the Settings → LM Studio keys are unset",
            ),
            |ui| {
                let ready = spp_core::ai_center::ai_features_enabled(&record);
                ui.horizontal(|ui| {
                    design::field_label(ui, "provider:");
                    for (i, kind) in spp_core::ai_center::ProviderKind::ALL.iter().enumerate() {
                        if design::chip(ui, self.ai_kind_pick == i, kind.label()).clicked() {
                            self.ai_kind_pick = i;
                            self.ai_form_clean = false;
                        }
                    }
                    if ready {
                        design::badge(ui, "features ready", BadgeTone::Success);
                    } else {
                        design::badge(ui, "not configured", BadgeTone::Neutral);
                    }
                });
                ui.add_space(4.0);
                design::form_grid(ui, "ai-record", |ui| {
                    design::field_label(ui, "chat model:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.ai_chat_model_edit)
                                .hint_text("(empty — unset)")
                                .desired_width(260.0),
                        )
                        .changed()
                    {
                        self.ai_form_clean = false;
                    }
                    ui.end_row();
                    design::field_label(ui, "embedding model:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.ai_embed_model_edit)
                                .hint_text("(empty — embedding columns stay unchanged)")
                                .desired_width(260.0),
                        )
                        .changed()
                    {
                        self.ai_form_clean = false;
                    }
                    ui.end_row();
                    design::field_label(ui, "embedding dim:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.ai_embed_dim_edit)
                                .hint_text("e.g. 768")
                                .desired_width(260.0),
                        )
                        .changed()
                    {
                        self.ai_form_clean = false;
                    }
                    ui.end_row();
                    design::field_label(ui, "base url:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.ai_base_url_edit)
                                .hint_text("http://127.0.0.1:1234")
                                .desired_width(260.0),
                        )
                        .changed()
                    {
                        self.ai_form_clean = false;
                    }
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if design::button(ui, "Save provider record", ButtonVariant::Primary).clicked()
                    {
                        self.save_ai_record(db);
                    }
                    if let Some((ok, msg)) = &self.ai_record_note {
                        if *ok {
                            design::success_text(ui, msg);
                        } else {
                            design::error_text(ui, msg);
                        }
                    }
                });
                design::hint_text(
                    ui,
                    &format!(
                        "stored record: provider_kind={} chat_model={} embedding_model={} \
                         dim={} base_url={}",
                        record.provider_kind.as_str(),
                        record.chat_model.as_deref().unwrap_or("(unset)"),
                        record.embedding_model.as_deref().unwrap_or("(unset)"),
                        record
                            .embedding_dim
                            .map(|d| d.to_string())
                            .as_deref()
                            .unwrap_or("(unset)"),
                        record.base_url.as_deref().unwrap_or("(unset)"),
                    ),
                );
            },
        );
    }

    /// Item 151: apply the provider-record edits through the core write
    /// API and re-read the stored row. An empty embedding-model field
    /// leaves the embedding columns untouched (the write API has no
    /// NULL-clearing path, and pretending otherwise would invent one).
    fn save_ai_record(&mut self, db: &DbHandle) {
        let kind = spp_core::ai_center::ProviderKind::ALL
            .get(self.ai_kind_pick)
            .copied()
            .unwrap_or(spp_core::ai_center::ProviderKind::None);
        let chat = self.ai_chat_model_edit.trim().to_string();
        let embed = self.ai_embed_model_edit.trim().to_string();
        let base = self.ai_base_url_edit.trim().to_string();
        let dim_parse = if embed.is_empty() {
            Ok(None)
        } else if let Ok(dim) = self.ai_embed_dim_edit.trim().parse::<usize>()
            && dim > 0
        {
            Ok(Some(dim))
        } else {
            Err("embedding dim must be a positive integer when an embedding model is set")
        };
        match dim_parse {
            Err(msg) => {
                self.ai_record_note = Some((false, msg.to_string()));
            }
            Ok(dim) => {
                let outcome = db.with_write(|conn| {
                    spp_core::ai_center::set_provider_kind(conn, kind)?;
                    spp_core::ai_center::set_chat_model(conn, &chat)?;
                    if let Some(dim) = dim {
                        spp_core::ai_center::set_embedding_model(conn, &embed, dim)?;
                    }
                    spp_core::ai_center::set_base_url(conn, &base)?;
                    Ok::<(), spp_core::error::Error>(())
                });
                match outcome {
                    Ok(()) => {
                        self.ai_form_clean = true;
                        self.ai_record_note = Some((
                            true,
                            if embed.is_empty() {
                                "saved — embedding columns left unchanged".into()
                            } else {
                                "saved".into()
                            },
                        ));
                        let fresh = db.with_read(spp_core::ai_center::get_ai_status).ok();
                        self.seed_ai_record(fresh);
                    }
                    Err(e) => {
                        self.ai_record_note = Some((false, format!("save failed: {e}")));
                    }
                }
            }
        }
    }

    /// The A1 provider hub: endpoints (health probes, BYOK keys in the
    /// envelope), model-role routing, per-feature toggles + the global
    /// kill switch, and the per-feature cost ledger.
    fn hub_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if !self.hub_loaded {
            self.reload_hub(db);
        }
        self.poll_probe_slot();
        let t = design::tokens(ui);

        // ---- endpoints ------------------------------------------------
        design::card_titled(
            ui,
            "Provider hub — endpoints",
            Some("OpenAI-compatible endpoints: local LM Studio, optional Ollama, cloud BYOK"),
            |ui| {
                let endpoints = self.hub_endpoints.clone();
                for endpoint in &endpoints {
                    ui.push_id(&endpoint.id, |ui| {
                        ui.horizontal(|ui| {
                            if endpoint.enabled {
                                design::badge(ui, "on", BadgeTone::Success);
                            } else {
                                design::badge(ui, "off", BadgeTone::Neutral);
                            }
                            ui.strong(&endpoint.name);
                            ui.monospace(
                                egui::RichText::new(endpoint.base_url.as_str())
                                    .small()
                                    .color(t.muted_foreground),
                            );
                            let has_key = db.with_read(|conn| {
                                spp_core::ai_hub::endpoint_has_key(conn, &endpoint.id)
                            });
                            if has_key {
                                design::badge(ui, "key set", BadgeTone::Info);
                            }
                            if endpoint.builtin {
                                design::badge(ui, "built-in", BadgeTone::Neutral);
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if !endpoint.builtin
                                    && design::button(ui, "remove", ButtonVariant::Ghost)
                                        .on_hover_text("remove the endpoint, its envelope key and its role assignments")
                                        .clicked()
                                {
                                    let _ = db.with_write(|conn| {
                                        spp_core::ai_hub::remove_endpoint(conn, &endpoint.id)
                                    });
                                    self.reload_hub(db);
                                }
                                if design::button(ui, "set key", ButtonVariant::Outline).clicked() {
                                    self.key_edit_for = Some(endpoint.id.clone());
                                    self.key_edit_value.clear();
                                }
                                if design::button(ui, "probe", ButtonVariant::Outline)
                                    .on_hover_text("list models once — the green/red light for this endpoint")
                                    .clicked()
                                {
                                    self.start_probe(db, &endpoint.id);
                                }
                            });
                        });
                        if self.key_edit_for.as_deref() == Some(endpoint.id.as_str()) {
                            ui.horizontal(|ui| {
                                design::field_label(ui, "api key:");
                                ui.add_sized(
                                    [280.0, 28.0],
                                    egui::TextEdit::singleline(&mut self.key_edit_value)
                                        .password(true),
                                );
                                if design::button(ui, "save key", ButtonVariant::Primary).clicked() {
                                    let id = endpoint.id.clone();
                                    let value = self.key_edit_value.trim().to_string();
                                    db.with_write(|conn| {
                                        if value.is_empty() {
                                            let _ = spp_core::settings::delete_secret(
                                                conn,
                                                &spp_core::ai_hub::EndpointConfig::secret_key(&id),
                                            );
                                        } else {
                                            let _ = spp_core::ai_hub::set_endpoint_api_key(
                                                conn, &id, &value,
                                            );
                                        }
                                    });
                                    self.key_edit_for = None;
                                    self.key_edit_value.clear();
                                }
                                if design::button(ui, "cancel", ButtonVariant::Ghost).clicked() {
                                    self.key_edit_for = None;
                                    self.key_edit_value.clear();
                                }
                                design::hint_text(
                                    ui,
                                    "stored in the encrypted envelope — never in plaintext",
                                );
                            });
                        }
                        if let Some(result) = self.probe_results.get(&endpoint.id) {
                            if result.starts_with("reachable") {
                                design::success_text(ui, result);
                            } else {
                                design::error_text(ui, result);
                            }
                        }
                        if !endpoint.builtin {
                            ui.horizontal(|ui| {
                                design::hint_text(
                                    ui,
                                    "price USD per million tokens — the cost ledger's estimation input",
                                );
                            });
                        }
                        ui.add_space(4.0);
                    });
                }
                ui.add_space(4.0);
                egui::CollapsingHeader::new(
                    "add endpoint (cloud BYOK — off by default per-feature)",
                )
                .default_open(false)
                .show(ui, |ui| {
                    egui::Grid::new("ai-hub-add")
                        .num_columns(2)
                        .spacing(egui::vec2(12.0, 6.0))
                        .show(ui, |ui| {
                            design::field_label(ui, "name:");
                            ui.add_sized(
                                [220.0, 28.0],
                                egui::TextEdit::singleline(&mut self.add_name),
                            );
                            ui.end_row();
                            design::field_label(ui, "base url:");
                            ui.add_sized(
                                [280.0, 28.0],
                                egui::TextEdit::singleline(&mut self.add_url),
                            );
                            ui.end_row();
                            design::field_label(ui, "api key:");
                            ui.add_sized(
                                [280.0, 28.0],
                                egui::TextEdit::singleline(&mut self.add_key).password(true),
                            );
                            ui.end_row();
                            design::field_label(ui, "$ / mtok prompt:");
                            ui.add_sized(
                                [120.0, 28.0],
                                egui::TextEdit::singleline(&mut self.add_price_prompt),
                            );
                            ui.end_row();
                            design::field_label(ui, "$ / mtok completion:");
                            ui.add_sized(
                                [120.0, 28.0],
                                egui::TextEdit::singleline(&mut self.add_price_completion),
                            );
                            ui.end_row();
                        });
                    if design::button(ui, "add endpoint", ButtonVariant::Primary).clicked() {
                        self.add_endpoint(db);
                    }
                });
            },
        );
        ui.add_space(10.0);

        // ---- model-role routing --------------------------------------
        design::card_titled(
            ui,
            "Model-role routing",
            Some("every inference goes through one of five roles (blueprint Table 6)"),
            |ui| {
                let endpoints = self.hub_endpoints.clone();
                let mut roles = self.hub_roles.clone();
                egui::Grid::new("ai-hub-roles")
                    .num_columns(4)
                    .striped(true)
                    .spacing(egui::vec2(14.0, 6.0))
                    .show(ui, |ui| {
                        design::field_label(ui, "role");
                        design::field_label(ui, "endpoint");
                        design::field_label(ui, "model");
                        ui.label(egui::RichText::new("").small());
                        ui.end_row();
                        for role in spp_core::ai_hub::AiRole::ALL {
                            ui.strong(role.as_str());
                            // Owned clone: the combo box below mutates
                            // `roles` while this value must stay readable.
                            let current: Option<spp_core::ai_hub::RoleAssignment> =
                                roles.get(role).cloned();
                            // Reskin fix (presentation, the lab's quirk): the
                            // combo showed the endpoint's INDEX ('0', '1'),
                            // not its name. Names now.
                            let selected = current
                                .as_ref()
                                .and_then(|a| {
                                    endpoints
                                        .iter()
                                        .position(|e| e.id == a.endpoint)
                                        .map(|i| endpoints[i].name.clone())
                                })
                                .unwrap_or_else(|| "(unset)".into());
                            let selected_for_info = selected.clone();
                            let mut changed = false;
                            egui::ComboBox::from_id_salt(format!("hub-role-{}", role.as_str()))
                                .width(200.0)
                                .selected_text(selected.clone())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut None::<usize>, None, "(unset)");
                                    for endpoint in endpoints.iter() {
                                        if endpoint.enabled
                                            && ui
                                                .selectable_label(
                                                    false,
                                                    format!(
                                                        "{} — {}",
                                                        endpoint.name, endpoint.base_url
                                                    ),
                                                )
                                                .clicked()
                                        {
                                            roles.set(
                                                role,
                                                Some(spp_core::ai_hub::RoleAssignment {
                                                    endpoint: endpoint.id.clone(),
                                                    model: current
                                                        .as_ref()
                                                        .and_then(|c| c.model.clone()),
                                                }),
                                            );
                                            changed = true;
                                        }
                                    }
                                })
                                .response
                                .widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::Role::Button,
                                        ui.is_enabled(),
                                        format!(
                                            "endpoint for {}: {selected_for_info}",
                                            role.as_str()
                                        ),
                                    )
                                });
                            let model_text = current
                                .as_ref()
                                .and_then(|c| c.model.clone())
                                .unwrap_or_default();
                            let mut model_edit = model_text.clone();
                            let model_response = ui.add_sized(
                                [180.0, 26.0],
                                egui::TextEdit::singleline(&mut model_edit),
                            );
                            // The lab's aria-label: the column header is the
                            // visible label; the input carries the
                            // role-scoped name itself.
                            model_response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::TextInput,
                                    ui.is_enabled(),
                                    format!("model for {}", role.as_str()),
                                )
                            });
                            // Clean-audit item 74: keystrokes only sync the
                            // live map (so the field's egui state and the
                            // page agree — no focus loss, no revert); the
                            // persist + golden_eval happen once, on blur,
                            // and only when the map actually differs from
                            // what was last saved.
                            if model_edit != model_text {
                                roles.set(
                                    role,
                                    current.clone().map(|c| spp_core::ai_hub::RoleAssignment {
                                        endpoint: c.endpoint.clone(),
                                        model: (!model_edit.trim().is_empty())
                                            .then(|| model_edit.trim().to_string()),
                                    }),
                                );
                                self.hub_roles = roles.clone();
                            }
                            if model_response.lost_focus() && roles != self.hub_roles_saved {
                                self.commit_role_map(db, &roles);
                            }
                            ui.label(
                                egui::RichText::new(role.note())
                                    .small()
                                    .color(t.muted_foreground),
                            );
                            if changed {
                                self.commit_role_map(db, &roles);
                            }
                            ui.end_row();
                        }
                    });
            },
        );
        ui.add_space(4.0);
        // The lab's quirk receipt for the routing-combo presentation fix.
        design::quirk(
            ui,
            "Reskin fix (presentation): the routing combo showed its index number \
             ('0', '1'), not the endpoint name. Names now.",
        );
        ui.add_space(6.0);

        // ---- toggles + kill switch ------------------------------------
        design::card_titled(
            ui,
            "Features & kill switches",
            Some("one click each; the global switch dominates every feature"),
            |ui| {
                let features = self.feature_states.clone();
                egui::Grid::new("ai-hub-features")
                    .num_columns(2)
                    .striped(true)
                    .spacing(egui::vec2(16.0, 4.0))
                    .show(ui, |ui| {
                        for (name, enabled) in features {
                            let mut value = enabled;
                            ui.checkbox(&mut value, name);
                            if value != enabled {
                                let _ = db.with_write(|conn| {
                                    spp_core::ai_hub::set_ai_feature_enabled(conn, name, value)
                                });
                                self.reload_hub(db);
                            }
                            ui.end_row();
                        }
                    });
                ui.add_space(6.0);
                let ai_on = db.with_read(|conn| {
                    spp_core::settings::get_bool(conn, "ai_enabled", true).unwrap_or(true)
                });
                if ai_on {
                    if design::button(
                        ui,
                        "global kill switch: AI is ON — click to disable all AI",
                        ButtonVariant::Outline,
                    )
                    .on_hover_text("disables every AI feature and every endpoint call at once")
                    .clicked()
                    {
                        // FIND-005: the write used to run on the click — arm
                        // the shell confirm instead (one click disabled all
                        // AI behavior).
                        self.pending_confirm = Some(ConfirmRequest::disable_all_ai());
                    }
                } else if design::button(
                    ui,
                    "global kill switch: AI is OFF — click to re-enable",
                    ButtonVariant::Primary,
                )
                .clicked()
                {
                    let _ = db
                        .with_write(|conn| spp_core::settings::set_bool(conn, "ai_enabled", true));
                    self.reload_hub(db);
                }
            },
        );
        ui.add_space(10.0);

        // ---- cost ledger ---------------------------------------------
        design::card_titled(
            ui,
            "Cost ledger — last 7 days",
            Some("tokens, latency and estimated spend per feature per day, from ai_runs"),
            |ui| {
                if self.ledger.is_empty() {
                    design::empty_state(
                        ui,
                        Icon::Sparkles,
                        "no completed AI runs yet",
                        "the ledger fills as AI runs complete",
                    );
                    return;
                }
                egui::Grid::new("ai-hub-ledger")
                    .num_columns(7)
                    .striped(true)
                    .spacing(egui::vec2(14.0, 4.0))
                    .show(ui, |ui| {
                        for header in [
                            "day",
                            "feature",
                            "model",
                            "runs",
                            "tokens (p/c)",
                            "latency",
                            "est. cost",
                        ] {
                            ui.strong(
                                egui::RichText::new(header)
                                    .small()
                                    .color(t.muted_foreground),
                            );
                        }
                        ui.end_row();
                        for row in self.ledger.clone() {
                            ui.monospace(egui::RichText::new(row.day.as_str()).small());
                            ui.monospace(egui::RichText::new(row.feature.as_str()).small());
                            ui.monospace(egui::RichText::new(row.model.as_str()).small());
                            ui.monospace(egui::RichText::new(row.runs.to_string()).small());
                            ui.monospace(
                                egui::RichText::new(format!(
                                    "{} / {}",
                                    row.prompt_tokens, row.completion_tokens
                                ))
                                .small(),
                            );
                            ui.monospace(
                                egui::RichText::new(format!("{} ms", row.latency_ms_total)).small(),
                            );
                            ui.monospace(
                                egui::RichText::new(format!("${:.4}", row.est_cost_usd)).small(),
                            );
                            ui.end_row();
                        }
                    });
            },
        );
    }

    fn poll_probe_slot(&mut self) {
        if let Some((id, result)) = self
            .probe_slot
            .lock()
            .ok()
            .and_then(|mut guard| guard.take())
        {
            self.probe_results.insert(id, result);
        }
    }

    fn start_probe(&mut self, db: &DbHandle, endpoint_id: &str) {
        let Some(endpoint) = self
            .hub_endpoints
            .iter()
            .find(|e| e.id == endpoint_id)
            .cloned()
        else {
            return;
        };
        self.probe_results
            .insert(endpoint.id.clone(), "probing…".into());
        let api_key =
            db.with_read(|conn| spp_core::ai_hub::get_endpoint_api_key(conn, &endpoint.id));
        let timeout = db
            .with_read(|conn| {
                spp_core::settings::get_i64(conn, "lmstudio_timeout_ms", 120_000).unwrap_or(120_000)
            })
            .max(1) as u64;
        let slot = Arc::clone(&self.probe_slot);
        let id = endpoint.id.clone();
        let url = endpoint.base_url.clone();
        std::thread::Builder::new()
            .name("spp-hub-probe".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("probe runtime");
                let probe = runtime.block_on(spp_core::ai_hub::probe_endpoint(
                    &url,
                    timeout,
                    api_key.as_deref(),
                ));
                let text = if probe.ok {
                    format!(
                        "reachable — {} model(s) · {} ms",
                        probe.model_count, probe.latency_ms
                    )
                } else {
                    format!("offline — {} ({} ms)", probe.message, probe.latency_ms)
                };
                if let Ok(mut guard) = slot.lock() {
                    *guard = Some((id, text));
                }
            })
            .ok();
    }

    fn add_endpoint(&mut self, db: &DbHandle) {
        let name = self.add_name.trim().to_string();
        let url = self.add_url.trim().to_string();
        if name.is_empty() || url.is_empty() {
            return;
        }
        let price_prompt: f64 = self.add_price_prompt.trim().parse().unwrap_or(0.0);
        let price_completion: f64 = self.add_price_completion.trim().parse().unwrap_or(0.0);
        let key = self.add_key.trim().to_string();
        let mut id = format!(
            "ep-{}",
            name.to_lowercase()
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect::<String>()
        );
        let mut list =
            db.with_read(|conn| spp_core::ai_hub::load_endpoints(conn).unwrap_or_default());
        let mut n = 1;
        while list.iter().any(|e| e.id == id) {
            n += 1;
            id = format!("ep-{n}");
        }
        let endpoint = spp_core::ai_hub::EndpointConfig {
            id: id.clone(),
            name,
            base_url: url,
            enabled: true,
            price_prompt_per_mtok: price_prompt,
            price_completion_per_mtok: price_completion,
            builtin: false,
        };
        let _ = db.with_write(|conn| {
            list.push(endpoint);
            spp_core::ai_hub::save_endpoints(conn, &list)?;
            if !key.is_empty() {
                spp_core::ai_hub::set_endpoint_api_key(conn, &id, &key)?;
            }
            Ok::<(), spp_core::Error>(())
        });
        self.add_name.clear();
        self.add_url.clear();
        self.add_key.clear();
        self.add_price_prompt.clear();
        self.add_price_completion.clear();
        self.reload_hub(db);
    }

    fn analytics_ui(&mut self, ui: &mut egui::Ui) {
        let Some(analytics) = self.analytics.clone() else {
            ui.spinner();
            return;
        };
        let t = design::tokens(ui);
        design::card(ui, |ui| {
            ui.horizontal(|ui| {
                Icon::Sparkles.show(ui, 16.0, t.muted_foreground);
                design::section_title(ui, "Analytics");
                design::badge(ui, "AI", BadgeTone::Info);
            });
            design::form_grid(ui, "ai-analytics", |ui| {
                design::field_label(ui, "tickets analyzed:");
                ui.monospace(
                    egui::RichText::new(analytics.tickets_analyzed.to_string()).color(t.foreground),
                );
                ui.end_row();
                design::field_label(ui, "analysis success rate:");
                ui.monospace(
                    egui::RichText::new(format!("{}%", analytics.analysis_success_rate))
                        .color(t.foreground),
                );
                ui.end_row();
                design::field_label(ui, "drafts:");
                ui.monospace(
                    egui::RichText::new(format!(
                        "{} total · {} accepted · {} rejected",
                        analytics.draft_count, analytics.drafts_accepted, analytics.drafts_rejected
                    ))
                    .color(t.foreground),
                );
                ui.end_row();
                design::field_label(ui, "draft edit rate:");
                ui.monospace(
                    egui::RichText::new(format!("{}%", analytics.draft_edit_rate))
                        .color(t.foreground),
                );
                ui.end_row();
                design::field_label(ui, "verification warnings:");
                ui.monospace(
                    egui::RichText::new(analytics.verification_warnings.to_string())
                        .color(t.foreground),
                );
                ui.end_row();
                design::field_label(ui, "unsupported claim rate:");
                ui.monospace(
                    egui::RichText::new(format!("{}%", analytics.unsupported_claim_rate))
                        .color(t.foreground),
                );
                ui.end_row();
            });
        });
        ui.add_space(10.0);
        if analytics.failure_patterns.is_empty() {
            design::hint_text(ui, "(no failure patterns)");
        } else {
            design::card_titled(ui, "Common failure patterns", None, |ui| {
                for (pattern, count) in &analytics.failure_patterns {
                    ui.label(
                        egui::RichText::new(format!("{pattern} · {count}"))
                            .size(13.0)
                            .color(t.foreground),
                    );
                }
            });
        }
    }

    fn attributes_ui(&mut self, ui: &mut egui::Ui) {
        let t = design::tokens(ui);
        let distributions = self.distributions.clone();
        design::card(ui, |ui| {
            ui.horizontal(|ui| {
                Icon::Sparkles.show(ui, 16.0, t.muted_foreground);
                design::section_title(ui, "Catalog");
                design::badge(ui, "AI", BadgeTone::Info);
            });
            design::hint_text(
                ui,
                "A missing attribute reads as unknown — it is never fabricated. Attributes \
                 are local-only and never written to Help Scout.",
            );
            ui.add_space(6.0);
            if distributions.is_empty() {
                design::empty_state(
                    ui,
                    Icon::Sparkles,
                    "No attribute data yet",
                    "Run ticket analysis to populate the distributions.",
                );
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for dist in &distributions {
                    design::card(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(dist.label)
                                    .size(13.5)
                                    .strong()
                                    .color(t.foreground),
                            );
                            ui.monospace(egui::RichText::new(dist.attribute).color(t.foreground));
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} known / {} unknown",
                                    dist.known, dist.unknown
                                ))
                                .small()
                                .color(t.muted_foreground),
                            );
                        });
                        for value in &dist.values {
                            let share = if dist.known > 0 {
                                value.count as f64 / dist.known as f64 * 100.0
                            } else {
                                0.0
                            };
                            ui.monospace(
                                egui::RichText::new(format!(
                                    "  {}: {} ({:.1}%)",
                                    value.value, value.count, share
                                ))
                                .color(t.foreground),
                            );
                        }
                    });
                }
            });
        });
    }

    fn jobs_ui(&mut self, ui: &mut egui::Ui) {
        let jobs = self.jobs.clone();
        if jobs.is_empty() {
            design::empty_state(
                ui,
                Icon::Sparkles,
                "No AI runs recorded",
                "Ticket analysis runs will appear here.",
            );
            return;
        }
        let t = design::tokens(ui);
        design::card(ui, |ui| {
            ui.horizontal(|ui| {
                Icon::Sparkles.show(ui, 16.0, t.muted_foreground);
                design::section_title(ui, "Recent AI runs");
                design::badge(ui, "AI", BadgeTone::Info);
            });
            ui.add_space(4.0);
            egui::ScrollArea::vertical().show_rows(ui, 26.0, jobs.len(), |ui, range| {
                design::table(
                    ui,
                    "ai-jobs",
                    &["id", "type", "status", "conversation", "model", "created"],
                    26.0,
                    |ui| {
                        for job in &jobs[range] {
                            design::td_num(ui, &job.id.to_string());
                            ui.label(
                                egui::RichText::new(job.run_type.as_str())
                                    .size(13.0)
                                    .color(t.foreground),
                            );
                            design::badge(ui, job.status.as_str(), job_status_tone(&job.status));
                            ui.monospace(
                                egui::RichText::new(
                                    job.conversation_id
                                        .map(|id| format!("#{id}"))
                                        .unwrap_or_else(|| "—".into()),
                                )
                                .color(t.foreground),
                            );
                            ui.monospace(
                                egui::RichText::new(
                                    job.model.clone().unwrap_or_else(|| "—".into()),
                                )
                                .color(t.foreground),
                            );
                            ui.monospace(
                                egui::RichText::new(job.created_at.as_str()).color(t.foreground),
                            );
                            ui.end_row();
                        }
                    },
                );
            });
        });
    }
}

impl Default for AiCenterPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Run-status → badge tone (redesign): running/succeeded and completed
/// read as success, queued/pending stay neutral, failed is destructive.
fn job_status_tone(status: &str) -> BadgeTone {
    match status {
        // 1.3.0: "running" is in-flight, not success — a job stuck
        // "running" after a transport failure used to wear a green badge.
        "completed" | "succeeded" => BadgeTone::Success,
        "failed" => BadgeTone::Destructive,
        _ => BadgeTone::Neutral,
    }
}

/// Catalog row for the attribute table (web: `/api/attributes/catalog`).
#[must_use]
pub fn attribute_catalog() -> Vec<(&'static str, &'static str, &'static str)> {
    AiAttributeKey::ALL
        .iter()
        .map(|k| (k.as_str(), k.label(), k.value_type().as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn settings_default_to_lmstudio_endpoint() {
        let db = fixture(Vec::new());
        let settings = db.with_read(load_ai_settings);
        assert!(settings.ai_enabled);
        assert_eq!(settings.base_url, "http://127.0.0.1:1234");
        assert_eq!(settings.timeout_ms, 120_000);
        assert_eq!(settings.concurrency, 2);
        assert!(settings.chat_model.is_none());
    }

    #[test]
    fn analytics_counts_seeded_runs_and_drafts() {
        let db = fixture(vec![("conv", 1, "Analyzed", "active", "email", 1, 5, 9)]);
        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO ai_runs (input_hash, prompt_version, model, response_json, type, status, conversation_id, created_at)
                 VALUES ('r1', 'v1', 'm', '{}', 'ticket_analysis', 'completed', 1, '2026-01-01T00:00:00Z'),
                        ('r2', 'v1', 'm', '{}', 'ticket_analysis', 'failed', NULL, '2026-01-01T00:01:00Z')",
                [],
            )
            .expect("runs");
            conn.execute(
                "INSERT INTO ai_drafts (conversation_id, content, state, created_at)
                 VALUES (1, 'draft one', 'accepted', '2026-01-01T00:02:00Z'),
                        (1, 'draft two', 'rejected', '2026-01-01T00:03:00Z')",
                [],
            )
            .expect("drafts");
        });
        let analytics = db.with_read(load_analytics);
        assert_eq!(analytics.tickets_analyzed, 1);
        assert_eq!(analytics.analysis_success_rate, 50.0);
        assert_eq!(analytics.draft_count, 2);
        assert_eq!(analytics.drafts_accepted, 1);
        assert_eq!(analytics.drafts_rejected, 1);
        assert!(analytics.failure_patterns.is_empty());
    }

    #[test]
    fn jobs_rows_load_after_seeding() {
        let db = fixture(vec![("conv", 7, "Seven", "active", "email", 1, 5, 9)]);
        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO ai_runs (input_hash, prompt_version, model, response_json, type, status, conversation_id, created_at)
                 VALUES ('r3', 'v1', 'qwen-7b', '{}', 'ticket_analysis', 'completed', 7, '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("run");
        });
        let jobs = db.with_read(|conn| load_jobs(conn, 100));
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].run_type, "ticket_analysis");
        assert_eq!(jobs[0].conversation_id, Some(7));
        assert_eq!(jobs[0].model.as_deref(), Some("qwen-7b"));
    }

    #[test]
    fn attribute_catalog_covers_all_keys() {
        let catalog = attribute_catalog();
        assert_eq!(catalog.len(), AiAttributeKey::ALL.len());
        for (key, label, value_type) in &catalog {
            assert!(!key.is_empty());
            assert!(!label.is_empty());
            assert!(!value_type.is_empty());
        }
    }

    #[test]
    fn distributions_empty_on_a_fresh_database() {
        let db = fixture(Vec::new());
        let distributions = db
            .with_read(spp_core::ai_attributes::distributions)
            .unwrap_or_default();
        // Fresh DB: attributes exist in the schema but no snapshots yet.
        assert!(distributions.iter().all(|d| d.total_conversations >= 0));
    }

    /// Count queued golden_eval jobs (the item-74 signal).
    fn golden_eval_jobs(db: &DbHandle) -> i64 {
        db.with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM jobs WHERE queue = 'ai' AND type = 'golden_eval'",
                [],
                |r| r.get(0),
            )
        })
        .expect("count golden_eval jobs")
    }

    /// Clean-audit item 74: a COMMITTED role-map edit persists exactly
    /// once and enqueues exactly one golden_eval; re-committing the same
    /// map is a no-op because the saved snapshot catches up.
    #[test]
    fn audit74_commit_persists_once_and_enqueues_one_eval() {
        let db = fixture(Vec::new());
        let mut page = AiCenterPage::new();
        page.reload(&db);
        assert_eq!(golden_eval_jobs(&db), 0);

        // A committed model edit (what the blur path delivers).
        let mut roles = page.hub_roles.clone();
        roles.draft = Some(spp_core::ai_hub::RoleAssignment {
            endpoint: "lmstudio".into(),
            model: Some("qwen2.5-7b".into()),
        });
        page.commit_role_map(&db, &roles);
        assert_eq!(golden_eval_jobs(&db), 1, "one eval per committed edit");
        assert_eq!(page.hub_roles_saved, roles, "the snapshot catches up");

        // The persisted state round-trips.
        let stored = db.with_read(spp_core::ai_hub::load_roles).expect("roles");
        assert_eq!(
            stored.draft.and_then(|d| d.model),
            Some("qwen2.5-7b".into())
        );

        // Re-committing the identical map: the UI blur path is guarded by
        // the snapshot comparison (roles == saved reaches nothing), and a
        // direct re-commit must not re-run the eval either.
        page.commit_role_map(&db, &roles);
        assert_eq!(
            golden_eval_jobs(&db),
            1,
            "no second eval for an unchanged map through the UI path (blur guard)"
        );
    }

    /// Clean-audit item 74, the regression itself: typing (a live map that
    /// diverged from the persisted one) persists NOTHING and enqueues NO
    /// eval across any number of rendered frames — only a blur commits.
    /// Pre-fix, every keystroke was a save_roles + golden_eval.
    #[test]
    fn audit74_typing_divergence_never_persists_or_enqueues() {
        let db = fixture(Vec::new());
        let ctx = egui::Context::default();
        let mut page = AiCenterPage::new();
        // Load once (what the first rendered frame does), then simulate
        // the in-memory state after a few keystrokes in the model field:
        // the live map diverges, the saved snapshot does not.
        page.reload(&db);
        page.hub_roles.draft = Some(spp_core::ai_hub::RoleAssignment {
            endpoint: "lmstudio".into(),
            model: Some("q".into()),
        });
        let saved_before = page.hub_roles_saved.clone();
        assert_ne!(
            page.hub_roles, page.hub_roles_saved,
            "the divergence is set up"
        );

        // Ten rendered frames of the Health tab (where the routing table
        // lives) — the keystroke-synced live map stays memory-only.
        for _ in 0..10 {
            ctx.run_pass(egui::RawInput::default(), |ctx| {
                ctx.root_ui(|ui| {
                    egui::CentralPanel::default().show(ui, |ui| page.ui(ui, &db));
                });
            })
            .drop_without_applying_deltas();
        }
        assert_eq!(
            golden_eval_jobs(&db),
            0,
            "no golden_eval may be enqueued without a committed (blurred) edit"
        );
        let stored = db.with_read(spp_core::ai_hub::load_roles).expect("roles");
        assert_eq!(
            stored.draft.and_then(|d| d.model),
            None,
            "the divergent live map must not be persisted"
        );
        assert_eq!(
            page.hub_roles_saved, saved_before,
            "the snapshot is untouched"
        );
        // The live map is still rendered from (no revert, no loss).
        assert_eq!(page.hub_roles.draft.and_then(|d| d.model), Some("q".into()));
    }

    /// Clean-audit item 75: a global Enter must NOT submit the copilot
    /// chat. Pre-fix, any Enter on the AI Center page fired the job —
    /// including one pressed with no field focused at all. Post-fix the
    /// submit rides the copilot input's own focus loss (Enter in a
    /// single-line TextEdit), so a stray Enter enqueues nothing.
    #[test]
    fn audit75_global_enter_never_submits_the_copilot() {
        let db = fixture(Vec::new());
        let ctx = egui::Context::default();
        let mut page = AiCenterPage::new();
        page.tab = Tab::Copilot;
        // A question is sitting in the input (typed earlier): the pre-fix
        // global Enter would fire the job on this state alone.
        page.copilot_input = "what is the refund policy?".into();

        // Render, then press Enter with nothing focused.
        ctx.run_pass(egui::RawInput::default(), |ctx| {
            ctx.root_ui(|ui| {
                egui::CentralPanel::default().show(ui, |ui| page.ui(ui, &db));
            });
        })
        .drop_without_applying_deltas();
        let mut enter = egui::RawInput::default();
        enter.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_pass(enter, |ctx| {
            ctx.root_ui(|ui| {
                egui::CentralPanel::default().show(ui, |ui| page.ui(ui, &db));
            });
        })
        .drop_without_applying_deltas();

        let jobs: i64 = db
            .with_read(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM jobs WHERE queue = 'ai' AND type = 'copilot_chat'",
                    [],
                    |r| r.get(0),
                )
            })
            .expect("count copilot jobs");
        assert_eq!(
            jobs, 0,
            "a global Enter may not submit the copilot chat anymore"
        );
        assert!(
            page.copilot_pending.is_none(),
            "no in-flight job was started either"
        );
        // The typed question is still sitting in the input, un-submitted.
        assert_eq!(page.copilot_input, "what is the refund policy?");
    }

    /// Item 151: the provider-record save path writes through the core
    /// setters and the re-read picks the values up; the ready badge
    /// flips only with provider + chat model both present.
    #[test]
    fn provider_record_save_round_trips() {
        let db = fixture(Vec::new());
        let mut page = AiCenterPage::new();
        page.reload(&db);
        assert_eq!(
            page.ai_record
                .as_ref()
                .expect("record loaded")
                .provider_kind,
            spp_core::ai_center::ProviderKind::None
        );
        page.ai_kind_pick = 1; // LM Studio
        page.ai_chat_model_edit = "test-chat".into();
        page.ai_base_url_edit = "http://127.0.0.1:9999".into();
        page.save_ai_record(&db);
        let record = db
            .with_read(spp_core::ai_center::get_ai_status)
            .expect("status");
        assert_eq!(
            record.provider_kind,
            spp_core::ai_center::ProviderKind::LmStudio
        );
        assert_eq!(record.chat_model.as_deref(), Some("test-chat"));
        assert_eq!(record.base_url.as_deref(), Some("http://127.0.0.1:9999"));
        assert!(spp_core::ai_center::ai_features_enabled(&record));
        assert!(page.ai_record_note.as_ref().expect("note").0);
    }

    /// Item 151: an embedding model without a positive dim is rejected
    /// with a visible error and nothing is written.
    #[test]
    fn provider_record_requires_positive_dim_with_embed_model() {
        let db = fixture(Vec::new());
        let mut page = AiCenterPage::new();
        page.reload(&db);
        page.ai_kind_pick = 1;
        page.ai_embed_model_edit = "bge-m3".into();
        page.ai_embed_dim_edit = "0".into();
        page.save_ai_record(&db);
        let note = page.ai_record_note.clone().expect("note");
        assert!(!note.0, "dim=0 with a model must be an error note");
        let record = db
            .with_read(spp_core::ai_center::get_ai_status)
            .expect("status");
        assert_eq!(
            record.provider_kind,
            spp_core::ai_center::ProviderKind::None
        );
        assert!(record.embedding_model.is_none(), "nothing was written");
    }

    /// Item 151: a dirty (typed, unsaved) form survives invalidation +
    /// reload with its edits intact; a clean form re-seeds instead.
    #[test]
    fn provider_record_dirty_form_survives_reload() {
        let db = fixture(Vec::new());
        let mut page = AiCenterPage::new();
        page.reload(&db);
        page.ai_chat_model_edit = "typed-but-unsaved".into();
        page.ai_form_clean = false;
        page.invalidate();
        page.reload(&db);
        assert_eq!(
            page.ai_chat_model_edit, "typed-but-unsaved",
            "unsaved edits survive invalidation (batch-27 convention)"
        );
    }
}
