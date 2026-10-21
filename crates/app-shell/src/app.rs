//! The eframe shell application.
//!
//! `App::logic` drains the background-task bridge and marks the affected
//! ported pages dirty (the same invalidation map the web UI used);
//! `App::ui` renders the redesigned shadcn-style shell — full-height
//! sidebar with grouped navigation and icons, an application header with
//! the command-palette trigger and theme switch, and the active page —
//! and keeps the Phase 2 shell diagnostics (theme toggle persisted in the
//! settings table, database status, live bridge counters) on the Shell
//! page. Global overlays: the command palette (Ctrl/Cmd+K) and the toast
//! stack.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::eventing::EventBus;
use spp_core::sync_engine::SyncEngine;

use crate::ShellOptions;
use crate::bridge::TaskBridge;
use crate::design::{self, ButtonVariant};
use crate::icons::Icon;
use crate::overlays::{ConfirmDialog, DialogOutcome};
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
    diagnostics: DiagnosticsPage,
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
        design::install(&cc.egui_ctx);
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
        // The workspace reset clears the vector store that lives next to
        // the database (1.1.3).
        settings.set_data_dir(options.data_dir.clone());
        // Worker handle for restart-on-interval-change (see save_general).
        settings.set_workers(options.workers.clone());
        // v1.4.0 (R1/R3): the swappable provider handle replaces the
        // boot-time real-provider snapshot. Settings activates saved
        // credentials in-session through it and runs the Connection
        // Doctor against whatever provider is currently live — no more
        // "restart SupportOS+".
        if let Some(handle) = options.handle.clone() {
            settings.set_provider_handle(handle);
        }
        settings.set_sync_engine(sync.clone());
        settings.set_bus(options.bus.clone());
        let mut connectors = ConnectorsPage::new();
        connectors.set_data_dir(options.data_dir.clone());
        // The knowledge import panel reads the `knowledge-import` watch
        // folder under the data dir (route parity).
        let mut knowledge = KnowledgePage::new();
        knowledge.set_data_dir(options.data_dir);
        // A4: the shared vector index rides into the pages that fuse
        // FTS + sqlite-vec — the same store the workers embed into.
        let vector_index = options.vector_index.clone();
        let mut inbox = InboxPage::new();
        inbox.set_vector_index(vector_index.clone());
        let mut search = SearchPage::new();
        search.set_vector_index(vector_index.clone());
        let mut ai_center = AiCenterPage::new();
        ai_center.set_vector_index(vector_index.clone());
        // `mut` is only needed by the (feature-gated) screenshot harness.
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
        };
        #[cfg(feature = "ui-screenshot")]
        app.apply_screenshot_env(&cc.egui_ctx);
        app
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
                PageKey::Diagnostics => self.diagnostics.invalidate(),
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
        design::page_header(
            ui,
            "Shell",
            Some("native shell diagnostics (Phase 2 surface)"),
            |ui| {
                if ui
                    .add(egui::Button::new(format!(
                        "switch to {}",
                        self.theme.toggle().label()
                    )))
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
             notifications, search, support graph, connectors, sync health, settings, onboarding",
        );
    }

    /// The redesigned full-height sidebar: brand, command-palette trigger,
    /// grouped icon navigation, and the theme switch pinned to the bottom.
    fn sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let t = design::tokens(ui);
        // Brand block.
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

        // Command-palette trigger (bordered, full width).
        if palette_trigger(ui).clicked() {
            self.palette.open();
        }
        ui.add_space(12.0);

        // Grouped navigation over the full page surface.
        egui::ScrollArea::vertical()
            .id_salt("spp-nav-scroll")
            .show(ui, |ui| {
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

        // Theme switch pinned to the bottom of the sidebar.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
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
    }

    /// The application header above the page content: command-palette
    /// trigger and the theme switch on the right, nothing else (each page
    /// owns its own title header).
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
                self.palette.open();
            }
            // R8 (D10): the global freshness ring — one glance answers
            // "is this thing alive". Click through to Sync Health for
            // the full state machine, checkpoints and runs.
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

    /// The header freshness state (R8): tone + label for the ring.
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
                // Fresh matches the busy poll (30 s) and the 1-minute
                // default ceiling; anything over half an hour is stale.
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
    /// Runs before every frame — and while the window is hidden, whenever
    /// the bridge requested a repaint. No painting is allowed here.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1.3.0: a periodic repaint heartbeat. Everything the user does
        // now crosses an async boundary (outbound dispatch ~2s ticks,
        // sync progress, the 1-minute poll) — with repaints only on
        // input, "sending…" status lines and live progress never
        // refreshed on screen.
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
                    // v1.4.0 (R5, D20): see `sync_completed_toast` — a
                    // zero-record completion is a visible result.
                    let (kind, title, detail) = sync_completed_toast(e);
                    self.toasts.push(kind, title, detail);
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
                        // The single most important toast in the app:
                        // "it did not go through" must never be silent
                        // again.
                        self.toasts.push(
                            crate::toasts::ToastKind::Error,
                            format!("{} failed", human_outbound_kind(&e.kind)),
                            e.error.clone(),
                        );
                    }
                }
                // R5 (D4): the first live surfacing of background
                // failures. `outbound` is exempt from the toast — the
                // OutboundJobFinished event for the same failure
                // already carries the exact message — but every
                // service still invalidates its pages below.
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

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let t = design::tokens(ui);
        // Full-height sidebar with the theme's elevated sidebar surface.
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

        // Application header over the page area (right-side global
        // controls + a bottom hairline).
        egui::Panel::top("spp-header")
            .exact_size(52.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(t.background)
                    .inner_margin(egui::Margin::symmetric(20, 0)),
            )
            .show(ui, |ui| self.header_ui(ui));

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
        // Every page scrolls vertically as a whole (shadcn app-body
        // behavior) so no content is unreachable at small window sizes —
        // except the Inbox, whose two panes manage their own scrolling.
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
                                        self.knowledge.ui(ui, &self.db, sync.as_ref());
                                    }
                                    PageKey::Reports => {
                                        self.reports.ui(ui, &self.db, sync.as_ref());
                                    }
                                    PageKey::Operations => {
                                        nav = self.operations.ui(ui, &self.db);
                                    }
                                    PageKey::AiCenter => self.ai_center.ui(ui, &self.db),
                                    PageKey::Automation => self.automation.ui(ui, &self.db),
                                    PageKey::Connectors => self.connectors.ui(ui, &self.db),
                                    PageKey::CustomObjects => {
                                        self.custom_objects.ui(ui, &self.db);
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
                                    PageKey::Outreach => self.outreach.ui(ui, &self.db),
                                    PageKey::Search => self.search.ui(ui, &self.db),
                                    PageKey::Settings => self.settings.ui(ui, &self.db),
                                    PageKey::SupportGraph => self.support_graph.ui(ui, &self.db),
                                    PageKey::Diagnostics => {
                                        self.diagnostics.ui(ui, &self.db);
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

        // Destructive-action confirmations arm the dialog (sample wiring:
        // deleting an automation rule).
        self.toasts.ui(ui.ctx());
    }
}

