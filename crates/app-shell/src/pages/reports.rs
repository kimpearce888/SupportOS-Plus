//! Reports page (port of the web `pages/reports.rs`).
//!
//! Tabs: overview (dashboard facts + per-day chart), SLA (typed
//! `spp_core::sla::sla_report` tables + per-mailbox bar chart), why
//! contacting, top questions, and issue radar — the same data the
//! `/api/reports/*` routes serve, computed in-process via the promoted
//! `*_impl` helpers. Charts render with the shell's native painter
//! (see `crate::chart`; egui_plot does not build against egui 0.37 yet).

use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::sla::SlaReport;

use super::{ConfirmRequest, NavRequest, i64_field, str_field};
use crate::chart;
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;
use crate::pages::dashboard::{DashboardData, DashboardScope, load_dashboard};

/// Report tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Sla,
    WhyContacting,
    TopQuestions,
    Quality,
    Radar,
    Digest,
    Happiness,
    HelpScout,
    /// Item 56: the report builder (metric × dimension over the catalog,
    /// filters, comparison, saved-report CRUD).
    Builder,
    /// Item 56: the dashboard/insights surface (health facts,
    /// effectiveness, friction, graph nodes).
    Insights,
}

impl Tab {
    const ALL: [Self; 11] = [
        Self::Overview,
        Self::Sla,
        Self::WhyContacting,
        Self::TopQuestions,
        Self::Quality,
        Self::Radar,
        Self::Digest,
        Self::Happiness,
        Self::HelpScout,
        Self::Builder,
        Self::Insights,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Sla => "SLA",
            Self::WhyContacting => "Why contacting",
            Self::TopQuestions => "Top questions",
            Self::Quality => "Quality",
            Self::Radar => "Issue radar",
            Self::Digest => "Digest",
            Self::Happiness => "Happiness",
            Self::HelpScout => "Help Scout",
            Self::Builder => "Builder",
            Self::Insights => "Insights",
        }
    }
}

/// Chart points for the SLA per-mailbox first-response averages:
/// `(x = mailbox index, y = avg business minutes)`.
#[must_use]
pub fn sla_first_response_bars(report: &SlaReport) -> Vec<chart::Point> {
    report
        .mailboxes
        .iter()
        .filter_map(|m| m.first_response.avg_business_min)
        .enumerate()
        .map(|(i, v)| [i as f64, v as f64])
        .collect()
}

/// The interactive reports page state.
pub struct ReportsPage {
    /// R8 (D5): outcome note for the page's "Sync now" action — the
    /// completion itself is reported by the global sync toast.
    pub sync_note: String,
    /// FIND-005: armed shell-confirm request for a destructive action —
    /// raised by [`Self::ui`] at the end of the frame (the click happens
    /// inside nested render closures).
    pending_confirm: Option<ConfirmRequest>,

    tab: Tab,
    days: i64,
    /// RS-4-11 clock seam: the instant the page's windows and builder date
    /// inputs derive from. `None` (production) = the wall clock; tests pin
    /// a fixed instant so the rendered dates are constant text.
    today: Option<chrono::DateTime<chrono::Utc>>,
    overview: Option<DashboardData>,
    sla: Option<SlaReport>,
    why_contacting: Vec<serde_json::Value>,
    top_questions: Vec<serde_json::Value>,
    radar: Vec<serde_json::Value>,
    /// A3.3: post-resolution QA aggregates (`quality::qa_overview`).
    quality: Option<serde_json::Value>,
    /// A3.3: recent closed conversations with their QA rows.
    quality_rows: Vec<serde_json::Value>,
    // K4: the digest archive.
    digests: Vec<spp_core::digest::DigestRow>,
    digest_search: String,
    digest_pick: Option<i64>,
    // P1: the local happiness readout.
    happiness: Option<serde_json::Value>,
    // K5: cached Help Scout report fetches + the capability verdict.
    hs_capability: spp_core::reports_api::ReportsCapability,
    hs_cached: Vec<(&'static str, &'static str, String, serde_json::Value)>,
    hs_pick: usize,
    // Item 150: the persisted breach tracking (the intelligence sweep's
    // sla_breaches rows) + the per-mailbox tracking-threshold editor.
    tracked_breaches: u32,
    sla_cfg_pick: Option<i64>,
    sla_cfg_fr: String,
    sla_cfg_res: String,
    sla_cfg_note: Option<(bool, String)>,
    // Item 56: the report builder.
    b_metric: usize,
    b_dimension: usize,
    b_from: String,
    b_to: String,
    b_compare: bool,
    b_sort: usize,
    b_limit: String,
    b_f_channel: String,
    b_f_statuses: String,
    b_f_tags_any: String,
    b_f_tags_none: String,
    b_f_min_priority: String,
    b_f_attr_key: String,
    b_f_attr_value: String,
    b_f_state_key: String,
    b_f_mailbox_ids: String,
    b_f_assignee_ids: String,
    b_outcome: Option<std::result::Result<spp_core::reports::ReportRunOutcome, String>>,
    b_saved: Vec<spp_core::reports::SavedReport>,
    b_save_name: String,
    b_note: Option<(bool, String)>,
    // Item 56: the insights surface.
    i_health: Option<spp_core::reports::HealthFacts>,
    i_effectiveness: Option<spp_core::reports::EffectivenessReport>,
    i_high_friction: u32,
    i_friction_top: Vec<(i64, f64, serde_json::Value)>,
    i_graph_nodes: Vec<spp_core::reports::GraphNode>,
    i_graph_pick: Option<i64>,
    i_graph_neighbors: Vec<spp_core::support_graph::GraphNodeRef>,
    i_g_kind: usize,
    i_g_label: String,
    i_g_entity: String,
    i_g_props: String,
    i_note: Option<(bool, String)>,
    error: Option<String>,
}

