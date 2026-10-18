//! The eframe shell application.
//!
//! `App::logic` drains the background-task bridge and marks the affected
//! ported pages dirty (the same invalidation map the web UI used);
//! `App::ui` renders the sidebar navigation plus the active page — the
//! full Phase 3 surface — and keeps the Phase 2 shell diagnostics (theme
//! toggle persisted in the settings table, database status, live bridge
//! counters) on the Shell page. Global overlays: the command palette
//! (Ctrl/Cmd+K) and the toast stack.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::eventing::EventBus;
use spp_core::sync_engine::SyncEngine;

use crate::ShellOptions;
use crate::bridge::TaskBridge;
use crate::overlays::{ConfirmDialog, DialogOutcome};
use crate::pages::NavRequest;
use crate::pages::PageKey;
use crate::pages::ai_center::AiCenterPage;
use crate::pages::automation::AutomationPage;
use crate::pages::connectors::ConnectorsPage;
use crate::pages::custom_objects::CustomObjectsPage;
use crate::pages::customers::CustomersPage;
use crate::pages::dashboard::DashboardPage;
use crate::pages::inbox::InboxPage;
use crate::pages::incidents::IncidentsPage;
use crate::pages::issue_radar::IssueRadarPage;
use crate::pages::knowledge::KnowledgePage;
use crate::pages::notifications::NotificationsPage;
use crate::pages::onboarding::OnboardingPage;
use crate::pages::operations::OperationsPage;
use crate::pages::organizations::OrganizationsPage;
use crate::pages::outreach::OutreachPage;
use crate::pages::reports::ReportsPage;
use crate::pages::search::SearchPage;
use crate::pages::settings::SettingsPage;
use crate::pages::support_graph::SupportGraphPage;
use crate::pages::sync_health::SyncHealthPage;
use crate::palette::CommandPalette;
use crate::theme::Theme;
use crate::toasts::Toasts;

