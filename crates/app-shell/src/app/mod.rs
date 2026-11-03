use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::event_bus::EventBus;
use spp_core::sync_engine::SyncEngine;

use crate::ShellOptions;
use crate::design::{self, ButtonVariant};
use crate::icons::Icon;
use crate::overlays::{ConfirmDialog, DialogOutcome};
use crate::pages::DestructiveAction;
use crate::pages::NavRequest;
use crate::pages::PageKey;
use crate::pages::ai_center::AiCenterPage;
use crate::pages::automation::AutomationPage;
use crate::pages::connectors::ConnectorsPage;
use crate::pages::custom_objects::CustomObjectsPage;
use crate::pages::customers::CustomersPage;
use crate::pages::dashboard::DashboardPage;
use crate::pages::diagnostics::DiagnosticsPage;
use crate::pages::inbox::InboxPage;
use crate::pages::incidents::IncidentsPage;
use crate::pages::issue_radar::IssueRadarPage;
use crate::pages::knowledge::KnowledgePage;
use crate::pages::live_chats::LiveChatsPage;
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
use crate::task_bridge::TaskBridge;
use crate::theme::Theme;
use crate::toasts::Toasts;

pub(crate) struct ShellApp {
    db: DbHandle,
    bus: EventBus,
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
    live_chats: LiveChatsPage,
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
    diagnostics: DiagnosticsPage,
    sync_health: SyncHealthPage,
    palette: CommandPalette,
    toasts: Toasts,
    dialog: Option<ConfirmDialog>,
    pending_confirm: Option<DestructiveAction>,
}