impl ReportsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sync_note: String::new(),
            tab: Tab::Overview,
            days: 30,
            today: None,
            overview: None,
            sla: None,
            why_contacting: Vec::new(),
            top_questions: Vec::new(),
            radar: Vec::new(),
            quality: None,
            quality_rows: Vec::new(),
            digests: Vec::new(),
            digest_search: String::new(),
            digest_pick: None,
            happiness: None,
            hs_capability: spp_core::reports_api::ReportsCapability::Unknown,
            hs_cached: Vec::new(),
            hs_pick: 0,
            tracked_breaches: 0,
            sla_cfg_pick: None,
            sla_cfg_fr: String::new(),
            sla_cfg_res: String::new(),
            sla_cfg_note: None,
            b_metric: 0,
            b_dimension: 1,
            b_from: {
                let iso = spp_core::business_hours::iso_days_ago(30);
                iso.get(..10).unwrap_or("1970-01-01").to_string()
            },
            b_to: {
                let iso = spp_core::business_hours::now_iso_millis();
                iso.get(..10).unwrap_or("2100-01-01").to_string()
            },
            b_compare: false,
            b_sort: 0,
            b_limit: String::new(),
            b_f_channel: String::new(),
            b_f_statuses: String::new(),
            b_f_tags_any: String::new(),
            b_f_tags_none: String::new(),
            b_f_min_priority: String::new(),
            b_f_attr_key: String::new(),
            b_f_attr_value: String::new(),
            b_f_state_key: String::new(),
            b_f_mailbox_ids: String::new(),
            b_f_assignee_ids: String::new(),
            b_outcome: None,
            b_saved: Vec::new(),
            b_save_name: String::new(),
            b_note: None,
            pending_confirm: None,
            i_health: None,
            i_effectiveness: None,
            i_high_friction: 0,
            i_friction_top: Vec::new(),
            i_graph_nodes: Vec::new(),
            i_graph_pick: None,
            i_graph_neighbors: Vec::new(),
            i_g_kind: 0,
            i_g_label: String::new(),
            i_g_entity: String::new(),
            i_g_props: String::new(),
            i_note: None,
            error: None,
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.overview = None;
        self.sla = None;
        self.why_contacting.clear();
        self.top_questions.clear();
        self.radar.clear();
        self.digests.clear();
        self.happiness = None;
        self.hs_capability = spp_core::reports_api::ReportsCapability::Unknown;
        self.hs_cached.clear();
        // Item 150: the tracked breach count reloads; the threshold editor
        // keeps its state.
        self.tracked_breaches = 0;
        // Item 56: the builder outcome describes the previous mirror
        // snapshot — clear it; the saved list + insights data reload.
        self.b_outcome = None;
        self.b_saved.clear();
        self.i_health = None;
        self.i_effectiveness = None;
        self.i_friction_top.clear();
        self.i_graph_nodes.clear();
        self.i_graph_pick = None;
        self.i_graph_neighbors.clear();
    }

    fn reload(&mut self, db: &DbHandle) {
        // RS-4-11 clock seam: the windows derive from the pinned anchor when
        // one is set; production leaves it `None` (wall clock).
        let now = self.today.unwrap_or_else(chrono::Utc::now);
        let from = spp_core::business_hours::iso_days_ago_at(self.days, now);
        let to = spp_core::business_hours::now_iso_millis_at(now);
        let (overview, sla, why, questions, radar, quality, quality_rows) = db.with_read(|conn| {
            (
                Some(load_dashboard(
                    conn,
                    &DashboardScope {
                        days: self.days,
                        today: self.today,
                        ..DashboardScope::days(self.days)
                    },
                )),
                spp_core::sla::sla_report(conn, &from, &to, None).ok(),
                spp_core::analytics::why_contacting_impl(conn, self.days),
                spp_core::analytics::top_questions_impl(conn, self.days),
                spp_core::analytics::issue_radar_impl(conn),
                // A3.3: the deterministic QA aggregates + the most recent
                // closed conversations' QA cards (computed lazily).
                Some(spp_core::quality::qa_overview(conn)),
                {
                    let ids: Vec<i64> = conn
                        .prepare(
                            "SELECT id FROM conversations
                              WHERE deleted_at IS NULL AND status = 'closed'
                              ORDER BY COALESCE(closed_at, updated_at) DESC LIMIT 25",
                        )
                        .and_then(|mut stmt| {
                            stmt.query_map([], |r| r.get(0))
                                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                        })
                        .unwrap_or_default();
                    ids.into_iter()
                        .filter_map(|id| spp_core::quality::get_qa(conn, id, false))
                        .collect()
                },
            )
        });
        self.overview = overview;
        self.sla = sla;
        self.why_contacting = why;
        self.top_questions = questions;
        self.radar = radar;
        self.quality = quality;
        self.quality_rows = quality_rows;
        let (digests, happiness, capability, hs_cached, tracked) = db.with_read(|conn| {
            (
                spp_core::digest::list_digests(
                    conn,
                    if self.digest_search.is_empty() {
                        None
                    } else {
                        Some(self.digest_search.as_str())
                    },
                    30,
                ),
                Some(spp_core::reports::happiness_overview(conn, self.days)),
                spp_core::reports_api::get_capability(conn),
                spp_core::reports_api::REPORTS
                    .iter()
                    .filter_map(|spec| {
                        spp_core::reports_api::cached_report(conn, spec.key).map(
                            |(fetched_at, _start, _end, data)| {
                                (spec.key, spec.name, fetched_at, data)
                            },
                        )
                    })
                    .collect(),
                // Item 150: the persisted breach log's size.
                spp_core::intelligence_features::count_sla_breaches(conn).unwrap_or(0),
            )
        });
        self.digests = digests;
        self.happiness = happiness;
        self.hs_capability = capability;
        self.hs_cached = hs_cached;
        self.tracked_breaches = tracked;
        // Item 56: saved reports + the insights surface read in one pass.
        let (saved, health, effectiveness, high_friction, friction_top, graph_nodes) = db
            .with_read(|conn| {
                (
                    spp_core::reports::list_saved_reports(conn).unwrap_or_default(),
                    spp_core::reports::get_health_facts(
                        conn,
                        u32::try_from(self.days).unwrap_or(30),
                    )
                    .ok(),
                    spp_core::reports::get_effectiveness_report(conn).ok(),
                    spp_core::reports::count_high_friction(conn).unwrap_or(0),
                    {
                        conn.prepare(
                            "SELECT conversation_id, effort_score, factors_json
                             FROM friction_scores ORDER BY effort_score DESC LIMIT 5",
                        )
                        .and_then(|mut stmt| {
                            stmt.query_map([], |r| {
                                Ok((
                                    r.get::<_, i64>(0)?,
                                    r.get::<_, f64>(1)?,
                                    r.get::<_, Option<String>>(2)?
                                        .and_then(|t| serde_json::from_str(&t).ok())
                                        .unwrap_or(serde_json::Value::Null),
                                ))
                            })
                            .map(|rows| rows.flatten().collect())
                        })
                        .unwrap_or_default()
                    },
                    spp_core::reports::list_graph_nodes(conn, Some(50)).unwrap_or_default(),
                )
            });
        self.b_saved = saved;
        self.i_health = health;
        self.i_effectiveness = effectiveness;
        self.i_high_friction = high_friction;
        self.i_friction_top = friction_top;
        self.i_graph_nodes = graph_nodes;
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.overview.is_none() {
            self.reload(db);
        }
    }

    /// Render the page.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        engine: Option<&std::sync::Arc<spp_core::sync_engine::SyncEngine>>,
    ) -> Option<NavRequest> {
        self.ensure_loaded(db);

        design::page_header(
            ui,
            "Reports",
            Some("local analytics over the selected range"),
            |ui| {
                for (d, label) in crate::pages::dashboard::RANGES {
                    if design::chip(ui, self.days == d, label).clicked() {
                        self.days = d;
                        self.reload(db);
                    }
                }
                if design::button_with_icon(ui, Icon::Refresh, "Sync now", ButtonVariant::Outline)
                    .on_hover_text(
                        "pull fresh data from Help Scout now — the completion toast \
                     reports the outcome; the page re-reads on every sync anyway",
                    )
                    .clicked()
                {
                    // R8 (D5): this used to be a local-only "Reload" that silently
                    // re-read sqlite and looked like a refresh. It now performs a
                    // real scoped fetch and reports the outcome; the invalidation
                    // below re-reads whatever arrives.
                    match engine {
                        Some(engine) => {
                            if crate::pages::sync_health::spawn_sync_action(
                                db,
                                engine,
                                crate::pages::sync_health::SyncAction::Incremental,
                            ) {
                                self.sync_note = "syncing\u{2026}".into();
                            } else {
                                self.sync_note = "a sync is already running".into();
                            }
                        }
                        None => {
                            self.invalidate();
                            self.sync_note = "re-read the local mirror (no engine)".into();
                        }
                    }
                }
                if !self.sync_note.is_empty() {
                    ui.label(
                        egui::RichText::new(&self.sync_note)
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                }
            },
        );
        // DS-C-09 grouped tab rail — the lab's three groups over the same
        // eleven labels and the same global-index state machine.
        let labels: Vec<&str> = Tab::ALL.iter().map(|t| t.label()).collect();
        if let Some(clicked) = design::tab_groups(
            ui,
            &[
                ("Volume & quality", &labels[0..5]),
                ("AI & digests", &labels[5..8]),
                ("Platform & custom", &labels[8..11]),
            ],
            Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0),
        ) && let Some(tab) = Tab::ALL.get(clicked)
        {
            self.tab = *tab;
        }
        ui.add_space(6.0);
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }

        let (why, questions) = (self.why_contacting.clone(), self.top_questions.clone());
        match self.tab {
            Tab::Overview => self.overview_ui(ui),
            Tab::Sla => self.sla_ui(ui, db),
            Tab::WhyContacting => Self::list_ui(ui, &why, "why contacting"),
            Tab::TopQuestions => Self::list_ui(ui, &questions, "top questions"),
            Tab::Radar => self.radar_ui(ui),
            Tab::Quality => self.quality_ui(ui),
            Tab::Digest => self.digest_ui(ui, db),
            Tab::Happiness => self.happiness_ui(ui),
            Tab::HelpScout => self.help_scout_ui(ui, db),
            Tab::Builder => self.builder_ui(ui, db),
            Tab::Insights => self.insights_ui(ui, db),
        }
        // FIND-005: raise the armed confirm request (see the field doc).
        if let Some(request) = self.pending_confirm.take() {
            return Some(NavRequest::Confirm(request));
        }
        None
    }

    fn overview_ui(&mut self, ui: &mut egui::Ui) {
        let Some(data) = &self.overview else {
            ui.spinner();
            return;
        };
        let width = ui.available_width();
        let cols = if width < 640.0 {
            2
        } else if width < 980.0 {
            3
        } else {
            4
        };
        let facts: [(&str, i64); 8] = [
            ("New", data.new_conversations),
            ("Closed", data.closed_conversations),
            ("Replies sent", data.replies_sent),
            ("Backlog", data.backlog),
            ("Great ratings", data.ratings_great),
            ("Okay ratings", data.ratings_okay),
            ("Not-good ratings", data.ratings_not_good),
            ("Unassigned", data.unassigned),
        ];
        egui::Grid::new("reports-overview")
            .num_columns(cols)
            .spacing(egui::vec2(10.0, 10.0))
            .show(ui, |ui| {
                for row in facts.chunks(cols) {
                    for (label, value) in row {
                        design::metric_card(ui, label, &value.to_string(), None);
                    }
                    ui.end_row();
                }
            });
        ui.add_space(4.0);
        // R6 honesty: these rating counts mirror what Help Scout pushes to us
        // (satisfaction.ratings webhook events). Help Scout has no list-ratings
        // API to poll, and its own happiness *reports* need a Plus/Pro plan —
        // neither is silently pretended away here.
        ui.label(
            egui::RichText::new(
                "Ratings come from the satisfaction webhook; Help Scout's own \
                 happiness reports need a Plus/Pro plan and are not fetched.",
            )
            .small()
            .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(10.0);

        design::card_titled(ui, "New conversations per day", None, |ui| {
            let points: Vec<chart::Point> = crate::pages::dashboard::daily_new_points(data);
            if points.is_empty() {
                design::empty_state(
                    ui,
                    Icon::BarChart,
                    "no data in range",
                    "Nothing to plot for the selected period.",
                );
            } else {
                chart::line_chart(ui, "reports-daily-new", &points, "new conversations");
            }
        });
    }

    fn sla_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let Some(report) = &self.sla else {
            design::empty_state(
                ui,
                Icon::Clock,
                "no SLA report",
                "No SLA data is available for the selected range.",
            );
            return;
        };
        if !report.unconfigured_mailboxes.is_empty() {
            design::card(ui, |ui| {
                let t = design::tokens(ui);
                let (id, rect) = ui.allocate_space(egui::vec2(16.0, 16.0));
                Icon::AlertCircle.paint_at(ui.painter(), rect, t.warning_foreground);
                ui.advance_cursor_after_rect(rect);
                let _ = id;
                ui.label(
                    egui::RichText::new(format!(
                        "unconfigured mailboxes: {}",
                        report.unconfigured_mailboxes.join(", ")
                    ))
                    .small()
                    .color(t.warning_foreground),
                );
            });
            ui.add_space(8.0);
        }

        // The lab's first SLA card is the DS-C-23 bar-row list (a real
        // label/value per mailbox in the a11y tree, slim tracks) — the
        // painter bar chart it replaces kept no per-mailbox text.
        let bars: Vec<(&str, i64)> = report
            .mailboxes
            .iter()
            .filter_map(|m| {
                m.first_response
                    .avg_business_min
                    .map(|v| (m.mailbox_name.as_str(), v))
            })
            .collect();
        design::card_titled(ui, "First response avg (business minutes)", None, |ui| {
            if bars.is_empty() {
                design::hint_text(ui, "(no first-response data in range)");
            } else {
                let max = bars.iter().map(|(_, v)| *v).max().unwrap_or(1);
                for (label, value) in &bars {
                    design::bar_row(ui, label, *value, max, true);
                }
                design::hint_text(ui, &format!("first response avg · max {max}"));
            }
        });
        ui.add_space(10.0);

        design::card(ui, |ui| {
            design::table(
                ui,
                "reports-sla-table",
                &[
                    "mailbox",
                    "in range",
                    "fr met/missed",
                    "fr avg (min)",
                    "res met/missed",
                    "res avg (min)",
                    "waiting oldest (min)",
                ],
                28.0,
                |ui| {
                    let t = design::tokens(ui);
                    for row in &report.mailboxes {
                        ui.label(
                            egui::RichText::new(row.mailbox_name.as_str())
                                .size(13.0)
                                .strong()
                                .color(t.foreground),
                        );
                        design::td_num(ui, &row.conversations_in_range.to_string());
                        design::td_num(
                            ui,
                            &format!("{}/{}", row.first_response.met, row.first_response.missed),
                        );
                        design::td_num(
                            ui,
                            &row.first_response
                                .avg_business_min
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "—".into()),
                        );
                        design::td_num(
                            ui,
                            &format!("{}/{}", row.resolution.met, row.resolution.missed),
                        );
                        design::td_num(
                            ui,
                            &row.resolution
                                .avg_business_min
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "—".into()),
                        );
                        design::td_num(
                            ui,
                            &row.waiting
                                .oldest_business_min
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "—".into()),
                        );
                        ui.end_row();
                    }
                },
            );
        });

        // Item 150: the persisted breach tracking — the intelligence sweep
        // stores the live SLA verdicts here (deduped per conversation +
        // kind), and mailboxes WITHOUT business-hours config are tracked
        // coarsely against the thresholds edited below (the sla_configs
        // write API, previously dead).
        let mailboxes: Vec<(i64, String)> = report
            .mailboxes
            .iter()
            .map(|m| (m.mailbox_id, m.mailbox_name.clone()))
            .collect();
        design::card_titled(
            ui,
            "Persisted breach tracking",
            Some(
                "recorded breach events; the thresholds feed the coarse \
                 tracker for mailboxes without business-hours config",
            ),
            |ui| {
                design::badge(
                    ui,
                    &format!("{} recorded breach event(s)", self.tracked_breaches),
                    BadgeTone::Info,
                );
                ui.add_space(6.0);
                design::section_title(ui, "Tracking thresholds (hours)");
                ui.horizontal_wrapped(|ui| {
                    if mailboxes.is_empty() {
                        design::hint_text(ui, "(no mailboxes synced yet)");
                    }
                    for (id, name) in &mailboxes {
                        if design::chip(ui, self.sla_cfg_pick == Some(*id), name).clicked() {
                            self.pick_sla_cfg_mailbox(db, *id);
                        }
                    }
                });
                design::form_grid(ui, "sla-track-cfg", |ui| {
                    let fr_label = design::field_label(ui, "first response hours:");
                    if ui
                        .add(egui::TextEdit::singleline(&mut self.sla_cfg_fr).desired_width(120.0))
                        .labelled_by(fr_label.id)
                        .changed()
                    {
                        self.sla_cfg_note = None;
                    }
                    ui.end_row();
                    let res_label = design::field_label(ui, "resolution hours:");
                    if ui
                        .add(egui::TextEdit::singleline(&mut self.sla_cfg_res).desired_width(120.0))
                        .labelled_by(res_label.id)
                        .changed()
                    {
                        self.sla_cfg_note = None;
                    }
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if design::button(ui, "Save thresholds", ButtonVariant::Primary).clicked() {
                        self.save_sla_cfg(db);
                    }
                    if let Some((ok, msg)) = &self.sla_cfg_note {
                        if *ok {
                            design::success_text(ui, msg);
                        } else {
                            design::error_text(ui, msg);
                        }
                    }
                });
                design::hint_text(
                    ui,
                    "Mailboxes with business-hours config are tracked by the live \
                     evaluation; these plain hours apply only where it has no \
                     config (defaults 24h / 72h). Breaches persist deduped per \
                     conversation and kind, and land on the customer's milestone \
                     timeline.",
                );
            },
        );
    }

    /// Item 150: selecting a mailbox loads its current tracking
    /// thresholds (or the schema defaults when unset) into the editor.
    fn pick_sla_cfg_mailbox(&mut self, db: &DbHandle, mailbox_id: i64) {
        self.sla_cfg_pick = Some(mailbox_id);
        self.sla_cfg_note = None;
        let cfg = db.with_read(|conn| {
            spp_core::intelligence_features::get_sla_config(conn, mailbox_id)
                .ok()
                .flatten()
        });
        match cfg {
            Some(cfg) => {
                self.sla_cfg_fr = cfg.first_response_hours.to_string();
                self.sla_cfg_res = cfg.resolution_hours.to_string();
            }
            None => {
                self.sla_cfg_fr = "24".into();
                self.sla_cfg_res = "72".into();
            }
        }
    }

    /// Item 150: save the per-mailbox tracking thresholds through the
    /// core write API (set_sla_config), then run the coarse tracker for
    /// the unmonitored mailboxes so the recorded count reflects the new
    /// thresholds immediately.
    fn save_sla_cfg(&mut self, db: &DbHandle) {
        let Some(mailbox_id) = self.sla_cfg_pick else {
            self.sla_cfg_note = Some((false, "pick a mailbox first".into()));
            return;
        };
        let fr = self.sla_cfg_fr.trim().parse::<f64>();
        let res = self.sla_cfg_res.trim().parse::<f64>();
        let (fr, res) = match (fr, res) {
            (Ok(fr), Ok(res)) if fr > 0.0 && res > 0.0 => (fr, res),
            _ => {
                self.sla_cfg_note =
                    Some((false, "both thresholds must be positive numbers".into()));
                return;
            }
        };
        let outcome = db.with_write(|conn| {
            spp_core::intelligence_features::set_sla_config(conn, mailbox_id, fr, res)?;
            spp_core::intelligence_features::sweep_unmonitored_sla(conn)?;
            Ok::<(), spp_core::error::Error>(())
        });
        match outcome {
            Ok(()) => {
                self.sla_cfg_note = Some((true, "thresholds saved".into()));
                self.tracked_breaches = db
                    .with_read(spp_core::intelligence_features::count_sla_breaches)
                    .unwrap_or(0);
            }
            Err(e) => {
                self.sla_cfg_note = Some((false, format!("save failed: {e}")));
            }
        }
    }

    // ------------------------------------------------------------------
    // Item 56: the Builder tab — the report-builder surface, previously
    // computed but never rendered, is live here: catalog-driven
    // metric × dimension, the filter set, comparison, sort, limit, and
    // the saved-report CRUD over report_definitions.
    // ------------------------------------------------------------------

    /// The form's current config. Err carries the validation message.
    fn build_config(&self) -> std::result::Result<spp_core::reports::ReportBuilderConfig, String> {
        let metric = spp_core::catalog::ReportMetricKey::ALL
            .get(self.b_metric)
            .copied()
            .ok_or_else(|| "pick a metric".to_string())?;
        let dimension = spp_core::catalog::ReportDimensionKey::ALL
            .get(self.b_dimension)
            .copied()
            .ok_or_else(|| "pick a dimension".to_string())?;
        if self.b_from.trim().len() != 10 || self.b_to.trim().len() != 10 {
            return Err("dates use the YYYY-MM-DD form".into());
        }
        let mut filters = spp_core::reports::ReportFilters::default();
        let text = |s: &str| {
            let t = s.trim();
            (!t.is_empty()).then(|| t.to_string())
        };
        filters.channel = text(&self.b_f_channel);
        filters.min_priority = text(&self.b_f_min_priority);
        filters.attribute_key = text(&self.b_f_attr_key);
        filters.attribute_value = text(&self.b_f_attr_value);
        filters.state_key = text(&self.b_f_state_key);
        let list = |s: &str| {
            let items: Vec<String> = s
                .split(',')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .collect();
            (!items.is_empty()).then_some(items)
        };
        filters.statuses = list(&self.b_f_statuses);
        filters.tags_any = list(&self.b_f_tags_any);
        filters.tags_none = list(&self.b_f_tags_none);
        let ids = |s: &str| -> std::result::Result<Option<Vec<i64>>, String> {
            let mut out = Vec::new();
            for item in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
                out.push(
                    item.parse::<i64>()
                        .map_err(|_| format!("'{item}' is not an id"))?,
                );
            }
            Ok((!out.is_empty()).then_some(out))
        };
        filters.mailbox_local_ids = ids(&self.b_f_mailbox_ids)?;
        filters.assignee_local_ids = ids(&self.b_f_assignee_ids)?;
        let limit = if self.b_limit.trim().is_empty() {
            None
        } else {
            Some(
                self.b_limit
                    .trim()
                    .parse::<i64>()
                    .map_err(|_| "limit must be a number".to_string())?,
            )
        };
        Ok(spp_core::reports::ReportBuilderConfig {
            metric,
            dimension,
            date_from: self.b_from.trim().to_string(),
            date_to: self.b_to.trim().to_string(),
            comparison: if self.b_compare {
                spp_core::reports::ReportComparison::PreviousPeriod
            } else {
                spp_core::reports::ReportComparison::None
            },
            filters,
            sort: match self.b_sort {
                1 => spp_core::reports::ReportSort::MetricAsc,
                2 => spp_core::reports::ReportSort::DimensionAsc,
                _ => spp_core::reports::ReportSort::MetricDesc,
            },
            limit,
        })
    }

    fn builder_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        use spp_core::catalog::{ReportDimensionKey, ReportMetricKey};
        design::card(ui, |ui| {
            design::section_title(ui, "Report definition");
            ui.horizontal_wrapped(|ui| {
                design::field_label(ui, "metric:");
                let metrics = ReportMetricKey::ALL;
                let metric_label =
                    spp_core::catalog::reporting::metric_entry(metrics[self.b_metric]).label;
                egui::ComboBox::from_id_salt("b-metric")
                    .selected_text(metric_label)
                    .show_ui(ui, |ui| {
                        for (i, key) in metrics.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.b_metric,
                                i,
                                spp_core::catalog::reporting::metric_entry(*key).label,
                            );
                        }
                    })
                    .response
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            ui.is_enabled(),
                            format!("metric: {metric_label}"),
                        )
                    });
                design::field_label(ui, "grouped by:");
                let dims = ReportDimensionKey::ALL;
                let dim_label =
                    spp_core::catalog::reporting::dimension_entry(dims[self.b_dimension]).label;
                egui::ComboBox::from_id_salt("b-dim")
                    .selected_text(dim_label)
                    .show_ui(ui, |ui| {
                        for (i, key) in dims.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.b_dimension,
                                i,
                                spp_core::catalog::reporting::dimension_entry(*key).label,
                            );
                        }
                    })
                    .response
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            ui.is_enabled(),
                            format!("grouped by: {dim_label}"),
                        )
                    });
                ui.checkbox(&mut self.b_compare, "compare with previous period");
                design::field_label(ui, "sort:");
                for (i, label) in ["metric ↓", "metric ↑", "label A→Z"].iter().enumerate() {
                    if design::chip(ui, self.b_sort == i, label).clicked() {
                        self.b_sort = i;
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                let from_label = design::field_label(ui, "from (YYYY-MM-DD):");
                ui.add(egui::TextEdit::singleline(&mut self.b_from).desired_width(110.0))
                    .labelled_by(from_label.id);
                let to_label = design::field_label(ui, "to:");
                ui.add(egui::TextEdit::singleline(&mut self.b_to).desired_width(110.0))
                    .labelled_by(to_label.id);
                let limit_label = design::field_label(ui, "limit:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.b_limit)
                        .hint_text("50")
                        .desired_width(60.0),
                )
                .labelled_by(limit_label.id);
            });
            ui.add_space(4.0);
            design::section_title(ui, "Filters (blank = unfiltered)");
            design::form_grid(ui, "b-filters", |ui| {
                for (label, field) in [
                    ("channel:", &mut self.b_f_channel),
                    ("statuses (csv):", &mut self.b_f_statuses),
                    ("tags any (csv):", &mut self.b_f_tags_any),
                    ("tags none (csv):", &mut self.b_f_tags_none),
                    ("min priority:", &mut self.b_f_min_priority),
                    ("attribute key:", &mut self.b_f_attr_key),
                    ("attribute value:", &mut self.b_f_attr_value),
                    ("state key:", &mut self.b_f_state_key),
                    ("mailbox ids (csv):", &mut self.b_f_mailbox_ids),
                    ("assignee ids (csv):", &mut self.b_f_assignee_ids),
                ] {
                    let label = design::field_label(ui, label);
                    ui.add(egui::TextEdit::singleline(field).desired_width(180.0))
                        .labelled_by(label.id);
                    ui.end_row();
                }
            });
            ui.horizontal_wrapped(|ui| {
                if design::button(ui, "Run report", ButtonVariant::Primary).clicked() {
                    self.b_outcome = Some(match self.build_config() {
                        Ok(config) => db
                            .with_read(|conn| spp_core::reports::run_builder_report(conn, &config)),
                        Err(e) => Err(e),
                    });
                }
                let save_label = design::field_label(ui, "save as:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.b_save_name)
                        .hint_text("name")
                        .desired_width(160.0),
                )
                .labelled_by(save_label.id);
                if design::button(ui, "Save", ButtonVariant::Outline).clicked() {
                    self.save_report_config(db);
                }
                if let Some((ok, msg)) = &self.b_note {
                    if *ok {
                        design::success_text(ui, msg);
                    } else {
                        design::error_text(ui, msg);
                    }
                }
            });
        });
        ui.add_space(8.0);

        design::card_titled(
            ui,
            "Saved reports",
            Some("load or delete a saved configuration"),
            |ui| {
                let saved = self.b_saved.clone();
                if saved.is_empty() {
                    design::hint_text(ui, "(none saved yet)");
                }
                for saved in &saved {
                    ui.horizontal(|ui| {
                        ui.monospace(&saved.name);
                        design::hint_text(ui, &saved.created_at);
                        if design::button(ui, "load", ButtonVariant::Outline).clicked() {
                            self.load_saved_report(&saved.config);
                        }
                        // The lab's saved rows pair the outline load with a
                        // danger delete (the arm stays the shell's confirm
                        // dialog — FIND-005).
                        if design::button(ui, "delete", ButtonVariant::Destructive).clicked() {
                            // FIND-005: the delete used to run immediately —
                            // arm the shell confirm instead (it executes only
                            // when the dialog resolves Confirmed).
                            self.pending_confirm =
                                Some(ConfirmRequest::delete_saved_report(saved.id, &saved.name));
                        }
                    });
                }
            },
        );
        ui.add_space(8.0);

        match &self.b_outcome {
            None => design::hint_text(ui, "run a report to see its rows"),
            Some(Err(e)) => design::error_text(ui, &format!("error: {e}")),
            Some(Ok(outcome)) => {
                design::card_titled(
                    ui,
                    &format!("{} — {}", outcome.metric.label, outcome.dimension.label),
                    Some(&format!(
                        "{} → {}",
                        outcome.date_range.date_from, outcome.date_range.date_to
                    )),
                    |ui| {
                        design::badge(ui, outcome.origin, BadgeTone::Info);
                        ui.add_space(4.0);
                        if outcome.rows.is_empty() {
                            design::empty_state(
                                ui,
                                Icon::BarChart,
                                "no rows",
                                "Nothing matched in the range.",
                            );
                        } else {
                            let format = outcome.metric.format;
                            let has_comparison = outcome.comparison_rows.is_some();
                            let headers: Vec<&str> = if has_comparison {
                                vec!["group", "value", "Δ vs previous"]
                            } else {
                                vec!["group", "value"]
                            };
                            design::table(ui, "b-rows", &headers, 24.0, |ui| {
                                for row in &outcome.rows {
                                    ui.label(&row.dimension_label);
                                    design::td_num(
                                        ui,
                                        &Self::format_metric_value(format, row.value),
                                    );
                                    if has_comparison {
                                        let delta =
                                            outcome.comparison_rows.as_ref().and_then(|prev| {
                                                prev.iter()
                                                    .find(|p| {
                                                        p.dimension_value == row.dimension_value
                                                    })
                                                    .map(|p| row.value - p.value)
                                            });
                                        match delta {
                                            Some(d) => design::td_num(ui, &format!("{d:+.1}")),
                                            None => design::td_num(ui, "—"),
                                        }
                                    }
                                    ui.end_row();
                                }
                            });
                        }
                        ui.add_space(4.0);
                        for note in &outcome.notes {
                            design::hint_text(ui, note);
                        }
                    },
                );
            }
        }
    }

    /// Metric values render by their catalog format (counts as integers,
    /// rates as percentages, durations with their unit).
    fn format_metric_value(format: spp_core::catalog::reporting::MetricFormat, v: f64) -> String {
        use spp_core::catalog::reporting::MetricFormat;
        match format {
            MetricFormat::Count => format!("{}", v.round() as i64),
            MetricFormat::Rate => format!("{:.1}%", v * 100.0),
            MetricFormat::Minutes => format!("{:.1} min", v),
            MetricFormat::Hours => format!("{:.1} h", v),
            MetricFormat::Score => format!("{:.2}", v),
        }
    }

    fn save_report_config(&mut self, db: &DbHandle) {
        let name = self.b_save_name.trim().to_string();
        if name.is_empty() {
            self.b_note = Some((false, "give the report a name first".into()));
            return;
        }
        match self.build_config() {
            Ok(config) => {
                match db.with_write(|conn| spp_core::reports::save_report(conn, &name, &config)) {
                    Ok(saved) => {
                        self.b_note = Some((true, format!("saved as \"{}\"", saved.name)));
                        self.b_save_name.clear();
                        self.b_saved = db
                            .with_read(spp_core::reports::list_saved_reports)
                            .unwrap_or_default();
                    }
                    Err(e) => self.b_note = Some((false, format!("save failed: {e}"))),
                }
            }
            Err(e) => self.b_note = Some((false, e)),
        }
    }

    fn load_saved_report(&mut self, config: &serde_json::Value) {
        match spp_core::reports::parse_report_config(config) {
            Ok(config) => {
                self.b_metric = spp_core::catalog::ReportMetricKey::ALL
                    .iter()
                    .position(|k| *k == config.metric)
                    .unwrap_or(0);
                self.b_dimension = spp_core::catalog::ReportDimensionKey::ALL
                    .iter()
                    .position(|k| *k == config.dimension)
                    .unwrap_or(0);
                self.b_from = config.date_from;
                self.b_to = config.date_to;
                self.b_compare =
                    config.comparison == spp_core::reports::ReportComparison::PreviousPeriod;
                self.b_sort = match config.sort {
                    spp_core::reports::ReportSort::MetricAsc => 1,
                    spp_core::reports::ReportSort::DimensionAsc => 2,
                    spp_core::reports::ReportSort::MetricDesc => 0,
                };
                self.b_limit = config.limit.map(|l| l.to_string()).unwrap_or_default();
                let f = config.filters;
                self.b_f_channel = f.channel.unwrap_or_default();
                self.b_f_statuses = f.statuses.unwrap_or_default().join(", ");
                self.b_f_tags_any = f.tags_any.unwrap_or_default().join(", ");
                self.b_f_tags_none = f.tags_none.unwrap_or_default().join(", ");
                self.b_f_min_priority = f.min_priority.unwrap_or_default();
                self.b_f_attr_key = f.attribute_key.unwrap_or_default();
                self.b_f_attr_value = f.attribute_value.unwrap_or_default();
                self.b_f_state_key = f.state_key.unwrap_or_default();
                self.b_f_mailbox_ids = f
                    .mailbox_local_ids
                    .map(|v| {
                        v.iter()
                            .map(|i| i.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                self.b_f_assignee_ids = f
                    .assignee_local_ids
                    .map(|v| {
                        v.iter()
                            .map(|i| i.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                self.b_note = Some((true, "loaded — run to refresh".into()));
            }
            Err(issues) => {
                let first = issues
                    .first()
                    .map(|(f, m)| format!("{f}: {m}"))
                    .unwrap_or_else(|| "invalid saved config".into());
                self.b_note = Some((false, first));
            }
        }
    }

    /// FIND-005: runs only after the shell dialog confirms; keeps the
    /// Builder-tab note behavior and reports the outcome to the shell.
    pub fn delete_saved_report(&mut self, db: &DbHandle, id: i64) -> spp_core::Result<()> {
        match db.with_write(|conn| spp_core::reports::delete_saved_report(conn, id)) {
            Ok(true) => {
                self.b_note = Some((true, "deleted".into()));
                self.b_saved = db
                    .with_read(spp_core::reports::list_saved_reports)
                    .unwrap_or_default();
                Ok(())
            }
            Ok(false) => {
                self.b_note = Some((false, "already gone".into()));
                Ok(())
            }
            Err(e) => {
                self.b_note = Some((false, format!("delete failed: {e}")));
                Err(e)
            }
        }
    }

    // ------------------------------------------------------------------
    // Item 56: the Insights tab — health facts (dashboard metrics composed
    // with the friction count), the QA effectiveness report, the friction
    // stack with a bounded recompute action, and the graph-node surface.
    // ------------------------------------------------------------------

    fn insights_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let health = self.i_health.clone();
        let effectiveness = self.i_effectiveness.clone();
        if let Some(health) = &health {
            design::card_titled(
                ui,
                "Health facts",
                Some(&format!(
                    "the last {} days, over the whole mirror",
                    self.days
                )),
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        design::metric_card(
                            ui,
                            "conversations",
                            &health.total_conversations.to_string(),
                            None,
                        );
                        design::metric_card(
                            ui,
                            "SLA breaches (tracked)",
                            &health.sla_breach_count.to_string(),
                            health
                                .sla_breach_rate
                                .map(|r| format!("{:.1}% of conversations", r * 100.0))
                                .as_deref(),
                        );
                        design::metric_card(
                            ui,
                            "high friction",
                            &health.high_friction_count.to_string(),
                            health
                                .high_friction_rate
                                .map(|r| format!("{:.1}% of conversations", r * 100.0))
                                .as_deref(),
                        );
                        design::metric_card(
                            ui,
                            "avg first response",
                            &health
                                .avg_first_response_minutes
                                .map(|m| format!("{m:.0} min"))
                                .unwrap_or_else(|| "—".into()),
                            None,
                        );
                        design::metric_card(
                            ui,
                            "resolution rate",
                            &health
                                .resolution_rate
                                .map(|r| format!("{:.1}%", r * 100.0))
                                .unwrap_or_else(|| "—".into()),
                            None,
                        );
                    });
                },
            );
            ui.add_space(8.0);
        }

        if let Some(eff) = &effectiveness {
            design::card_titled(
                ui,
                "Post-resolution QA effectiveness",
                Some("avg QA score and the score distribution over every stored check"),
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        design::metric_card(
                            ui,
                            "avg QA score",
                            &eff.avg_qa_score
                                .map(|s| format!("{s:.2}"))
                                .unwrap_or_else(|| "—".into()),
                            None,
                        );
                        design::metric_card(
                            ui,
                            "checks stored",
                            &eff.total_qa_checks.to_string(),
                            None,
                        );
                        for (bucket, count) in &eff.score_distribution {
                            design::badge(ui, &format!("{bucket}: {count}"), BadgeTone::Neutral);
                        }
                    });
                },
            );
            ui.add_space(8.0);
        }

        design::card_titled(
            ui,
            "Customer effort (friction)",
            Some(
                "scores persist per conversation; crossing the threshold records a customer milestone",
            ),
            |ui| {
                ui.horizontal(|ui| {
                    design::badge(
                        ui,
                        &format!("{} high-friction conversation(s)", self.i_high_friction),
                        BadgeTone::Warning,
                    );
                    if design::button(ui, "Recompute (last 100 closed)", ButtonVariant::Outline)
                        .clicked()
                    {
                        match db.with_write(|conn| {
                            spp_core::reports::recompute_recent_friction(conn, 100)
                        }) {
                            Ok((computed, high)) => {
                                self.i_note = Some((
                                    true,
                                    format!("recomputed {computed} conversation(s), {high} high-friction"),
                                ));
                                self.reload(db);
                            }
                            Err(e) => {
                                self.i_note = Some((false, format!("recompute failed: {e}")))
                            }
                        }
                    }
                    if let Some((ok, msg)) = &self.i_note {
                        if *ok {
                            design::success_text(ui, msg);
                        } else {
                            design::error_text(ui, msg);
                        }
                    }
                });
                if self.i_friction_top.is_empty() {
                    design::hint_text(
                        ui,
                        "(no friction scores stored — recompute to fill the table)",
                    );
                } else {
                    design::table(
                        ui,
                        "insights-friction",
                        &[
                            "conversation",
                            "effort",
                            "messages",
                            "resolution",
                            "reopens",
                            "escalated",
                        ],
                        24.0,
                        |ui| {
                            for (conv, score, factors) in &self.i_friction_top {
                                design::td_num(ui, &conv.to_string());
                                design::td_num(ui, &format!("{score:.2}"));
                                design::td_num(
                                    ui,
                                    &factors
                                        .get("message_count")
                                        .and_then(serde_json::Value::as_u64)
                                        .map(|v| v.to_string())
                                        .unwrap_or_else(|| "—".into()),
                                );
                                design::td_num(
                                    ui,
                                    &factors
                                        .get("resolution_hours")
                                        .and_then(serde_json::Value::as_f64)
                                        .map(|v| format!("{v:.1} h"))
                                        .unwrap_or_else(|| "—".into()),
                                );
                                design::td_num(
                                    ui,
                                    &factors
                                        .get("reopen_count")
                                        .and_then(serde_json::Value::as_u64)
                                        .map(|v| v.to_string())
                                        .unwrap_or_else(|| "—".into()),
                                );
                                ui.label(
                                    factors
                                        .get("escalated")
                                        .and_then(serde_json::Value::as_bool)
                                        .map(|b| if b { "yes" } else { "no" })
                                        .unwrap_or("—"),
                                );
                                ui.end_row();
                            }
                        },
                    );
                }
            },
        );
        ui.add_space(8.0);

        let nodes = self.i_graph_nodes.clone();
        design::card_titled(
            ui,
            "Graph nodes",
            Some("custom nodes over the support graph; neighbors come from the live human edges"),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    design::field_label(ui, "kind:");
                    let kinds = spp_core::catalog::GraphNodeKind::ALL;
                    let kind_label = kinds[self.i_g_kind].as_str();
                    egui::ComboBox::from_id_salt("g-kind")
                        .selected_text(kind_label)
                        .show_ui(ui, |ui| {
                            for (i, kind) in kinds.iter().enumerate() {
                                ui.selectable_value(&mut self.i_g_kind, i, kind.as_str());
                            }
                        })
                        .response
                        .widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::Button,
                                ui.is_enabled(),
                                format!("kind: {kind_label}"),
                            )
                        });
                    let label_label = design::field_label(ui, "label:");
                    ui.add(egui::TextEdit::singleline(&mut self.i_g_label).desired_width(140.0))
                        .labelled_by(label_label.id);
                    let entity_label = design::field_label(ui, "entity id:");
                    ui.add(egui::TextEdit::singleline(&mut self.i_g_entity).desired_width(80.0))
                        .labelled_by(entity_label.id);
                    let props_label = design::field_label(ui, "properties (JSON):");
                    ui.add(egui::TextEdit::singleline(&mut self.i_g_props).desired_width(180.0))
                        .labelled_by(props_label.id);
                    if design::button(ui, "Add node", ButtonVariant::Primary).clicked() {
                        self.add_graph_node_action(db);
                    }
                });
                ui.add_space(4.0);
                if nodes.is_empty() {
                    design::hint_text(ui, "(no nodes stored)");
                } else {
                    design::table(
                        ui,
                        "insights-graph",
                        &["kind", "label", "entity", "created", ""],
                        24.0,
                        |ui| {
                            for node in &nodes {
                                ui.label(node.kind.as_str());
                                ui.label(node.label.as_deref().unwrap_or("—"));
                                design::td_num(
                                    ui,
                                    &node
                                        .entity_id
                                        .map(|e| e.to_string())
                                        .unwrap_or_else(|| "—".into()),
                                );
                                design::td_num(ui, &node.created_at);
                                if design::button(ui, "inspect", ButtonVariant::Outline).clicked() {
                                    self.inspect_graph_node(db, node.id);
                                }
                                ui.end_row();
                            }
                        },
                    );
                }
                if self.i_graph_pick.is_some() && !self.i_graph_neighbors.is_empty() {
                    ui.add_space(4.0);
                    design::section_title(ui, "Neighbors (live human edges)");
                    ui.horizontal_wrapped(|ui| {
                        for neighbor in &self.i_graph_neighbors {
                            design::badge(
                                ui,
                                &format!("{} #{}", neighbor.kind, neighbor.local_id),
                                BadgeTone::Info,
                            );
                        }
                    });
                }
            },
        );
    }

    /// Item 56: add a graph node through the core write API (the kind is
    /// validated there against the closed catalog).
    fn add_graph_node_action(&mut self, db: &DbHandle) {
        let Some(kind) = spp_core::catalog::GraphNodeKind::ALL
            .get(self.i_g_kind)
            .copied()
        else {
            return;
        };
        let label = self.i_g_label.trim().to_string();
        let props = self.i_g_props.trim().to_string();
        if !props.is_empty() && serde_json::from_str::<serde_json::Value>(&props).is_err() {
            self.i_note = Some((false, "properties must be valid JSON".into()));
            return;
        }
        let entity = if self.i_g_entity.trim().is_empty() {
            Ok(None)
        } else {
            self.i_g_entity
                .trim()
                .parse::<i64>()
                .map(Some)
                .map_err(|_| "entity id must be a number".to_string())
        };
        let entity = match entity {
            Ok(e) => e,
            Err(msg) => {
                self.i_note = Some((false, msg));
                return;
            }
        };
        match db.with_write(|conn| {
            spp_core::reports::add_graph_node(
                conn,
                kind.as_str(),
                entity,
                (!label.is_empty()).then_some(label.as_str()),
                (!props.is_empty()).then_some(props.as_str()),
            )
        }) {
            Ok(_) => {
                self.i_note = Some((true, "node added".into()));
                self.i_g_label.clear();
                self.i_g_entity.clear();
                self.i_g_props.clear();
                self.refresh_graph(db);
            }
            Err(e) => self.i_note = Some((false, format!("add failed: {e}"))),
        }
    }

    /// Item 56: inspect a node — get_graph_node for the stored row and
    /// get_graph_neighbors for the live human edges touching its entity.
    fn inspect_graph_node(&mut self, db: &DbHandle, node_id: Option<i64>) {
        let Some(node_id) = node_id else {
            return;
        };
        let picked = db.with_read(|conn| {
            spp_core::reports::get_graph_node(conn, node_id)
                .ok()
                .flatten()
        });
        self.i_graph_pick = Some(node_id);
        self.i_graph_neighbors = picked
            .as_ref()
            .filter(|n| n.entity_id.is_some())
            .map(|n| {
                db.with_read(|conn| {
                    spp_core::reports::get_graph_neighbors(conn, &n.kind, n.entity_id.unwrap_or(0))
                        .unwrap_or_default()
                })
            })
            .unwrap_or_default();
    }

    /// Item 56: refresh the node list (after an add) keeping the current
    /// selection's neighbors consistent.
    fn refresh_graph(&mut self, db: &DbHandle) {
        let pick = self.i_graph_pick;
        self.i_graph_nodes = db
            .with_read(|conn| spp_core::reports::list_graph_nodes(conn, Some(50)))
            .unwrap_or_default();
        if let Some(pick) = pick {
            self.inspect_graph_node(db, Some(pick));
        }
    }

    /// A3.3: the Quality tab — post-resolution QA aggregates plus the most
    /// recent closed conversations' deterministic QA cards. Computed
    /// locally from the mirror; the per-conversation AI layer stays on the
    /// inbox detail.
    /// K4: the weekly AI digest archive — on-demand generation, search,
    /// and the selected digest's narrative + facts + anomalies.
    fn digest_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::hint_text(
            ui,
            "AI-written weekly summaries over SQL-computed facts — volume, topics, sentiment, \
             anomalies, workload, and what changed. Archived and searchable.",
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if design::button_with_icon(
                ui,
                Icon::Sparkles,
                "Generate digest now",
                ButtonVariant::Primary,
            )
            .on_hover_text(
                "Collects the deterministic facts and asks the routed model for the narrative \
                 (facts are archived even without a model).",
            )
            .clicked()
            {
                let payload = r#"{"triggerKind":"manual","days":7}"#;
                match db.with_write(|conn| {
                    spp_core::jobs::enqueue_on(conn, "ai", "weekly_digest", payload, 3, 2)
                }) {
                    Ok(_) => self.sync_note = "digest requested…".into(),
                    Err(e) => self.error = Some(format!("Could not queue the digest: {e}")),
                }
            }
            let mut search_edit = self.digest_search.clone();
            let response = ui.add(
                egui::TextEdit::singleline(&mut search_edit)
                    .hint_text("search digests…")
                    .desired_width(220.0),
            );
            if response.changed() {
                self.digest_search = search_edit.clone();
                let digests = db.with_read(|conn| {
                    spp_core::digest::list_digests(
                        conn,
                        if search_edit.is_empty() {
                            None
                        } else {
                            Some(search_edit.as_str())
                        },
                        30,
                    )
                });
                self.digests = digests;
            }
        });
        ui.add_space(6.0);
        if self.digests.is_empty() {
            design::empty_state(
                ui,
                Icon::Sparkles,
                "no digests yet",
                "Generate the first weekly digest, or wait for the scheduled one.",
            );
            return;
        }

        for digest in &self.digests {
            let selected = self.digest_pick == Some(digest.id);
            let status_chip = match digest.status.as_str() {
                "complete" => "facts + narrative",
                "facts_only" => "facts only (no model routed)",
                _ => "model failed (facts kept)",
            };
            if design::chip(
                ui,
                selected,
                &format!(
                    "{} — {}",
                    // Clean-audit item 127: the cap must be `min`, not
                    // `max` — `16.max(len)` sliced a short stamp at byte
                    // 16 (panic on out-of-bounds) and a long stamp at the
                    // full length (the truncation never happened). Take
                    // the first 16 characters, or the whole stamp when
                    // shorter.
                    &digest.created_at[..16.min(digest.created_at.len())],
                    status_chip
                ),
            )
            .clicked()
            {
                self.digest_pick = Some(digest.id);
            }
        }
        let Some(pick) = self.digest_pick else {
            self.digest_pick = self.digests.first().map(|d| d.id);
            return;
        };
        let Some(digest) = self.digests.iter().find(|d| d.id == pick) else {
            return;
        };
        ui.add_space(6.0);
        design::card_titled(
            ui,
            &format!(
                "Digest {} → {}",
                &digest.period_start[..10.min(digest.period_start.len())],
                &digest.period_end[..10.min(digest.period_end.len())]
            ),
            Some("AI-generated narrative — numbers are SQL facts, not model output"),
            |ui| {
                match &digest.narrative {
                    Some(n) => {
                        ui.label(n);
                    }
                    None => {
                        design::hint_text(
                            ui,
                            match digest.status.as_str() {
                                "facts_only" => {
                                    "No model was routed when this digest ran. Route \
                                                 a Summarize model in AI Center, then generate a \
                                                 new digest."
                                }
                                _ => "The model run failed; the facts below are the record.",
                            },
                        );
                    }
                }
                ui.add_space(6.0);
                let anomalies = digest.facts["anomalies"]["anomalies"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                if !anomalies.is_empty() {
                    design::section_title(ui, "Anomalies");
                    for a in &anomalies {
                        let detail = a["detail"].as_str().unwrap_or_default();
                        ui.horizontal(|ui| {
                            design::badge(ui, "anomaly", BadgeTone::Warning);
                            ui.label(detail);
                        });
                    }
                    ui.add_space(4.0);
                }
                let new_gaps = digest.facts["what_changed"]["new_gap_candidates"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let new_articles = digest.facts["what_changed"]["changed_help_center_articles"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                design::section_title(ui, "What changed");
                if new_gaps.is_empty() && new_articles.is_empty() {
                    design::hint_text(
                        ui,
                        "No new gaps or article changes since the previous digest.",
                    );
                } else {
                    for g in &new_gaps {
                        let q = g["question"].as_str().unwrap_or_default();
                        design::hint_text(ui, &format!("new gap: {q}"));
                    }
                    for a in &new_articles {
                        let name = a["name"].as_str().unwrap_or_default();
                        design::hint_text(ui, &format!("article changed: {name}"));
                    }
                }
            },
        );
    }

    /// P1: the happiness tab — local CSAT from webhook ratings (always
    /// truthful, empty means "none received"), with the honest source
    /// note; Plus/Pro users see remote happiness in the Help Scout tab.
    fn happiness_ui(&mut self, ui: &mut egui::Ui) {
        let Some(data) = &self.happiness else {
            ui.spinner();
            return;
        };
        let great = i64_field(data, "great");
        let okay = i64_field(data, "okay");
        let not_good = i64_field(data, "not_good");
        let total = i64_field(data, "total");
        let csat = i64_field(data, "csat_percent");
        let width = ui.available_width();
        let cols = if width < 640.0 { 2 } else { 4 };
        egui::Grid::new("p1-happiness")
            .num_columns(cols)
            .spacing(egui::vec2(10.0, 10.0))
            .show(ui, |ui| {
                design::metric_card(ui, "Great", &great.to_string(), None);
                design::metric_card(ui, "Okay", &okay.to_string(), None);
                design::metric_card(ui, "Not good", &not_good.to_string(), None);
                design::metric_card(ui, "CSAT (great ÷ all)", &format!("{csat}%"), None);
                ui.end_row();
            });
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!(
                "{} ratings received in the last {} days. {} — Help Scout has no list-ratings \
                   API, so an empty table means none arrived, not zero happened.",
                total,
                i64_field(data, "range_days"),
                "Ratings arrive via the satisfaction webhook",
            ))
            .small()
            .color(ui.visuals().weak_text_color()),
        );
        if self.hs_capability == spp_core::reports_api::ReportsCapability::Available {
            ui.label(
                egui::RichText::new(
                    "Your plan allows the Reports API — Help Scout's own happiness report is \
                     in the Help Scout tab.",
                )
                .small()
                .color(ui.visuals().hyperlink_color),
            );
        }
        ui.add_space(8.0);
        let recent = data["recent"].as_array().cloned().unwrap_or_default();
        if recent.is_empty() {
            design::empty_state(
                ui,
                Icon::HeartPulse,
                "no ratings received in range",
                "When a customer rates a conversation, the webhook delivers it here.",
            );
            return;
        }
        design::table(
            ui,
            "p1-happiness-list",
            &["Rating", "Conversation", "Customer", "Comment"],
            28.0,
            |ui| {
                for r in &recent {
                    let rating = str_field(r, "rating");
                    if rating == "great" {
                        ui.label(egui::RichText::new("Great").color(ui.visuals().hyperlink_color));
                    } else if rating == "not-good" {
                        design::badge(ui, "Not good", BadgeTone::Destructive);
                    } else {
                        ui.label("Okay");
                    }
                    let number = r["conversation_number"].as_i64();
                    design::td_num(
                        ui,
                        &number.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                    );
                    ui.label(str_field(r, "customer"));
                    ui.label(str_field(r, "comments"));
                    ui.end_row();
                }
            },
        );
    }

    /// K5: the Help Scout Reports API tab — the 32-endpoint registry behind
    /// capability detection; Standard plans get the one honest sentence.
    fn help_scout_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let banner = self.hs_capability.banner();
        if !banner.is_empty() {
            design::hint_text(ui, banner);
            ui.add_space(6.0);
        } else {
            ui.label(
                egui::RichText::new("Reports API available — fetched reports render below.")
                    .small()
                    .color(ui.visuals().hyperlink_color),
            );
            ui.add_space(6.0);
        }
        ui.horizontal(|ui| {
            let specs = spp_core::reports_api::REPORTS;
            let current = &specs[self.hs_pick.min(specs.len() - 1)];
            let selected_label = format!("{} — {}", current.family.label(), current.name);
            egui::ComboBox::from_id_salt("k5-report-pick")
                .selected_text(selected_label.clone())
                .width(340.0)
                .show_ui(ui, |ui| {
                    for (i, spec) in specs.iter().enumerate() {
                        ui.selectable_value(
                            &mut self.hs_pick,
                            i,
                            format!("{} — {}", spec.family.label(), spec.name),
                        );
                    }
                })
                .response
                .widget_info(|| {
                    // The visible "report picker" has no text label of its
                    // own — the combo button carries the name or kittest's
                    // a11y check trips on an unlabeled node.
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        ui.is_enabled(),
                        format!("report: {selected_label}"),
                    )
                });
            if design::button(ui, "Fetch from Help Scout", ButtonVariant::Outline)
                .on_hover_text(
                    "GET the report for the selected range. A 403 answer means the plan does \
                     not include the Reports API and is recorded as such.",
                )
                .clicked()
            {
                let start = spp_core::business_hours::iso_days_ago_at(
                    self.days,
                    self.today.unwrap_or_else(chrono::Utc::now),
                );
                let end = spp_core::business_hours::now_iso_millis_at(
                    self.today.unwrap_or_else(chrono::Utc::now),
                );
                let payload = serde_json::json!({
                    "key": spp_core::reports_api::REPORTS[self.hs_pick].key,
                    "start": start,
                    "end": end,
                })
                .to_string();
                match db
                    .with_write(|conn| spp_core::jobs::enqueue(conn, "fetch_hs_report", &payload))
                {
                    Ok(_) => self.sync_note = "fetching report…".into(),
                    Err(e) => self.error = Some(format!("Could not queue the fetch: {e}")),
                }
            }
        });
        ui.add_space(8.0);
        if self.hs_cached.is_empty() {
            design::empty_state(
                ui,
                Icon::BarChart,
                "no reports fetched yet",
                "Pick a report and fetch it — results cache here with their range.",
            );
            return;
        }
        for (key, name, fetched_at, data) in &self.hs_cached {
            let is_pick = spp_core::reports_api::REPORTS[self.hs_pick].key == *key;
            ui.collapsing(
                egui::RichText::new(format!("{name} — fetched {fetched_at}")).strong(),
                |ui| {
                    let flat = spp_core::reports_api::flatten_report(data);
                    if flat.is_empty() {
                        ui.label(
                            egui::RichText::new(
                                "The report returned no summary rows (drilldown-style reports \
                                 paginate; fetch returns the raw page).",
                            )
                            .small()
                            .color(ui.visuals().weak_text_color()),
                        );
                        return;
                    }
                    egui::Grid::new(format!("k5-flat-{key}"))
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            for (label, value) in flat.iter().take(40) {
                                ui.label(egui::RichText::new(label.as_str()).small());
                                ui.label(egui::RichText::new(value.as_str()).small().strong());
                                ui.end_row();
                            }
                        });
                    let _ = is_pick;
                },
            );
        }
    }

    fn quality_ui(&mut self, ui: &mut egui::Ui) {
        let Some(overview) = self.quality.clone() else {
            ui.spinner();
            return;
        };
        design::card_titled(
            ui,
            "Post-resolution QA",
            Some(
                "deterministic checks over closed conversations (AI layer per conversation in the inbox)",
            ),
            |ui| {
                let render_metric = |ui: &mut egui::Ui, key: &str, label: &str| {
                    if let Some(value) = overview.get(key) {
                        ui.horizontal(|ui| {
                            design::field_label(ui, label);
                            ui.monospace(
                                egui::RichText::new(value.as_str().unwrap_or(&value.to_string()))
                                    .color(design::tokens(ui).foreground),
                            );
                        });
                    }
                };
                for (key, label) in [
                    ("closed_conversations", "Closed conversations"),
                    ("qa_rows", "QA rows computed"),
                    ("with_ai_layer", "Conversations with the AI layer"),
                ] {
                    render_metric(ui, key, label);
                }
                design::hint_text(
                    ui,
                    "Deterministic checks: back-and-forth, repeated information, messages after close, handoffs, first-response and resolution minutes — per conversation in the inbox detail.",
                );
            },
        );
        ui.add_space(10.0);
        design::card_titled(
            ui,
            "Recent closed conversations",
            Some("the 25 most recent closures with their QA rows"),
            |ui| {
                if self.quality_rows.is_empty() {
                    design::hint_text(
                        ui,
                        "No closed conversations yet — QA rows appear here (and in the inbox detail) after the first closure.",
                    );
                    return;
                }
                egui::ScrollArea::vertical()
                    .id_salt("reports-quality-rows")
                    .max_height(420.0)
                    .show_rows(ui, 46.0, self.quality_rows.len(), |ui, range| {
                        for row in &self.quality_rows[range] {
                            let id = row.get("conversation_id").and_then(|v| v.as_i64());
                            let det = row.get("deterministic").cloned().unwrap_or_default();
                            ui.horizontal(|ui| {
                                // Clean-audit item 128: `id` is an Option —
                                // the Debug format rendered `#Some(42)`.
                                ui.monospace(match id {
                                    Some(n) => format!("#{n}"),
                                    None => "#?".to_string(),
                                });
                                for (key, label) in [
                                    ("back_and_forth_count", "b&f"),
                                    ("handoff_count", "handoffs"),
                                    ("resolution_minutes", "res min"),
                                ] {
                                    if let Some(value) = det.get(key) {
                                        design::badge(
                                            ui,
                                            &format!("{label} {}", value.as_i64().unwrap_or(0)),
                                            BadgeTone::Neutral,
                                        );
                                    }
                                }
                                if row.get("ai").is_some() {
                                    design::badge(ui, "AI review", BadgeTone::Success);
                                }
                            });
                        }
                    });
            },
        );
    }

    fn radar_ui(&mut self, ui: &mut egui::Ui) {
        design::card_titled(
            ui,
            "Issue radar",
            Some("AI-derived alerts from local conversation data"),
            |ui| {
                if self.radar.is_empty() {
                    design::empty_state(
                        ui,
                        Icon::Target,
                        "(no alerts)",
                        "No AI-derived alerts for the current data.",
                    );
                    return;
                }
                for alert in &self.radar {
                    let title = str_field(alert, "title");
                    let severity = str_field(alert, "severity");
                    let count = alert
                        .get("conversation_ids")
                        .and_then(|v| v.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    let tone = if severity == "high" {
                        (BadgeTone::Destructive, "high")
                    } else {
                        (BadgeTone::Warning, "medium")
                    };
                    ui.horizontal(|ui| {
                        design::badge(ui, tone.1, tone.0);
                        ui.label(
                            egui::RichText::new(format!("{title} · {count} conversations"))
                                .size(13.0)
                                .color(design::tokens(ui).foreground),
                        );
                    });
                    ui.add_space(2.0);
                }
            },
        );
        // The lab's DS-C-15 quirk receipt — the raw severity token stays
        // visible (DC C-1) and the current engine never emits 'high'.
        design::quirk(
            ui,
            "Reskin honesty: this tab's badge is always 'medium'/Warning in the current build \
             because the engine never emits 'high' (reports.rs:1997-2001). The raw severity \
             token is shown here.",
        );
    }

    fn list_ui(ui: &mut egui::Ui, rows: &[serde_json::Value], title: &str) {
        design::card_titled(ui, title, None, |ui| {
            if rows.is_empty() {
                design::empty_state(
                    ui,
                    Icon::BarChart,
                    "(no data)",
                    "No rows were returned for this report.",
                );
                return;
            }
            let t = design::tokens(ui);
            for row in rows {
                let question = str_field(row, "question");
                let label = if question.is_empty() {
                    str_field(row, "category")
                } else {
                    question
                };
                let count = i64_field(row, "count");
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(label).size(13.0).color(t.foreground));
                    if count > 0 {
                        design::badge(ui, &count.to_string(), BadgeTone::Neutral);
                    }
                });
                ui.add_space(2.0);
            }
        });
    }
}