/// The root application state of the native shell.
pub struct ShellApp {
    db: DbHandle,
    /// Kept alive so background workers can keep emitting after the
    /// HTTP server is replaced by in-process tasks (Phase 3+), and used
    /// by view-side actions to emit live-update events.
    bus: EventBus,
    /// The shared sync engine (helps the sync-health and onboarding pages
    /// run syncs; `None` mirrors the web ServiceUnavailable response).
    sync: Option<Arc<SyncEngine>>,
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
    operations: OperationsPage,
    ai_center: AiCenterPage,
    automation: AutomationPage,
    connectors: ConnectorsPage,
    custom_objects: CustomObjectsPage,
    incidents: IncidentsPage,
    issue_radar: IssueRadarPage,
    notifications: NotificationsPage,
    onboarding: OnboardingPage,
    organizations: OrganizationsPage,
    outreach: OutreachPage,
    search: SearchPage,
    settings: SettingsPage,
    support_graph: SupportGraphPage,
    sync_health: SyncHealthPage,
    palette: CommandPalette,
    toasts: Toasts,
    dialog: Option<ConfirmDialog>,
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
        let _data_dir = Some(options.data_dir.clone());
        let sync = options.sync;
        let mut settings = SettingsPage::new();
        settings.set_backups_dir(options.data_dir.join("backups"));
        let mut connectors = ConnectorsPage::new();
        connectors.set_data_dir(options.data_dir);
        Self {
            db: options.db,
            bus: options.bus,
            sync,
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
            operations: OperationsPage::new(),
            ai_center: AiCenterPage::new(),
            automation: AutomationPage::new(),
            connectors,
            custom_objects: CustomObjectsPage::new(),
            incidents: IncidentsPage::new(),
            issue_radar: IssueRadarPage::new(),
            notifications: NotificationsPage::new(),
            onboarding: OnboardingPage::new(),
            organizations: OrganizationsPage::new(),
            outreach: OutreachPage::new(),
            search: SearchPage::new(),
            settings,
            support_graph: SupportGraphPage::new(),
            sync_health: SyncHealthPage::new(),
            palette: CommandPalette::new(),
            toasts: Toasts::new(),
            dialog: None,
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
                PageKey::Operations => self.operations.invalidate(),
                PageKey::AiCenter => self.ai_center.invalidate(),
                PageKey::Automation => self.automation.invalidate(),
                PageKey::Connectors => self.connectors.invalidate(),
                PageKey::CustomObjects => self.custom_objects.invalidate(),
                PageKey::Incidents => self.incidents.invalidate(),
                PageKey::IssueRadar => self.issue_radar.invalidate(),
                PageKey::Notifications => self.notifications.invalidate(),
                PageKey::Onboarding => self.onboarding.invalidate(),
                PageKey::Organizations => self.organizations.invalidate(),
                PageKey::Outreach => self.outreach.invalidate(),
                PageKey::Search => self.search.invalidate(),
                PageKey::Settings => self.settings.invalidate(),
                PageKey::SupportGraph => self.support_graph.invalidate(),
                PageKey::SyncHealth => self.sync_health.invalidate(),
                PageKey::Shell => {}
            }
        }
    }

    /// Resolve a navigation request raised by a page or the palette.
    fn apply_nav(&mut self, nav: NavRequest) {
        match nav {
            NavRequest::InboxTile(tile) => {
                self.page = PageKey::Inbox;
                self.inbox.bar.ops = Some(tile);
                self.inbox.invalidate();
            }
            NavRequest::Page(page) => {
                self.page = page;
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
        let sync_kind = self
            .sync
            .as_ref()
            .map(|engine| {
                if engine.is_running() {
                    "running"
                } else {
                    "idle"
                }
            })
            .unwrap_or("unavailable");
        ui.monospace(format!("sync engine: {sync_kind}"));

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.weak("all pages ported: dashboard, inbox, operations, customers, organizations,");
            ui.weak("knowledge/docs, reports, issue radar, incidents, custom objects,");
            ui.weak("automation, AI center, outreach, notifications, search, support graph,");
            ui.weak("connectors, sync health, settings, onboarding");
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
                ui.separator();
                if ui
                    .button("⌘K search")
                    .on_hover_text("command palette (Ctrl/Cmd+K)")
                    .clicked()
                {
                    self.palette.open();
                }
            });

        // Global overlays first so they layer above the page content.
        if let Some(nav) = self.palette.ui(ui.ctx(), &self.db) {
            self.apply_nav(nav);
        }
        if let Some(dialog) = &mut self.dialog {
            let outcome = dialog.ui(ui.ctx());
            if outcome.is_done() {
                let dialog = self.dialog.take();
                if outcome == DialogOutcome::Confirmed {
                    // The confirm action ran when the requester armed the
                    // dialog; the toast records the decision.
                    if let Some(dialog) = dialog {
                        self.toasts
                            .push(crate::toasts::ToastKind::Info, dialog.title, None);
                    }
                }
            }
        }

        let bus = self.bus.clone();
        let sync = self.sync.clone();
        let mut nav: Option<NavRequest> = None;
        egui::CentralPanel::default().show(ui, |ui| match self.page {
            PageKey::Dashboard => self.dashboard.ui(ui, &self.db),
            PageKey::Inbox => self.inbox.ui(ui, &self.db, &bus),
            PageKey::Customers => self.customers.ui(ui, &self.db),
            PageKey::Knowledge => self.knowledge.ui(ui, &self.db),
            PageKey::Reports => self.reports.ui(ui, &self.db),
            PageKey::Operations => {
                nav = self.operations.ui(ui, &self.db);
            }
            PageKey::AiCenter => self.ai_center.ui(ui, &self.db),
            PageKey::Automation => self.automation.ui(ui, &self.db),
            PageKey::Connectors => self.connectors.ui(ui, &self.db),
            PageKey::CustomObjects => self.custom_objects.ui(ui, &self.db),
            PageKey::Incidents => self.incidents.ui(ui, &self.db),
            PageKey::IssueRadar => self.issue_radar.ui(ui, &self.db),
            PageKey::Notifications => self.notifications.ui(ui, &self.db),
            PageKey::Onboarding => self.onboarding.ui(ui, &self.db, sync.as_ref()),
            PageKey::Organizations => self.organizations.ui(ui, &self.db),
            PageKey::Outreach => self.outreach.ui(ui, &self.db),
            PageKey::Search => self.search.ui(ui, &self.db),
            PageKey::Settings => self.settings.ui(ui, &self.db),
            PageKey::SupportGraph => self.support_graph.ui(ui, &self.db),
            PageKey::SyncHealth => self.sync_health.ui(ui, &self.db, sync.as_ref()),
            PageKey::Shell => self.shell_ui(ui),
        });
        if let Some(nav) = nav {
            self.apply_nav(nav);
        }

        // Destructive-action confirmations arm the dialog (sample wiring:
        // deleting an automation rule).
        self.toasts.ui(ui.ctx());
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
        assert!(dirty.contains(&PageKey::Operations));
    }

    #[test]
    fn every_page_key_has_a_label_and_covers_the_full_surface() {
        assert_eq!(PageKey::all().len(), 21);
        for key in PageKey::all() {
            assert!(!key.label().is_empty());
        }
    }

    #[test]
    fn campaign_events_invalidate_outreach_and_notifications_arrive() {
        let event = ServerEvent::campaign_updated(1, "queued", 0, 0, 0, 0);
        assert_eq!(invalidated_pages(&event), vec![PageKey::Outreach]);

        let event = ServerEvent::notification_received(
            5,
            "sync",
            "info",
            "sync done",
            None,
            None,
            None,
            None,
            0,
        );
        let pages = invalidated_pages(&event);
        assert!(pages.contains(&PageKey::Notifications));
        assert!(pages.contains(&PageKey::Inbox));
    }
}
