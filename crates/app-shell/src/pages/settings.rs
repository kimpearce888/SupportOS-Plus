//! Settings page (port of the web `pages/settings.rs`).
//!
//! Tabs (web: `SettingsTab`): Synchronization & AI (the
//! `application_settings` keys behind `/api/settings`), Help Scout
//! connection (OAuth status/disconnect/connect — the loopback listener
//! serves the callback), LM Studio (the `/api/settings/lmstudio` keys),
//! Vector index (the sqlite-vec store, decision D4), Business hours (the
//! `mailbox_business_hours` table behind `/api/settings/business-hours`),
//! and Backups & export (`backup_service::{backup,list_backups,restore,
//! export_json,export_conversations_csv}` — the `/api/backups/*` routes).

use std::path::PathBuf;

use eframe::egui;
use spp_core::db::DbHandle;

use super::{i64_field, str_field};
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

/// Page tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    General,
    HelpScout,
    LmStudio,
    VectorIndex,
    Hours,
    Backups,
}

impl Tab {
    const ALL: [Self; 6] = [
        Self::General,
        Self::HelpScout,
        Self::LmStudio,
        Self::VectorIndex,
        Self::Hours,
        Self::Backups,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::General => "Sync & AI",
            Self::HelpScout => "Help Scout",
            Self::LmStudio => "LM Studio",
            Self::VectorIndex => "Vector index",
            Self::Hours => "Business hours",
            Self::Backups => "Backups & export",
        }
    }
}

/// General settings model (typed port of `/api/settings`).
#[derive(Debug, Clone, PartialEq)]
pub struct GeneralSettings {
    pub sync_interval_minutes: i64,
    pub api_concurrency: i64,
    pub ai_enabled: bool,
    pub automatic_analysis_enabled: bool,
    pub automatic_note_enabled: bool,
    pub automatic_draft_enabled: bool,
    pub automation_enabled: bool,
    pub automation_write_actions_enabled: bool,
    pub attachment_auto_download: bool,
    pub automatic_reply_sending: bool,
    /// v1.4.0 (R4, D9): the status a conversation moves to when a reply
    /// is sent (decision D-b). `pending` matches Help Scout semantics
    /// (awaiting the customer), `open` is the platform's own status for
    /// "await agent" (clean-audit item 25: the pre-fix dropdown offered
    /// `waiting`, which is in neither Help Scout's nor the loopback
    /// provider's status vocabulary — every send with it configured was
    /// rejected by the platform), `keep` leaves the status untouched
    /// (the pre-1.4.0 behavior).
    pub reply_status_after: String,
    pub log_level: String,
    pub agent_language: String,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            sync_interval_minutes: 1,
            api_concurrency: 2,
            ai_enabled: true,
            automatic_analysis_enabled: true,
            automatic_note_enabled: false,
            automatic_draft_enabled: false,
            automation_enabled: false,
            automation_write_actions_enabled: false,
            attachment_auto_download: true,
            automatic_reply_sending: false,
            reply_status_after: "pending".into(),
            log_level: "info".into(),
            agent_language: "en".into(),
        }
    }
}

impl GeneralSettings {
    /// Load from the settings table (the route's defaults).
    #[must_use]
    pub fn load(conn: &rusqlite::Connection) -> Self {
        let defaults = Self::default();
        Self {
            sync_interval_minutes: spp_core::settings::get_i64(
                conn,
                "sync_interval_minutes",
                defaults.sync_interval_minutes,
            )
            .unwrap_or(defaults.sync_interval_minutes),
            api_concurrency: spp_core::settings::get_i64(
                conn,
                "api_concurrency",
                defaults.api_concurrency,
            )
            .unwrap_or(defaults.api_concurrency),
            ai_enabled: spp_core::settings::get_bool(conn, "ai_enabled", defaults.ai_enabled)
                .unwrap_or(defaults.ai_enabled),
            automatic_analysis_enabled: spp_core::settings::get_bool(
                conn,
                "automatic_analysis_enabled",
                defaults.automatic_analysis_enabled,
            )
            .unwrap_or(defaults.automatic_analysis_enabled),
            automatic_note_enabled: spp_core::settings::get_bool(
                conn,
                "automatic_note_enabled",
                defaults.automatic_note_enabled,
            )
            .unwrap_or(defaults.automatic_note_enabled),
            automatic_draft_enabled: spp_core::settings::get_bool(
                conn,
                "automatic_draft_enabled",
                defaults.automatic_draft_enabled,
            )
            .unwrap_or(defaults.automatic_draft_enabled),
            automation_enabled: spp_core::settings::get_bool(
                conn,
                "automation_enabled",
                defaults.automation_enabled,
            )
            .unwrap_or(defaults.automation_enabled),
            automation_write_actions_enabled: spp_core::settings::get_bool(
                conn,
                "automation_write_actions_enabled",
                defaults.automation_write_actions_enabled,
            )
            .unwrap_or(defaults.automation_write_actions_enabled),
            // `qdrant_enabled` is owned by the Vector-index tab (clean-audit
            // item 139) — the General form neither loads nor saves it.
            attachment_auto_download: spp_core::settings::get_bool(
                conn,
                "attachment_auto_download",
                defaults.attachment_auto_download,
            )
            .unwrap_or(defaults.attachment_auto_download),
            automatic_reply_sending: spp_core::settings::get_bool(
                conn,
                "automatic_reply_sending",
                defaults.automatic_reply_sending,
            )
            .unwrap_or(defaults.automatic_reply_sending),
            reply_status_after: spp_core::settings::get_string(conn, "reply_status_after")
                .ok()
                .flatten()
                .map(|v| match v.as_str() {
                    // Audit item 25: `waiting` was never a platform status —
                    // map the stored legacy value onto the real one so old
                    // installs stop sending it.
                    "waiting" => "open".to_string(),
                    "pending" | "open" | "keep" => v,
                    _ => defaults.reply_status_after.clone(),
                })
                .unwrap_or_else(|| defaults.reply_status_after.clone()),
            log_level: spp_core::settings::get_string(conn, "log_level")
                .ok()
                .flatten()
                .unwrap_or(defaults.log_level),
            agent_language: spp_core::settings::get_string(conn, "agent_language")
                .ok()
                .flatten()
                .unwrap_or(defaults.agent_language),
        }
    }

    /// Persist (the update route's writes).
    ///
    /// # Errors
    /// Propagates the settings-table write failure.
    pub fn save(&self, conn: &rusqlite::Connection) -> spp_core::Result<()> {
        spp_core::settings::set_i64(conn, "sync_interval_minutes", self.sync_interval_minutes)?;
        spp_core::settings::set_i64(conn, "api_concurrency", self.api_concurrency)?;
        spp_core::settings::set_bool(conn, "ai_enabled", self.ai_enabled)?;
        spp_core::settings::set_bool(
            conn,
            "automatic_analysis_enabled",
            self.automatic_analysis_enabled,
        )?;
        spp_core::settings::set_bool(conn, "automatic_note_enabled", self.automatic_note_enabled)?;
        spp_core::settings::set_bool(
            conn,
            "automatic_draft_enabled",
            self.automatic_draft_enabled,
        )?;
        spp_core::settings::set_bool(conn, "automation_enabled", self.automation_enabled)?;
        spp_core::settings::set_bool(
            conn,
            "automation_write_actions_enabled",
            self.automation_write_actions_enabled,
        )?;
        // `qdrant_enabled` is owned by the Vector-index tab (clean-audit
        // item 139: the setting had no control anywhere; the General
        // form's blind save would have clobbered the tab's toggle).
        spp_core::settings::set_bool(
            conn,
            "attachment_auto_download",
            self.attachment_auto_download,
        )?;
        spp_core::settings::set_bool(
            conn,
            "automatic_reply_sending",
            self.automatic_reply_sending,
        )?;
        spp_core::settings::set_string(conn, "reply_status_after", &self.reply_status_after)?;
        spp_core::settings::set_string(conn, "log_level", &self.log_level)?;
        spp_core::settings::set_string(conn, "agent_language", &self.agent_language)?;
        Ok(())
    }
}

/// Help Scout connection status (port of `/api/oauth/status` read side).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HelpScoutStatus {
    pub configured: bool,
    /// OAuth client credentials are available — from the environment or
    /// the in-app form — so the OAuth connect flow can start.
    pub credentials: bool,
    /// Per-field credential source for the form badges (1.1.3):
    /// environment, app settings, or not set.
    pub sources: spp_core::helpscout_real::CredentialSources,
    /// The redirect URI this instance expects (1.1.4): it must exactly
    /// match the callback URL registered on the Help Scout custom app,
    /// and the loopback listener binds its port — so the UI must show
    /// it next to the connect button.
    pub redirect_uri: String,
}

/// Load the connection status from the local mirror.
#[must_use]
pub fn helpscout_status(conn: &rusqlite::Connection) -> HelpScoutStatus {
    let tokens: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM oauth_tokens WHERE access_token IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let (credentials, sources) = spp_core::helpscout_real::HsCredentials::resolve(conn);
    let redirect_uri = credentials.redirect_uri.clone();
    HelpScoutStatus {
        configured: tokens > 0,
        credentials: credentials.is_configured(),
        sources,
        redirect_uri,
    }
}

/// Business-hours row (typed port of `/api/settings/business-hours`).
#[derive(Debug, Clone)]
pub struct BusinessHoursRow {
    pub mailbox_id: i64,
    pub name: String,
    pub configured: bool,
    pub timezone: String,
    pub days: String,
    pub start_minute: i64,
    pub end_minute: i64,
    pub first_response_target_min: i64,
    pub resolution_target_min: i64,
}

/// A stored business-hours config row:
/// `(timezone, days, start_minute, end_minute, first response target, resolution target)`.
type HoursConfigRow = (String, String, i64, i64, Option<i64>, Option<i64>);

/// Load business-hours rows for every mailbox.
#[must_use]
pub fn load_business_hours(conn: &rusqlite::Connection) -> Vec<BusinessHoursRow> {
    let mut rows: std::collections::HashMap<i64, HoursConfigRow> = std::collections::HashMap::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT mailbox_local_id, timezone, days, start_minute, end_minute,
                first_response_target_min, resolution_target_min
           FROM mailbox_business_hours",
    ) {
        let mapped: Vec<(i64, HoursConfigRow)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    (
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                        r.get::<_, Option<i64>>(6)?,
                    ),
                ))
            })
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default();
        for (id, cfg) in mapped {
            rows.insert(id, cfg);
        }
    }
    let mailboxes: Vec<(i64, String)> = conn
        .prepare("SELECT id, name FROM mailboxes ORDER BY name")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                .map(|mapped| mapped.flatten().collect())
        })
        .unwrap_or_default();
    mailboxes
        .into_iter()
        .map(|(id, name)| {
            let row = rows.get(&id);
            BusinessHoursRow {
                mailbox_id: id,
                name,
                configured: row.is_some(),
                timezone: row.map(|r| r.0.clone()).unwrap_or_else(|| "UTC".into()),
                days: row
                    .map(|r| r.1.clone())
                    .unwrap_or_else(|| "[1,2,3,4,5]".into()),
                start_minute: row.map_or(540, |r| r.2),
                end_minute: row.map_or(1020, |r| r.3),
                first_response_target_min: row.and_then(|r| r.4).unwrap_or(240),
                resolution_target_min: row.and_then(|r| r.5).unwrap_or(1440),
            }
        })
        .collect()
}