/// Human label for an outbound job kind (toast copy).
fn human_outbound_kind(kind: &str) -> &str {
    match kind {
        "create_reply" => "Reply",
        "create_note" => "Note",
        "update_status" => "Status change",
        "assign" => "Assignment",
        other => other,
    }
}

/// v1.4.0 (R5, D20): map a finished sync to its toast. A zero-record
/// completion is a *visible* result now — "up to date, nothing new" —
/// where it used to be indistinguishable from a dead connection.
#[must_use]
fn sync_completed_toast(
    e: &spp_core::events::SyncCompletedEvent,
) -> (crate::toasts::ToastKind, String, Option<String>) {
    if e.processed > 0 {
        let detail = match e.kind.as_str() {
            "initial" => "Your Help Scout history is in the local mirror now.".to_string(),
            "docs" => "Help Center articles refreshed.".to_string(),
            _ => "Latest changes pulled from Help Scout.".to_string(),
        };
        (
            crate::toasts::ToastKind::Success,
            format!("Sync finished: {} record(s) updated", e.processed),
            Some(detail),
        )
    } else if e.errors > 0 {
        (
            crate::toasts::ToastKind::Warning,
            "Sync finished with errors".to_string(),
            Some("See Diagnostics for the exact failures.".to_string()),
        )
    } else {
        (
            crate::toasts::ToastKind::Info,
            "Up to date — nothing new from Help Scout".to_string(),
            Some(
                "The last sync checked the account and found no changes. \
                 This is what a healthy, quiet account looks like."
                    .to_string(),
            ),
        )
    }
}