impl ShellApp {
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>, options: ShellOptions) -> Self {
        Self::new_with_context(&cc.egui_ctx, options)
    }

    #[must_use]
    pub(crate) fn new_with_context(ctx: &egui::Context, options: ShellOptions) -> Self {
        design::install(ctx);
        let theme = Theme::load(&options.db);
        theme.apply(ctx);
        let bridge = TaskBridge::spawn(&options.bus, ctx);
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
        settings.set_data_dir(options.data_dir.clone());
        settings.set_workers(options.workers.clone());
        if let Some(handle) = options.handle.clone() {
            settings.set_provider_handle(handle);
        }
        settings.set_sync_engine(sync.clone());
        settings.set_bus(options.bus.clone());
        let mut connectors = ConnectorsPage::new();
        connectors.set_data_dir(options.data_dir.clone());
        let mut knowledge = KnowledgePage::new();
        knowledge.set_data_dir(options.data_dir);
        let vector_index = options.vector_index.clone();
        let mut inbox = InboxPage::new();
        inbox.set_vector_index(vector_index.clone());
        let mut search = SearchPage::new();
        search.set_vector_index(vector_index.clone());
        let mut ai_center = AiCenterPage::new();
        ai_center.set_vector_index(vector_index.clone());
        #[cfg_attr(
            not(feature = "ui-screenshot"),
            expect(unused_mut, reason = "only mutated by the ui-screenshot harness")
        )]
        let mut app = Self {
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
            inbox,
            live_chats: LiveChatsPage::new(),
            customers: CustomersPage::new(),
            knowledge,
            reports: ReportsPage::new(),
            operations: OperationsPage::new(),
            ai_center,
            automation: AutomationPage::new(),
            connectors,
            custom_objects: CustomObjectsPage::new(),
            incidents: IncidentsPage::new(),
            issue_radar: IssueRadarPage::new(),
            notifications: NotificationsPage::new(),
            onboarding: OnboardingPage::new(),
            organizations: OrganizationsPage::new(),
            outreach: OutreachPage::new(),
            search,
            settings,
            support_graph: SupportGraphPage::new(),
            diagnostics: DiagnosticsPage::new(),
            sync_health: SyncHealthPage::new(),
            palette: CommandPalette::new(),
            toasts: Toasts::new(),
            dialog: None,
            pending_confirm: None,
        };
        #[cfg(feature = "ui-screenshot")]
        app.apply_screenshot_env(ctx);
        app
    }

    pub(crate) fn select_page(&mut self, page: PageKey) {
        self.page = page;
    }

    fn apply_invalidations(&mut self, pages: &[PageKey]) {
        for page in pages {
            match page {
                PageKey::Dashboard => self.dashboard.invalidate(),
                PageKey::Inbox => self.inbox.invalidate(),
                PageKey::LiveChats => self.live_chats.invalidate(),
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
                PageKey::Diagnostics => self.diagnostics.invalidate(),
                PageKey::SyncHealth => self.sync_health.invalidate(),
                PageKey::Shell => {}
            }
        }
    }

    fn apply_nav(&mut self, nav: NavRequest) {
        match nav {
            NavRequest::InboxTile(tile) => {
                self.page = PageKey::Inbox;
                self.inbox.bar = crate::pages::inbox::FilterBar {
                    ops: Some(tile),
                    ..crate::pages::inbox::FilterBar::default()
                };
                self.inbox.invalidate();
            }
            NavRequest::Page(page) => {
                self.page = page;
            }
            NavRequest::Confirm(request) => {
                let dialog = request.dialog();
                self.pending_confirm = Some(request.action);
                self.dialog = Some(dialog);
            }
        }
    }

    fn execute_confirmed(&mut self, action: DestructiveAction) {
        let (label, verb, outcome): (&str, &str, spp_core::Result<()>) = match action {
            DestructiveAction::DeleteAutomationRule { id } => {
                let outcome = self
                    .db
                    .with_write(|conn| spp_core::automation::delete_rule(conn, id))
                    .map(|_| ());
                self.automation.invalidate();
                ("automation rule", "deleted", outcome)
            }
            DestructiveAction::DeleteKnowledgeDoc { id } => (
                "knowledge document",
                "deleted",
                self.knowledge.delete_document(&self.db, id),
            ),
            DestructiveAction::DeleteCustomType { id } => (
                "custom type",
                "deleted",
                self.custom_objects.delete_type(&self.db, id),
            ),
            DestructiveAction::DeleteCustomObject { id } => (
                "custom object",
                "deleted",
                self.custom_objects.delete_object(&self.db, id),
            ),
            DestructiveAction::DeleteConnector { id } => {
                ("connector", "deleted", self.connectors.delete(&self.db, id))
            }
            DestructiveAction::DeleteSegment { id } => (
                "segment",
                "deleted",
                self.outreach.delete_segment(&self.db, id),
            ),
            DestructiveAction::ClearDiagnosticsLog => {
                ("event log", "cleared", self.diagnostics.clear_log(&self.db))
            }
            DestructiveAction::DeleteSavedReport { id } => (
                "saved report",
                "deleted",
                self.reports.delete_saved_report(&self.db, id),
            ),
            DestructiveAction::RemoveFromDnc { customer_id } => (
                "do-not-contact entry",
                "removed",
                self.outreach.remove_dnc(&self.db, customer_id),
            ),
            DestructiveAction::CancelCampaignRemaining { id } => (
                "campaign sends",
                "cancelled",
                self.outreach
                    .cancel_campaign_remaining_confirmed(&self.db, id),
            ),
            DestructiveAction::DisableAllAi => (
                "AI",
                "disabled",
                self.ai_center.disable_all_ai_confirmed(&self.db),
            ),
            DestructiveAction::DeleteSavedReply {
                mailbox_remote_id,
                remote_id,
            } => (
                "saved reply",
                "deleted",
                self.operations
                    .delete_saved_reply(&self.db, mailbox_remote_id, remote_id),
            ),
        };
        match outcome {
            Ok(()) => self.toasts.success(format!("{label} {verb}")),
            Err(e) => self
                .toasts
                .error(format!("Could not delete the {label}: {e}")),
        }
    }

    fn shell_ui(&mut self, ui: &mut egui::Ui) {
        design::page_header(
            ui,
            "Shell",
            Some("native shell diagnostics (Phase 2 surface)"),
            |ui| {
                if design::button(
                    ui,
                    &format!("switch to {}", self.theme.toggle().label()),
                    ButtonVariant::Outline,
                )
                .clicked()
                {
                    self.theme = self.theme.toggle();
                    if let Err(e) = self.theme.store(&self.db) {
                        tracing::warn!(error = %e, "failed to persist the theme setting");
                    }
                    self.theme.apply(ui.ctx());
                }
            },
        );

        let db_display = self
            .db_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<in-memory>".to_string());
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
        design::card_titled(ui, "Runtime", None, |ui| {
            egui::Grid::new("shell-diagnostics")
                .num_columns(2)
                .spacing(egui::vec2(16.0, 6.0))
                .show(ui, |ui| {
                    for (label, value) in [
                        ("database", db_display),
                        ("schema", format!("v{}", self.schema_version)),
                        ("background events", self.events_seen.to_string()),
                        ("last event", self.last_event.clone().unwrap_or_default()),
                        ("sync engine", sync_kind.to_string()),
                    ] {
                        design::field_label(ui, label);
                        ui.monospace(
                            egui::RichText::new(value).color(design::tokens(ui).foreground),
                        );
                        ui.end_row();
                    }
                });
        });
        ui.add_space(8.0);
        design::hint_text(
            ui,
            "all pages ported: dashboard, inbox, operations, customers, organizations, knowledge/docs, \
             reports, issue radar, incidents, custom objects, automation, AI center, outreach, \
             notifications, search, support graph, connectors, sync health, settings, onboarding \
             — stale checklist — omits live chats",
        );
    }

    fn sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let t = design::tokens(ui);
        ui.horizontal(|ui| {
            let (id, rect) = ui.allocate_space(egui::vec2(28.0, 28.0));
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(7), t.primary);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "S+",
                egui::FontId::proportional(13.0),
                t.primary_foreground,
            );
            ui.advance_cursor_after_rect(rect);
            let _ = id;
            ui.label(
                egui::RichText::new("SupportOS+")
                    .size(15.5)
                    .strong()
                    .color(t.foreground),
            );
        });
        ui.label(
            egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                .small()
                .color(t.muted_foreground),
        );
        ui.add_space(10.0);

        if palette_trigger(ui).clicked() {
            self.palette.toggle();
        }
        ui.add_space(12.0);

        egui::Panel::bottom("spp-sidebar-theme")
            .show_separator_line(false)
            .frame(egui::Frame::new().inner_margin(egui::Margin {
                left: 10,
                right: 10,
                top: 2,
                bottom: 6,
            }))
            .show(ui, |ui| {
                ui.separator();
                let next = self.theme.toggle();
                if theme_row(ui, next) {
                    self.theme = next;
                    if let Err(e) = self.theme.store(&self.db) {
                        tracing::warn!(error = %e, "failed to persist the theme setting");
                    }
                    self.theme.apply(ui.ctx());
                }
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("spp-nav-scroll")
                    .show(ui, |ui| {
                        if ui.rect_contains_pointer(ui.max_rect()) {
                            let wheel_y = ui.input(|i| i.smooth_scroll_delta.y);
                            if wheel_y != 0.0 {
                                ui.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
                                ui.scroll_with_delta(egui::vec2(0.0, wheel_y));
                            }
                        }
                        for (group, keys) in NAV_GROUPS {
                            ui.label(
                                egui::RichText::new(*group)
                                    .small()
                                    .strong()
                                    .color(t.muted_foreground),
                            );
                            ui.add_space(2.0);
                            for key in keys.iter() {
                                if nav_row(ui, self.page, *key) {
                                    self.page = *key;
                                }
                            }
                            ui.add_space(8.0);
                        }
                    });
            });
    }

    fn header_ui(&mut self, ui: &mut egui::Ui) {
        let t = design::tokens(ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let toggle = if self.theme == Theme::Dark {
                (Icon::Sun, "switch to light theme")
            } else {
                (Icon::Moon, "switch to dark theme")
            };
            if design::icon_button(ui, toggle.0, ButtonVariant::Ghost, toggle.1).clicked() {
                self.theme = self.theme.toggle();
                if let Err(e) = self.theme.store(&self.db) {
                    tracing::warn!(error = %e, "failed to persist the theme setting");
                }
                self.theme.apply(ui.ctx());
            }
            if design::button_with_icon(ui, Icon::Search, "Search  ⌘K", ButtonVariant::Outline)
                .on_hover_text("command palette (Ctrl/Cmd+K)")
                .clicked()
            {
                self.palette.toggle();
            }
            let (tone, label) = self.freshness();
            let (fill, fg) = design::tone_colors(ui, tone);
            let freshness_btn =
                egui::Button::new(egui::RichText::new(label).size(12.0).strong().color(fg))
                    .fill(fill)
                    .corner_radius(egui::CornerRadius::same(6))
                    .small();
            if ui
                .add(freshness_btn)
                .on_hover_text("open Sync Health (last sync, checkpoints, runs)")
                .clicked()
            {
                self.page = PageKey::SyncHealth;
            }
        });
        let rect = ui.max_rect();
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            egui::Stroke::new(1.0, t.border),
        );
    }

    fn freshness(&self) -> (design::BadgeTone, String) {
        if self.sync.as_ref().is_some_and(|e| e.is_running()) {
            return (design::BadgeTone::Info, "syncing…".into());
        }
        let age = self
            .db
            .with_read(spp_core::sync_engine::last_sync_age_seconds);
        match age {
            None => (design::BadgeTone::Neutral, "never synced".into()),
            Some(age) => {
                let label = if age < 60 {
                    format!("synced {age}s ago")
                } else if age < 3600 {
                    format!("synced {}m ago", age / 60)
                } else {
                    format!("synced {}h ago", age / 3600)
                };
                let tone = if age <= 120 {
                    design::BadgeTone::Success
                } else if age <= 1800 {
                    design::BadgeTone::Warning
                } else {
                    design::BadgeTone::Destructive
                };
                (tone, label)
            }
        }
    }
}