/// Parse the free-text `days` field ("1,2,3" or "1, 2, 3") into the
/// normalized JSON list the SLA layer reads. Every token must be a
/// weekday number (0=Sun…6=Sat), deduped and sorted, and at least one
/// day must remain (clean-audit item 141: the raw string used to save
/// verbatim, and anything it could not parse silently meant "the default
/// schedule" on the SLA side).
fn parse_weekdays(raw: &str) -> spp_core::Result<String> {
    // Accept both the editor's plain "1,2,3" and the JSON "[1,2,3]" the
    // form pre-fills from the stored row.
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(trimmed);
    let mut days: Vec<i64> = Vec::new();
    for token in inner.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let day: i64 = token.parse().map_err(|_| {
            spp_core::error::Error::Validation(format!(
                "invalid weekday {token:?} — use numbers 0=Sun…6=Sat separated by commas"
            ))
        })?;
        if !(0..=6).contains(&day) {
            return Err(spp_core::error::Error::Validation(format!(
                "weekday {day} is out of range — use 0=Sun…6=Sat"
            )));
        }
        if !days.contains(&day) {
            days.push(day);
        }
    }
    if days.is_empty() {
        return Err(spp_core::error::Error::Validation(
            "at least one weekday is required — use 0=Sun…6=Sat separated by commas".into(),
        ));
    }
    days.sort_unstable();
    serde_json::to_string(&days).map_err(|e| {
        spp_core::error::Error::Validation(format!("could not serialize the weekday list: {e}"))
    })
}

/// Save one mailbox's business hours (the set route's upsert).
///
/// # Errors
/// Propagates SQL failures, an invalid timezone, or an invalid weekday
/// list.
pub fn save_business_hours(
    conn: &rusqlite::Connection,
    row: &BusinessHoursRow,
) -> spp_core::Result<()> {
    if !spp_core::business_hours::is_valid_timezone(&row.timezone) {
        return Err(spp_core::error::Error::Validation(format!(
            "unknown timezone {}",
            row.timezone
        )));
    }
    if row.end_minute <= row.start_minute {
        return Err(spp_core::error::Error::Validation(
            "the end time must be after the start time".into(),
        ));
    }
    let days = parse_weekdays(&row.days)?;
    conn.execute(
        "INSERT INTO mailbox_business_hours (mailbox_local_id, timezone, days, start_minute,
                end_minute, first_response_target_min, resolution_target_min, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
         ON CONFLICT(mailbox_local_id) DO UPDATE SET
           timezone=excluded.timezone, days=excluded.days, start_minute=excluded.start_minute,
           end_minute=excluded.end_minute, first_response_target_min=excluded.first_response_target_min,
           resolution_target_min=excluded.resolution_target_min, updated_at=excluded.updated_at",
        rusqlite::params![
            row.mailbox_id,
            row.timezone,
            days,
            row.start_minute,
            row.end_minute,
            row.first_response_target_min,
            row.resolution_target_min
        ],
    )?;
    Ok(())
}

/// The interactive settings page state.
pub struct SettingsPage {
    tab: Tab,
    general: Option<GeneralSettings>,
    /// Clean-audit item 77: the last PERSISTED snapshot of the General
    /// form. The form is dirty when it differs from this snapshot — and
    /// a dirty form is user state: `reload` keeps it instead of
    /// re-reading persisted values over unsaved edits (a sync
    /// invalidation used to drop the form wholesale and the next
    /// render's reload silently reverted whatever the user was typing).
    general_persisted: Option<GeneralSettings>,
    /// Clean-audit item 77: an invalidation is pending its mirror
    /// refresh (the mirrors re-read on the next render without ever
    /// clobbering the form).
    stale: bool,
    helpscout: Option<HelpScoutStatus>,
    authorize_url: Option<String>,
    /// Help Scout credential form (1.1.3). The three secrets render as
    /// password fields; `hs_show_secrets` toggles the mask.
    hs_client_id: String,
    hs_client_secret: String,
    hs_webhook_secret: String,
    hs_docs_api_key: String,
    hs_form_loaded: bool,
    hs_show_secrets: bool,
    /// P4: the public relay URL the webhook health check re-registers to.
    webhook_public_url: String,
    /// Mirrors the relay URL field's one-time load (like `hs_form_loaded`).
    wh_loaded: bool,
    /// The last webhook health check outcome (P4), re-read on invalidation.
    wh_outcome: Option<spp_core::webhook_health::WebhookHealthOutcome>,
    /// Application data directory: the demo purge clears the vector
    /// store that lives next to the database.
    data_dir: Option<PathBuf>,
    /// Two-step confirmation state for the demo purge.
    purge_armed: bool,
    /// Two-step confirmation state for backup restores (audit item
    /// 11): holds the backup file path awaiting the second click.
    restore_armed: Option<String>,
    /// The swappable provider handle (v1.4.0 R3): credential saves and
    /// clears activate/deactivate the provider through it in-session.
    /// `None` in tests that exercise the page without boot services.
    provider: Option<std::sync::Arc<spp_core::provider_handle::ProviderHandle>>,
    /// The shared sync engine — after an in-session activation the
    /// initial sync is kicked directly through it (no restart, no
    /// waiting for the auto-sync timer).
    engine: Option<std::sync::Arc<spp_core::sync_engine::SyncEngine>>,
    /// Event bus handle for connect-failure events (R5/D4).
    bus: Option<spp_core::eventing::EventBus>,
    /// In-flight client-credentials connect (1.1.4): the detached
    /// thread writes its outcome here; the UI polls it each frame.
    hs_connect_result: std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>,
    /// Mirrors `hs_connect_result` being in flight (UI-thread state).
    hs_connecting: bool,
    /// In-flight Connection Doctor run (R1): the detached thread writes
    /// the five steps here; the UI polls every frame while visible.
    doctor_result:
        std::sync::Arc<std::sync::Mutex<Option<Vec<spp_core::connection_doctor::DoctorStep>>>>,
    /// Mirrors `doctor_result` being in flight (UI-thread state).
    doctor_running: bool,
    /// The latest completed doctor report, kept until the next run.
    doctor_steps: Vec<spp_core::connection_doctor::DoctorStep>,
    lmstudio_base_url: String,
    lmstudio_chat_model: String,
    lmstudio_embedding_model: String,
    /// Cached sqlite-vec store path shown on the Vector index tab.
    vector_store_path: String,
    hours: Vec<BusinessHoursRow>,
    hours_edit: Option<BusinessHoursRow>,
    backups: Vec<serde_json::Value>,
    backups_dir: Option<PathBuf>,
    /// Background worker host: restarts the timers when the sync
    /// interval changes (the old settings-route behavior). Absent in
    /// tests, where background timers must not run.
    workers: Option<std::sync::Arc<crate::workers_host::WorkersHost>>,
    /// R7 version transparency: the detached release-check thread writes
    /// its outcome here; the UI polls each frame. Strictly user-initiated
    /// — the app never phones home on its own.
    update_result: std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>,
    /// Mirrors `update_result` being in flight (UI-thread state).
    update_running: bool,
    message: Option<String>,
    error: Option<String>,
}

