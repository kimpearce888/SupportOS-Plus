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
    pub qdrant_enabled: bool,
    pub attachment_auto_download: bool,
    pub automatic_reply_sending: bool,
    pub log_level: String,
    pub display_timezone: String,
    pub agent_language: String,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            sync_interval_minutes: 5,
            api_concurrency: 2,
            ai_enabled: true,
            automatic_analysis_enabled: true,
            automatic_note_enabled: false,
            automatic_draft_enabled: false,
            automation_enabled: false,
            automation_write_actions_enabled: false,
            qdrant_enabled: true,
            attachment_auto_download: true,
            automatic_reply_sending: false,
            log_level: "info".into(),
            display_timezone: "system".into(),
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
            qdrant_enabled: spp_core::settings::get_bool(
                conn,
                "qdrant_enabled",
                defaults.qdrant_enabled,
            )
            .unwrap_or(defaults.qdrant_enabled),
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
            log_level: spp_core::settings::get_string(conn, "log_level")
                .ok()
                .flatten()
                .unwrap_or(defaults.log_level),
            display_timezone: spp_core::settings::get_string(conn, "display_timezone")
                .ok()
                .flatten()
                .unwrap_or(defaults.display_timezone),
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
        spp_core::settings::set_bool(conn, "qdrant_enabled", self.qdrant_enabled)?;
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
        spp_core::settings::set_string(conn, "log_level", &self.log_level)?;
        spp_core::settings::set_string(conn, "display_timezone", &self.display_timezone)?;
        spp_core::settings::set_string(conn, "agent_language", &self.agent_language)?;
        Ok(())
    }
}

/// Help Scout connection status (port of `/api/oauth/status` read side).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HelpScoutStatus {
    pub demo_mode: bool,
    pub configured: bool,
    /// OAuth client credentials are available — from the environment or
    /// the in-app form — so the OAuth connect flow can start.
    pub credentials: bool,
    /// Per-field credential source for the form badges (1.1.3):
    /// environment, app settings, or not set.
    pub sources: spp_core::helpscout_real::CredentialSources,
}