impl eframe::App for ShellApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.logic_pass(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui_pass(ui);
    }
}

impl ShellApp {
    pub(crate) fn logic_pass(&mut self, ctx: &egui::Context) {
        ctx.request_repaint_after(std::time::Duration::from_millis(1500));

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
            match event {
                spp_core::events::ServerEvent::SyncProgress(p) => {
                    self.sync_health.note_progress(p.clone());
                }
                spp_core::events::ServerEvent::SyncCompleted(e) => {
                    if let Some((kind, title, detail)) = sync_completed_toast(e) {
                        self.toasts.push(kind, title, detail);
                    }
                }
                spp_core::events::ServerEvent::OutboundJobFinished(e) => {
                    if e.success {
                        if matches!(e.kind.as_str(), "create_reply" | "create_note") {
                            let message: String = if e.kind == "create_reply" {
                                "Reply delivered to Help Scout".to_string()
                            } else {
                                "Note delivered to Help Scout".to_string()
                            };
                            self.toasts
                                .push(crate::toasts::ToastKind::Success, message, None);
                        }
                    } else {
                        self.toasts.push(
                            crate::toasts::ToastKind::Error,
                            format!("{} failed", human_outbound_kind(&e.kind)),
                            e.error.clone(),
                        );
                    }
                }
                spp_core::events::ServerEvent::ErrorOccurred(e) if e.service != "outbound" => {
                    let kind = match e.severity.as_str() {
                        "warn" | "warning" => crate::toasts::ToastKind::Warning,
                        _ => crate::toasts::ToastKind::Error,
                    };
                    self.toasts.push(kind, e.message.clone(), None);
                }
                spp_core::events::ServerEvent::ErrorOccurred(_) => {}
                _ => {}
            }
        }
        if !dirty.is_empty() {
            self.apply_invalidations(&dirty);
        }
    }

    pub(crate) fn ui_pass(&mut self, ui: &mut egui::Ui) {
        let t = design::tokens(ui);
        egui::Panel::left("spp-nav")
            .exact_size(216.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(t.sidebar)
                    .inner_margin(egui::Margin {
                        left: 10,
                        right: 10,
                        top: 14,
                        bottom: 10,
                    }),
            )
            .show(ui, |ui| self.sidebar_ui(ui));

        egui::Panel::top("spp-header")
            .exact_size(52.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(t.background)
                    .inner_margin(egui::Margin::symmetric(20, 0)),
            )
            .show(ui, |ui| self.header_ui(ui));

        if let Some(nav) = self.palette.ui(ui.ctx(), &self.db) {
            self.apply_nav(nav);
        }
        if let Some(dialog) = &mut self.dialog {
            let outcome = dialog.ui(ui.ctx());
            if outcome.is_done() {
                let dialog = self.dialog.take();
                let action = self.pending_confirm.take();
                if outcome == DialogOutcome::Confirmed {
                    if let Some(action) = action {
                        self.execute_confirmed(action);
                    } else if let Some(dialog) = dialog {
                        self.toasts
                            .push(crate::toasts::ToastKind::Info, dialog.title, None);
                    }
                }
            }
        }

        let bus = self.bus.clone();
        let sync = self.sync.clone();
        let mut nav: Option<NavRequest> = None;
        if self.page == PageKey::Inbox {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(t.background)
                        .inner_margin(egui::Margin::symmetric(22, 14)),
                )
                .show(ui, |ui| self.inbox.ui(ui, &self.db, &bus, sync.as_ref()));
            nav = None;
        } else {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(t.background))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("spp-page-scroll")
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            ui.add_space(2.0);
                            egui::Frame::new()
                                .fill(egui::Color32::TRANSPARENT)
                                .inner_margin(egui::Margin::symmetric(22, 14))
                                .show(ui, |ui| match self.page {
                                    PageKey::Dashboard => {
                                        self.dashboard.ui(ui, &self.db, sync.as_ref());
                                    }
                                    PageKey::Customers => {
                                        self.customers.ui(ui, &self.db, sync.as_ref());
                                    }
                                    PageKey::Knowledge => {
                                        nav = self.knowledge.ui(ui, &self.db, sync.as_ref());
                                    }
                                    PageKey::Reports => {
                                        nav = self.reports.ui(ui, &self.db, sync.as_ref());
                                    }
                                    PageKey::LiveChats => {
                                        self.live_chats.ui(ui, &self.db);
                                    }
                                    PageKey::Operations => {
                                        nav = self.operations.ui(ui, &self.db);
                                    }
                                    PageKey::AiCenter => {
                                        nav = self.ai_center.ui(ui, &self.db);
                                    }
                                    PageKey::Automation => {
                                        nav = self.automation.ui(ui, &self.db);
                                    }
                                    PageKey::Connectors => {
                                        nav = self.connectors.ui(ui, &self.db);
                                    }
                                    PageKey::CustomObjects => {
                                        nav = self.custom_objects.ui(ui, &self.db);
                                    }
                                    PageKey::Incidents => self.incidents.ui(ui, &self.db),
                                    PageKey::IssueRadar => self.issue_radar.ui(ui, &self.db),
                                    PageKey::Notifications => {
                                        self.notifications.ui(ui, &self.db);
                                    }
                                    PageKey::Onboarding => {
                                        self.onboarding.ui(ui, &self.db, sync.as_ref());
                                    }
                                    PageKey::Organizations => {
                                        self.organizations.ui(ui, &self.db, sync.as_ref());
                                    }
                                    PageKey::Outreach => {
                                        nav = self.outreach.ui(ui, &self.db);
                                    }
                                    PageKey::Search => self.search.ui(ui, &self.db),
                                    PageKey::Settings => self.settings.ui(ui, &self.db),
                                    PageKey::SupportGraph => self.support_graph.ui(ui, &self.db),
                                    PageKey::Diagnostics => {
                                        nav = self.diagnostics.ui(ui, &self.db);
                                    }
                                    PageKey::SyncHealth => {
                                        self.sync_health.ui(ui, &self.db, sync.as_ref());
                                    }
                                    PageKey::Shell => self.shell_ui(ui),
                                    PageKey::Inbox => unreachable!("inbox handled above"),
                                });
                        });
                });
        }
        if let Some(nav) = nav {
            self.apply_nav(nav);
        }

        self.toasts.ui(ui.ctx());
    }
}