/// Sidebar navigation groups (the web nav order, partitioned into labeled
/// sections: work queues first, reference pages last).
const NAV_GROUPS: &[(&str, &[PageKey])] = &[
    (
        "Workspace",
        &[PageKey::Dashboard, PageKey::Inbox, PageKey::Operations],
    ),
    (
        "Customers & knowledge",
        &[
            PageKey::Customers,
            PageKey::Organizations,
            PageKey::Knowledge,
        ],
    ),
    (
        "Insights",
        &[PageKey::Reports, PageKey::IssueRadar, PageKey::Incidents],
    ),
    (
        "Automation & AI",
        &[
            PageKey::CustomObjects,
            PageKey::Automation,
            PageKey::AiCenter,
            PageKey::Outreach,
        ],
    ),
    (
        "System",
        &[
            PageKey::Notifications,
            PageKey::Search,
            PageKey::SupportGraph,
            PageKey::Connectors,
            PageKey::Diagnostics,
            PageKey::SyncHealth,
            PageKey::Settings,
            PageKey::Onboarding,
            PageKey::Shell,
        ],
    ),
];

/// One full-width sidebar navigation row with a leading icon.
///
/// DEFECT-01 (qa-audit/20261019T050847Z): with plain `selectable_label`
/// rows, the four shortest labels (Inbox, Reports, Search, Shell) only
/// accepted clicks in a left sliver of the painted row while hover
/// feedback spanned the label — egui 0.37's atom layout decoupled the
/// click-acceptance rect from the painted row on short galleys. Rows are
/// now hand-allocated: the sensed rect is exactly the full-width row we
/// paint, so click and hover geometry cannot diverge, whatever the label
/// length. The row paints the shadcn-style active/hover surfaces and the
/// button role is preserved for assistive technologies.
fn nav_row(ui: &mut egui::Ui, current: PageKey, key: PageKey) -> bool {
    let selected = current == key;
    let t = design::tokens(ui);
    let fg = if selected {
        t.selection_foreground
    } else {
        t.foreground
    };
    let galley = egui::WidgetText::from(egui::RichText::new(key.label()).size(13.5).color(fg))
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Button,
        );
    let (auto_id, rect) = ui.allocate_space(egui::vec2(ui.available_width(), 32.0));
    let response = ui.interact(rect, auto_id.with(key.label()), egui::Sense::click());
    let fill = if selected {
        t.selection
    } else if response.hovered() || response.has_focus() {
        t.accent
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), fill);
    }
    let icon_size = 16.0;
    let icon_rect = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + 10.0, rect.center().y - icon_size / 2.0),
        egui::pos2(
            rect.min.x + 10.0 + icon_size,
            rect.center().y + icon_size / 2.0,
        ),
    );
    Icon::for_page(key).paint_at(
        ui.painter(),
        icon_rect,
        if selected { fg } else { t.muted_foreground },
    );
    ui.painter().galley(
        egui::pos2(
            icon_rect.max.x + 9.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            egui::CornerRadius::same(8),
            egui::Stroke::new(2.0, t.ring),
            egui::StrokeKind::Outside,
        );
    }
    if let Some(cursor) = ui.visuals().interact_cursor
        && response.hovered()
    {
        ui.ctx().set_cursor_icon(cursor);
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, key.label())
    });
    response.clicked()
}

/// The sidebar's full-width command-palette trigger row (bordered).
fn palette_trigger(ui: &mut egui::Ui) -> egui::Response {
    let t = design::tokens(ui);
    let (auto_id, rect) = ui.allocate_space(egui::vec2(ui.available_width(), 34.0));
    let response = ui
        .interact(
            rect,
            auto_id.with("spp-nav-palette-trigger"),
            egui::Sense::click(),
        )
        .on_hover_text("command palette (Ctrl/Cmd+K)");
    let fill = if response.hovered() || response.has_focus() {
        t.accent
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(7), fill);
    }
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(7),
        egui::Stroke::new(1.0, t.border),
        egui::StrokeKind::Inside,
    );
    let icon_rect = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + 10.0, rect.center().y - 8.0),
        egui::pos2(rect.min.x + 26.0, rect.center().y + 8.0),
    );
    Icon::Search.paint_at(ui.painter(), icon_rect, t.muted_foreground);
    ui.painter().text(
        egui::pos2(icon_rect.max.x + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Search…",
        egui::FontId::proportional(13.5),
        t.muted_foreground,
    );
    let kbd = "⌘K";
    let kbd_font = egui::FontId::proportional(11.0);
    let kbd_width = 26.0;
    let kbd_rect = egui::Rect::from_min_max(
        egui::pos2(rect.max.x - kbd_width - 8.0, rect.center().y - 9.0),
        egui::pos2(rect.max.x - 8.0, rect.center().y + 9.0),
    );
    ui.painter()
        .rect_filled(kbd_rect, egui::CornerRadius::same(4), t.muted);
    ui.painter().text(
        kbd_rect.center(),
        egui::Align2::CENTER_CENTER,
        kbd,
        kbd_font,
        t.muted_foreground,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            egui::CornerRadius::same(9),
            egui::Stroke::new(2.0, t.ring),
            egui::StrokeKind::Outside,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            ui.is_enabled(),
            "search (command palette)".to_owned(),
        )
    });
    response
}

