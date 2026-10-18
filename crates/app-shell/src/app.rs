//! The eframe shell application (Phase 2 skeleton).
//!
//! `App::logic` drains the background-task bridge; `App::ui` renders the
//! placeholder home surface: identity, theme toggle (persisted in the
//! settings table), database status, and live bridge counters. Pages
//! replace the central panel contents one by one in Phase 3.

use std::path::PathBuf;

use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::http::EventBus;

use crate::ShellOptions;
use crate::bridge::TaskBridge;
use crate::theme::Theme;

/// The root application state of the native shell.
pub struct ShellApp {
    db: DbHandle,
    /// Kept alive so background workers can keep emitting after the
    /// HTTP server is replaced by in-process tasks (Phase 3+).
    _bus: EventBus,
    bridge: TaskBridge,
    theme: Theme,
    db_path: Option<PathBuf>,
    schema_version: u32,
    events_seen: usize,
    last_event: Option<String>,
}

impl ShellApp {
    /// Build the shell state. Never fails: theme/DB status problems
    /// degrade to defaults instead of refusing to start.
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>, options: ShellOptions) -> Self {
        let theme = Theme::load(&options.db);
        theme.apply(&cc.egui_ctx);
        let bridge = TaskBridge::spawn(&options.bus, &cc.egui_ctx);
        let (db_path, schema_version) = options.db.with_read(|conn| {
            (
                spp_core::db::db_file_path(conn),
                spp_core::db::latest_version(conn).unwrap_or(0),
            )
        });
        Self {
            db: options.db,
            _bus: options.bus,
            bridge,
            theme,
            db_path,
            schema_version,
            events_seen: 0,
            last_event: None,
        }
    }
}

impl eframe::App for ShellApp {
    /// Runs before every frame — and while the window is hidden, whenever
    /// the bridge requested a repaint. No painting is allowed here.
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let events = self.bridge.drain();
        if let Some(n) = events.len().checked_sub(0) {
            self.events_seen += n;
        }
        if let Some(last) = events.last() {
            self.last_event = Some(last.wire_name().to_string());
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("SupportOs-Plus");
            ui.weak(format!("native shell v{}", env!("CARGO_PKG_VERSION")));
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Theme:");
                if ui
                    .button(format!("switch to {}", self.theme.toggle().label()))
                    .clicked()
                {
                    self.theme = self.theme.toggle();
                    if let Err(e) = self.theme.store(&self.db) {
                        tracing::warn!(error = %e, "failed to persist the theme setting");
                    }
                    self.theme.apply(ui.ctx());
                }
            });
            ui.separator();

            let db_display = self
                .db_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<in-memory>".to_string());
            ui.monospace(format!("database: {db_display}"));
            ui.monospace(format!("schema: v{}", self.schema_version));
            ui.monospace(format!("background events: {}", self.events_seen));
            if let Some(last) = &self.last_event {
                ui.monospace(format!("last event: {last}"));
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.weak("placeholder shell — pages arrive in Phase 3");
            });
        });
    }
}