mod chrome;

use chrome::{
    NAV_GROUPS, human_outbound_kind, nav_row, palette_trigger, sync_completed_toast, theme_row,
};

#[cfg(feature = "ui-screenshot")]
impl ShellApp {
    fn apply_screenshot_env(&mut self, ctx: &egui::Context) {
        if let Ok(theme) = std::env::var("SPP_SHOT_THEME") {
            let theme = if theme.eq_ignore_ascii_case("dark") {
                Theme::Dark
            } else {
                Theme::Light
            };
            self.theme = theme;
            theme.apply(ctx);
        }
        if let Ok(page) = std::env::var("SPP_SHOT_PAGE") {
            for key in PageKey::all() {
                if key.label().eq_ignore_ascii_case(&page) {
                    self.page = key;
                    break;
                }
            }
        }
        if std::env::var("SPP_SHOT_SEED").as_deref() == Ok("1")
            && let Err(e) = self.db.with_write(seed_fixture_for_screenshots)
        {
            tracing::warn!(error = %e, "screenshot seed failed");
        }
        if std::env::var("SPP_SHOT_PALETTE").as_deref() == Ok("1") {
            self.palette.open();
        }
    }
}

#[cfg(feature = "ui-screenshot")]
fn seed_fixture_for_screenshots(conn: &mut rusqlite::Connection) -> spp_core::error::Result<()> {
    use spp_core::error::Result;
    let seed_conv = |conn: &rusqlite::Connection,
                     id: i64,
                     remote: i64,
                     subject: &str,
                     status: &str,
                     channel: &str,
                     mailbox: i64,
                     assignee: i64,
                     customer: i64,
                     days_ago: i64|
     -> Result<()> {
        conn.execute(
            "INSERT INTO conversations (id, remote_id, number, subject, preview, status, type,
                 mailbox_local_id, assignee_local_id, customer_local_id, priority,
                 created_at, updated_at, closed_at, first_customer_message_at, local_created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                 CASE WHEN ?9 = 0 THEN NULL ELSE ?9 END,
                 ?10, 'none',
                 strftime('%Y-%m-%dT%H:%M:%fZ','now', printf('-%d day', ?11)),
                 strftime('%Y-%m-%dT%H:%M:%fZ','now', printf('-%d day', ?11)),
                 CASE WHEN ?6 = 'closed'
                      THEN strftime('%Y-%m-%dT%H:%M:%fZ','now', printf('-%d day', ?11)) ELSE NULL END,
                 strftime('%Y-%m-%dT%H:%M:%fZ','now', printf('-%d day', ?11)),
                 strftime('%Y-%m-%dT%H:%M:%fZ','now', printf('-%d day', ?11)))",
            rusqlite::params![
                id,
                remote,
                500 + id,
                subject,
                format!("{subject} preview"),
                status,
                channel,
                mailbox,
                assignee,
                customer,
                days_ago,
            ],
        )?;
        Ok(())
    };
    conn.execute(
        "INSERT INTO users (id, remote_id, first_name, last_name, email, mention)
         VALUES (5, 505, 'Ada', 'Lovelace', 'ada@example.com', 'ada'),
                (6, 506, 'Grace', 'Hopper', 'grace@example.com', NULL)",
        [],
    )?;
    conn.execute(
        "INSERT INTO mailboxes (id, remote_id, name, email)
         VALUES (1, 101, 'Support', 'support@example.com'),
                (2, 102, 'Billing', 'billing@example.com')",
        [],
    )?;
    conn.execute(
        "INSERT INTO customers (id, remote_id, first_name, last_name, email)
         VALUES (9, 909, 'Kim', 'Pearce', 'kim@example.com'),
                (10, 910, 'Ada', 'Customer', 'adac@example.com')",
        [],
    )?;
    conn.execute(
        "INSERT INTO customer_emails (customer_id, value, type)
         VALUES (9, 'kim@example.com', 'work'),
                (10, 'adac@example.com', 'work')",
        [],
    )?;
    seed_conv(
        conn,
        1,
        1001,
        "Export to CSV fails on large files",
        "active",
        "email",
        1,
        5,
        9,
        0,
    )?;
    seed_conv(
        conn,
        2,
        1002,
        "Billing after trial period",
        "pending",
        "email",
        2,
        6,
        10,
        0,
    )?;
    seed_conv(
        conn,
        3,
        1003,
        "Cannot sign in with SSO",
        "active",
        "chat",
        1,
        5,
        9,
        1,
    )?;
    seed_conv(
        conn,
        4,
        1004,
        "Feature request: dark mode",
        "closed",
        "email",
        1,
        6,
        10,
        2,
    )?;
    seed_conv(
        conn,
        5,
        1005,
        "Refund for duplicate charge",
        "pending",
        "chat",
        2,
        0,
        9,
        3,
    )?;
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, body_text, from_type, state,
             created_by_customer_id, created_at)
         VALUES (1, 'customer_message', '<p>Hi! Exporting to CSV fails with a 500 error whenever the file is over 10&nbsp;MB. This started yesterday.</p>', 'customer', 'published', 9,
                 strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                (1, 'reply', '<p>Thanks for the report — we reproduced it and a fix is rolling out today. <b>Workaround:</b> split the export into two files.</p>', 'user', 'published', NULL,
                 strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                (1, 'note', '<p>@ada triaged: the streaming writer buffers the whole file. Fix in release 1.0.1.</p>', 'user', 'published', NULL,
                 strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        [],
    )?;
    conn.execute(
        "INSERT INTO ratings (conversation_id, rating, remote_created_at)
         VALUES (1, 'great', strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                (2, 'okay', strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        [],
    )?;
    conn.execute(
        "INSERT INTO tags (remote_id, name, slug) VALUES (1, 'bug', 'bug'), (2, 'billing', 'billing')",
        [],
    )?;
    conn.execute(
        "INSERT INTO conversation_tags (conversation_id, tag_id) VALUES (1, 1), (2, 2)",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO docs_collections (id, remote_id, name, slug)
         VALUES (1, 1, 'Guides', 'guides')",
        [],
    )?;
    conn.execute(
        "INSERT INTO docs_articles (collection_local_id, number, name, status, text, preview,
             views, words, remote_created_at, remote_updated_at)
         VALUES (1, 1, 'Exporting your data', 'published', 'How to export conversations to CSV and JSON.', 'How to export conversations…', 42, 7,
                 strftime('%Y-%m-%dT%H:%M:%fZ','now','-1 day'),
                 strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                (1, 2, 'Troubleshooting sign-in', 'draft', 'Common SSO sign-in problems and fixes.', 'Common SSO sign-in problems…', 3, 5,
                 strftime('%Y-%m-%dT%H:%M:%fZ','now','-2 day'),
                 strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        [],
    )?;
    let _ = spp_core::notifications::record_notification(
        conn,
        None,
        &spp_core::notifications::NotificationInput {
            notification_type: spp_core::catalog::NotificationType::Mentioned,
            title: "Ada mentioned you on #501".into(),
            body: Some("@ada triaged: the streaming writer buffers the whole file.".into()),
            dedup_key: "shot:n1".into(),
            ..Default::default()
        },
    );
    let _ = spp_core::notifications::record_notification(
        conn,
        None,
        &spp_core::notifications::NotificationInput {
            notification_type: spp_core::catalog::NotificationType::Mentioned,
            title: "Sync completed".into(),
            body: Some("Incremental sync finished: 5 conversations, 0 errors.".into()),
            dedup_key: "shot:n2".into(),
            ..Default::default()
        },
    );
    let _ = spp_core::search::rebuild_indexes(conn);
    Ok(())
}

#[cfg(test)]
mod headless_tests {
    use super::*;
    use crate::ShellOptions;
    use crate::pages::ConfirmRequest;

    fn headless_shell(
        convs: Vec<crate::testsupport::ConvSeed>,
    ) -> (ShellApp, spp_core::db::DbHandle) {
        let db = crate::testsupport::fixture(convs);
        let bus = spp_core::event_bus::EventBus::default();
        let sync = crate::build_sync_engine(&db, &bus);
        let options = ShellOptions {
            db: db.clone(),
            bus,
            data_dir: std::env::temp_dir(),
            sync: Some(sync),
            handle: None,
            workers: None,
            vector_index: None,
        };
        let ctx = egui::Context::default();
        (ShellApp::new_with_context(&ctx, options), db)
    }

    #[test]
    fn audit71_the_shell_renders_every_page_headlessly() {
        let (mut app, _db) = headless_shell(vec![(
            "conv",
            1,
            "Export broken",
            "active",
            "email",
            1,
            5,
            9,
        )]);
        let ctx = egui::Context::default();
        for key in PageKey::all() {
            app.select_page(key);
            let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                app.logic_pass(ui.ctx());
                app.ui_pass(ui);
            });
            let shapes = output.shapes.len();
            output.drop_without_applying_deltas();
            assert!(
                shapes > 0,
                "page {} must paint a non-empty frame",
                key.label()
            );
        }
    }

    #[test]
    fn audit70_confirm_requests_arm_execute_and_cancel_correctly() {
        let (mut app, db) = headless_shell(Vec::new());
        let rule_id = db
            .with_write(|conn| {
                spp_core::automation::create_rule(
                    conn,
                    &spp_core::automation::AutomationRule {
                        id: None,
                        name: "Escalate VIP".into(),
                        trigger: spp_core::automation::Trigger::SlaRisk,
                        action: spp_core::automation::Action::ChangeStatus {
                            new_status: "pending".into(),
                        },
                        enabled: true,
                        created_at: None,
                    },
                )
            })
            .expect("create rule");
        let rules_now = |db: &spp_core::db::DbHandle| -> usize {
            db.with_read(spp_core::automation::list_rules)
                .map(|r| r.len())
                .unwrap_or_default()
        };
        assert_eq!(rules_now(&db), 1);

        app.apply_nav(NavRequest::Confirm(ConfirmRequest::delete_automation_rule(
            rule_id,
            "Escalate VIP",
        )));
        assert!(app.dialog.is_some(), "the confirm dialog is armed");
        assert_eq!(
            app.pending_confirm,
            Some(DestructiveAction::DeleteAutomationRule { id: rule_id })
        );
        assert_eq!(rules_now(&db), 1, "arming the dialog deletes nothing");

        let cancelled = app.pending_confirm.take();
        assert!(cancelled.is_some());
        assert_eq!(rules_now(&db), 1, "a cancelled confirmation keeps the rule");

        app.apply_nav(NavRequest::Confirm(ConfirmRequest::delete_automation_rule(
            rule_id,
            "Escalate VIP",
        )));
        let action = app
            .pending_confirm
            .take()
            .expect("the action is armed again");
        app.execute_confirmed(action);
        assert_eq!(rules_now(&db), 0, "confirming deletes the rule");
    }

    #[test]
    fn audit142_ops_drill_down_resets_stale_bar_filters() {
        let (mut app, _db) = headless_shell(Vec::new());

        app.inbox.bar.view = Some(crate::pages::inbox::ViewFilter::Closed);
        app.inbox.bar.query = Some("stale search".into());
        app.inbox.bar.mailbox = Some(9);

        app.apply_nav(NavRequest::InboxTile("unassigned".into()));

        assert_eq!(app.page, PageKey::Inbox);
        assert_eq!(
            app.inbox.bar,
            crate::pages::inbox::FilterBar {
                ops: Some("unassigned".into()),
                ..crate::pages::inbox::FilterBar::default()
            },
            "the tile replaces the bar instead of layering onto it"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::chrome::{NAV_GROUPS, sync_completed_toast};
    use super::*;
    use crate::pages::invalidated_pages;
    use spp_core::events::ServerEvent;

    #[test]
    fn invalidation_batch_marks_each_page_once() {
        let events = vec![
            ServerEvent::sync_completed("incremental", 1, 0, spp_core::events::SyncTrigger::Manual),
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
        assert_eq!(PageKey::all().len(), 23);
        for key in PageKey::all() {
            assert!(!key.label().is_empty());
        }
        let mut grouped = Vec::new();
        for (_, keys) in NAV_GROUPS {
            grouped.extend_from_slice(keys);
        }
        assert_eq!(grouped, PageKey::all());
    }

    #[test]
    fn r5_d20_zero_record_syncs_and_error_runs_have_visible_outcomes() {
        let Some((kind, title, _)) = sync_completed_toast(&spp_core::events::SyncCompletedEvent {
            kind: "incremental".into(),
            processed: 0,
            errors: 0,
            trigger: spp_core::events::SyncTrigger::Manual,
            at: "2026-10-20T00:00:00Z".into(),
        }) else {
            panic!("a manual run must always toast")
        };
        assert!(matches!(kind, crate::toasts::ToastKind::Info));
        assert!(title.contains("Up to date"), "title: {title}");

        let Some((kind, _, detail)) = sync_completed_toast(&spp_core::events::SyncCompletedEvent {
            kind: "incremental".into(),
            processed: 0,
            errors: 2,
            trigger: spp_core::events::SyncTrigger::Manual,
            at: "2026-10-20T00:00:00Z".into(),
        }) else {
            panic!("a manual run must always toast")
        };
        assert!(matches!(kind, crate::toasts::ToastKind::Warning));
        assert!(
            detail.as_deref().is_some_and(|d| d.contains("Diagnostics")),
            "must point at the record: {detail:?}"
        );

        let Some((kind, title, _)) = sync_completed_toast(&spp_core::events::SyncCompletedEvent {
            kind: "initial".into(),
            processed: 25_000,
            errors: 0,
            trigger: spp_core::events::SyncTrigger::Manual,
            at: "2026-10-20T00:00:00Z".into(),
        }) else {
            panic!("a manual run must always toast")
        };
        assert!(matches!(kind, crate::toasts::ToastKind::Success));
        assert!(title.contains("25000"), "title: {title}");
    }

    #[test]
    fn audit23_auto_and_refresh_sync_completions_never_toast() {
        for trigger in [
            spp_core::events::SyncTrigger::Auto,
            spp_core::events::SyncTrigger::Refresh,
        ] {
            for (processed, errors) in [(0_u32, 0_u32), (12, 0), (0, 3)] {
                assert!(
                    sync_completed_toast(&spp_core::events::SyncCompletedEvent {
                        kind: "incremental".into(),
                        processed,
                        errors,
                        trigger,
                        at: "2026-10-20T00:00:00Z".into(),
                    })
                    .is_none(),
                    "trigger {trigger:?} with processed={processed} errors={errors} must stay silent"
                );
            }
        }
    }

    #[test]
    fn notification_events_invalidate_notifications_and_inbox() {
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