/// The sidebar's bottom theme-switch row (sun/moon icon + label).
fn theme_row(ui: &mut egui::Ui, next: Theme) -> bool {
    let t = design::tokens(ui);
    let (icon, label) = match next {
        Theme::Dark => (Icon::Moon, "switch to dark theme"),
        Theme::Light => (Icon::Sun, "switch to light theme"),
    };
    let galley = egui::WidgetText::from(
        egui::RichText::new(label)
            .size(13.0)
            .color(t.muted_foreground),
    )
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let (auto_id, rect) = ui.allocate_space(egui::vec2(ui.available_width(), 30.0));
    let response = ui.interact(rect, auto_id.with("spp-nav-theme"), egui::Sense::click());
    let fill = if response.hovered() || response.has_focus() {
        t.accent
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), fill);
    }
    let icon_rect = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + 10.0, rect.center().y - 8.0),
        egui::pos2(rect.min.x + 26.0, rect.center().y + 8.0),
    );
    icon.paint_at(ui.painter(), icon_rect, t.muted_foreground);
    ui.painter().galley(
        egui::pos2(
            icon_rect.max.x + 9.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        t.muted_foreground,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            egui::CornerRadius::same(8),
            egui::Stroke::new(2.0, t.ring),
            egui::StrokeKind::Outside,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label.to_owned())
    });
    response.clicked()
}

/// Visual verification harness (only compiled with the `ui-screenshot`
/// feature, which no normal build enables): the shell renders one frame
/// headless under Xvfb with software GL and eframe's `__screenshot` hook
/// saves a PNG. Environment knobs:
///
/// * `SPP_SHOT_PAGE` — the page to open (any `PageKey::label`, lowercased).
/// * `SPP_SHOT_THEME` — `light` or `dark`.
/// * `SPP_SHOT_SEED` — `1` seeds a small screenshot-fixture dataset so
///   pages render rows in the headless capture harness.
/// * `SPP_SHOT_PALETTE` — `1` opens the command palette overlay.
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

/// Seed a compact fixture dataset so the screenshots show real rows, badges,
/// charts and thread entries (feature-gated; never runs in normal builds).
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
        assert_eq!(PageKey::all().len(), 22);
        for key in PageKey::all() {
            assert!(!key.label().is_empty());
        }
        // The redesigned sidebar groups every page exactly once, in the
        // canonical order.
        let mut grouped = Vec::new();
        for (_, keys) in NAV_GROUPS {
            grouped.extend_from_slice(keys);
        }
        assert_eq!(grouped, PageKey::all());
    }

    #[test]
    fn r5_d20_zero_record_syncs_and_error_runs_have_visible_outcomes() {
        // Healthy quiet account: "up to date" is said out loud now.
        let (kind, title, _) = sync_completed_toast(&spp_core::events::SyncCompletedEvent {
            kind: "incremental".into(),
            processed: 0,
            errors: 0,
            at: "2026-10-20T00:00:00Z".into(),
        });
        assert!(matches!(kind, crate::toasts::ToastKind::Info));
        assert!(title.contains("Up to date"), "title: {title}");

        // A run that failed something says so and points at Diagnostics.
        let (kind, _, detail) = sync_completed_toast(&spp_core::events::SyncCompletedEvent {
            kind: "incremental".into(),
            processed: 0,
            errors: 2,
            at: "2026-10-20T00:00:00Z".into(),
        });
        assert!(matches!(kind, crate::toasts::ToastKind::Warning));
        assert!(
            detail.as_deref().is_some_and(|d| d.contains("Diagnostics")),
            "must point at the record: {detail:?}"
        );

        // Rows landed: the success copy is unchanged.
        let (kind, title, _) = sync_completed_toast(&spp_core::events::SyncCompletedEvent {
            kind: "initial".into(),
            processed: 25_000,
            errors: 0,
            at: "2026-10-20T00:00:00Z".into(),
        });
        assert!(matches!(kind, crate::toasts::ToastKind::Success));
        assert!(title.contains("25000"), "title: {title}");
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