impl SettingsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::General,
            general: None,
            general_persisted: None,
            stale: false,
            helpscout: None,
            authorize_url: None,
            hs_client_id: String::new(),
            hs_client_secret: String::new(),
            hs_webhook_secret: String::new(),
            hs_docs_api_key: String::new(),
            hs_form_loaded: false,
            hs_show_secrets: false,
            webhook_public_url: String::new(),
            wh_loaded: false,
            wh_outcome: None,
            data_dir: None,
            purge_armed: false,
            restore_armed: None,
            provider: None,
            engine: None,
            bus: None,
            hs_connect_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            hs_connecting: false,
            doctor_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            doctor_running: false,
            doctor_steps: Vec::new(),
            lmstudio_base_url: String::new(),
            lmstudio_chat_model: String::new(),
            lmstudio_embedding_model: String::new(),
            vector_store_path: String::new(),
            hours: Vec::new(),
            hours_edit: None,
            backups: Vec::new(),
            backups_dir: None,
            workers: None,
            update_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            update_running: false,
            message: None,
            error: None,
        }
    }

    /// Application data directory (set by the shell at boot; the demo
    /// purge needs it to clear the vector store).
    pub fn set_data_dir(&mut self, dir: PathBuf) {
        self.data_dir = Some(dir);
    }

    /// Backups directory (set by the shell at boot).
    pub fn set_backups_dir(&mut self, dir: PathBuf) {
        self.backups_dir = Some(dir);
    }

    /// Install the background worker host (restart-on-interval-change).
    pub fn set_workers(
        &mut self,
        workers: Option<std::sync::Arc<crate::workers_host::WorkersHost>>,
    ) {
        self.workers = workers;
    }

    /// Install the swappable provider handle (v1.4.0 R3, replaces the
    /// boot-time real-provider snapshot): saving credentials activates
    /// the provider through it immediately — the restart ritual is
    /// gone — and the Connection Doctor runs against whatever provider
    /// is currently live.
    pub fn set_provider_handle(
        &mut self,
        handle: std::sync::Arc<spp_core::provider_handle::ProviderHandle>,
    ) {
        self.provider = Some(handle);
    }

    /// Install the shared sync engine (the activation path kicks the
    /// initial sync through it).
    pub fn set_sync_engine(
        &mut self,
        engine: Option<std::sync::Arc<spp_core::sync_engine::SyncEngine>>,
    ) {
        self.engine = engine;
    }

    /// Install the event bus (connect failures emit live errors, R5/D4).
    pub fn set_bus(&mut self, bus: spp_core::eventing::EventBus) {
        self.bus = Some(bus);
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        // Clean-audit item 77: the pre-fix invalidation dropped the
        // General form (`general = None`), so the next render's reload
        // silently re-read the persisted values over unsaved edits — a
        // sync landing while the user typed lost their changes. The
        // form is user state now: invalidation only marks the page
        // stale; the mirrors (connection status, business hours,
        // backups) re-read on the next render, and the dirty guard in
        // `reload` keeps the form.
        self.stale = true;
        self.helpscout = None;
        self.hours.clear();
        self.backups.clear();
    }

    fn reload(&mut self, db: &DbHandle) {
        let (general, helpscout, hours) = db.with_read(|conn| {
            (
                GeneralSettings::load(conn),
                helpscout_status(conn),
                load_business_hours(conn),
            )
        });
        // Clean-audit item 77: a dirty General form (one that differs
        // from the last persisted snapshot) is user state — keep it and
        // its snapshot; only a clean or absent form takes the fresh
        // read. Every mirror below refreshes unconditionally.
        let dirty = self
            .general
            .as_ref()
            .is_some_and(|g| self.general_persisted.as_ref() != Some(g));
        if !dirty {
            self.general = Some(general.clone());
            self.general_persisted = Some(general);
        }
        self.helpscout = Some(helpscout);
        self.hours = hours;
        // Prefill the credential form once from the envelope (like the
        // LM Studio fields); after that the fields belong to the user.
        if !self.hs_form_loaded {
            let stored = db.with_read(spp_core::helpscout_real::HsCredentials::from_envelope);
            self.hs_client_id = stored.client_id;
            self.hs_client_secret = stored.client_secret;
            self.hs_webhook_secret = stored.webhook_secret;
            self.hs_docs_api_key = stored.docs_api_key;
            self.hs_form_loaded = true;
        }
        // P4: the relay URL form + the last health outcome.
        if !self.wh_loaded {
            self.webhook_public_url = db.with_read(|conn| {
                spp_core::settings::get_string(conn, "webhook_public_url")
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            });
            self.wh_loaded = true;
        }
        self.wh_outcome = db
            .with_read(spp_core::webhook_health::load_outcome)
            .ok()
            .flatten();
        if self.lmstudio_base_url.is_empty() {
            let (base, chat, embed) = db.with_read(|conn| {
                (
                    spp_core::settings::get_string(conn, "lmstudio_base_url")
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    spp_core::settings::get_string(conn, "lmstudio_chat_model")
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    spp_core::settings::get_string(conn, "lmstudio_embedding_model")
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                )
            });
            self.lmstudio_base_url = base;
            self.lmstudio_chat_model = chat;
            self.lmstudio_embedding_model = embed;
        }
        if self.tab == Tab::Backups {
            self.reload_backups();
        }
    }

    fn reload_backups(&mut self) {
        if let Some(dir) = &self.backups_dir {
            self.backups = spp_core::backup_service::list_backups(dir);
        }
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        // Clean-audit item 77: a pending invalidation refreshes the
        // mirrors too, not just a first load (the form survives either
        // way — see the dirty guard in `reload`).
        if self.general.is_none() || self.stale {
            self.reload(db);
            self.stale = false;
        }
    }

    fn save_general(&mut self, db: &DbHandle) {
        let Some(general) = self.general.clone() else {
            return;
        };
        // The workers arm the auto-sync timer from this setting at start;
        // capture the pre-save value so a change can restart the timers
        // (mirrors the old PUT /api/settings stop/start behavior).
        let interval_before = db.with_read(|conn| {
            spp_core::settings::get_i64(conn, "sync_interval_minutes", 1).unwrap_or(1)
        });
        // Audit item 31: `api_concurrency` was parsed and saved but never
        // applied — capture the pre-save value so a change reaches the
        // live provider's queues in-session instead of waiting for a
        // restart that never mattered.
        let concurrency_before = db
            .with_read(|conn| spp_core::settings::get_i64(conn, "api_concurrency", 2).unwrap_or(2));
        // Clean-audit item 138: same for `log_level` — the saved row
        // reaches the live tracing filter instead of sitting unread.
        let level_before = db
            .with_read(|conn| {
                spp_core::settings::get_string(conn, "log_level")
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            })
            .trim()
            .to_ascii_lowercase();
        let result = db.with_write(|conn| general.save(conn));
        match result {
            Ok(()) => {
                // Clean-audit item 77: the saved form is the new
                // persisted snapshot — the form reads clean from here
                // on (until edited again), so a later invalidation or
                // reload may take fresh reads without losing anything.
                self.general_persisted = Some(general.clone());
                if interval_before != general.sync_interval_minutes
                    && let Some(workers) = self.workers.clone()
                {
                    tracing::info!(
                        before = interval_before,
                        after = general.sync_interval_minutes,
                        "sync interval changed: restarting background workers"
                    );
                    workers.restart();
                }
                if concurrency_before != general.api_concurrency
                    && let Some(handle) = self.provider.clone()
                    && let Some(real) = handle.real()
                {
                    real.set_api_concurrency(general.api_concurrency.max(1) as usize);
                    tracing::info!(
                        before = concurrency_before,
                        after = general.api_concurrency,
                        current = real.api_concurrency(),
                        "api concurrency changed: applied to the live provider queues"
                    );
                }
                if level_before != general.log_level.to_ascii_lowercase() {
                    spp_core::logging::apply_log_level(&general.log_level);
                    tracing::info!(
                        level = %general.log_level,
                        "log level changed: applied to the live subscriber"
                    );
                }
                self.message = Some("settings saved".into())
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn save_lmstudio(&mut self, db: &DbHandle) {
        let (base, chat, embed) = (
            self.lmstudio_base_url.trim().to_string(),
            self.lmstudio_chat_model.trim().to_string(),
            self.lmstudio_embedding_model.trim().to_string(),
        );
        let result = db.with_write(|conn| {
            spp_core::settings::set_string(conn, "lmstudio_base_url", &base)?;
            spp_core::settings::set_string(conn, "lmstudio_chat_model", &chat)?;
            spp_core::settings::set_string(conn, "lmstudio_embedding_model", &embed)?;
            spp_core::error::Result::Ok(())
        });
        match result {
            Ok(()) => self.message = Some("LM Studio settings saved".into()),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn disconnect_helpscout(&mut self, db: &DbHandle) {
        // The route revoked remotely first; the local effect is dropping the
        // stored tokens. 1.4.0 (R7/D13): the tokens live in the encrypted
        // envelope, so both the envelope entries and the metadata row go.
        db.with_write(|conn| {
            let _ = spp_core::settings::delete_secret(
                conn,
                spp_core::helpscout_real::SECRET_OAUTH_ACCESS,
            );
            let _ = spp_core::settings::delete_secret(
                conn,
                spp_core::helpscout_real::SECRET_OAUTH_REFRESH,
            );
            let _ = conn.execute("DELETE FROM oauth_tokens", []);
        });
        self.message = Some("Disconnected. Local data is fully preserved.".into());
        self.reload(db);
    }

    /// Save the credential form (1.1.3): non-empty fields are stored in
    /// the encrypted envelope. v1.4.0 (R3, D2): the saved credentials
    /// activate the provider **immediately, in-session** — no restart —
    /// the Connection Doctor runs against the fresh connection, and the
    /// first sync starts right away (initial when the state machine was
    /// reset by an empty session, R2).
    fn save_helpscout_credentials(&mut self, db: &DbHandle) {
        let (cid, cs, ws, dk) = (
            self.hs_client_id.trim().to_string(),
            self.hs_client_secret.trim().to_string(),
            self.hs_webhook_secret.trim().to_string(),
            self.hs_docs_api_key.trim().to_string(),
        );
        let result = db.with_write(|conn| {
            spp_core::helpscout_real::HsCredentials::store_in_envelope(conn, &cid, &cs, &ws, &dk)?;
            spp_core::error::Result::Ok(())
        });
        match result {
            Ok(()) => {
                // Audit item 32: the platform caps webhook secrets at 40
                // characters — registration and signature verification both
                // use the truncation, so say it out loud when the form
                // stored more than the platform will keep.
                let secret_note = if ws.chars().count() > spp_core::webhook::SECRET_CAP {
                    format!(
                        " (note: the platform caps webhook secrets at {} characters — \
                         only the first {} of yours are used)",
                        spp_core::webhook::SECRET_CAP,
                        spp_core::webhook::SECRET_CAP
                    )
                } else {
                    String::new()
                };
                self.message = Some(format!(
                    "credentials stored (encrypted) — activating the connection now…{secret_note}"
                ));
                // The three secret fields do not linger on screen after a
                // successful save; the client id stays for reference.
                self.hs_client_secret.clear();
                self.hs_webhook_secret.clear();
                self.hs_docs_api_key.clear();
                self.activate_saved_credentials(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.reload(db);
    }

    /// v1.4.0 (R3): rebuild the real provider from the resolved
    /// credentials and swap it into the shared handle; apply the R2
    /// transition policy; run the Connection Doctor; kick the sync.
    /// Called after every credential save. When the resolved
    /// credentials are incomplete (the form left both id and secret
    /// blank on a clean install), nothing activates — the stored
    /// envelope is unchanged.
    fn activate_saved_credentials(&mut self, db: &DbHandle) {
        let Some(handle) = self.provider.clone() else {
            // Bare pages (unit tests) carry no handle; the message the
            // caller set already tells the truth about the stored state,
            // and the next real boot activates the credentials.
            return;
        };
        let (credentials, _) = db.with_read(spp_core::helpscout_real::HsCredentials::resolve);
        if !credentials.is_configured() {
            return;
        }
        let previous_kind = handle.kind();
        let real = std::sync::Arc::new(spp_core::helpscout_real::RealHelpScoutProvider::new(
            db.clone(),
            credentials,
        ));
        handle.activate_real(real);
        db.with_write(|conn| {
            spp_core::sync_engine::on_provider_kind_transition(
                conn,
                Some(previous_kind.as_str()),
                "real",
            );
            let _ = spp_core::settings::set_string(conn, "provider_kind", "real");
            let _ = spp_core::audit::log_event(
                conn,
                "info",
                "connect",
                "Help Scout credentials activated in-session (no restart)",
                None,
                None,
            );
        });
        // Side effects (doctor + sync kick) run when the page is wired
        // into the live shell (a bus is installed); bare test pages skip
        // them so no network is ever touched from a unit test.
        if self.bus.is_some() {
            self.message = Some(
                "credentials active — running the Connection Doctor and starting the sync…".into(),
            );
            self.run_connection_doctor();
            self.kick_sync_after_activation(db);
        }
    }

    /// Kick the first sync after an in-session activation (R3): the
    /// engine decides initial vs incremental from the state machine
    /// (R2 resets it to NEW when the previous session was poisoned),
    /// and the "already running" error is the healthy overlap case.
    fn kick_sync_after_activation(&mut self, db: &DbHandle) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let spawn_db = db.clone();
        let spawned = std::thread::Builder::new()
            .name("spp-activation-sync".to_string())
            .spawn(move || {
                let state = spawn_db.with_read(spp_core::sync_engine::get_state);
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        tracing::warn!(error = %e, "cannot start the activation sync runtime");
                        return;
                    }
                };
                let result = if state == "NEW" {
                    tracing::info!(state, "activation: starting the initial sync");
                    runtime.block_on(engine.initial_sync(spp_core::events::SyncTrigger::Manual))
                } else {
                    tracing::info!(state, "activation: starting an incremental sync");
                    runtime.block_on(engine.incremental_sync(spp_core::events::SyncTrigger::Manual))
                };
                if let Err(e) = result {
                    let msg = e.to_string();
                    if !msg.contains("already running") {
                        tracing::warn!(error = %msg, "activation sync failed");
                    }
                }
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "cannot spawn the activation sync thread");
        }
    }

    /// Run the Connection Doctor (R1) in a detached thread against the
    /// currently live provider; the UI polls `doctor_result`.
    fn run_connection_doctor(&mut self) {
        let Some(handle) = self.provider.clone() else {
            self.error = Some("the provider handle is not available (test session)".into());
            return;
        };
        if self.doctor_running {
            return;
        }
        self.doctor_running = true;
        let slot = std::sync::Arc::clone(&self.doctor_result);
        let bus = self.bus.clone();
        let spawned = std::thread::Builder::new()
            .name("spp-connection-doctor".to_string())
            .spawn(move || {
                let Some(real) = handle.real() else {
                    if let Ok(mut guard) = slot.lock() {
                        *guard = Some(Vec::new());
                    }
                    return;
                };
                let outcome = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("cannot start the doctor runtime: {e}"))
                    .map(|rt| {
                        rt.block_on(async { spp_core::connection_doctor::run_doctor(&real).await })
                    });
                match outcome {
                    Ok(steps) => {
                        if let Some(step) = steps
                            .iter()
                            .find(|s| s.status == spp_core::connection_doctor::DoctorStatus::Fail)
                        {
                            // R5 (D4): a failing step is a live error, not
                            // just a form detail.
                            if let Some(bus) = &bus {
                                spp_core::eventing::notify_error(
                                    bus,
                                    "error",
                                    &format!(
                                        "Connection check failed: {} — {}",
                                        step.name, step.detail
                                    ),
                                    "connect",
                                    None,
                                );
                            }
                        }
                        if let Ok(mut guard) = slot.lock() {
                            *guard = Some(steps);
                        }
                    }
                    Err(e) => {
                        if let Some(bus) = &bus {
                            spp_core::eventing::notify_error(
                                bus,
                                "error",
                                &format!("the Connection Doctor could not run: {e}"),
                                "connect",
                                None,
                            );
                        }
                        if let Ok(mut guard) = slot.lock() {
                            *guard = Some(Vec::new());
                        }
                    }
                }
            });
        if let Err(e) = spawned {
            self.doctor_running = false;
            self.error = Some(format!("cannot start the doctor thread: {e}"));
        }
    }

    /// Remove every credential stored by the form. The connection
    /// tokens are deliberately untouched. v1.4.0 (R3): the provider
    /// deactivates in-session — the app stays fully usable against the
    /// local mirror, and the R2 guard stops the sync timers from
    /// stamping watermarks against the empty provider.
    fn clear_helpscout_credentials(&mut self, db: &DbHandle) {
        let result =
            db.with_write(|conn| spp_core::helpscout_real::HsCredentials::clear_envelope(conn));
        match result {
            Ok(removed) => {
                self.message = Some(format!(
                    "stored credentials removed ({removed} values). The connection \
                     tokens are untouched; the provider deactivated in-session."
                ));
                self.hs_client_id.clear();
                self.hs_client_secret.clear();
                self.hs_webhook_secret.clear();
                self.hs_docs_api_key.clear();
                if let Some(handle) = self.provider.clone() {
                    let previous_kind = handle.deactivate();
                    db.with_write(|conn| {
                        spp_core::sync_engine::on_provider_kind_transition(
                            conn,
                            Some(previous_kind.as_str()),
                            "empty",
                        );
                        let _ = spp_core::settings::set_string(conn, "provider_kind", "empty");
                    });
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.reload(db);
    }

    /// One-click workspace reset (1.2.0): clears all business content —
    /// synced or left over from the pre-1.2.0 demo — keeps the
    /// KEEP_TABLES config, resets the vector store when the data
    /// directory is known, and arms a fresh first sync.
    fn reset_workspace(&mut self, db: &DbHandle) {
        let result = db.with_write(|conn| {
            spp_core::data_reset::reset_workspace_data(conn, self.data_dir.as_deref())
        });
        match result {
            Ok(report) => {
                self.message = Some(format!(
                    "workspace data deleted: {} rows across {} tables, {} vector \
                     entries. Restart SupportOS+ so every view reloads clean{}.",
                    report.rows_deleted,
                    report.tables_cleared,
                    report.vectors_dropped,
                    if db
                        .with_read(|conn| {
                            spp_core::helpscout_real::HsCredentials::resolve(conn).0
                        })
                        .is_configured()
                    {
                        " — the next start pulls your account fresh"
                    } else {
                        " — connect Help Scout from this page when ready"
                    }
                ));
            }
            Err(e) => self.error = Some(format!("reset failed (nothing was changed): {e}")),
        }
        self.purge_armed = false;
        self.invalidate();
        self.reload(db);
    }

    fn create_backup(&mut self, db: &DbHandle) {
        let Some(dir) = self.backups_dir.clone() else {
            self.error = Some("backup directory not configured".into());
            return;
        };
        let result = db.with_read(|conn| spp_core::backup_service::backup(conn, &dir));
        if result.ok {
            self.message = Some(format!(
                "backup {}",
                result
                    .path
                    .clone()
                    .unwrap_or_else(|| "(path unknown)".into())
            ));
        } else {
            self.error = Some(result.message);
        }
        self.reload_backups();
    }

    fn export_json(&mut self, db: &DbHandle) {
        let Some(dir) = self.backups_dir.clone() else {
            self.error = Some("backup directory not configured".into());
            return;
        };
        let result = db.with_read(|conn| spp_core::backup_service::export_json(conn, &dir));
        self.message = Some(format!(
            "export: {}",
            result
                .get("path")
                .and_then(|p| p.as_str())
                .unwrap_or("(failed)")
        ));
    }

    fn export_csv(&mut self, db: &DbHandle) {
        let Some(dir) = self.backups_dir.clone() else {
            self.error = Some("backup directory not configured".into());
            return;
        };
        let result =
            db.with_read(|conn| spp_core::backup_service::export_conversations_csv(conn, &dir));
        self.message = Some(format!(
            "export: {}",
            result
                .get("path")
                .and_then(|p| p.as_str())
                .unwrap_or("(failed)")
        ));
    }

    fn restore_backup(&mut self, db: &DbHandle, path: &str) {
        let Some(db_path) = db.file_path() else {
            self.error = Some("restore needs a file-backed database".into());
            return;
        };
        let result = spp_core::backup_service::restore(&db_path, std::path::Path::new(path));
        let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        if ok {
            self.message = Some(format!("restored {} — restart the app to reload it", path));
        } else {
            self.error = Some(format!(
                "restore failed: {}",
                result
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown")
            ));
        }
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        design::page_header(
            ui,
            "Settings",
            Some("local configuration, persisted in the settings table"),
            |ui| {
                for tab in Tab::ALL {
                    if design::chip(ui, self.tab == tab, tab.label()).clicked() {
                        self.tab = tab;
                        self.reload(db);
                    }
                }
            },
        );
        if let Some(message) = &self.message {
            design::success_text(ui, message);
        }
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        ui.add_space(4.0);

        match self.tab {
            Tab::General => self.general_ui(ui, db),
            Tab::HelpScout => self.helpscout_ui(ui, db),
            Tab::LmStudio => self.lmstudio_ui(ui, db),
            Tab::VectorIndex => self.vector_index_ui(ui, db),
            Tab::Hours => self.hours_ui(ui, db),
            Tab::Backups => self.backups_ui(ui, db),
        }
    }

    fn general_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let Some(mut general) = self.general.clone() else {
            ui.spinner();
            return;
        };
        design::card_titled(ui, "Synchronization", None, |ui| {
            design::form_grid(ui, "settings-general", |ui| {
                design::field_label(ui, "max quiet sync interval (minutes):");
                ui.add(egui::DragValue::new(&mut general.sync_interval_minutes).range(1..=1440))
                    .on_hover_text(
                        "1.4.0 R8: the sync polls every 30 s while the account is busy, \
                     then backs off by doubling toward this ceiling when quiet. \
                     Every send triggers an immediate poll.",
                    );
                ui.end_row();
                design::field_label(ui, "api concurrency:");
                ui.add(egui::DragValue::new(&mut general.api_concurrency).range(1..=8));
                ui.end_row();
                design::field_label(ui, "log level:");
                let combo = egui::ComboBox::from_id_salt("settings-log-level")
                    .width(160.0)
                    .selected_text(general.log_level.clone())
                    .show_ui(ui, |ui| {
                        for level in ["debug", "info", "warn", "error"] {
                            ui.selectable_value(&mut general.log_level, level.into(), level);
                        }
                    });
                combo.response.on_hover_text(
                    "Clean-audit item 138: drives the real tracing filter — \
                     applied at boot from the saved value and immediately on save.",
                );
                ui.end_row();
                design::field_label(ui, "agent language:");
                ui.add_sized(
                    [220.0, 28.0],
                    egui::TextEdit::singleline(&mut general.agent_language),
                );
                ui.end_row();
                design::field_label(ui, "status after reply:");
                egui::ComboBox::from_id_salt("settings-reply-status-after")
                    .width(190.0)
                    .selected_text(general.reply_status_after.clone())
                    .show_ui(ui, |ui| {
                        // Audit item 25: `open` is Help Scout's own status
                        // for "await agent" — the old `waiting` option was
                        // not in the platform's vocabulary and every send
                        // configured with it failed on the wire.
                        for (value, label) in [
                            ("pending", "pending (await customer)"),
                            ("open", "open (await agent)"),
                            ("keep", "keep current status"),
                        ] {
                            ui.selectable_value(
                                &mut general.reply_status_after,
                                value.into(),
                                label,
                            );
                        }
                    });
                ui.end_row();
            });
        });
        ui.add_space(10.0);
        design::card_titled(
            ui,
            "AI behaviour",
            Some("safe defaults — review before enabling write actions"),
            |ui| {
                egui::Grid::new(egui::IdSalt::new("settings-ai"))
                    .num_columns(2)
                    .striped(true)
                    .spacing(egui::vec2(16.0, 4.0))
                    .min_row_height(24.0)
                    .show(ui, |ui| {
                        for (label, value) in [
                            ("AI enabled", &mut general.ai_enabled),
                            (
                                "Automatic analysis",
                                &mut general.automatic_analysis_enabled,
                            ),
                            (
                                "Automatic internal notes",
                                &mut general.automatic_note_enabled,
                            ),
                            (
                                "Automatic draft replies",
                                &mut general.automatic_draft_enabled,
                            ),
                            (
                                "Automatic reply sending",
                                &mut general.automatic_reply_sending,
                            ),
                            ("Automation rules", &mut general.automation_enabled),
                            (
                                "Automation write actions",
                                &mut general.automation_write_actions_enabled,
                            ),
                            (
                                "Attachment auto download",
                                &mut general.attachment_auto_download,
                            ),
                        ] {
                            ui.checkbox(value, label);
                            ui.end_row();
                        }
                    });
                ui.add_space(6.0);
            },
        );
        ui.add_space(10.0);
        // R7: version transparency — running vs latest published release.
        // The check is user-initiated only; nothing phones home on its own.
        self.poll_update_result();
        design::card_titled(ui, "About & updates", None, |ui| {
            ui.label(format!("Running version: v{}", env!("CARGO_PKG_VERSION")));
            if let Some(result) = self.update_snapshot() {
                match result {
                    Ok(text) => {
                        ui.label(
                            egui::RichText::new(text)
                                .small()
                                .color(ui.visuals().text_color()),
                        );
                    }
                    Err(text) => {
                        ui.label(
                            egui::RichText::new(text)
                                .small()
                                .color(crate::design::tokens(ui).warning_foreground),
                        );
                    }
                }
            }
            if self.update_running {
                ui.spinner();
            } else if design::button(ui, "check for updates", ButtonVariant::Outline).clicked() {
                self.start_update_check();
            }
            ui.label(
                egui::RichText::new(
                    "The check reads the public GitHub release feed once, on \
                     click. The app never sends telemetry.",
                )
                .small()
                .color(ui.visuals().weak_text_color()),
            );
        });
        ui.add_space(10.0);
        if design::button(ui, "save settings", ButtonVariant::Primary).clicked() {
            self.general = Some(general.clone());
            self.save_general(db);
        }
        self.general = Some(general);
    }

    fn helpscout_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        // Pick up a finished client-credentials connect (1.1.4) and a
        // finished doctor run (R1).
        self.poll_connect_result(db);
        self.poll_doctor_result();
        let Some(status) = self.helpscout.clone() else {
            ui.spinner();
            return;
        };
        design::card_titled(
            ui,
            "Help Scout connection",
            Some(
                "Connect with client credentials (recommended — no browser, \
                 no redirect URL) or the advanced browser flow",
            ),
            |ui| {
                design::form_grid(ui, "settings-helpscout", |ui| {
                    design::field_label(ui, "credentials stored:");
                    ui.horizontal(|ui| {
                        design::badge(
                            ui,
                            if status.configured {
                                "connected"
                            } else if status.credentials {
                                "credentials saved"
                            } else {
                                "not configured"
                            },
                            // Any unconnected state reads the same: the
                            // badge tone only distinguishes "connected".
                            if status.configured {
                                BadgeTone::Success
                            } else {
                                BadgeTone::Neutral
                            },
                        );
                    });
                    ui.end_row();
                    design::field_label(ui, "redirect uri:");
                    ui.horizontal(|ui| {
                        ui.monospace(
                            egui::RichText::new(status.redirect_uri.as_str())
                                .size(12.5)
                                .color(design::tokens(ui).foreground),
                        )
                        .on_hover_text(
                            "only for the browser flow: register this EXACT URL as \
                             the callback URL of your Help Scout custom app \
                             (Profile → Apps & Integrations). Help Scout matches \
                             the redirect exactly. The local listener binds its \
                             port when SupportOS+ starts; change it with \
                             HELPSCOUT_REDIRECT_URI.",
                        );
                    });
                    ui.end_row();
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if status.configured
                        && design::button(ui, "disconnect", ButtonVariant::Destructive)
                            .on_hover_text("drops the stored tokens; local data is fully preserved")
                            .clicked()
                    {
                        self.disconnect_helpscout(db);
                    }
                    // R1: client credentials is the primary, recommended
                    // flow — it completes inside the app with no browser
                    // round-trip and no exact-match redirect dependency.
                    if !status.configured
                        && status.credentials
                        && !self.hs_connecting
                        && design::button(
                            ui,
                            "connect with client credentials (recommended)",
                            ButtonVariant::Primary,
                        )
                        .on_hover_text(
                            "no browser round-trip: verifies the stored client \
                             id/secret against Help Scout and stores the token \
                             directly",
                        )
                        .clicked()
                    {
                        self.connect_client_credentials();
                    }
                    if self.hs_connecting {
                        ui.spinner();
                    }
                });
                if !status.configured && status.credentials {
                    ui.add_space(4.0);
                    // R1: the browser flow is demoted to a clearly-labeled
                    // secondary with the exact-match warning the audit
                    // demanded (D3): loopback redirects are undocumented on
                    // the platform, and a mismatched callback URL dead-ends
                    // silently on Help Scout's side of the browser.
                    ui.collapsing(
                        egui::RichText::new("advanced: browser authorization flow")
                            .small()
                            .color(design::tokens(ui).muted_foreground),
                        |ui| {
                            design::hint_text(
                                ui,
                                "Help Scout redirects the browser to the callback URL \
                                 registered on your custom app — it must match the \
                                 redirect URI above EXACTLY, and loopback \
                                 (localhost) redirects are undocumented on the \
                                 platform. If the browser flow fails, use client \
                                 credentials above instead.",
                            );
                            ui.add_space(4.0);
                            if design::button(
                                ui,
                                "open the authorization page in the browser",
                                ButtonVariant::Outline,
                            )
                            .clicked()
                            {
                                self.begin_oauth(db);
                            }
                        },
                    );
                }
                if let Some(url) = &self.authorize_url {
                    ui.add_space(6.0);
                    ui.hyperlink_to(
                        egui::RichText::new("open the Help Scout authorization page").size(13.0),
                        url.clone(),
                    );
                    ui.add_space(2.0);
                    design::hint_text(
                        ui,
                        "after approving, Help Scout redirects the browser to the \
                         redirect URI above; leave SupportOS+ running.",
                    );
                }
            },
        );
        ui.add_space(10.0);

        // Connection Doctor (v1.4.0 R1): five sequential checks with
        // per-step results and remedies, so no owner ever has to guess
        // which link in the chain broke.
        self.connection_doctor_ui(ui);
        ui.add_space(10.0);

        // Credential form (1.1.3): values are stored in the encrypted
        // secrets envelope; saving activates the connection immediately
        // (R3 — no restart). Environment variables still win per field.
        design::card_titled(
            ui,
            "Help Scout credentials",
            Some(
                "Stored encrypted on this machine. Save them and the app \
                 connects immediately — the Connection Doctor verifies the \
                 chain and the first sync starts by itself. Environment \
                 variables override these values.",
            ),
            |ui| {
                let mask = !self.hs_show_secrets;
                design::form_grid(ui, "settings-hs-creds", |ui| {
                    design::field_label(ui, "client id:");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.hs_client_id)
                            .hint_text("Help Scout custom app — client ID"),
                    );
                    ui.end_row();
                    design::field_label(ui, "client secret:");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.hs_client_secret)
                            .password(mask)
                            .hint_text("leave blank to keep the stored value"),
                    );
                    ui.end_row();
                    design::field_label(ui, "webhook secret:");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.hs_webhook_secret)
                            .password(mask)
                            .hint_text("verifies incoming Help Scout webhooks"),
                    );
                    ui.end_row();
                    design::field_label(ui, "docs api key:");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.hs_docs_api_key)
                            .password(mask)
                            .hint_text("optional — leave blank to keep the stored value"),
                    );
                    ui.end_row();
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.hs_show_secrets, "show secrets");
                    if design::button(ui, "save credentials", ButtonVariant::Primary).clicked() {
                        self.save_helpscout_credentials(db);
                    }
                    let any_stored = status.sources.client_id.is_set()
                        || status.sources.client_secret.is_set()
                        || status.sources.webhook_secret.is_set()
                        || status.sources.docs_api_key.is_set();
                    if any_stored
                        && design::button(ui, "clear stored", ButtonVariant::Destructive).clicked()
                    {
                        self.clear_helpscout_credentials(db);
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let sources = status.sources;
                    for (label, source) in [
                        ("client id", sources.client_id),
                        ("client secret", sources.client_secret),
                        ("webhook secret", sources.webhook_secret),
                        ("docs api key", sources.docs_api_key),
                    ] {
                        design::hint_text(ui, &format!("{label}:"));
                        design::badge(
                            ui,
                            source.label(),
                            if source.is_set() {
                                BadgeTone::Success
                            } else {
                                BadgeTone::Neutral
                            },
                        );
                    }
                });
            },
        );
        ui.add_space(10.0);

        // P4 (D21): the webhook relay + the self-healing guard.
        design::card_titled(
            ui,
            "Webhook relay & health",
            Some(
                "the listener stays loopback-only by design; a user-run relay \
                 bridges the NAT, and the health check re-registers the hook \
                 when the platform silently drops it",
            ),
            |ui| {
                design::field_label(ui, "public relay URL:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.webhook_public_url)
                        .hint_text("https://your-relay → forwards to this app's loopback listener"),
                );
                ui.horizontal(|ui| {
                    if design::button(ui, "save relay URL", ButtonVariant::Outline).clicked() {
                        self.save_webhook_relay_url(db);
                    }
                    if design::button(ui, "check now", ButtonVariant::Primary).clicked() {
                        self.queue_webhook_health_check(db);
                    }
                });
                // The user-run tunnel guide: what to run, how to prove the
                // chain, and the honest limit on secret rotation.
                design::hint_text(
                    ui,
                    "relay options: cloudflared tunnel --url http://127.0.0.1:PORT · \
                     ngrok http 127.0.0.1:PORT · ssh -N -R 80:127.0.0.1:PORT serveo.net \
                     — the registered URL must forward to the loopback listener",
                );
                design::hint_text(
                    ui,
                    "prove the chain from outside: curl -X POST <relay>/webhook -d '{}' \
                     must answer 401 (signature check) — anything else means the relay \
                     is not reaching the app",
                );
                design::hint_text(
                    ui,
                    "one honest limit: the API never reveals a hook's stored secret, so \
                     rotating the local webhook secret needs a re-register — the check \
                     does that for you when it finds the hook stale",
                );
                match &self.wh_outcome {
                    Some(outcome) => {
                        let tone = if outcome.is_healthy() {
                            BadgeTone::Success
                        } else {
                            BadgeTone::Neutral
                        };
                        ui.horizontal(|ui| {
                            design::field_label(ui, "last check:");
                            design::badge(ui, &outcome.verdict, tone);
                            ui.label(format!("· {}", outcome.checked_at));
                        });
                        design::hint_text(ui, &outcome.detail);
                    }
                    None => {
                        design::hint_text(
                            ui,
                            "the webhook guard has not run yet (it checks every ~6 hours, \
                             or press check now)",
                        );
                    }
                }
            },
        );
        ui.add_space(10.0);

        // Workspace reset (1.2.0): one click, two-step confirmation.
        design::card_titled(
            ui,
            "Workspace data",
            Some(
                "Deletes every conversation, customer, thread, rating, \
                 knowledge document, incident, report and AI record — all \
                 synced content (and anything left over from the seeded \
                 content that shipped before 1.2.0) — and arms a fresh first sync. \
                 Keeps: credentials, settings, business hours, automation \
                 rules, saved views and replies, custom object types.",
            ),
            |ui| {
                if !self.purge_armed {
                    if design::button(ui, "delete all workspace data", ButtonVariant::Destructive)
                        .clicked()
                    {
                        self.purge_armed = true;
                    }
                } else {
                    design::error_text(
                        ui,
                        "This cannot be undone. Everything listed above will be deleted.",
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if design::button(ui, "cancel", ButtonVariant::Outline).clicked() {
                            self.purge_armed = false;
                        }
                        if design::button(
                            ui,
                            "really delete everything",
                            ButtonVariant::Destructive,
                        )
                        .clicked()
                        {
                            self.reset_workspace(db);
                        }
                    });
                }
            },
        );
    }

    /// Save the public relay URL the health check re-registers to (P4).
    fn save_webhook_relay_url(&mut self, db: &DbHandle) {
        let url = self.webhook_public_url.trim().to_string();
        let saved =
            db.with_write(|conn| spp_core::settings::set_string(conn, "webhook_public_url", &url));
        match saved {
            Ok(()) => {
                self.message =
                    Some("relay URL saved — the next health check registers the hook there".into());
            }
            Err(e) => self.error = Some(format!("could not save the relay URL: {e}")),
        }
    }

    /// Queue the on-demand webhook health check (P4): the job runs within
    /// seconds and stores the outcome this card reads.
    fn queue_webhook_health_check(&mut self, db: &DbHandle) {
        let queued = db.with_write(|conn| {
            spp_core::jobs::enqueue_on(
                conn,
                "sync",
                "webhook_health_check",
                "{\"triggerKind\":\"manual\"}",
                1,
                2,
            )
        });
        match queued {
            Ok(_) => {
                self.message =
                    Some("webhook health check queued — the outcome lands here in a moment".into());
            }
            Err(e) => self.error = Some(format!("could not queue the check: {e}")),
        }
    }

    /// The Connection Doctor card (R1): a run button, the five steps
    /// with pass/warn/fail state, what each saw, and the remedy for a
    /// failure. Runs automatically after every credential save and can
    /// be re-run any time.
    fn connection_doctor_ui(&mut self, ui: &mut egui::Ui) {
        design::card_titled(
            ui,
            "Connection Doctor",
            Some(
                "five checks: reach the API → sign in → read the user → \
                 list mailboxes → rate-limit budget",
            ),
            |ui| {
                if self.doctor_running {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("checking the connection…");
                    });
                } else {
                    let live = self.provider.as_ref().is_some_and(|p| p.is_real());
                    if live
                        && design::button(ui, "run the checks", ButtonVariant::Outline)
                            .on_hover_text(
                                "verifies the whole connection chain right now: \
                                 network, credentials, token, identity, mailboxes, \
                                 rate limits",
                            )
                            .clicked()
                    {
                        self.run_connection_doctor();
                    }
                    if !live {
                        design::hint_text(
                            ui,
                            "save credentials above (or press connect) and the doctor \
                             runs automatically — or run it any time while a \
                             connection is active.",
                        );
                    }
                }
                if !self.doctor_steps.is_empty() {
                    let steps = self.doctor_steps.clone();
                    ui.add_space(6.0);
                    egui::Grid::new(egui::IdSalt::new("doctor-steps"))
                        .num_columns(2)
                        .striped(true)
                        .spacing(egui::vec2(16.0, 4.0))
                        .show(ui, |ui| {
                            use spp_core::connection_doctor::DoctorStatus;
                            for step in &steps {
                                let (tone, label) = match step.status {
                                    DoctorStatus::Pass => {
                                        (egui::Color32::from_rgb(34, 197, 94), "✓ ok")
                                    }
                                    DoctorStatus::Warn => {
                                        (egui::Color32::from_rgb(234, 179, 8), "! warning")
                                    }
                                    DoctorStatus::Fail => {
                                        (egui::Color32::from_rgb(239, 68, 68), "✗ failed")
                                    }
                                };
                                ui.label(
                                    egui::RichText::new(format!("{label} — {}", step.name))
                                        .color(tone)
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(format!("{} {}", step.detail, step.remedy))
                                        .small(),
                                );
                                ui.end_row();
                            }
                        });
                }
            },
        );
    }

    /// Pick up a finished doctor run (called from the UI each frame
    /// while the Help Scout tab is visible); the report stays on screen
    /// until the next run replaces it.
    fn poll_doctor_result(&mut self) {
        if !self.doctor_running {
            return;
        }
        let done = self
            .doctor_result
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        if let Some(steps) = done {
            self.doctor_running = false;
            self.doctor_steps = steps;
        }
    }

    /// R7 version transparency: launch the user-initiated release check on
    /// a detached thread (own runtime inside `latest_release_blocking`).
    fn start_update_check(&mut self) {
        self.update_running = true;
        let slot = std::sync::Arc::clone(&self.update_result);
        std::thread::spawn(move || {
            let outcome = match spp_core::version_check::latest_release_blocking() {
                Ok(info) => {
                    let ordering = spp_core::version_check::compare_versions(
                        env!("CARGO_PKG_VERSION"),
                        &info.tag_name,
                    );
                    Ok(format!(
                        "Latest published release: {} — {}.",
                        info.tag_name,
                        ordering.label()
                    ))
                }
                Err(e) => Err(format!("update check failed: {e}")),
            };
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(outcome);
            }
        });
    }

    /// Take a finished update-check outcome, if any (UI thread).
    fn poll_update_result(&mut self) {
        if !self.update_running {
            return;
        }
        let done = self
            .update_result
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        if done.is_some() {
            self.update_running = false;
        }
    }

    /// The update-check outcome currently on display (cloned for render).
    fn update_snapshot(&self) -> Option<Result<String, String>> {
        self.update_result
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Start the browser OAuth flow: issue a single-use state and show the
    /// authorization URL (the loopback listener serves the callback).
    ///
    /// 1.1.4 guards stay (the listener must really be listening on the
    /// redirect port); the 1.1.4 restart guard is gone — v1.4.0 (R3)
    /// activates saved credentials in-session, so the callback listener
    /// exchanges against the live provider whenever the browser lands.
    fn begin_oauth(&mut self, db: &DbHandle) {
        let live = self.provider.as_ref().and_then(|p| p.real());
        let Some(_real) = live else {
            self.error = Some(
                "the browser flow needs an active Help Scout connection. \
                 Save your client credentials in the form below first — \
                 they activate immediately, no restart needed."
                    .into(),
            );
            self.authorize_url = None;
            return;
        };
        let (creds, _) = db.with_read(spp_core::helpscout_real::HsCredentials::resolve);
        let redirect_uri = creds.redirect_uri.clone();
        self.begin_oauth_at(db, &creds, &redirect_uri);
    }

    /// Testable core of [`Self::begin_oauth`]: guard the listener, then
    /// issue the single-use state and the authorization URL.
    fn begin_oauth_at(
        &mut self,
        db: &DbHandle,
        creds: &spp_core::helpscout_real::HsCredentials,
        redirect_uri: &str,
    ) {
        let Some(port) = spp_core::loopback::port_from_redirect_uri(redirect_uri) else {
            self.error = Some(format!(
                "the redirect URI {redirect_uri:?} is not a local http URL with \
                 a port; set HELPSCOUT_REDIRECT_URI to something like \
                 http://localhost:3000/oauth/callback"
            ));
            self.authorize_url = None;
            return;
        };
        if !spp_core::loopback::listener_answers(port) {
            self.error = Some(format!(
                "the SupportOS+ callback listener is not answering on port \
                 {port}. Either SupportOS+ was not restarted after the \
                 credentials were saved, or another program is using the \
                 port. Restart SupportOS+ and try again; if the port is \
                 taken, close that program (or set HELPSCOUT_REDIRECT_URI \
                 to a free port and register the same URL on your Help \
                 Scout app)."
            ));
            self.authorize_url = None;
            return;
        }
        match spp_core::loopback::begin_oauth(db, creds, redirect_uri) {
            Ok(url) => {
                self.message = Some(
                    "Authorization URL ready. Complete the flow in the browser; the loopback \
                     listener stores the token."
                        .into(),
                );
                self.authorize_url = Some(url);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Connect with the client-credentials grant in the background
    /// (1.1.4): the immediate in-app answer to "are my credentials any
    /// good?" — and a full connection for users whose Help Scout app
    /// authenticates that way (no browser round-trip). v1.4.0 (R5):
    /// failures emit a live error event, and (R3) the provider comes from
    /// the swappable handle, not a boot-time snapshot. The detached
    /// thread writes its outcome into `hs_connect_result`; the UI polls
    /// it every frame and turns it into a message/error.
    fn connect_client_credentials(&mut self) {
        let Some(real) = self.provider.as_ref().and_then(|p| p.real()) else {
            self.error = Some(
                "connect needs an active Help Scout connection — save your \
                 credentials in the form below first (they activate \
                 immediately, no restart)."
                    .into(),
            );
            return;
        };
        if self.hs_connecting {
            return;
        }
        self.hs_connecting = true;
        self.message = Some("connecting with client credentials…".into());
        let slot = std::sync::Arc::clone(&self.hs_connect_result);
        let bus = self.bus.clone();
        let spawned = std::thread::Builder::new()
            .name("spp-hs-client-credentials".to_string())
            .spawn(move || {
                let outcome = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("cannot start the connect runtime: {e}"))
                    .and_then(|rt| {
                        rt.block_on(async { real.client_credentials_login().await.map(|_| ()) })
                            .map_err(|e| e.to_string())
                    })
                    .map_or_else(
                        |e| {
                            // R5 (D4): connect failures reach the UI live,
                            // even from another page.
                            if let Some(bus) = &bus {
                                spp_core::eventing::notify_error(
                                    bus,
                                    "error",
                                    &format!("Connecting with client credentials failed: {e}"),
                                    "connect",
                                    None,
                                );
                            }
                            Err(e)
                        },
                        |()| {
                            Ok("connected with client credentials — tokens stored. \
                                The sync pulls your account automatically."
                                .to_string())
                        },
                    );
                if let Ok(mut guard) = slot.lock() {
                    *guard = Some(outcome);
                }
            });
        if let Err(e) = spawned {
            self.hs_connecting = false;
            self.error = Some(format!("cannot start the connect thread: {e}"));
        }
    }

    /// Poll the in-flight client-credentials connect (called from the
    /// UI each frame while the Help Scout tab is visible).
    fn poll_connect_result(&mut self, db: &DbHandle) {
        if !self.hs_connecting {
            return;
        }
        let outcome = self
            .hs_connect_result
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        let Some(outcome) = outcome else {
            return;
        };
        self.hs_connecting = false;
        match outcome {
            Ok(msg) => {
                self.message = Some(msg);
                self.invalidate();
                self.reload(db);
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn lmstudio_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card_titled(
            ui,
            "LM Studio (local AI gateway)",
            Some("Test the connection from the AI Center page."),
            |ui| {
                design::form_grid(ui, "settings-lmstudio", |ui| {
                    design::field_label(ui, "base url:");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.lmstudio_base_url)
                            .hint_text("http://localhost:1234/v1"),
                    );
                    ui.end_row();
                    design::field_label(ui, "chat model:");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.lmstudio_chat_model),
                    );
                    ui.end_row();
                    design::field_label(ui, "embedding model:");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.lmstudio_embedding_model),
                    );
                    ui.end_row();
                });
                ui.add_space(8.0);
                if design::button(ui, "save", ButtonVariant::Primary).clicked() {
                    self.save_lmstudio(db);
                }
            },
        );
    }

    fn vector_index_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card_titled(
            ui,
            "Vector index",
            Some(
                "Dense vectors live in SQLite next to the application database (decision D4: the chunk count is far below the 100k gate). The index follows the qdrant_enabled setting; searches fall back to FTS5 when disabled.",
            ),
            |ui| {
                if self.vector_store_path.is_empty() {
                    // Reuse the lazy-load slot to remember where the store lives.
                    self.vector_store_path = db
                        .file_path()
                        .and_then(|p| p.parent().map(|d| d.join("vectors").join("vectors.db")))
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                }
                design::form_grid(ui, "settings-vector-index", |ui| {
                    design::field_label(ui, "store:");
                    ui.monospace(
                        egui::RichText::new(if self.vector_store_path.is_empty() {
                            "(unknown data directory)".to_string()
                        } else {
                            self.vector_store_path.clone()
                        })
                        .color(design::tokens(ui).foreground),
                    );
                    ui.end_row();
                    // Clean-audit item 139: the card subtitle references the
                    // qdrant_enabled setting, but no control exposed it
                    // anywhere — the toggle lives here now. The workers open
                    // the store at boot, so a change lands on the next start.
                    let prev = db
                        .with_read(|conn| {
                            spp_core::settings::get_bool(conn, "qdrant_enabled", true)
                        })
                        .unwrap_or(true);
                    let mut enabled = prev;
                    let toggle = ui.checkbox(&mut enabled, "vector index enabled");
                    toggle.on_hover_text(
                        "Dense vectors are embedded and searched while enabled; searches \
                         fall back to FTS5 when disabled. Applies on the next start — \
                         the workers open the store at boot.",
                    );
                    ui.end_row();
                    if enabled != prev {
                        let result = db.with_write(|conn| {
                            spp_core::settings::set_bool(conn, "qdrant_enabled", enabled)
                        });
                        match result {
                            Ok(()) => {
                                self.message = Some(
                                    "vector index preference saved — applies on the next start"
                                        .into(),
                                );
                            }
                            Err(e) => self.error = Some(e.to_string()),
                        }
                    }
                });
            },
        );
    }

    fn hours_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card_titled(ui, "Business hours per mailbox", None, |ui| {
            let hours = self.hours.clone();
            let mut edit: Option<BusinessHoursRow> = None;
            egui::ScrollArea::vertical()
                .id_salt("settings-hours")
                .show(ui, |ui| {
                    for row in &hours {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(row.name.as_str())
                                    .size(13.5)
                                    .strong()
                                    .color(design::tokens(ui).foreground),
                            );
                            if row.configured {
                                design::badge(ui, "configured", BadgeTone::Success);
                                design::hint_text(
                                    ui,
                                    &format!(
                                        "{} · {} · {}-{} · fr {}m · res {}m",
                                        row.timezone,
                                        row.days,
                                        minutes_label(row.start_minute),
                                        minutes_label(row.end_minute),
                                        row.first_response_target_min,
                                        row.resolution_target_min
                                    ),
                                );
                            } else {
                                design::badge(ui, "not configured", BadgeTone::Neutral);
                            }
                            if design::button(ui, "edit", ButtonVariant::Outline).clicked() {
                                edit = Some(row.clone());
                            }
                        });
                        ui.add_space(2.0);
                    }
                    if hours.is_empty() {
                        design::hint_text(ui, "(no mailboxes synced yet)");
                    }
                });
            if let Some(row) = edit {
                self.hours_edit = Some(row);
            }
        });

        let Some(mut row) = self.hours_edit.clone() else {
            return;
        };
        ui.add_space(10.0);
        design::card_titled(ui, &format!("Edit: {}", row.name), None, |ui| {
            design::form_grid(ui, "settings-hours-edit", |ui| {
                design::field_label(ui, "timezone:");
                ui.add_sized([220.0, 28.0], egui::TextEdit::singleline(&mut row.timezone));
                ui.end_row();
                design::field_label(ui, "days (0=Sun…6=Sat):");
                ui.add_sized([220.0, 28.0], egui::TextEdit::singleline(&mut row.days));
                ui.end_row();
                design::field_label(ui, "start (minutes):");
                ui.add(egui::DragValue::new(&mut row.start_minute).range(0..=1440));
                ui.end_row();
                design::field_label(ui, "end (minutes):");
                ui.add(egui::DragValue::new(&mut row.end_minute).range(0..=1440));
                ui.end_row();
                design::field_label(ui, "first response target (min):");
                ui.add(egui::DragValue::new(&mut row.first_response_target_min).range(1..=10_000));
                ui.end_row();
                design::field_label(ui, "resolution target (min):");
                ui.add(egui::DragValue::new(&mut row.resolution_target_min).range(1..=100_000));
                ui.end_row();
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if design::button(ui, "save hours", ButtonVariant::Primary).clicked() {
                    let result = db.with_write(|conn| save_business_hours(conn, &row));
                    match result {
                        Ok(()) => {
                            self.message = Some("business hours saved".into());
                            self.hours_edit = None;
                            self.reload(db);
                        }
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
                if design::button(ui, "cancel", ButtonVariant::Ghost).clicked() {
                    self.hours_edit = None;
                }
            });
        });
        if self.hours_edit.is_some() {
            self.hours_edit = Some(row);
        }
    }

    fn backups_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card_titled(ui, "Backups & export", None, |ui| {
            ui.horizontal(|ui| {
                if design::button_with_icon(
                    ui,
                    Icon::Download,
                    "create backup",
                    ButtonVariant::Primary,
                )
                .clicked()
                {
                    self.create_backup(db);
                }
                if design::button(ui, "export JSON", ButtonVariant::Secondary).clicked() {
                    self.export_json(db);
                }
                if design::button(ui, "export conversations CSV", ButtonVariant::Secondary)
                    .clicked()
                {
                    self.export_csv(db);
                }
            });
            ui.add_space(8.0);
            let backups = self.backups.clone();
            if backups.is_empty() {
                design::hint_text(ui, "(no backups yet)");
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt("settings-backups")
                .show(ui, |ui| {
                    let t = design::tokens(ui);
                    for backup in &backups {
                        ui.horizontal(|ui| {
                            ui.monospace(
                                egui::RichText::new(str_field(backup, "file")).color(t.foreground),
                            );
                            design::hint_text(
                                ui,
                                &format!("{} bytes", i64_field(backup, "size_bytes")),
                            );
                            if let Some(created) = backup.get("created_at").and_then(|a| a.as_str())
                            {
                                design::hint_text(ui, created);
                            }
                            if design::button(ui, "restore", ButtonVariant::Outline)
                                .on_hover_text("restores the database file — restart the app after")
                                .clicked()
                                && let Some(dir) = &self.backups_dir
                            {
                                let path = dir.join(str_field(backup, "file"));
                                // Audit item 11: the first click only ARMS
                                // the restore — a single unconfirmed click
                                // must never replace the live database.
                                self.restore_armed = Some(path.display().to_string());
                            }
                        });
                        ui.add_space(2.0);
                    }
                });
            // Second step of the two-step restore confirmation (audit
            // item 11): nothing is swapped until the user reads the
            // consequence and clicks again.
            if let Some(path) = self.restore_armed.clone() {
                ui.add_space(6.0);
                design::error_text(
                    ui,
                    &format!(
                        "Restore {}? The current database will be REPLACED by this backup \
                         (verified first). Restart SupportOS+ afterwards to use it.",
                        path
                    ),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if design::button(ui, "cancel", ButtonVariant::Ghost).clicked() {
                        self.restore_armed = None;
                    }
                    if design::button(ui, "really restore", ButtonVariant::Destructive).clicked() {
                        self.restore_armed = None;
                        self.restore_backup(db, &path);
                    }
                });
            }
        });
    }
}