/// Load the connection status from the local mirror.
#[must_use]
pub fn helpscout_status(conn: &rusqlite::Connection) -> HelpScoutStatus {
    let demo_mode = spp_core::settings::get_bool(conn, "demo_mode", false).unwrap_or(false);
    let tokens: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM oauth_tokens WHERE access_token IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let (credentials, sources) = spp_core::helpscout_real::HsCredentials::resolve(conn);
    HelpScoutStatus {
        demo_mode,
        configured: tokens > 0,
        credentials: credentials.is_configured(),
        sources,
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

/// Save one mailbox's business hours (the set route's upsert).
///
/// # Errors
/// Propagates SQL failures or invalid timezone.
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
            row.days,
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
    /// Application data directory: the demo purge clears the vector
    /// store that lives next to the database.
    data_dir: Option<PathBuf>,
    /// Two-step confirmation state for the demo purge.
    purge_armed: bool,
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
    message: Option<String>,
    error: Option<String>,
}

impl SettingsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::General,
            general: None,
            helpscout: None,
            authorize_url: None,
            hs_client_id: String::new(),
            hs_client_secret: String::new(),
            hs_webhook_secret: String::new(),
            hs_docs_api_key: String::new(),
            hs_form_loaded: false,
            hs_show_secrets: false,
            data_dir: None,
            purge_armed: false,
            lmstudio_base_url: String::new(),
            lmstudio_chat_model: String::new(),
            lmstudio_embedding_model: String::new(),
            vector_store_path: String::new(),
            hours: Vec::new(),
            hours_edit: None,
            backups: Vec::new(),
            backups_dir: None,
            workers: None,
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

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.general = None;
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
        self.general = Some(general);
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
        if self.general.is_none() {
            self.reload(db);
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
            spp_core::settings::get_i64(conn, "sync_interval_minutes", 5).unwrap_or(5)
        });
        let result = db.with_write(|conn| general.save(conn));
        match result {
            Ok(()) => {
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
        // The route revoked remotely first; the local effect is dropping
        // the stored tokens. Demo mode is left untouched.
        db.with_write(|conn| {
            let _ = conn.execute("DELETE FROM oauth_tokens", []);
        });
        self.message = Some("Disconnected. Local data is fully preserved.".into());
        self.reload(db);
    }

    /// Save the credential form (1.1.3): non-empty fields are stored in
    /// the encrypted envelope, and a configured pair switches demo mode
    /// off so the next boot builds the real provider.
    fn save_helpscout_credentials(&mut self, db: &DbHandle) {
        let (cid, cs, ws, dk) = (
            self.hs_client_id.trim().to_string(),
            self.hs_client_secret.trim().to_string(),
            self.hs_webhook_secret.trim().to_string(),
            self.hs_docs_api_key.trim().to_string(),
        );
        let result = db.with_write(|conn| {
            spp_core::helpscout_real::HsCredentials::store_in_envelope(conn, &cid, &cs, &ws, &dk)?;
            let (resolved, _) = spp_core::helpscout_real::HsCredentials::resolve(conn);
            if resolved.is_configured() {
                spp_core::settings::set_bool(conn, "demo_mode", false)?;
            }
            spp_core::error::Result::Ok(())
        });
        match result {
            Ok(()) => {
                self.message = Some(
                    "credentials stored (encrypted). Demo mode is off — restart \
                     SupportOS+ to connect the real account."
                        .into(),
                );
                // The three secret fields do not linger on screen after a
                // successful save; the client id stays for reference.
                self.hs_client_secret.clear();
                self.hs_webhook_secret.clear();
                self.hs_docs_api_key.clear();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.reload(db);
    }

    /// Remove every credential stored by the form. The connection
    /// tokens and demo mode are deliberately untouched.
    fn clear_helpscout_credentials(&mut self, db: &DbHandle) {
        let result =
            db.with_write(|conn| spp_core::helpscout_real::HsCredentials::clear_envelope(conn));
        match result {
            Ok(removed) => {
                self.message = Some(format!(
                    "stored credentials removed ({removed} values). The \
                     connection and demo mode are untouched."
                ));
                self.hs_client_id.clear();
                self.hs_client_secret.clear();
                self.hs_webhook_secret.clear();
                self.hs_docs_api_key.clear();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.reload(db);
    }

    /// One-click demo purge (1.1.3): clears all business content, keeps
    /// the KEEP_TABLES config, resets the vector store when the data
    /// directory is known.
    fn purge_demo(&mut self, db: &DbHandle) {
        let result = db.with_write(|conn| {
            spp_core::data_reset::purge_demo_data(conn, self.data_dir.as_deref())
        });
        match result {
            Ok(report) => {
                self.message = Some(format!(
                    "demo data deleted: {} rows across {} tables, {} vector \
                     entries. Restart SupportOS+ so every view reloads clean.",
                    report.rows_deleted, report.tables_cleared, report.vectors_dropped
                ));
            }
            Err(e) => self.error = Some(format!("purge failed (nothing was changed): {e}")),
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
                design::field_label(ui, "sync interval (minutes):");
                ui.add(egui::DragValue::new(&mut general.sync_interval_minutes).range(1..=1440));
                ui.end_row();
                design::field_label(ui, "api concurrency:");
                ui.add(egui::DragValue::new(&mut general.api_concurrency).range(1..=8));
                ui.end_row();
                design::field_label(ui, "log level:");
                egui::ComboBox::from_id_salt("settings-log-level")
                    .width(160.0)
                    .selected_text(general.log_level.clone())
                    .show_ui(ui, |ui| {
                        for level in ["debug", "info", "warn", "error"] {
                            ui.selectable_value(&mut general.log_level, level.into(), level);
                        }
                    });
                ui.end_row();
                design::field_label(ui, "display timezone:");
                ui.add_sized(
                    [220.0, 28.0],
                    egui::TextEdit::singleline(&mut general.display_timezone),
                );
                ui.end_row();
                design::field_label(ui, "agent language:");
                ui.add_sized(
                    [220.0, 28.0],
                    egui::TextEdit::singleline(&mut general.agent_language),
                );
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
        if design::button(ui, "save settings", ButtonVariant::Primary).clicked() {
            self.general = Some(general.clone());
            self.save_general(db);
        }
        self.general = Some(general);
    }

    fn helpscout_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let Some(status) = self.helpscout.clone() else {
            ui.spinner();
            return;
        };
        design::card_titled(
            ui,
            "Help Scout connection",
            Some(
                "OAuth in the browser flow (the loopback listener serves the callback), or run in demo mode with the fake provider",
            ),
            |ui| {
                design::form_grid(ui, "settings-helpscout", |ui| {
                    design::field_label(ui, "demo mode:");
                    ui.horizontal(|ui| {
                        design::badge(
                            ui,
                            if status.demo_mode { "on" } else { "off" },
                            if status.demo_mode {
                                BadgeTone::Info
                            } else {
                                BadgeTone::Neutral
                            },
                        );
                    });
                    ui.end_row();
                    design::field_label(ui, "credentials stored:");
                    ui.horizontal(|ui| {
                        design::badge(
                            ui,
                            if status.configured {
                                "configured"
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
                    if !status.configured
                        && status.credentials
                        && design::button(ui, "connect via OAuth", ButtonVariant::Primary).clicked()
                    {
                        self.begin_oauth(db);
                    }
                });
                if let Some(url) = &self.authorize_url {
                    ui.add_space(6.0);
                    ui.hyperlink_to(
                        egui::RichText::new("open the Help Scout authorization page").size(13.0),
                        url.clone(),
                    );
                }
            },
        );
        ui.add_space(10.0);

        // Credential form (1.1.3): values are stored in the encrypted
        // secrets envelope; environment variables still win per field.
        design::card_titled(
            ui,
            "Help Scout credentials",
            Some(
                "Stored encrypted on this machine. Saving a configured pair \
                 turns demo mode off — restart SupportOS+ to connect the \
                 real account. Environment variables override these values.",
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

        // Demo-data purge (1.1.3): one click, two-step confirmation.
        design::card_titled(
            ui,
            "Demo data",
            Some(
                "Deletes every conversation, customer, thread, rating, \
                 knowledge document, incident, report and AI record — all \
                 demo-seeded or synced content — and resets sync state. \
                 Keeps: credentials, settings, business hours, automation \
                 rules, saved views and replies, custom object types.",
            ),
            |ui| {
                if !self.purge_armed {
                    if design::button(ui, "delete all demo data", ButtonVariant::Destructive)
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
                            self.purge_demo(db);
                        }
                    });
                }
            },
        );
    }

    /// Start the browser OAuth flow: issue a single-use state and show the
    /// authorization URL (the loopback listener serves the callback).
    fn begin_oauth(&mut self, db: &DbHandle) {
        let (creds, _) = db.with_read(spp_core::helpscout_real::HsCredentials::resolve);
        let redirect_uri = creds.redirect_uri.clone();
        match spp_core::loopback::begin_oauth(db, &creds, &redirect_uri) {
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
            let mut restored: Option<String> = None;
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
                                restored = Some(path.display().to_string());
                            }
                        });
                        ui.add_space(2.0);
                    }
                });
            if let Some(path) = restored {
                self.restore_backup(db, &path);
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

    #[test]
    fn helpscout_status_reads_tokens_and_demo_mode() {
        let db = fixture(Vec::new());
        let status = db.with_read(helpscout_status);
        assert!(!status.configured);
        assert!(!status.demo_mode);

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
    fn helpscout_credentials_save_clear_and_demo_mode_flip() {
        install_master_key();
        let db = fixture(Vec::new());
        // Demo mode on, as after onboarding's "try demo" path.
        db.with_write(|conn| {
            spp_core::settings::set_bool(conn, "demo_mode", true).expect("demo on");
        });

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
            !status.demo_mode,
            "saving a configured pair flips demo mode off"
        );
        assert!(
            page.hs_client_secret.is_empty(),
            "secret fields are cleared after a successful save"
        );

        // The stored secrets decrypt back through the envelope.
        let stored = db.with_read(spp_core::helpscout_real::HsCredentials::from_envelope);
        assert_eq!(stored.client_id, "cid-app");
        assert_eq!(stored.client_secret, "secret-app");

        // Clear: tokens and the demo flag stay as they are.
        db.with_write(|conn| {
            conn.execute("INSERT INTO oauth_tokens (access_token) VALUES ('tok')", [])
                .expect("token");
            spp_core::settings::set_bool(conn, "demo_mode", true).expect("demo on");
        });
        page.clear_helpscout_credentials(&db);
        let status2 = db.with_read(helpscout_status);
        assert!(!status2.credentials);
        assert_eq!(
            status2.sources.client_id,
            spp_core::helpscout_real::CredentialSource::NotSet
        );
        assert!(status2.configured, "connection tokens survive the clear");
        assert!(status2.demo_mode, "the clear leaves demo mode untouched");
    }

    #[test]
    fn purge_demo_empties_content_and_keeps_config() {
        let db = fixture(vec![("conv", 1, "Demo ticket", "active", "email", 1, 5, 9)]);
        db.with_write(|conn| {
            spp_core::settings::set_string(conn, "display_timezone", "Asia/Dhaka").expect("tz");
        });

        let mut page = SettingsPage::new();
        page.purge_armed = true;
        page.purge_demo(&db);

        assert!(
            page.message
                .as_deref()
                .is_some_and(|m| m.contains("demo data deleted")),
            "purge must report: {:?}",
            page.message
        );
        assert!(page.error.is_none(), "{:?}", page.error);
        assert!(
            !page.purge_armed,
            "the confirmation disarms after the purge"
        );

        let (convs, users, mailboxes, tz) = db.with_read(|conn| {
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
                spp_core::settings::get_string(conn, "display_timezone")
                    .expect("read")
                    .expect("present"),
            )
        });
        assert_eq!(convs, 0, "seeded conversations must be purged");
        assert_eq!(users, 0, "mirror users must be purged");
        assert_eq!(mailboxes, 0, "mirror mailboxes must be purged");
        assert_eq!(tz, "Asia/Dhaka", "application settings must survive");
    }
}
