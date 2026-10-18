//! The eframe shell application.
//!
//! `App::logic` drains the background-task bridge and marks the affected
//! ported pages dirty (the same invalidation map the web UI used);
//! `App::ui` renders the sidebar navigation plus the active page —
//! dashboard, inbox, customers, knowledge/docs, or reports — and keeps
//! the Phase 2 shell diagnostics (theme toggle persisted in the settings
//! table, database status, live bridge counters) on the Shell page.

use std::path::PathBuf;

use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::http::EventBus;

use crate::ShellOptions;
use crate::bridge::TaskBridge;
use crate::pages::PageKey;
use crate::pages::customers::CustomersPage;
use crate::pages::dashboard::DashboardPage;
use crate::pages::inbox::InboxPage;
use crate::pages::knowledge::KnowledgePage;
use crate::pages::reports::ReportsPage;
use crate::theme::Theme;

/// The root application state of the native shell.
pub struct ShellApp {
    db: DbHandle,
    /// Kept alive so background workers can keep emitting after the
    /// HTTP server is replaced by in-process tasks (Phase 3+), and used
    /// by view-side actions to emit live-update events.
    bus: EventBus,
    bridge: TaskBridge,
    theme: Theme,
    db_path: Option<PathBuf>,
    schema_version: u32,
    events_seen: usize,
    last_event: Option<String>,
    page: PageKey,
    dashboard: DashboardPage,
    inbox: InboxPage,
    customers: CustomersPage,
    knowledge: KnowledgePage,
    reports: ReportsPage,
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
            bus: options.bus,
            bridge,
            theme,
            db_path,
            schema_version,
            events_seen: 0,
            last_event: None,
            page: PageKey::Dashboard,
            dashboard: DashboardPage::new(),
            inbox: InboxPage::new(),
            customers: CustomersPage::new(),
            knowledge: KnowledgePage::new(),
            reports: ReportsPage::new(),
        }
    }

    /// Apply an invalidation batch to the ported pages.
    fn apply_invalidations(&mut self, pages: &[PageKey]) {
        for page in pages {
            match page {
                PageKey::Dashboard => self.dashboard.invalidate(),
                PageKey::Inbox => self.inbox.invalidate(),
                PageKey::Customers => self.customers.invalidate(),
                PageKey::Knowledge => self.knowledge.invalidate(),
                PageKey::Reports => self.reports.invalidate(),
                PageKey::Shell => {}
            }
        }
    }

    fn shell_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Shell");
        ui.weak("native shell diagnostics (Phase 2 surface)");
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
            ui.weak("pages ported: dashboard, inbox, customers, knowledge/docs, reports");
        });
    }
}

impl eframe::App for ShellApp {
    /// Runs before every frame — and while the window is hidden, whenever
    /// the bridge requested a repaint. No painting is allowed here.
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let events = self.bridge.drain();
        self.events_seen += events.len();
        if let Some(last) = events.last() {
            self.last_event = Some(last.wire_name().to_string());
        }
        let mut dirty: Vec<PageKey> = Vec::new();
        for event in &events {
            for page in crate::pages::invalidated_pages(event) {
                if !dirty.contains(&page) {
                    dirty.push(page);
                }
            }
        }
        if !dirty.is_empty() {
            self.apply_invalidations(&dirty);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("spp-nav")
            .default_size(150.0)
            .show(ui, |ui| {
                ui.heading("SupportOs-Plus");
                ui.weak(format!("v{}", env!("CARGO_PKG_VERSION")));
                ui.separator();
                for key in PageKey::all() {
                    if ui.selectable_label(self.page == key, key.label()).clicked() {
                        self.page = key;
                    }
                }
            });

        let bus = self.bus.clone();
        egui::CentralPanel::default().show(ui, |ui| match self.page {
            PageKey::Dashboard => self.dashboard.ui(ui, &self.db),
            PageKey::Inbox => self.inbox.ui(ui, &self.db, &bus),
            PageKey::Customers => self.customers.ui(ui, &self.db),
            PageKey::Knowledge => self.knowledge.ui(ui, &self.db),
            PageKey::Reports => self.reports.ui(ui, &self.db),
            PageKey::Shell => self.shell_ui(ui),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::invalidated_pages;
    use spp_core::events::ServerEvent;

    #[test]
    fn invalidation_batch_marks_each_page_once() {
        // Two events that both invalidate the inbox must not double-apply.
        let events = vec![
            ServerEvent::sync_completed("incremental", 1, 0),
            ServerEvent::conversation_updated(Some(1), Some(1), None, None, "sync"),
        ];
        let mut dirty: Vec<PageKey> = Vec::new();
        for event in &events {
            for page in invalidated_pages(event) {
                if !dirty.contains(&page) {
                    dirty.push(page);
                }
            }
        }
        assert_eq!(dirty.iter().filter(|p| **p == PageKey::Inbox).count(), 1);
        assert!(dirty.contains(&PageKey::Dashboard));
    }
}