impl Default for SettingsPage {
    fn default() -> Self {
        Self::new()
    }
}

/// `HH:MM` label for a minute-of-day value.
#[must_use]
pub fn minutes_label(minutes: i64) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn general_settings_round_trip_through_the_settings_table() {
        let db = fixture(Vec::new());
        let original = db.with_read(GeneralSettings::load);
        assert_eq!(original, GeneralSettings::default());

        let mut changed = original;
        changed.sync_interval_minutes = 15;
        changed.ai_enabled = false;
        changed.log_level = "debug".into();
        db.with_write(|conn| changed.save(conn)).expect("save");

        let reloaded = db.with_read(GeneralSettings::load);
        assert_eq!(reloaded, changed);
        assert_ne!(reloaded, GeneralSettings::default());
    }

    /// Clean-audit item 25: `waiting` was never in the platform's status
    /// vocabulary. The loader maps a stored legacy value onto the real
    /// await-agent status (`open`) so old installs stop offering and
    /// sending the phantom one; unknown values still fall back to the
    /// default.
    #[test]
    fn audit25_legacy_waiting_setting_loads_as_open() {
        let db = fixture(Vec::new());
        for (stored, expected) in [
            ("waiting", "open"),
            ("open", "open"),
            ("pending", "pending"),
            ("keep", "keep"),
            ("garbage", "pending"),
        ] {
            db.with_write(|conn| {
                spp_core::settings::set_string(conn, "reply_status_after", stored)
            })
            .expect("set");
            let loaded = db.with_read(GeneralSettings::load);
            assert_eq!(
                loaded.reply_status_after, expected,
                "stored '{stored}' must load as '{expected}'"
            );
        }
    }

    #[test]
    fn helpscout_status_reads_tokens() {
        let db = fixture(Vec::new());
        let status = db.with_read(helpscout_status);
        assert!(!status.configured);
        assert!(!status.credentials, "no credentials on a fresh database");

        db.with_write(|conn| {
            conn.execute("INSERT INTO oauth_tokens (access_token) VALUES ('tok')", [])
                .expect("token");
        });
        let status2 = db.with_read(helpscout_status);
        assert!(status2.configured);
    }

    #[test]
    fn business_hours_upsert_per_mailbox() {
        let db = fixture(Vec::new());
        let rows = db.with_read(load_business_hours);
        assert_eq!(rows.len(), 2, "Support and Billing mailboxes");
        assert!(rows.iter().all(|r| !r.configured));

        let mut row = rows.into_iter().find(|r| r.name == "Support").unwrap();
        row.timezone = "Asia/Dhaka".into();
        row.start_minute = 600;
        row.end_minute = 1100;
        db.with_write(|conn| save_business_hours(conn, &row))
            .expect("save hours");

        let reloaded = db.with_read(load_business_hours);
        let saved = reloaded.iter().find(|r| r.name == "Support").expect("row");
        assert!(saved.configured);
        assert_eq!(saved.timezone, "Asia/Dhaka");
        assert_eq!(saved.start_minute, 600);
        assert_eq!(minutes_label(saved.start_minute), "10:00");
    }

    #[test]
    fn business_hours_validation_rejects_bad_rows() {
        let db = fixture(Vec::new());
        let rows = db.with_read(load_business_hours);
        let mut row = rows.into_iter().next().unwrap();
        row.end_minute = row.start_minute;
        let result = db.with_write(|conn| save_business_hours(conn, &row));
        assert!(result.is_err());

        row.end_minute = row.start_minute + 60;
        row.timezone = "Mars/Olympus".into();
        let result2 = db.with_write(|conn| save_business_hours(conn, &row));
        assert!(result2.is_err());
    }

    /// Clean-audit item 141: the `days` field used to save verbatim —
    /// "1-5", "mon-fri" or "1; 2; 3" parsed as nothing on the SLA side
    /// and silently meant the default schedule. The save now rejects
    /// anything that is not a real weekday list and normalizes the
    /// stored value.
    #[test]
    fn business_hours_days_are_validated_and_normalized() {
        let db = fixture(Vec::new());
        let rows = db.with_read(load_business_hours);
        let mut row = rows.into_iter().find(|r| r.name == "Support").unwrap();
        row.timezone = "UTC".into();
        row.end_minute = row.start_minute + 480;

        for bad in ["1-5", "mon-fri", "1; 2; 3", "9", "", "  "] {
            row.days = bad.into();
            let result = db.with_write(|conn| save_business_hours(conn, &row));
            assert!(result.is_err(), "{bad:?} must be rejected");
        }

        row.days = "5, 1, 3".into();
        db.with_write(|conn| save_business_hours(conn, &row))
            .expect("save");
        let saved = db
            .with_read(load_business_hours)
            .into_iter()
            .find(|r| r.name == "Support")
            .unwrap();
        assert_eq!(
            saved.days, "[1,3,5]",
            "the stored list is normalized (deduped, sorted, JSON)"
        );

        // The default row text round-trips unchanged.
        row.days = "[1,2,3,4,5]".into();
        db.with_write(|conn| save_business_hours(conn, &row))
            .expect("save");
        let saved = db
            .with_read(load_business_hours)
            .into_iter()
            .find(|r| r.name == "Support")
            .unwrap();
        assert_eq!(saved.days, "[1,2,3,4,5]");
    }

    #[test]
    fn backup_create_lists_and_restores() {
        let db = fixture(vec![("conv", 1, "Keep me", "active", "email", 1, 5, 9)]);
        let dir = std::env::temp_dir().join(format!("spp-backup-{}", std::process::id()));
        let result = db.with_read(|conn| spp_core::backup_service::backup(conn, &dir));
        assert!(result.ok, "{}", result.message);
        assert!(result.path.is_some());

        let list = spp_core::backup_service::list_backups(&dir);
        assert_eq!(list.len(), 1);
        let file = str_field(&list[0], "file");
        assert!(file.contains("supportos-backup-"), "unexpected name {file}");
        assert!(i64_field(&list[0], "size_bytes") > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Install a master key for envelope tests (pub API; in-memory
    /// databases do not get one from bootstrap by design). Only these
    /// page tests touch the key in this test binary, so there is no
    /// parallel-flip race.
    fn install_master_key() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!(
            "spp-settings-page-key-{}-{nanos}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        spp_core::settings::init_master_key(&path).expect("master key");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn helpscout_credentials_save_and_clear() {
        install_master_key();
        let db = fixture(Vec::new());

        let mut page = SettingsPage::new();
        // The real UI loads the form once on the first frame (see
        // reload); simulate that so the save-time field clear is not
        // immediately overwritten by the first prefill.
        page.hs_form_loaded = true;
        page.hs_client_id = "cid-app".into();
        page.hs_client_secret = "secret-app".into();
        page.hs_webhook_secret = "wh-app".into();
        page.save_helpscout_credentials(&db);

        assert!(
            page.message
                .as_deref()
                .is_some_and(|m| m.contains("stored")),
            "save must confirm: {:?}",
            page.message
        );
        assert!(page.error.is_none(), "{:?}", page.error);
        let status = db.with_read(helpscout_status);
        assert!(status.credentials, "credentials resolve from the envelope");
        assert_eq!(
            status.sources.client_id,
            spp_core::helpscout_real::CredentialSource::AppSettings
        );
        assert_eq!(
            status.sources.webhook_secret,
            spp_core::helpscout_real::CredentialSource::AppSettings
        );
        assert!(
            page.hs_client_secret.is_empty(),
            "secret fields are cleared after a successful save"
        );

        // The stored secrets decrypt back through the envelope.
        let stored = db.with_read(spp_core::helpscout_real::HsCredentials::from_envelope);
        assert_eq!(stored.client_id, "cid-app");
        assert_eq!(stored.client_secret, "secret-app");

        // Clear: connection tokens survive.
        db.with_write(|conn| {
            conn.execute("INSERT INTO oauth_tokens (access_token) VALUES ('tok')", [])
                .expect("token");
        });
        page.clear_helpscout_credentials(&db);
        let status2 = db.with_read(helpscout_status);
        assert!(!status2.credentials);
        assert_eq!(
            status2.sources.client_id,
            spp_core::helpscout_real::CredentialSource::NotSet
        );
        assert!(status2.configured, "connection tokens survive the clear");
    }

    #[test]
    fn reset_workspace_empties_content_and_keeps_config() {
        let db = fixture(vec![(
            "conv",
            1,
            "Seeded ticket",
            "active",
            "email",
            1,
            5,
            9,
        )]);
        db.with_write(|conn| {
            spp_core::settings::set_string(conn, "agent_language", "English").expect("lang");
        });

        let mut page = SettingsPage::new();
        page.purge_armed = true;
        page.reset_workspace(&db);

        assert!(
            page.message
                .as_deref()
                .is_some_and(|m| m.contains("workspace data deleted")),
            "reset must report: {:?}",
            page.message
        );
        assert!(page.error.is_none(), "{:?}", page.error);
        assert!(
            !page.purge_armed,
            "the confirmation disarms after the reset"
        );

        let (convs, users, mailboxes, lang, sync_state) = db.with_read(|conn| {
            let count = |table: &str| -> i64 {
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| {
                    r.get::<_, i64>(0)
                })
                .unwrap_or_else(|e| panic!("count {table}: {e}"))
            };
            (
                count("conversations"),
                count("users"),
                count("mailboxes"),
                spp_core::settings::get_string(conn, "agent_language")
                    .expect("read")
                    .expect("present"),
                spp_core::sync_engine::get_state(conn),
            )
        });
        assert_eq!(convs, 0, "seeded conversations must be purged");
        assert_eq!(users, 0, "mirror users must be purged");
        assert_eq!(mailboxes, 0, "mirror mailboxes must be purged");
        assert_eq!(lang, "English", "application settings must survive");
        assert_eq!(sync_state, "NEW", "the reset arms a fresh first sync");
    }

    #[test]
    fn begin_oauth_without_an_active_connection_explains_itself() {
        // v1.4.0 (R3): the restart ritual is gone; the browser flow's
        // only remaining precondition is an active connection. Without
        // one (fresh page, no handle), the click explains what to do
        // instead of dead-ending in a browser.
        install_master_key();
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            spp_core::helpscout_real::HsCredentials::store_in_envelope(
                conn, "cid", "secret", "", "",
            )
            .expect("store");
        });

        let mut page = SettingsPage::new();
        page.begin_oauth(&db);
        assert!(page.authorize_url.is_none(), "no URL without a connection");
        assert!(
            page.error
                .as_deref()
                .is_some_and(|e| e.contains("no restart")),
            "the error must point at the activation path: {:?}",
            page.error
        );
    }

    #[test]
    fn begin_oauth_succeeds_against_a_live_listener() {
        let db = fixture(Vec::new());
        let mut server = spp_core::loopback::LoopbackServer::start(
            db.clone(),
            std::sync::Arc::new(spp_core::provider_handle::ProviderHandle::new_empty()),
            0,
        )
        .expect("listener");
        let redirect_uri = format!("http://localhost:{}/oauth/callback", server.port());
        let creds = spp_core::helpscout_real::HsCredentials {
            client_id: "cid-live".into(),
            client_secret: "secret-live".into(),
            redirect_uri: redirect_uri.clone(),
            ..Default::default()
        };

        let mut page = SettingsPage::new();
        page.set_provider_handle(std::sync::Arc::new(
            spp_core::provider_handle::ProviderHandle::new_real(std::sync::Arc::new(
                spp_core::helpscout_real::RealHelpScoutProvider::new(db.clone(), creds.clone()),
            )),
        ));
        page.begin_oauth_at(&db, &creds, &redirect_uri);

        let url = page.authorize_url.clone().expect("authorization URL");
        assert!(
            url.contains("client_id=cid-live"),
            "the URL carries the client id: {url}"
        );
        assert!(url.contains("state="));
        assert!(page.error.is_none(), "{:?}", page.error);
        let states: i64 = db.with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM oauth_states WHERE consumed_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0)
        });
        assert_eq!(states, 1, "one single-use state backs the URL");
        server.stop();
    }

    #[test]
    fn client_credentials_connect_reports_failures_in_app() {
        // The in-app answer to "are my credentials any good?" (1.1.4):
        // a provider pointed at an unreachable endpoint must land its
        // friendly error in the page, not in a browser tab.
        let db = fixture(Vec::new());
        let creds = spp_core::helpscout_real::HsCredentials {
            client_id: "cid-x".into(),
            client_secret: "secret-x".into(),
            api_base: "http://127.0.0.1:1".into(), // nothing listens here
            ..Default::default()
        };
        let mut page = SettingsPage::new();
        page.set_provider_handle(std::sync::Arc::new(
            spp_core::provider_handle::ProviderHandle::new_real(std::sync::Arc::new(
                spp_core::helpscout_real::RealHelpScoutProvider::new(db.clone(), creds),
            )),
        ));
        page.connect_client_credentials();
        assert!(page.hs_connecting, "the connect runs in the background");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while page.hs_connecting && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
            page.poll_connect_result(&db);
        }
        assert!(!page.hs_connecting, "the connect must finish");
        assert!(
            page.error
                .as_deref()
                .is_some_and(|e| e.contains("rejected the login credentials")),
            "the failure must be reported in-app with the friendly message: {:?}",
            page.error
        );
    }

    #[test]
    fn r3_saving_credentials_activates_the_provider_in_session() {
        // D2 regression test: saving credentials used to demand a
        // restart; now the shared handle swaps to the real provider
        // immediately and the R2 transition policy resets a poisoned
        // state machine.
        install_master_key();
        let db = fixture(Vec::new());
        let handle = std::sync::Arc::new(spp_core::provider_handle::ProviderHandle::new_empty());
        // A poisoned pre-1.4.0 state: the empty session stamped LIVE.
        db.with_write(|conn| {
            spp_core::sync_engine::set_state(conn, "LIVE");
            let _ = spp_core::settings::set_string(conn, "provider_kind", "empty");
        });
        let mut page = SettingsPage::new();
        page.set_provider_handle(std::sync::Arc::clone(&handle));
        page.hs_client_id = "cid-activate".into();
        page.hs_client_secret = "secret-activate".into();
        page.save_helpscout_credentials(&db);
        assert!(
            handle.is_real(),
            "the provider must be live immediately after the save"
        );
        assert!(handle.real().is_some());
        let (state, kind) = db.with_read(|conn| {
            (
                spp_core::sync_engine::get_state(conn),
                spp_core::settings::get_string(conn, "provider_kind")
                    .ok()
                    .flatten(),
            )
        });
        assert_eq!(state, "NEW", "the poisoned LIVE watermark is reset (R2)");
        assert_eq!(kind.as_deref(), Some("real"));
        // The doctor + sync kick are live-shell side effects (this bare
        // test page has no bus), so nothing touched the network here.
        assert!(
            !page.doctor_running,
            "bare pages skip the network side effects"
        );
    }

    /// Clean-audit batch 18 (item 11): a restore runs against a
    /// file-backed database only through the armed two-step path, a
    /// verified backup lands, and an unverified file is refused with
    /// the live database left untouched.
    #[test]
    fn restore_backup_lands_a_verified_file_and_refuses_junk() {
        let tmp = std::env::temp_dir().join(format!(
            "spp-restore-page-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&tmp).expect("tmp dir");
        let db_path = tmp.join("live.db");
        let mut file_conn = rusqlite::Connection::open(&db_path).expect("file db");
        spp_core::bootstrap::apply_all(&mut file_conn).expect("migrations");
        drop(file_conn);
        let db = DbHandle::new(rusqlite::Connection::open(&db_path).expect("reopen file db"));

        let mut page = SettingsPage::new();
        // The first click arms; the confirm card runs restore_backup —
        // simulate having armed and confirmed.
        page.restore_armed = None;

        // 1) A real backup restores cleanly.
        let backups_dir = tmp.join("backups");
        let result = db.with_read(|conn| spp_core::backup_service::backup(conn, &backups_dir));
        assert!(result.ok, "backup for the test: {}", result.message);
        let backup_path = result.path.expect("backup path");
        page.restore_backup(&db, &backup_path);
        assert!(
            page.message
                .as_deref()
                .is_some_and(|m| m.contains("restored") && m.contains("restart")),
            "success reports the restart requirement: {:?}",
            page.message
        );

        // 2) An unverified file is refused before the live db is touched.
        page.message = None;
        page.error = None;
        let junk = tmp.join("junk.db");
        std::fs::write(&junk, b"definitely not a database").expect("junk");
        page.restore_backup(&db, junk.to_str().expect("path"));
        assert!(
            page.error
                .as_deref()
                .is_some_and(|e| e.contains("integrity") || e.contains("refused")),
            "the refusal is reported in-app: {:?}",
            page.error
        );
        let still_live: bool = db.with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='app_state'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false)
        });
        assert!(still_live, "the live database survived the refusal");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}

