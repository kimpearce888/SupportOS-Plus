use std::path::PathBuf;

use eframe::egui;
use spp_core::db::DbHandle;

use super::{NavRequest, PageKey, human_bytes, i64_field, relative_time, str_field};
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GeneralSettings {
    pub sync_interval_minutes: i64,
    pub api_concurrency: i64,
    pub ai_enabled: bool,
    pub automatic_analysis_enabled: bool,
    pub automatic_note_enabled: bool,
    pub automatic_draft_enabled: bool,
    pub automation_enabled: bool,
    pub automation_write_actions_enabled: bool,
    pub attachment_auto_download: bool,
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
            reply_status_after: "pending".into(),
            log_level: "info".into(),
            agent_language: "en".into(),
        }
    }
}

impl GeneralSettings {
    #[must_use]
    pub(crate) fn load(conn: &rusqlite::Connection) -> Self {
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
            attachment_auto_download: spp_core::settings::get_bool(
                conn,
                "attachment_auto_download",
                defaults.attachment_auto_download,
            )
            .unwrap_or(defaults.attachment_auto_download),
            reply_status_after: spp_core::settings::get_string(conn, "reply_status_after")
                .ok()
                .flatten()
                .map(|v| match v.as_str() {
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
        spp_core::settings::set_bool(
            conn,
            "attachment_auto_download",
            self.attachment_auto_download,
        )?;
        spp_core::settings::set_string(conn, "reply_status_after", &self.reply_status_after)?;
        spp_core::settings::set_string(conn, "log_level", &self.log_level)?;
        spp_core::settings::set_string(conn, "agent_language", &self.agent_language)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HelpScoutStatus {
    pub configured: bool,
    pub credentials: bool,
    pub sources: spp_core::helpscout_real::CredentialSources,
    pub redirect_uri: String,
}

#[must_use]
pub(crate) fn helpscout_status(conn: &rusqlite::Connection) -> HelpScoutStatus {
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

#[derive(Debug, Clone)]
pub(crate) struct BusinessHoursRow {
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

type HoursConfigRow = (String, String, i64, i64, Option<i64>, Option<i64>);

#[must_use]
pub(crate) fn load_business_hours(conn: &rusqlite::Connection) -> Vec<BusinessHoursRow> {
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

fn parse_weekdays(raw: &str) -> spp_core::Result<String> {
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

pub(crate) fn save_business_hours(
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

pub struct SettingsPage {
    tab: Tab,
    general: Option<GeneralSettings>,
    general_persisted: Option<GeneralSettings>,
    stale: bool,
    helpscout: Option<HelpScoutStatus>,
    authorize_url: Option<String>,
    hs_client_id: String,
    hs_client_secret: String,
    hs_webhook_secret: String,
    hs_docs_api_key: String,
    hs_form_loaded: bool,
    hs_show_secrets: bool,
    webhook_public_url: String,
    wh_loaded: bool,
    wh_outcome: Option<spp_core::webhook_health::WebhookHealthOutcome>,
    data_dir: Option<PathBuf>,
    purge_armed: bool,
    restore_armed: Option<String>,
    provider: Option<std::sync::Arc<spp_core::provider_handle::ProviderHandle>>,
    engine: Option<std::sync::Arc<spp_core::sync_engine::SyncEngine>>,
    bus: Option<spp_core::event_bus::EventBus>,
    hs_connect_result: std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>,
    hs_connecting: bool,
    doctor_result:
        std::sync::Arc<std::sync::Mutex<Option<Vec<spp_core::connection_doctor::DoctorStep>>>>,
    doctor_running: bool,
    doctor_steps: Vec<spp_core::connection_doctor::DoctorStep>,
    lmstudio_base_url: String,
    lmstudio_chat_model: String,
    lmstudio_embedding_model: String,
    lmstudio_persisted: (String, String, String),
    vector_store_path: String,
    hours: Vec<BusinessHoursRow>,
    hours_edit: Option<BusinessHoursRow>,
    backups: Vec<serde_json::Value>,
    backups_dir: Option<PathBuf>,
    workers: Option<std::sync::Arc<crate::workers_host::WorkersHost>>,
    update_result: std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>,
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
            lmstudio_persisted: (String::new(), String::new(), String::new()),
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

    pub(crate) fn set_data_dir(&mut self, dir: PathBuf) {
        self.data_dir = Some(dir);
    }

    /// Screenshot-harness hook (ux-audit evidence): select a settings tab by
    /// its user-facing label, case-insensitively. Feature-gated — the env
    /// var is only read in `ui-screenshot` builds, never in normal ones.
    /// Returns false when no tab label matches, so the caller can warn.
    #[cfg(feature = "ui-screenshot")]
    pub(crate) fn set_tab_by_label(&mut self, label: &str) -> bool {
        for tab in Tab::ALL {
            if tab.label().eq_ignore_ascii_case(label) {
                self.tab = tab;
                return true;
            }
        }
        false
    }

    pub(crate) fn set_backups_dir(&mut self, dir: PathBuf) {
        self.backups_dir = Some(dir);
    }

    pub(crate) fn set_workers(
        &mut self,
        workers: Option<std::sync::Arc<crate::workers_host::WorkersHost>>,
    ) {
        self.workers = workers;
    }

    pub(crate) fn set_provider_handle(
        &mut self,
        handle: std::sync::Arc<spp_core::provider_handle::ProviderHandle>,
    ) {
        self.provider = Some(handle);
    }

    pub(crate) fn set_sync_engine(
        &mut self,
        engine: Option<std::sync::Arc<spp_core::sync_engine::SyncEngine>>,
    ) {
        self.engine = engine;
    }

    pub(crate) fn set_bus(&mut self, bus: spp_core::event_bus::EventBus) {
        self.bus = Some(bus);
    }

    pub(crate) fn invalidate(&mut self) {
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
        if !self.hs_form_loaded {
            let stored = db.with_read(spp_core::helpscout_real::HsCredentials::from_envelope);
            self.hs_client_id = stored.client_id;
            self.hs_client_secret = stored.client_secret;
            self.hs_webhook_secret = stored.webhook_secret;
            self.hs_docs_api_key = stored.docs_api_key;
            self.hs_form_loaded = true;
        }
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
        let lmstudio_dirty = (
            self.lmstudio_base_url.as_str(),
            self.lmstudio_chat_model.as_str(),
            self.lmstudio_embedding_model.as_str(),
        ) != (
            self.lmstudio_persisted.0.as_str(),
            self.lmstudio_persisted.1.as_str(),
            self.lmstudio_persisted.2.as_str(),
        );
        if !lmstudio_dirty {
            self.lmstudio_base_url = base.clone();
            self.lmstudio_chat_model = chat.clone();
            self.lmstudio_embedding_model = embed.clone();
            self.lmstudio_persisted = (base, chat, embed);
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
        if self.general.is_none() || self.stale {
            self.reload(db);
            self.stale = false;
        }
    }

    fn save_general(&mut self, db: &DbHandle) {
        let Some(general) = self.general.clone() else {
            return;
        };
        let interval_before = db.with_read(|conn| {
            spp_core::settings::get_i64(conn, "sync_interval_minutes", 1).unwrap_or(1)
        });
        let concurrency_before = db
            .with_read(|conn| spp_core::settings::get_i64(conn, "api_concurrency", 2).unwrap_or(2));
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
        // UX-23: validate before persisting. An empty URL is legal (the app
        // falls back to its default endpoint), but a non-empty value that is
        // not an http(s) URL is the classic misconfiguration — saving it
        // silently used to just move the failure to the first AI request.
        if !base.is_empty() && !(base.starts_with("http://") || base.starts_with("https://")) {
            self.message = None;
            self.error = Some(format!(
                "the base URL must start with http:// or https:// — got \u{201c}{base}\u{201d}"
            ));
            return;
        }
        let result = db.with_write(|conn| {
            spp_core::settings::set_string(conn, "lmstudio_base_url", &base)?;
            spp_core::settings::set_string(conn, "lmstudio_chat_model", &chat)?;
            spp_core::settings::set_string(conn, "lmstudio_embedding_model", &embed)?;
            spp_core::error::Result::Ok(())
        });
        match result {
            Ok(()) => {
                self.lmstudio_persisted = (base, chat, embed);
                self.message = Some("LM Studio settings saved".into());
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn disconnect_helpscout(&mut self, db: &DbHandle) {
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
        // AU2-11: the tokens are gone — the in-session provider must be
        // deactivated too, or every queued call keeps firing with the dead
        // handle and lands 401s until restart (the same transition
        // clear_helpscout_credentials already performs).
        if let Some(handle) = self.provider.clone() {
            let previous_kind = handle.deactivate();
            db.with_write(|conn| {
                spp_core::sync_engine::on_provider_kind_transition(
                    conn,
                    Some(previous_kind.as_str()),
                    "empty",
                );
                let _ = spp_core::settings::set_string(conn, "provider_kind", "empty");
                let _ = spp_core::audit::log_event(
                    conn,
                    "info",
                    "connect",
                    "Help Scout disconnected: tokens deleted, provider deactivated in-session",
                    None,
                    None,
                );
            });
        }
        self.message = Some("Disconnected. Local data is fully preserved.".into());
        self.reload(db);
    }

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
                // UX-26B: the app now verifies webhook signatures with the
                // FULL stored secret (Help Scout imposes no length cap — the
                // audit's live probe proved it stores 48+ characters), so
                // the old "keep it at 40 characters or fewer" warning is
                // gone: any length works as long as both sides match.
                self.message =
                    Some("credentials stored (encrypted) — activating the connection now…".into());
                self.hs_client_secret.clear();
                self.hs_webhook_secret.clear();
                self.hs_docs_api_key.clear();
                self.activate_saved_credentials(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.reload(db);
    }

    fn activate_saved_credentials(&mut self, db: &DbHandle) {
        let Some(handle) = self.provider.clone() else {
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
        if self.bus.is_some() {
            self.message = Some(
                "credentials active — running the Connection Doctor and starting the sync…".into(),
            );
            self.run_connection_doctor();
            self.kick_sync_after_activation(db);
        }
    }

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
                            && let Some(bus) = &bus
                        {
                            spp_core::event_bus::notify_error(
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
                        if let Ok(mut guard) = slot.lock() {
                            *guard = Some(steps);
                        }
                    }
                    Err(e) => {
                        if let Some(bus) = &bus {
                            spp_core::event_bus::notify_error(
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

    fn reset_workspace(&mut self, db: &DbHandle) {
        // AU2-11: pause the background workers BEFORE the wipe — otherwise the
        // auto-sync timer re-pulls the account within a minute while the
        // message claims the re-pull happens on next start.
        if let Some(workers) = &self.workers {
            workers.pause();
        }
        let result = db.with_write(|conn| {
            spp_core::data_reset::reset_workspace_data(conn, self.data_dir.as_deref())
        });
        match result {
            Ok(report) => {
                self.message = Some(format!(
                    "workspace data deleted: {} rows across {} tables, {} vector \
                     entries. Background sync is paused — restart SupportOS+ so \
                     every view reloads clean{}.",
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

    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        self.ensure_loaded(db);

        design::page_header(
            ui,
            "Settings",
            Some(
                "Workspace preferences, connections, and local data — everything stays on this machine",
            ),
            |_| {},
        );
        if let Some(i) = design::tabs(
            ui,
            &[
                Tab::General.label(),
                Tab::HelpScout.label(),
                Tab::LmStudio.label(),
                Tab::VectorIndex.label(),
                Tab::Hours.label(),
                Tab::Backups.label(),
            ],
            Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0),
        ) && let Some(tab) = Tab::ALL.get(i)
        {
            // AU2-11: feedback belongs to the tab that produced it — a stale
            // message/error must not glue above every tab until restart.
            if self.tab != *tab {
                self.message = None;
                self.error = None;
            }
            self.tab = *tab;
            self.reload(db);
        }
        if self.message.is_some() || self.error.is_some() {
            ui.horizontal(|ui| {
                if let Some(message) = &self.message {
                    design::success_text(ui, message);
                }
                if let Some(err) = &self.error {
                    design::error_text(ui, &format!("error: {err}"));
                }
                if ui.small_button("Dismiss").clicked() {
                    self.message = None;
                    self.error = None;
                }
            });
        }
        ui.add_space(4.0);

        match self.tab {
            Tab::General => {
                self.general_ui(ui, db);
                None
            }
            Tab::HelpScout => {
                self.helpscout_ui(ui, db);
                None
            }
            // UX-23: settings can now navigate (the LM Studio tab's "Open
            // AI Center" act) — the page returns the request to the shell.
            Tab::LmStudio => self.lmstudio_ui(ui, db),
            Tab::VectorIndex => {
                self.vector_index_ui(ui, db);
                None
            }
            Tab::Hours => {
                self.hours_ui(ui, db);
                None
            }
            Tab::Backups => {
                self.backups_ui(ui, db);
                None
            }
        }
    }

    fn general_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let Some(mut general) = self.general.clone() else {
            ui.spinner();
            return;
        };
        design::card_titled_wide(ui, "Synchronization", None, |ui| {
            design::form_grid(ui, "settings-general", |ui| {
                // UX-30: explicit input sizes — the default-width DragValue
                // was a cramped two-digit sliver that read as static text.
                let interval_label = design::field_label(ui, "Max quiet sync interval (minutes)");
                ui.add_sized(
                    [110.0, 26.0],
                    egui::DragValue::new(&mut general.sync_interval_minutes).range(1..=1440),
                )
                .labelled_by(interval_label.id)
                .on_hover_text(
                    "The sync polls every 30 s while the account is busy, then \
                     backs off by doubling toward this ceiling when quiet. Every \
                     send triggers an immediate poll.",
                );
                ui.end_row();
                let concurrency_label = design::field_label(ui, "Api concurrency");
                ui.add_sized(
                    [110.0, 26.0],
                    egui::DragValue::new(&mut general.api_concurrency).range(1..=8),
                )
                .labelled_by(concurrency_label.id)
                .on_hover_text("How many Help Scout API requests may run in parallel.");
                ui.end_row();
                let log_label = design::field_label(ui, "Log level");
                let combo = egui::ComboBox::from_id_salt("settings-log-level")
                    .width(160.0)
                    .selected_text(general.log_level.clone())
                    .show_ui(ui, |ui| {
                        for level in ["debug", "info", "warn", "error"] {
                            ui.selectable_value(&mut general.log_level, level.into(), level);
                        }
                    });
                combo.response.labelled_by(log_label.id).on_hover_text(
                    "Drives the real logging filter — applied at boot from the \
                     saved value and immediately on save.",
                );
                ui.end_row();
                let lang_label = design::field_label(ui, "Agent language");
                ui.add_sized(
                    [220.0, 28.0],
                    egui::TextEdit::singleline(&mut general.agent_language),
                )
                .labelled_by(lang_label.id);
                ui.end_row();
                let reply_label = design::field_label(ui, "Status after reply");
                let reply_combo = egui::ComboBox::from_id_salt("settings-reply-status-after")
                    .width(190.0)
                    .selected_text(general.reply_status_after.clone())
                    .show_ui(ui, |ui| {
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
                reply_combo.response.labelled_by(reply_label.id);
                ui.end_row();
            });
        });
        ui.add_space(10.0);
        design::card_titled_wide(
            ui,
            "AI behaviour",
            Some("safe defaults — review before enabling write actions"),
            |ui| {
                // UX-30/UXE-1-3: one consistent control list. The old
                // striped grid alternated row backgrounds and the plain
                // checkbox rows read as static "✓" text; every row is
                // now a switch row on an even rhythm.
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
                    ui.horizontal(|ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            egui::RichText::new(label)
                                .size(design::scale::text::BODY)
                                .color(design::tokens(ui).foreground),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let next = design::switch(ui, *value);
                            if next != *value {
                                *value = next;
                            }
                        });
                    });
                    ui.add_space(4.0);
                }
                ui.add_space(6.0);
            },
        );
        ui.add_space(10.0);
        self.poll_update_result();
        design::card_titled_wide(ui, "About & updates", None, |ui| {
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
            } else if design::button(ui, "Check for Updates", ButtonVariant::Outline).clicked() {
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
        // UX-30: the save row now reflects form state. A dirty form offers
        // "Save Changes" with an unsaved-changes note; a clean one confirms
        // "All changes saved" — the old permanently-active "Save Settings"
        // button was easy to miss and gave no feedback either way.
        let dirty = self.general_persisted.as_ref() != Some(&general);
        ui.horizontal(|ui| {
            if dirty {
                if design::button(ui, "Save Changes", ButtonVariant::Primary).clicked() {
                    self.general = Some(general.clone());
                    self.save_general(db);
                }
                design::hint_text(ui, "You have unsaved changes on this tab.");
            } else {
                design::badge(ui, "All changes saved", BadgeTone::Success);
            }
        });
        self.general = Some(general);
    }

    fn helpscout_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.poll_connect_result(db);
        self.poll_doctor_result();
        let Some(status) = self.helpscout.clone() else {
            ui.spinner();
            return;
        };
        match spp_core::loopback::bound_port() {
            Some(port) => design::hint_text(
                ui,
                &format!(
                    "loopback listener answering on :{port} (webhook ingress + OAuth callback)"
                ),
            ),
            None => design::error_text(
                ui,
                "loopback listener NOT running — webhook ingress and the OAuth \
                 callback are unavailable; check the startup log for the bind error",
            ),
        }
        ui.add_space(4.0);
        design::card_titled_wide(
            ui,
            "Help Scout connection",
            Some(
                "Connect with client credentials (recommended — no browser, \
                 no redirect URL) or the advanced browser flow",
            ),
            |ui| {
                design::form_grid(ui, "settings-helpscout", |ui| {
                    design::field_label(ui, "Credentials stored");
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
                            if status.configured {
                                BadgeTone::Success
                            } else {
                                BadgeTone::Neutral
                            },
                        );
                    });
                    ui.end_row();
                    design::field_label(ui, "Redirect URI");
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
                        && design::button(ui, "Disconnect", ButtonVariant::Destructive)
                            .on_hover_text("drops the stored tokens; local data is fully preserved")
                            .clicked()
                    {
                        self.disconnect_helpscout(db);
                    }
                    if !status.configured
                        && status.credentials
                        && !self.hs_connecting
                        && design::button(
                            ui,
                            "Connect with Client Credentials (Recommended)",
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
                // UX-25: the browser authorization flow must stay reachable
                // whenever credentials exist — gating it on `!configured`
                // made it vanish exactly when the app was connected, and
                // after a disconnect the provider is gone, so the only way
                // in was racing an in-flight connect.
                if status.credentials {
                    ui.add_space(4.0);
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
                                 platform. This flow signs in as your Help Scout \
                                 user; the client-credentials path above is the \
                                 recommended default.",
                            );
                            ui.add_space(4.0);
                            if design::button(
                                ui,
                                "Open the Authorization Page in the Browser",
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

        self.connection_doctor_ui(ui);
        ui.add_space(10.0);

        design::card_titled_wide(
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
                    let cid_label = design::field_label(ui, "Client id");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.hs_client_id)
                            .hint_text("Help Scout custom app — client ID"),
                    )
                    .labelled_by(cid_label.id);
                    ui.end_row();
                    let secret_label = design::field_label(ui, "Client secret");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.hs_client_secret)
                            .password(mask)
                            .hint_text("leave blank to keep the stored value"),
                    )
                    .labelled_by(secret_label.id);
                    ui.end_row();
                    let whs_label = design::field_label(ui, "Webhook secret");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.hs_webhook_secret)
                            .password(mask)
                            .hint_text("verifies incoming Help Scout webhooks"),
                    )
                    .labelled_by(whs_label.id)
                    .on_hover_text(
                        // UX-26B: verification uses the full secret, same as
                        // Help Scout — any length works, keep both sides
                        // identical.
                        "Help Scout signs each delivery with the full secret you \
                         set there, and SupportOS+ verifies against the full \
                         secret too — paste the exact same value on both sides.",
                    );
                    ui.end_row();
                    let docs_label = design::field_label(ui, "Docs API key");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.hs_docs_api_key)
                            .password(mask)
                            .hint_text("optional — leave blank to keep the stored value"),
                    )
                    .labelled_by(docs_label.id);
                    ui.end_row();
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let next = design::switch(ui, self.hs_show_secrets);
                    if next != self.hs_show_secrets {
                        self.hs_show_secrets = next;
                    }
                    ui.label("Show secrets");
                    if design::button(ui, "Save Credentials", ButtonVariant::Primary).clicked() {
                        self.save_helpscout_credentials(db);
                    }
                    let any_stored = status.sources.client_id.is_set()
                        || status.sources.client_secret.is_set()
                        || status.sources.webhook_secret.is_set()
                        || status.sources.docs_api_key.is_set();
                    if any_stored
                        && design::button(
                            ui,
                            "Clear Stored Credentials",
                            ButtonVariant::Destructive,
                        )
                        .clicked()
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

        design::card_titled_wide(
            ui,
            "Webhook relay & health",
            Some(
                "the listener stays loopback-only by design; a user-run relay \
                 bridges the NAT, and the health check re-registers the hook \
                 when the platform silently drops it",
            ),
            |ui| {
                let relay_label = design::field_label(ui, "Public relay URL");
                ui.add(
                    egui::TextEdit::singleline(&mut self.webhook_public_url)
                        .hint_text("https://your-relay → forwards to this app's loopback listener"),
                )
                .labelled_by(relay_label.id);
                ui.horizontal(|ui| {
                    if design::button(ui, "Save Relay URL", ButtonVariant::Outline).clicked() {
                        self.save_webhook_relay_url(db);
                    }
                    if design::button(ui, "Check Now", ButtonVariant::Primary).clicked() {
                        self.queue_webhook_health_check(db);
                    }
                });
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
                            design::field_label(ui, "Last check");
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

        design::card_titled_wide(
            ui,
            "Danger zone — Workspace data",
            Some("irreversible"),
            |ui| {
                design::hint_text(
                    ui,
                    "Deletes every conversation, customer, thread, rating, \
                     knowledge document, incident, report and AI record — all \
                     synced content (and anything left over from the seeded \
                     content that shipped before 1.2.0) — and arms a fresh first sync. \
                     Keeps: credentials, settings, business hours, automation \
                     rules, saved views and replies, custom object types.",
                );
                if !self.purge_armed {
                    if design::button(ui, "Delete All Workspace Data", ButtonVariant::Destructive)
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
                        if design::button(ui, "Cancel", ButtonVariant::Outline).clicked() {
                            self.purge_armed = false;
                        }
                        if design::button(
                            ui,
                            "Really Delete Everything",
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

    fn connection_doctor_ui(&mut self, ui: &mut egui::Ui) {
        design::card_titled_wide(
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
                        && design::button(ui, "Run the Checks", ButtonVariant::Outline)
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
                            let dt = design::tokens(ui);
                            for step in &steps {
                                let (tone, label) = match step.status {
                                    DoctorStatus::Pass => (dt.success, "✓ ok"),
                                    DoctorStatus::Warn => (dt.warning, "! warning"),
                                    DoctorStatus::Fail => (dt.destructive, "✗ failed"),
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

    fn update_snapshot(&self) -> Option<Result<String, String>> {
        self.update_result
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

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

    fn begin_oauth_at(
        &mut self,
        db: &DbHandle,
        creds: &spp_core::helpscout_real::HsCredentials,
        redirect_uri: &str,
    ) {
        let bound = spp_core::loopback::bound_port()
            .or_else(|| spp_core::loopback::port_from_redirect_uri(redirect_uri));
        self.begin_oauth_on(db, creds, redirect_uri, bound);
    }

    fn begin_oauth_on(
        &mut self,
        db: &DbHandle,
        creds: &spp_core::helpscout_real::HsCredentials,
        redirect_uri: &str,
        bound: Option<u16>,
    ) {
        let Some(bound_port) = bound else {
            self.error = Some(
                "the SupportOS+ callback listener is not running — webhooks and \
                 the OAuth callback are unavailable. Restart SupportOS+ and \
                 check the log for the listener bind error."
                    .into(),
            );
            self.authorize_url = None;
            return;
        };
        let effective_redirect = match spp_core::loopback::port_from_redirect_uri(redirect_uri) {
            Some(port) if port == bound_port => redirect_uri.to_string(),
            Some(_) => format!("http://localhost:{bound_port}/oauth/callback"),
            None => redirect_uri.to_string(),
        };
        if !spp_core::loopback::listener_answers(bound_port) {
            self.error = Some(format!(
                "the SupportOS+ callback listener is not answering on port \
                 {bound_port}. Either SupportOS+ was not restarted after the \
                 credentials were saved, or another program is using the \
                 port. Restart SupportOS+ and try again; if the port is \
                 taken, close that program (or set HELPSCOUT_REDIRECT_URI \
                 to a free port and register the same URL on your Help \
                 Scout app)."
            ));
            self.authorize_url = None;
            return;
        }
        match spp_core::loopback::begin_oauth(db, creds, &effective_redirect) {
            Ok(url) => {
                self.message = Some(format!(
                    "Authorization URL ready — the callback listener answers on \
                     :{bound_port}. Register {effective_redirect} EXACTLY as the \
                     callback URL on your Help Scout app if it differs from what \
                     is registered. Complete the flow in the browser; the loopback \
                     listener stores the token."
                ));
                self.authorize_url = Some(url);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

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
                            if let Some(bus) = &bus {
                                spp_core::event_bus::notify_error(
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

    fn lmstudio_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        let mut nav = None;
        design::card_titled_wide(
            ui,
            "LM Studio (local AI gateway)",
            Some(
                "Point SupportOS+ at a local LM Studio server to draft replies, \
                 summarize threads and power semantic search on this machine — \
                 nothing goes to a hosted service. An empty URL falls back to \
                 the default endpoint (http://127.0.0.1:1234).",
            ),
            |ui| {
                design::form_grid(ui, "settings-lmstudio", |ui| {
                    let base_label = design::field_label(ui, "Base URL");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.lmstudio_base_url)
                            .hint_text("http://localhost:1234/v1"),
                    )
                    .labelled_by(base_label.id)
                    .on_hover_text(
                        "Start the server in LM Studio (Developer tab) and paste \
                         its local address here — usually http://localhost:1234/v1.",
                    );
                    ui.end_row();
                    let chat_label = design::field_label(ui, "Chat model");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.lmstudio_chat_model)
                            .hint_text("e.g. qwen2.5-7b-instruct"),
                    )
                    .labelled_by(chat_label.id)
                    .on_hover_text(
                        "The model name exactly as LM Studio lists it — used for \
                         draft replies, notes and summaries.",
                    );
                    ui.end_row();
                    let embed_label = design::field_label(ui, "Embedding model");
                    ui.add_sized(
                        [320.0, 28.0],
                        egui::TextEdit::singleline(&mut self.lmstudio_embedding_model)
                            .hint_text("e.g. text-embedding-nomic-embed-text-v1.5"),
                    )
                    .labelled_by(embed_label.id)
                    .on_hover_text(
                        "The model that powers semantic search. It can stay empty \
                         if you only use keyword search.",
                    );
                    ui.end_row();
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if design::button(ui, "Save", ButtonVariant::Primary).clicked() {
                        self.save_lmstudio(db);
                    }
                    // UX-23: the old subtitle was a dead end — "Test the
                    // connection from the AI Center page" with no way to get
                    // there. The test now sits one click from the fields it
                    // tests.
                    if design::button(ui, "Open AI Center", ButtonVariant::Outline)
                        .on_hover_text(
                            "The AI Center page shows the connection status and \
                             runs a live test against this URL",
                        )
                        .clicked()
                    {
                        nav = Some(NavRequest::Page(PageKey::AiCenter));
                    }
                });
            },
        );
        nav
    }

    fn vector_index_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card_titled_wide(
            ui,
            "Vector index",
            Some(
                "Semantic search stores document vectors in a local database \
                 beside your app data. Keep this on for meaning-based search; \
                 keyword search still works when it is off. Changes apply on \
                 the next start.",
            ),
            |ui| {
                if self.vector_store_path.is_empty() {
                    self.vector_store_path = db
                        .file_path()
                        .and_then(|p| p.parent().map(|d| d.join("vectors").join("vectors.db")))
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                }
                design::form_grid(ui, "settings-vector-index", |ui| {
                    design::field_label(ui, "Store");
                    ui.horizontal(|ui| {
                        // UX-24: the path truncates instead of overflowing on
                        // narrow viewports, copies in one click, and shows
                        // the full location on hover — it was a raw
                        // unbounded monospace line before.
                        let path = if self.vector_store_path.is_empty() {
                            "(unknown data directory)".to_string()
                        } else {
                            self.vector_store_path.clone()
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(path.as_str())
                                    .size(12.5)
                                    .color(design::tokens(ui).foreground),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&path);
                        if !self.vector_store_path.is_empty()
                            && design::button(ui, "Copy Path", ButtonVariant::Ghost).clicked()
                        {
                            ui.ctx().output_mut(|o| {
                                o.commands.push(egui::OutputCommand::CopyText(
                                    self.vector_store_path.clone(),
                                ));
                            });
                        }
                    });
                    ui.end_row();
                    let prev = db
                        .with_read(|conn| {
                            spp_core::settings::get_bool(conn, "qdrant_enabled", true)
                        })
                        .unwrap_or(true);
                    let enabled = prev;
                    // UX-24/UXE-1-3: a labelled switch row. The bare
                    // checkbox read as static "✓ vector index enabled"
                    // text in the audit; the label plus the hover
                    // explanation make the control — and what it
                    // switches — obvious.
                    design::field_label(ui, "Semantic search");
                    let mut next = prev;
                    ui.horizontal(|ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            egui::RichText::new("Enabled")
                                .size(design::scale::text::BODY)
                                .color(design::tokens(ui).foreground),
                        )
                        .on_hover_text(
                            "Dense vectors are embedded and searched while enabled; searches \
                             fall back to FTS5 when disabled. Applies on the next start — \
                             the workers open the store at boot.",
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            next = design::switch(ui, enabled);
                        });
                    });
                    ui.end_row();
                    if next != prev {
                        let result = db.with_write(|conn| {
                            spp_core::settings::set_bool(conn, "qdrant_enabled", next)
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
        // UX-29: the card explains what business hours actually drive — it
        // was a bare table of mailboxes with no purpose statement.
        design::card_titled_wide(
            ui,
            "Business hours per mailbox",
            Some(
                "On-time targets per mailbox — reports and coaching use these \
                 windows to judge whether a first reply or a resolution was \
                 fast enough. Mailboxes appear here after the first Help Scout \
                 sync.",
            ),
            |ui| {
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
                                    // UX-29: spell out the abbreviations —
                                    // "fr 480m · res 1440m" read as noise.
                                    design::hint_text(
                                        ui,
                                        &format!(
                                            "{} · {} · {}-{} · first response {} min · resolution {} min",
                                            row.timezone,
                                            row.days,
                                            minutes_label(row.start_minute),
                                            minutes_label(row.end_minute),
                                            row.first_response_target_min,
                                            row.resolution_target_min
                                        ),
                                    );
                                } else {
                                    // UX-29: a warning tone — "not configured"
                                    // is a to-do, not a neutral fact, since
                                    // reports fall back to defaults until it
                                    // is set.
                                    design::badge(ui, "not configured", BadgeTone::Warning);
                                }
                                if design::button(ui, "Edit", ButtonVariant::Outline).clicked() {
                                    edit = Some(row.clone());
                                }
                            });
                            ui.add_space(2.0);
                        }
                        if hours.is_empty() {
                            design::hint_text(
                                ui,
                                "No mailboxes yet — they appear here after the first Help Scout sync.",
                            );
                        }
                    });
                if let Some(row) = edit {
                    self.hours_edit = Some(row);
                }
            },
        );

        let Some(mut row) = self.hours_edit.clone() else {
            return;
        };
        ui.add_space(10.0);
        design::card_titled_wide(ui, &format!("Edit: {}", row.name), None, |ui| {
            design::form_grid(ui, "settings-hours-edit", |ui| {
                let tz_label = design::field_label(ui, "Timezone");
                ui.add_sized([220.0, 28.0], egui::TextEdit::singleline(&mut row.timezone))
                    .labelled_by(tz_label.id);
                ui.end_row();
                let days_label = design::field_label(ui, "Days (0=Sun…6=Sat)");
                ui.add_sized([220.0, 28.0], egui::TextEdit::singleline(&mut row.days))
                    .labelled_by(days_label.id);
                ui.end_row();
                let start_label = design::field_label(ui, "Start (minutes)");
                ui.add(egui::DragValue::new(&mut row.start_minute).range(0..=1440))
                    .labelled_by(start_label.id);
                ui.end_row();
                let end_label = design::field_label(ui, "End (minutes)");
                ui.add(egui::DragValue::new(&mut row.end_minute).range(0..=1440))
                    .labelled_by(end_label.id);
                ui.end_row();
                let first_resp_label = design::field_label(ui, "First response target (min)");
                ui.add(egui::DragValue::new(&mut row.first_response_target_min).range(1..=10_000))
                    .labelled_by(first_resp_label.id);
                ui.end_row();
                let resolution_label = design::field_label(ui, "Resolution target (min)");
                ui.add(egui::DragValue::new(&mut row.resolution_target_min).range(1..=100_000))
                    .labelled_by(resolution_label.id);
                ui.end_row();
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if design::button(ui, "Save Hours", ButtonVariant::Primary).clicked() {
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
                if design::button(ui, "Cancel", ButtonVariant::Ghost).clicked() {
                    self.hours_edit = None;
                }
            });
        });
        if self.hours_edit.is_some() {
            self.hours_edit = Some(row);
        }
    }

    fn backups_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        // UX-32: the tab no longer opens with a card that repeats its own
        // name. Two purposeful cards instead: safe actions (create/export)
        // above, restore in its own card below — separated like the Help
        // Scout tab's danger zone, so the destructive control never sits
        // beside the export buttons it used to share a row with.
        design::card_titled_wide(ui, "Back up or export your data", None, |ui| {
            ui.horizontal(|ui| {
                if design::button_with_icon(
                    ui,
                    Icon::Download,
                    "Create Backup",
                    ButtonVariant::Primary,
                )
                .on_hover_text("a full copy of the local database, kept as a restore point")
                .clicked()
                {
                    self.create_backup(db);
                }
                // UX-32: the exports wore the flat Secondary gray, which
                // read as disabled next to the primary button — Outline
                // (bordered, like every secondary action in the header)
                // keeps them clearly enabled.
                if design::button(ui, "Export JSON", ButtonVariant::Outline)
                    .on_hover_text(
                        "the whole workspace as one JSON file — for keeping \
                             or moving data elsewhere",
                    )
                    .clicked()
                {
                    self.export_json(db);
                }
                if design::button(ui, "Export Conversations CSV", ButtonVariant::Outline)
                    .on_hover_text("one row per conversation — for spreadsheets")
                    .clicked()
                {
                    self.export_csv(db);
                }
            });
        });
        ui.add_space(10.0);
        let backups = self.backups.clone();
        design::card_titled_wide(
            ui,
            "Restore a backup",
            Some("replaces the current database"),
            |ui| {
                if backups.is_empty() {
                    design::hint_text(ui, "No backups yet — create one above.");
                    return;
                }
                egui::ScrollArea::vertical()
                    .id_salt("settings-backups")
                    .show(ui, |ui| {
                        let t = design::tokens(ui);
                        for backup in &backups {
                            ui.horizontal(|ui| {
                                ui.monospace(
                                    egui::RichText::new(str_field(backup, "file"))
                                        .color(t.foreground),
                                );
                                // UX-22: metadata in user terms — "2.0 MB" and
                                // "3h ago" instead of raw bytes and an RFC3339
                                // stamp; the exact values stay one hover away.
                                let size_bytes = i64_field(backup, "size_bytes");
                                ui.label(
                                    egui::RichText::new(human_bytes(size_bytes))
                                        .small()
                                        .color(design::tokens(ui).muted_foreground),
                                )
                                .on_hover_text(format!("{size_bytes} bytes"));
                                if let Some(created) =
                                    backup.get("created_at").and_then(|a| a.as_str())
                                {
                                    let rel = relative_time(created);
                                    let label = if rel == created {
                                        created.to_string()
                                    } else {
                                        rel
                                    };
                                    ui.label(
                                        egui::RichText::new(label)
                                            .small()
                                            .color(design::tokens(ui).muted_foreground),
                                    )
                                    .on_hover_text(created);
                                }
                                // UX-22: restore replaces the live database — a
                                // destructive action, so it wears the danger
                                // variant and arms a confirmation below (never a
                                // one-click apply).
                                if design::button(ui, "Restore", ButtonVariant::Destructive)
                                    .on_hover_text(
                                        "replaces the current database with this backup \
                                         (a confirmation step follows) — restart the app \
                                         afterwards",
                                    )
                                    .clicked()
                                    && let Some(dir) = &self.backups_dir
                                {
                                    let path = dir.join(str_field(backup, "file"));
                                    self.restore_armed = Some(path.display().to_string());
                                }
                            });
                            ui.add_space(2.0);
                        }
                    });
                if let Some(path) = self.restore_armed.clone() {
                    ui.add_space(6.0);
                    design::error_text(
                        ui,
                        &format!(
                            "Restore this backup? The current database will be REPLACED \
                             by \u{201c}{path}\u{201d} (the file is verified first). Restart \
                             SupportOS+ afterwards to use it.",
                            path = path.rsplit('/').next().unwrap_or(&path),
                        ),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if design::button(ui, "Cancel", ButtonVariant::Outline).clicked() {
                            self.restore_armed = None;
                        }
                        if design::button(ui, "Really Restore", ButtonVariant::Destructive)
                            .clicked()
                        {
                            self.restore_armed = None;
                            self.restore_backup(db, &path);
                        }
                    });
                }
            },
        );
    }
}

impl Default for SettingsPage {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub(crate) fn minutes_label(minutes: i64) -> String {
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

        let stored = db.with_read(spp_core::helpscout_real::HsCredentials::from_envelope);
        assert_eq!(stored.client_id, "cid-app");
        assert_eq!(stored.client_secret, "secret-app");

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
    fn begin_oauth_rewrites_the_recorded_uri_to_the_bound_port() {
        let db = fixture(Vec::new());
        let mut server = spp_core::loopback::LoopbackServer::start(
            db.clone(),
            std::sync::Arc::new(spp_core::provider_handle::ProviderHandle::new_empty()),
            0,
        )
        .expect("listener");
        let bound = server.port();
        let redirect_uri = "http://localhost:3000/oauth/callback";
        assert_ne!(
            spp_core::loopback::port_from_redirect_uri(redirect_uri),
            Some(bound),
            "the fixture must exercise the mismatch"
        );
        let creds = spp_core::helpscout_real::HsCredentials {
            client_id: "cid-live".into(),
            client_secret: "secret-live".into(),
            redirect_uri: redirect_uri.to_string(),
            ..Default::default()
        };

        let mut page = SettingsPage::new();
        page.begin_oauth_on(&db, &creds, redirect_uri, Some(bound));

        let url = page.authorize_url.clone().expect("authorization URL");
        assert!(page.error.is_none(), "{:?}", page.error);
        let state = url.split("state=").nth(1).unwrap_or_default().to_string();
        let recorded = db.with_read(|conn| {
            spp_core::oauth_state::redirect_uri_for(conn, &state).expect("redirect lookup")
        });
        assert_eq!(
            recorded.as_deref(),
            Some(format!("http://localhost:{bound}/oauth/callback").as_str()),
            "the recorded URI must be rewritten to the bound port"
        );
        server.stop();
    }

    #[test]
    fn client_credentials_connect_reports_failures_in_app() {
        let db = fixture(Vec::new());
        let creds = spp_core::helpscout_real::HsCredentials {
            client_id: "cid-x".into(),
            client_secret: "secret-x".into(),
            api_base: "http://127.0.0.1:1".into(),
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
        install_master_key();
        let db = fixture(Vec::new());
        let handle = std::sync::Arc::new(spp_core::provider_handle::ProviderHandle::new_empty());
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
        assert!(
            !page.doctor_running,
            "bare pages skip the network side effects"
        );
    }

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
        page.restore_armed = None;

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

#[cfg(test)]
mod batch22_audit31_tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn api_concurrency_save_reaches_the_live_provider() {
        let db = fixture(Vec::new());
        let real = std::sync::Arc::new(spp_core::helpscout_real::RealHelpScoutProvider::new(
            db.clone(),
            spp_core::helpscout_real::HsCredentials::default(),
        ));
        assert_eq!(real.api_concurrency(), 2, "unset: the default");

        let mut page = SettingsPage::new();
        page.set_provider_handle(std::sync::Arc::new(
            spp_core::provider_handle::ProviderHandle::new_real(std::sync::Arc::clone(&real)),
        ));

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

#[cfg(test)]
mod batch27_audit77_tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn invalidation_keeps_unsaved_general_edits() {
        let db = fixture(Vec::new());
        let mut page = SettingsPage::new();

        page.ensure_loaded(&db);
        let loaded_interval = page.general.as_ref().expect("loaded").sync_interval_minutes;

        page.general.as_mut().expect("form").sync_interval_minutes = 720;

        page.invalidate();

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

        page.general.as_mut().expect("form").api_concurrency = 4;
        page.save_general(&db);
        assert!(page.error.is_none(), "save failed: {:?}", page.error);

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

    #[test]
    fn lmstudio_invalidation_refreshes_a_clean_form_from_the_db() {
        let db = fixture(Vec::new());
        let mut page = SettingsPage::new();
        page.ensure_loaded(&db);

        db.with_write(|conn| {
            spp_core::settings::set_string(conn, "lmstudio_base_url", "http://192.168.1.10:1234")
        })
        .expect("write");

        page.invalidate();
        page.ensure_loaded(&db);

        assert_eq!(
            page.lmstudio_base_url, "http://192.168.1.10:1234",
            "a clean form still takes the fresh read"
        );
        assert_eq!(
            page.lmstudio_persisted.0, "http://192.168.1.10:1234",
            "the snapshot re-bases on the fresh read"
        );
    }

    #[test]
    fn lmstudio_dirty_form_survives_invalidation() {
        let db = fixture(Vec::new());
        let mut page = SettingsPage::new();
        page.ensure_loaded(&db);

        page.lmstudio_base_url = "http://localhost:9999".into();
        page.lmstudio_chat_model = "qwen2.5-7b-instruct".into();
        page.invalidate();
        page.ensure_loaded(&db);

        assert_eq!(
            page.lmstudio_base_url, "http://localhost:9999",
            "unsaved edits survive a reload (pre-fix the is_empty gate re-pulled stored values over them)"
        );
        assert_eq!(page.lmstudio_chat_model, "qwen2.5-7b-instruct");

        page.save_lmstudio(&db);
        assert!(page.error.is_none(), "save failed: {:?}", page.error);

        db.with_write(|conn| {
            spp_core::settings::set_string(conn, "lmstudio_base_url", "http://other:1234")
        })
        .expect("write");
        page.invalidate();
        page.ensure_loaded(&db);

        assert_eq!(
            page.lmstudio_base_url, "http://other:1234",
            "after save the form is clean, so the fresh read wins"
        );
        assert_eq!(
            page.lmstudio_chat_model, "qwen2.5-7b-instruct",
            "the SAVED edit is the persisted value now"
        );
    }
}
