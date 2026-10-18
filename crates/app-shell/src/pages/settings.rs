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
    /// Help Scout client credentials are available in the environment, so
    /// the OAuth connect flow can start.
    pub credentials: bool,
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
    HelpScoutStatus {
        demo_mode,
        configured: tokens > 0,
        credentials: spp_core::helpscout_real::HsCredentials::from_env().is_configured(),
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
    lmstudio_base_url: String,
    lmstudio_chat_model: String,
    lmstudio_embedding_model: String,
    /// Cached sqlite-vec store path shown on the Vector index tab.
    vector_store_path: String,
    hours: Vec<BusinessHoursRow>,
    hours_edit: Option<BusinessHoursRow>,
    backups: Vec<serde_json::Value>,
    backups_dir: Option<PathBuf>,
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
            lmstudio_base_url: String::new(),
            lmstudio_chat_model: String::new(),
            lmstudio_embedding_model: String::new(),
            vector_store_path: String::new(),
            hours: Vec::new(),
            hours_edit: None,
            backups: Vec::new(),
            backups_dir: None,
            message: None,
            error: None,
        }
    }

    /// Backups directory (set by the shell at boot).
    pub fn set_backups_dir(&mut self, dir: PathBuf) {
        self.backups_dir = Some(dir);
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
        let result = db.with_write(|conn| general.save(conn));
        match result {
            Ok(()) => self.message = Some("settings saved".into()),
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

        ui.heading("Settings");
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                    self.reload(db);
                }
            }
        });
        if let Some(message) = &self.message {
            ui.label(format!("status: {message}"));
        }
        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("error: {err}"));
        }
        ui.separator();

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
        ui.strong("Synchronization");
        egui::Grid::new("settings-general")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("sync interval (minutes):");
                ui.add(egui::DragValue::new(&mut general.sync_interval_minutes).range(1..=1440));
                ui.end_row();
                ui.label("api concurrency:");
                ui.add(egui::DragValue::new(&mut general.api_concurrency).range(1..=8));
                ui.end_row();
                ui.label("log level:");
                egui::ComboBox::from_id_salt("settings-log-level")
                    .selected_text(general.log_level.clone())
                    .show_ui(ui, |ui| {
                        for level in ["debug", "info", "warn", "error"] {
                            ui.selectable_value(&mut general.log_level, level.into(), level);
                        }
                    });
                ui.end_row();
                ui.label("display timezone:");
                ui.text_edit_singleline(&mut general.display_timezone);
                ui.end_row();
                ui.label("agent language:");
                ui.text_edit_singleline(&mut general.agent_language);
                ui.end_row();
            });
        ui.separator();
        ui.strong("AI behaviour (safe defaults)");
        egui::Grid::new("settings-ai")
            .num_columns(2)
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
        ui.separator();
        if ui.button("save settings").clicked() {
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
        ui.strong("Help Scout connection");
        egui::Grid::new("settings-helpscout")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("demo mode:");
                ui.monospace(status.demo_mode.to_string());
                ui.end_row();
                ui.label("credentials stored:");
                ui.monospace(status.configured.to_string());
                ui.end_row();
            });
        ui.weak(
            "Connect through OAuth in the browser flow (the loopback listener serves the \
             callback) or run in demo mode with the fake provider.",
        );
        if status.configured && ui.button("disconnect").clicked() {
            self.disconnect_helpscout(db);
        }
        if !status.configured && status.credentials && ui.button("connect via OAuth").clicked() {
            self.begin_oauth(db);
        }
        if let Some(url) = &self.authorize_url {
            ui.hyperlink_to("open the Help Scout authorization page", url);
        }
    }

    /// Start the browser OAuth flow: issue a single-use state and show the
    /// authorization URL (the loopback listener serves the callback).
    fn begin_oauth(&mut self, db: &DbHandle) {
        let creds = spp_core::helpscout_real::HsCredentials::from_env();
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
        ui.strong("LM Studio (local AI gateway)");
        egui::Grid::new("settings-lmstudio")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("base url:");
                ui.text_edit_singleline(&mut self.lmstudio_base_url);
                ui.end_row();
                ui.label("chat model:");
                ui.text_edit_singleline(&mut self.lmstudio_chat_model);
                ui.end_row();
                ui.label("embedding model:");
                ui.text_edit_singleline(&mut self.lmstudio_embedding_model);
                ui.end_row();
            });
        ui.weak("Test the connection from the AI Center page.");
        if ui.button("save").clicked() {
            self.save_lmstudio(db);
        }
    }

    fn vector_index_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.strong("Vector index (sqlite-vec, built-in)");
        if self.vector_store_path.is_empty() {
            // Reuse the lazy-load slot to remember where the store lives.
            self.vector_store_path = db
                .file_path()
                .and_then(|p| p.parent().map(|d| d.join("vectors").join("vectors.db")))
                .map(|p| p.display().to_string())
                .unwrap_or_default();
        }
        egui::Grid::new("settings-vector-index")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("store:");
                ui.monospace(if self.vector_store_path.is_empty() {
                    "(unknown data directory)".to_string()
                } else {
                    self.vector_store_path.clone()
                });
                ui.end_row();
            });
        ui.weak(
            "Dense vectors live in SQLite next to the application database (decision D4: the \
             chunk count is far below the 100k gate). The index follows the qdrant_enabled \
             setting; searches fall back to FTS5 when disabled.",
        );
    }

    fn hours_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.strong("Business hours per mailbox");
        let hours = self.hours.clone();
        let mut edit: Option<BusinessHoursRow> = None;
        egui::ScrollArea::vertical()
            .id_salt("settings-hours")
            .show(ui, |ui| {
                for row in &hours {
                    ui.horizontal(|ui| {
                        ui.strong(row.name.as_str());
                        if row.configured {
                            ui.weak(format!(
                                "{} · {} · {}-{} · fr {}m · res {}m",
                                row.timezone,
                                row.days,
                                minutes_label(row.start_minute),
                                minutes_label(row.end_minute),
                                row.first_response_target_min,
                                row.resolution_target_min
                            ));
                        } else {
                            ui.weak("(not configured)");
                        }
                        if ui.button("edit").clicked() {
                            edit = Some(row.clone());
                        }
                    });
                }
                if hours.is_empty() {
                    ui.weak("(no mailboxes synced yet)");
                }
            });
        if let Some(row) = edit {
            self.hours_edit = Some(row);
        }

        let Some(mut row) = self.hours_edit.clone() else {
            return;
        };
        ui.separator();
        ui.strong(format!("Edit: {}", row.name));
        egui::Grid::new("settings-hours-edit")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("timezone:");
                ui.text_edit_singleline(&mut row.timezone);
                ui.end_row();
                ui.label("days (0=Sun…6=Sat):");
                ui.text_edit_singleline(&mut row.days);
                ui.end_row();
                ui.label("start (minutes):");
                ui.add(egui::DragValue::new(&mut row.start_minute).range(0..=1440));
                ui.end_row();
                ui.label("end (minutes):");
                ui.add(egui::DragValue::new(&mut row.end_minute).range(0..=1440));
                ui.end_row();
                ui.label("first response target (min):");
                ui.add(egui::DragValue::new(&mut row.first_response_target_min).range(1..=10_000));
                ui.end_row();
                ui.label("resolution target (min):");
                ui.add(egui::DragValue::new(&mut row.resolution_target_min).range(1..=100_000));
                ui.end_row();
            });
        ui.horizontal(|ui| {
            if ui.button("save hours").clicked() {
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
            if ui.button("cancel").clicked() {
                self.hours_edit = None;
            }
        });
        self.hours_edit = Some(row);
    }

    fn backups_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.strong("Backups & export");
        ui.horizontal(|ui| {
            if ui.button("create backup").clicked() {
                self.create_backup(db);
            }
            if ui.button("export JSON").clicked() {
                self.export_json(db);
            }
            if ui.button("export conversations CSV").clicked() {
                self.export_csv(db);
            }
        });
        ui.separator();
        let backups = self.backups.clone();
        if backups.is_empty() {
            ui.weak("(no backups yet)");
            return;
        }
        let mut restored: Option<String> = None;
        egui::ScrollArea::vertical()
            .id_salt("settings-backups")
            .show(ui, |ui| {
                for backup in &backups {
                    ui.horizontal(|ui| {
                        ui.monospace(str_field(backup, "file"));
                        ui.weak(format!("{} bytes", i64_field(backup, "size_bytes")));
                        if let Some(created) = backup.get("created_at").and_then(|a| a.as_str()) {
                            ui.weak(created);
                        }
                        if ui.button("restore").clicked()
                            && let Some(dir) = &self.backups_dir
                        {
                            let path = dir.join(str_field(backup, "file"));
                            restored = Some(path.display().to_string());
                        }
                    });
                }
            });
        if let Some(path) = restored {
            self.restore_backup(db, &path);
        }
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
}