/// Clean-audit item 31: saving a changed `api_concurrency` reaches the
/// LIVE provider's queues in-session — the setting used to be parsed and
/// persisted while every queue ran the hard-coded default until restart
/// (and the restart didn't matter either: nothing ever applied it).
#[cfg(test)]
mod batch22_audit31_tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn api_concurrency_save_reaches_the_live_provider() {
        let db = fixture(Vec::new());
        // The boot-time provider (build_boot_services shape): default 2.
        let real = std::sync::Arc::new(spp_core::helpscout_real::RealHelpScoutProvider::new(
            db.clone(),
            spp_core::helpscout_real::HsCredentials::default(),
        ));
        assert_eq!(real.api_concurrency(), 2, "unset: the default");

        let mut page = SettingsPage::new();
        page.set_provider_handle(std::sync::Arc::new(
            spp_core::provider_handle::ProviderHandle::new_real(std::sync::Arc::clone(&real)),
        ));

        // Load the form, change the concurrency, save.
        page.reload(&db);
        page.general.as_mut().expect("loaded").api_concurrency = 6;
        page.save_general(&db);

        assert!(
            page.error.is_none(),
            "the save must succeed: {:?}",
            page.error
        );
        assert_eq!(
            real.api_concurrency(),
            6,
            "the live provider's queues resized in-session"
        );
        // And the write persisted: a fresh provider starts at the new width.
        let rebuilt = spp_core::helpscout_real::RealHelpScoutProvider::new(
            db.clone(),
            spp_core::helpscout_real::HsCredentials::default(),
        );
        assert_eq!(
            rebuilt.api_concurrency(),
            6,
            "construction applies the stored setting"
        );
    }
}