impl Default for ReportsPage {
    fn default() -> Self {
        Self::new()
    }
}

impl ReportsPage {
    /// RS-4-11 clock seam: pin the instant the page's windows and builder
    /// date inputs derive from. Tests call this with a fixed anchor so the
    /// rendered dates are constant; production never pins (the wall clock
    /// rules). Re-seeds the builder date inputs from the anchor — they are
    /// construction-time state, so pinning after construction must refresh
    /// them for the pin to be visible.
    pub fn pin_today(&mut self, today: chrono::DateTime<chrono::Utc>) {
        self.today = Some(today);
        self.b_from = spp_core::business_hours::iso_days_ago_at(30, today)
            .get(..10)
            .unwrap_or("1970-01-01")
            .to_string();
        self.b_to = spp_core::business_hours::now_iso_millis_at(today)
            .get(..10)
            .unwrap_or("2100-01-01")
            .to_string();
        self.overview = None; // the next reload runs through the anchor
    }
}

/// Extra constructor used by the reports page.
impl DashboardScope {
    /// Scope with only the day range set.
    #[must_use]
    pub fn days(days: i64) -> Self {
        Self {
            days,
            mailbox_ids: Vec::new(),
            channel: None,
            today: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn sla_report_loads_and_bar_points_map() {
        let db = fixture(Vec::new());
        let report = db.with_read(|conn| {
            spp_core::sla::sla_report(
                conn,
                &spp_core::business_hours::iso_days_ago(30),
                &spp_core::business_hours::now_iso_millis(),
                None,
            )
            .expect("sla report")
        });
        // Two seeded mailboxes, no business-hours config rows yet.
        assert!(report.mailboxes.len() >= 2);
        assert!(report.range.from.starts_with("20"));

        let bars = sla_first_response_bars(&report);
        assert!(bars.is_empty(), "no first-response data in fixture");
    }

    #[test]
    fn overview_and_lists_load_from_seeded_db() {
        let db = fixture(vec![("conv", 1, "Help export", "active", "email", 1, 5, 9)]);
        let mut page = ReportsPage::new();
        page.reload(&db);

        let overview = page.overview.clone().expect("overview");
        assert_eq!(overview.new_conversations, 1);
        assert_eq!(page.why_contacting.len(), 0);
        assert_eq!(page.top_questions.len(), 0);
        assert_eq!(page.radar.len(), 0);
        assert!(page.sla.is_some());

        // Day range switch reloads.
        page.days = 7;
        page.reload(&db);
        assert_eq!(page.overview.expect("overview 2").new_conversations, 1);
    }

    #[test]
    fn why_contacting_rows_have_category_shape() {
        // The list renderer reads `category` rows; a synthetic row maps.
        let row = serde_json::json!({ "category": "billing", "count": 3 });
        assert_eq!(i64_field(&row, "count"), 3);
        assert_eq!(str_field(&row, "category"), "billing");
    }

    /// Item 56: the builder form's config builds, runs through
    /// run_builder_report, saves, lists, loads back into the form, and
    /// deletes — the full Builder-tab path over a real fixture DB.
    #[test]
    fn builder_tab_run_save_load_delete_round_trip() {
        let db = fixture(Vec::new());
        let mut page = ReportsPage::new();
        page.reload(&db);
        page.b_save_name = "my report".into();
        page.save_report_config(&db);
        assert!(
            page.b_note.as_ref().expect("note").0,
            "save over a clean mirror succeeds"
        );
        assert_eq!(page.b_saved.len(), 1);

        // Loading restores the form fields from the stored JSON.
        let config_json = page.b_saved[0].config.clone();
        page.b_metric = 3;
        page.b_from = "2000-01-01".into();
        page.load_saved_report(&config_json);
        assert_eq!(page.b_metric, 0, "the saved metric is restored");
        assert!(page.b_note.as_ref().expect("note").0);

        // Running the loaded config produces an outcome with notes.
        let config = page.build_config().expect("config");
        let outcome = db
            .with_read(|conn| spp_core::reports::run_builder_report(conn, &config))
            .expect("run");
        assert!(!outcome.notes.is_empty());
        page.delete_saved_report(&db, page.b_saved[0].id).ok();
        assert!(page.b_saved.is_empty(), "deleted");
    }

    /// Item 56: the builder form validates its inputs instead of sending
    /// garbage to the core (bad ids / bad limit / short dates are errors).
    #[test]
    fn builder_form_validates_inputs() {
        let mut page = ReportsPage::new();
        page.b_f_mailbox_ids = "abc".into();
        assert!(page.build_config().is_err());
        page.b_f_mailbox_ids.clear();
        page.b_limit = "many".into();
        assert!(page.build_config().is_err());
        page.b_limit.clear();
        page.b_from = "bad".into();
        assert!(page.build_config().is_err());
        page.b_from = "2026-01-01".into();
        assert!(page.build_config().is_ok());
    }

    /// Item 56: the Insights tab's data loads with the page — health
    /// facts, effectiveness, friction, and the graph-node list all come
    /// from the reload pass.
    #[test]
    fn insights_tab_loads_surface_data() {
        let db = fixture(Vec::new());
        let mut page = ReportsPage::new();
        page.reload(&db);
        assert!(page.i_health.is_some(), "health facts loaded");
        assert!(page.i_effectiveness.is_some(), "effectiveness loaded");

        // The graph-node surface round-trips through the page action.
        page.i_g_kind = 8; // Campaign — any catalog kind
        page.i_g_label = "Spring push".into();
        page.add_graph_node_action(&db);
        assert!(page.i_note.as_ref().expect("note").0, "node added");
        assert_eq!(page.i_graph_nodes.len(), 1);
        let node = &page.i_graph_nodes[0];
        assert_eq!(node.kind, "campaign");
        assert_eq!(node.label.as_deref(), Some("Spring push"));

        // Invalid JSON properties are rejected before any write.
        page.i_g_label = "bad".into();
        page.i_g_props = "{not json".into();
        page.add_graph_node_action(&db);
        assert!(!page.i_note.as_ref().expect("note").0, "rejected");
        assert_eq!(page.i_graph_nodes.len(), 1, "nothing was written");
    }
}
