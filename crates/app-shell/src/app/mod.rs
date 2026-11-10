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
    /// AUD-P1-1 (C1): unread notification count driving the sidebar badge.
    /// Seeded from the DB at launch, live-updated from
    /// `NotificationReceived` events, and synced from the Notifications
    /// page whenever it reloads (mark-read / mark-all-read).
    unread_notifications: u32,
}

impl ShellApp {
    /// AUD-P1-1 (C1): seed the sidebar unread badge from the mirror at
    /// launch (events only carry the count once one arrives).
    fn unread_seed(db: &DbHandle) -> u32 {
        db.with_read(|conn| {
            let me = spp_core::notifications::me_user_local_id(conn)
                .ok()
                .flatten();
            spp_core::notifications::unread_count(conn, me)
        })
        .map(|n| n.clamp(0, u32::MAX as i64) as u32)
        .unwrap_or(0)
    }

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
        // AUD-P1-8 (D2): the Settings re-index action needs the vector
        // index handle.
        settings.set_vector_index(options.vector_index.clone());
        let mut connectors = ConnectorsPage::new();
        connectors.set_data_dir(options.data_dir.clone());
        let mut knowledge = KnowledgePage::new();
        knowledge.set_data_dir(options.data_dir);
        let vector_index = options.vector_index.clone();
        // AUD-P1-1 (C1): seed the sidebar unread badge before `options.db`
        // moves into the struct.
        let unread_notifications = Self::unread_seed(&options.db);
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
            unread_notifications,
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
            NavRequest::OpenConversation(id) => {
                self.page = PageKey::Inbox;
                self.inbox.select(&self.db, id);
            }
            NavRequest::OpenCustomer(id) => {
                self.page = PageKey::Customers;
                self.customers.open_profile(&self.db, id);
            }
            NavRequest::OpenKnowledgeDoc(id) => {
                self.page = PageKey::Knowledge;
                self.knowledge.go_to_documents(id);
            }
            NavRequest::OpenHelpArticle(id) => {
                self.page = PageKey::Knowledge;
                self.knowledge.open_help_article(&self.db, id);
            }
            NavRequest::OpenKnownIssues => {
                self.page = PageKey::IssueRadar;
                self.issue_radar.open_known_tab();
            }
            NavRequest::Confirm(request) => {
                // AU2-12: a second confirm request while one is armed must
                // not silently overwrite the pending destructive action —
                // the first dialog stays live until it is answered.
                if self.pending_confirm.is_some() {
                    tracing::warn!(
                        dialog = %request.dialog().title,
                        "confirm request ignored: a destructive confirm is already armed"
                    );
                } else {
                    let dialog = request.dialog();
                    // The armed confirm takes over the input: an open palette
                    // must not linger over (or under) it.
                    self.palette.close();
                    self.pending_confirm = Some(request.action);
                    self.dialog = Some(dialog);
                }
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
            DestructiveAction::DeleteIncident { id } => {
                // AUD-P1-3 (C4): incident deletion — the child tables
                // (links, notes, events, related, refs, releases) cascade.
                let outcome = self
                    .db
                    .with_write(|conn| spp_core::intelligence_features::delete_incident(conn, id))
                    .and_then(|removed| {
                        removed
                            .then_some(())
                            .ok_or_else(|| spp_core::Error::Config("incident not found".into()))
                    });
                self.incidents.invalidate();
                self.incidents.close_panel();
                ("incident", "deleted", outcome)
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
            DestructiveAction::DeleteOrganization { id } => {
                // AUD-P2-16 (C12): soft-delete via people_store.
                let outcome = self
                    .db
                    .with_write(|conn| spp_core::people_store::delete_organization(conn, id))
                    .and_then(|removed| {
                        removed
                            .ok_or_else(|| spp_core::Error::Config("organization not found".into()))
                    });
                self.organizations.invalidate();
                ("organization", "deleted", outcome)
            }
            DestructiveAction::DeleteGraphNode { id } => {
                // AUD-P2-14 (C10): node + touching human edges.
                let outcome = self
                    .db
                    .with_write(|conn| spp_core::reports::delete_graph_node(conn, id))
                    .and_then(|removed| {
                        removed
                            .ok_or_else(|| spp_core::Error::Config("graph node not found".into()))
                    });
                self.reports.invalidate();
                ("graph node", "deleted", outcome)
            }
            DestructiveAction::DeleteKnownIssue { id } => {
                // AUD-P2-12 (C8): known-issue deletion.
                let outcome = self
                    .db
                    .with_write(|conn| {
                        spp_core::intelligence_features::delete_known_issue(conn, id)
                    })
                    .and_then(|removed| {
                        removed
                            .ok_or_else(|| spp_core::Error::Config("known issue not found".into()))
                    });
                self.issue_radar.invalidate();
                ("known issue", "deleted", outcome)
            }
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
        // UX-3: this surface is an internal runtime/diagnostics view, not a
        // user checklist — presented as "About the app" with real actions.
        // Middle-truncate a long path so the monospace value stays readable.
        fn shorten_path(path: &str, max: usize) -> String {
            if path.chars().count() <= max {
                return path.to_string();
            }
            let head = max / 2 - 2;
            let tail = max - head - 1;
            let chars: Vec<char> = path.chars().collect();
            format!(
                "{}…{}",
                chars[..head].iter().collect::<String>(),
                chars[chars.len() - tail..].iter().collect::<String>()
            )
        }
        design::page_header(ui, "Shell", Some("app runtime information"), |ui| {
            if design::button(
                ui,
                &format!("switch to {} theme", self.theme.toggle().label()),
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
        });

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
        let db_display = shorten_path(&db_display, 72);
        design::card_titled(
            ui,
            "Runtime",
            Some("how this instance is doing right now"),
            |ui| {
                egui::Grid::new("shell-diagnostics")
                    .num_columns(2)
                    .spacing(egui::vec2(16.0, 6.0))
                    .min_col_width(220.0)
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
            },
        );
        ui.add_space(10.0);
        design::card_titled(
            ui,
            "Where things live",
            Some("the rest of the workspace"),
            |ui| {
                let t = design::tokens(ui);
                ui.label(
                    egui::RichText::new(
                        "Everything in SupportOS+ runs locally: conversations, customers \
                         and docs mirror from Help Scout, and every page lives in the \
                         sidebar. Use Search (Ctrl/Cmd+K) to jump anywhere, Diagnostics \
                         for application errors, and Sync Health to watch the mirror.",
                    )
                    .size(design::scale::text::BODY)
                    .color(t.foreground),
                );
            },
        );
    }

    fn sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let t = design::tokens(ui);
        // UXE-1-2: brand row — logo, name, and the version pill on one
        // line (the old two-line block crammed a tiny version label
        // under the name with no air).
        ui.horizontal(|ui| {
            let (id, rect) = ui.allocate_space(egui::vec2(26.0, 26.0));
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(7), t.primary);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "S+",
                egui::FontId::proportional(12.0),
                t.primary_foreground,
            );
            ui.advance_cursor_after_rect(rect);
            let _ = id;
            ui.label(
                egui::RichText::new("SupportOS+")
                    .size(14.5)
                    .strong()
                    .color(t.foreground),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let v = format!("v{}", env!("CARGO_PKG_VERSION"));
                let galley = egui::WidgetText::from(
                    egui::RichText::new(v.as_str())
                        .size(design::scale::text::META)
                        .color(t.muted_foreground),
                )
                .into_galley(
                    ui,
                    Some(egui::TextWrapMode::Extend),
                    f32::INFINITY,
                    egui::TextStyle::Small,
                );
                let pad = egui::vec2(6.0, 2.0);
                let size = galley.size() + 2.0 * pad;
                let (vid, vrect) = ui.allocate_space(size);
                ui.painter().rect_filled(
                    vrect,
                    egui::CornerRadius::same(design::scale::radius::BADGE),
                    t.muted,
                );
                ui.painter().galley(
                    egui::pos2(
                        vrect.min.x + pad.x,
                        vrect.center().y - galley.size().y / 2.0,
                    ),
                    galley,
                    t.muted_foreground,
                );
                let _ = vid;
            });
        });
        ui.add_space(design::scale::space::SP3);

        if self.dialog.is_none() && palette_trigger(ui).clicked() {
            self.palette.toggle();
        }
        ui.add_space(design::scale::space::SP3);

        egui::Panel::bottom("spp-sidebar-theme")
            .show_separator_line(false)
            .frame(egui::Frame::new().inner_margin(egui::Margin {
                left: 10,
                right: 10,
                top: 2,
                bottom: 6,
            }))
            .show(ui, |ui| {
                // UXE-1-2: the orphan hairline above the theme row read as
                // a broken divider under the nav scroll — the row is its
                // own affordance now.
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
                            // UXE-1-2: one micro-label convention — 11px
                            // muted, sentence case (matches the inbox
                            // views rail) instead of strong small text.
                            ui.label(
                                egui::RichText::new(*group)
                                    .size(design::scale::text::META)
                                    .color(t.muted_foreground),
                            );
                            ui.add_space(design::scale::space::SP1);
                            for key in keys.iter() {
                                // AUD-P1-1 (C1): the Notifications nav row
                                // carries the live unread count as a pill.
                                let badge = if *key == PageKey::Notifications
                                    && self.unread_notifications > 0
                                {
                                    Some(self.unread_notifications)
                                } else {
                                    None
                                };
                                if nav_row(ui, self.page, *key, badge) {
                                    self.page = *key;
                                }
                            }
                            ui.add_space(design::scale::space::SP2);
                        }
                    });
            });
    }

    fn header_ui(&mut self, ui: &mut egui::Ui) {
        let t = design::tokens(ui);
        ui.horizontal(|ui| {
            // UXG-1-8 (UXF-1-4): the active page's label lands on the left
            // of the top bar — window-level context, the modern helpdesk
            // breadcrumb register. The 48px bar carried only right-aligned
            // controls, which read unbalanced (and worse the wider the
            // display); the same anatomy as page_header (left title +
            // baseline-aligned right actions) balances it at every width.
            ui.label(
                egui::RichText::new(self.page.label())
                    .size(design::scale::text::LEAD)
                    .strong()
                    .color(t.foreground),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // UX-33: the theme toggle was a 30×30 icon-only ghost button —
                // auditors (and users) could not tell what it did without
                // hovering, and the crescent glyph even read as a notification
                // bell. It now carries its target state as a labelled Outline
                // button, consistent with the Search button beside it.
                let toggle = if self.theme == Theme::Dark {
                    (Icon::Sun, "Light", "switch to light theme")
                } else {
                    (Icon::Moon, "Dark", "switch to dark theme")
                };
                if design::button_with_icon(ui, toggle.0, toggle.1, ButtonVariant::Outline)
                    .on_hover_text(toggle.2)
                    .clicked()
                {
                    self.theme = self.theme.toggle();
                    if let Err(e) = self.theme.store(&self.db) {
                        tracing::warn!(error = %e, "failed to persist the theme setting");
                    }
                    self.theme.apply(ui.ctx());
                }
                if self.dialog.is_none()
                    && design::button_with_icon(
                        ui,
                        Icon::Search,
                        "Search  ⌘K",
                        ButtonVariant::Outline,
                    )
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
                    // UX-33: the pill now says what it measures (Help Scout
                    // freshness) both on its face and in the hover.
                    .on_hover_text(
                        "how fresh the local Help Scout mirror is — opens Sync Health \
                     (last sync, checkpoints, runs)",
                    )
                    .clicked()
                {
                    self.page = PageKey::SyncHealth;
                }
            });
        });
        let rect = ui.max_rect();
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            egui::Stroke::new(1.0, t.border),
        );
    }

    fn freshness(&self) -> (design::BadgeTone, String) {
        // UX-33: every label names what synced — the bare "synced Ns ago"
        // left the pill without context (users read it as a generic, or
        // even system, status instead of the Help Scout mirror's age).
        if self.sync.as_ref().is_some_and(|e| e.is_running()) {
            return (design::BadgeTone::Info, "Help Scout · syncing…".into());
        }
        let age = self
            .db
            .with_read(spp_core::sync_engine::last_sync_age_seconds);
        match age {
            None => (
                design::BadgeTone::Neutral,
                "Help Scout · never synced".into(),
            ),
            Some(age) => {
                let label = if age < 60 {
                    format!("Help Scout · synced {age}s ago")
                } else if age < 3600 {
                    format!("Help Scout · synced {}m ago", age / 60)
                } else {
                    format!("Help Scout · synced {}h ago", age / 3600)
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
                    // AU2-9: the "syncing…" note set at click time was never
                    // updated on completion — replace it with the outcome.
                    let outcome = if e.errors > 0 {
                        format!(
                            "sync finished with {} error(s) — {} record(s) processed",
                            e.errors, e.processed
                        )
                    } else {
                        format!("sync finished — {} record(s) processed", e.processed)
                    };
                    self.dashboard.sync_finished(&outcome);
                    self.customers.sync_finished(&outcome);
                    self.organizations.sync_finished(&outcome);
                    self.reports.sync_finished(&outcome);
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
                spp_core::events::ServerEvent::NotificationReceived(e) => {
                    // AUD-P1-1 (C1): the event carries the fresh unread
                    // count — feed the sidebar badge.
                    self.unread_notifications = e.unread_count;
                }
                // AUD-P1-2 (C2): the manual refresh answers "did it work?"
                // with a toast (the page reloads via invalidated_pages).
                spp_core::events::ServerEvent::ConnectorRefreshed(e) => {
                    if e.ok {
                        self.toasts.push(
                            crate::toasts::ToastKind::Success,
                            format!("Connector \"{}\" refreshed — {} rows", e.name, e.rows),
                            Some("Health, last sync and the rows view are up to date.".into()),
                        );
                    } else {
                        self.toasts.push(
                            crate::toasts::ToastKind::Error,
                            format!("Connector \"{}\" refresh failed", e.name),
                            e.error.clone(),
                        );
                    }
                }
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
            .exact_size(48.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(t.background)
                    .inner_margin(egui::Margin::symmetric(20, 0)),
            )
            .show(ui, |ui| self.header_ui(ui));

        if let Some(nav) = self.palette.ui(ui.ctx(), &self.db, self.dialog.is_some()) {
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
                            // UXG-1-1: one content-column discipline for
                            // every standard page — full width up to
                            // CONTENT_MAX (1800), a centered exact-width
                            // column above it. On 2K/4K/8K displays the
                            // page holds professional proportions instead
                            // of stretching to the full window; on windows
                            // under the cap the layout is pixel-identical
                            // to the uncapped render.
                            design::content_column(ui, |ui| {
                                egui::Frame::new()
                                    .fill(egui::Color32::TRANSPARENT)
                                    .inner_margin(egui::Margin::symmetric(22, 14))
                                    .show(ui, |ui| match self.page {
                                        PageKey::Dashboard => {
                                            self.dashboard.ui(ui, &self.db, sync.as_ref());
                                        }
                                        PageKey::Customers => {
                                            nav = self.customers.ui(ui, &self.db, sync.as_ref());
                                        }
                                        PageKey::Knowledge => {
                                            nav = self.knowledge.ui(ui, &self.db, sync.as_ref());
                                        }
                                        PageKey::Reports => {
                                            nav = self.reports.ui(ui, &self.db, sync.as_ref());
                                        }
                                        PageKey::LiveChats => {
                                            nav = self.live_chats.ui(ui, &self.db);
                                        }
                                        PageKey::Operations => {
                                            nav = self.operations.ui(ui, &self.db, &self.bus);
                                        }
                                        PageKey::AiCenter => {
                                            nav = self.ai_center.ui(ui, &self.db);
                                        }
                                        PageKey::Automation => {
                                            nav = self.automation.ui(ui, &self.db);
                                        }
                                        PageKey::Connectors => {
                                            nav = self.connectors.ui(ui, &self.db, &bus);
                                        }
                                        PageKey::CustomObjects => {
                                            nav = self.custom_objects.ui(ui, &self.db);
                                        }
                                        PageKey::Incidents => {
                                            nav = self.incidents.ui(ui, &self.db);
                                        }
                                        // AUD-P2-12 (C8): the radar drill-downs
                                        // surface as NavRequests now.
                                        PageKey::IssueRadar => {
                                            nav = self.issue_radar.ui(ui, &self.db);
                                        }
                                        PageKey::Notifications => {
                                            nav = self.notifications.ui(ui, &self.db);
                                            // AUD-P1-1 (C1): the page just
                                            // (re)loaded — its unread count
                                            // is fresher than the last event.
                                            self.unread_notifications =
                                                self.notifications.unread_badge();
                                        }
                                        PageKey::Onboarding => {
                                            self.onboarding.ui(ui, &self.db, sync.as_ref());
                                        }
                                        PageKey::Organizations => {
                                            nav =
                                                self.organizations.ui(ui, &self.db, sync.as_ref());
                                        }
                                        PageKey::Outreach => {
                                            nav = self.outreach.ui(ui, &self.db);
                                        }
                                        PageKey::Search => {
                                            nav = self.search.ui(ui, &self.db);
                                        }
                                        PageKey::Settings => {
                                            nav = self.settings.ui(ui, &self.db);
                                        }
                                        PageKey::SupportGraph => {
                                            nav = self.support_graph.ui(ui, &self.db);
                                        }
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
        // Settings tabs (Sync & AI, Help Scout, LM Studio, Vector index,
        // Business hours, Backups & export) — the audit evidence needs the
        // tab, not just the page. Ignored when SPP_SHOT_PAGE is not Settings.
        if let Ok(tab) = std::env::var("SPP_SHOT_TAB")
            && !tab.is_empty()
            && !self.settings.set_tab_by_label(&tab)
        {
            tracing::warn!(tab = %tab, "SPP_SHOT_TAB: no settings tab matches");
        }
        if std::env::var("SPP_SHOT_SEED").as_deref() == Ok("1")
            && let Err(e) = self.db.with_write(seed_fixture_for_screenshots)
        {
            tracing::warn!(error = %e, "screenshot seed failed");
        }
        if std::env::var("SPP_SHOT_PALETTE").as_deref() == Ok("1") {
            // UXA-4 evidence: the audit shot that flagged UX-31 showed the
            // palette mid-search (query typed, first hit selected, subtitles
            // visible) — an empty palette would not evidence the row fix.
            // SPP_SHOT_PALETTE_QUERY seeds the query and runs the search so
            // the shot lands on the search state.
            if let Ok(query) = std::env::var("SPP_SHOT_PALETTE_QUERY")
                && !query.is_empty()
            {
                self.palette.open_with_query(&query, &self.db);
            } else {
                self.palette.open();
            }
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
    // UX-35: a job-failure notification in the new shape — the same fields
    // `notification_sweep::sweep_failed_jobs` produces for the audit's exact
    // scenario (LM Studio unreachable at localhost). The sweep path itself is
    // unit-tested in core; this seed exists so the shot exercises the card
    // rendering: friendly title/body, structured payload on hover.
    let _ = spp_core::notifications::record_notification(
        conn,
        None,
        &spp_core::notifications::NotificationInput {
            notification_type: spp_core::catalog::NotificationType::JobFailure,
            severity: Some("warning"),
            title: "Background job failed: AI task".into(),
            body: Some(
                "A background task could not reach the service it needs at \
                 localhost:1234 — the connection was refused, timed out, or the \
                 address does not resolve. If this is the AI assistant, check that \
                 LM Studio (or your OpenAI-compatible server) is running and that \
                 its address in Settings is correct. (Hover for the technical \
                 detail.)"
                    .into(),
            ),
            payload: Some(
                "{\"code\":\"unreachable\",\"raw\":\"LM Studio chat failed: error \
                 sending request for url \
                 (http://localhost:1234/v1/chat/completions)\",\"queue\":\"ai\",\
                 \"job_type\":\"ai\"}"
                    .into(),
            ),
            dedup_key: "shot:n3".into(),
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