/// Clean-audit item 77: a sync invalidation must never revert unsaved
/// General-tab edits — the pre-fix `invalidate` dropped the form
/// (`general = None`), so the next render's reload silently re-read the
/// persisted values over whatever the user was typing.
#[cfg(test)]
mod batch27_audit77_tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn invalidation_keeps_unsaved_general_edits() {
        let db = fixture(Vec::new());
        let mut page = SettingsPage::new();

        // First load: the form mirrors the persisted settings.
        page.ensure_loaded(&db);
        let loaded_interval = page.general.as_ref().expect("loaded").sync_interval_minutes;

        // The user types an edit (say the sync interval) and saves nothing.
        page.general.as_mut().expect("form").sync_interval_minutes = 720;

        // A sync event lands → the shell invalidates the page.
        page.invalidate();

        // Next render: mirrors re-read, the unsaved edit survives.
        page.ensure_loaded(&db);
        assert_eq!(
            page.general
                .as_ref()
                .expect("form kept")
                .sync_interval_minutes,
            720,
            "the unsaved edit survives the invalidation + reload"
        );
        assert!(
            page.helpscout.is_some(),
            "the connection-status mirror still refreshed"
        );
        assert!(
            !page.stale,
            "the pending invalidation was consumed by the reload"
        );

        // And the DB still holds the persisted value (nothing was saved).
        assert_eq!(
            db.with_read(|conn| { spp_core::settings::get_i64(conn, "sync_interval_minutes", 1) })
                .unwrap_or(1),
            loaded_interval,
            "the invalidation saved nothing behind the user's back"
        );
    }

    #[test]
    fn invalidation_refreshes_a_clean_form_from_the_db() {
        let db = fixture(Vec::new());
        let mut page = SettingsPage::new();
        page.ensure_loaded(&db);

        // Another surface persists a new interval while the form is clean.
        db.with_write(|conn| spp_core::settings::set_i64(conn, "sync_interval_minutes", 45))
            .expect("write");

        page.invalidate();
        page.ensure_loaded(&db);

        assert_eq!(
            page.general.as_ref().expect("form").sync_interval_minutes,
            45,
            "a clean form still takes the fresh read"
        );
    }

    #[test]
    fn saved_edits_clear_the_dirty_guard() {
        let db = fixture(Vec::new());
        let mut page = SettingsPage::new();
        page.ensure_loaded(&db);

        // Edit + save: the snapshot must catch up so a later
        // invalidation takes fresh reads instead of pinning the form.
        page.general.as_mut().expect("form").api_concurrency = 4;
        page.save_general(&db);
        assert!(page.error.is_none(), "save failed: {:?}", page.error);

        // A DB-side change after the save must reach the form.
        db.with_write(|conn| spp_core::settings::set_i64(conn, "sync_interval_minutes", 30))
            .expect("write");
        page.invalidate();
        page.ensure_loaded(&db);

        let general = page.general.as_ref().expect("form");
        assert_eq!(
            general.sync_interval_minutes, 30,
            "fresh read after save (form was clean)"
        );
        assert_eq!(
            general.api_concurrency, 4,
            "the SAVED edit is the persisted value now"
        );
    }
}
