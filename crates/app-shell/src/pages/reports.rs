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

use super::{i64_field, str_field};
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
}

impl Tab {
    const ALL: [Self; 9] = [
        Self::Overview,
        Self::Sla,
        Self::WhyContacting,
        Self::TopQuestions,
        Self::Quality,
        Self::Radar,
        Self::Digest,
        Self::Happiness,
        Self::HelpScout,
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

    tab: Tab,
    days: i64,
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
    error: Option<String>,
}

impl ReportsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sync_note: String::new(),
            tab: Tab::Overview,
            days: 30,
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
    }

    fn reload(&mut self, db: &DbHandle) {
        let from = spp_core::business_hours::iso_days_ago(self.days);
        let to = spp_core::business_hours::now_iso_millis();
        let (overview, sla, why, questions, radar, quality, quality_rows) = db.with_read(|conn| {
            (
                Some(load_dashboard(conn, &DashboardScope::days(self.days))),
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
        let (digests, happiness, capability, hs_cached) = db.with_read(|conn| {
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
            )
        });
        self.digests = digests;
        self.happiness = happiness;
        self.hs_capability = capability;
        self.hs_cached = hs_cached;
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
    ) {
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
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                if design::chip(ui, self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
        });
        ui.add_space(6.0);
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }

        let (why, questions) = (self.why_contacting.clone(), self.top_questions.clone());
        match self.tab {
            Tab::Overview => self.overview_ui(ui),
            Tab::Sla => self.sla_ui(ui),
            Tab::WhyContacting => Self::list_ui(ui, &why, "why contacting"),
            Tab::TopQuestions => Self::list_ui(ui, &questions, "top questions"),
            Tab::Radar => self.radar_ui(ui),
            Tab::Quality => self.quality_ui(ui),
            Tab::Digest => self.digest_ui(ui, db),
            Tab::Happiness => self.happiness_ui(ui),
            Tab::HelpScout => self.help_scout_ui(ui, db),
        }
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

    fn sla_ui(&mut self, ui: &mut egui::Ui) {
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

        let bars = sla_first_response_bars(report);
        design::card_titled(ui, "First response avg (business minutes)", None, |ui| {
            if bars.is_empty() {
                design::hint_text(ui, "(no first-response data in range)");
            } else {
                chart::bar_chart(ui, "reports-sla-bars", &bars, "first response avg");
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
                    ui.label(egui::RichText::new("Anomalies").strong());
                    for a in &anomalies {
                        let detail = a["detail"].as_str().unwrap_or_default();
                        ui.label(
                            egui::RichText::new(format!("• {detail}"))
                                .small()
                                .color(ui.visuals().warn_fg_color),
                        );
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
                ui.label(egui::RichText::new("What changed").strong());
                if new_gaps.is_empty() && new_articles.is_empty() {
                    ui.label(
                        egui::RichText::new(
                            "No new gaps or article changes since the previous digest.",
                        )
                        .small()
                        .color(ui.visuals().weak_text_color()),
                    );
                } else {
                    for g in &new_gaps {
                        let q = g["question"].as_str().unwrap_or_default();
                        ui.label(
                            egui::RichText::new(format!("• new gap: {q}"))
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        );
                    }
                    for a in &new_articles {
                        let name = a["name"].as_str().unwrap_or_default();
                        ui.label(
                            egui::RichText::new(format!("• article changed: {name}"))
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        );
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
        egui::Grid::new("p1-happiness-list")
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Rating").strong());
                ui.label(egui::RichText::new("Conversation").strong());
                ui.label(egui::RichText::new("Customer").strong());
                ui.label(egui::RichText::new("Comment").strong());
                ui.end_row();
                for r in &recent {
                    let rating = str_field(r, "rating");
                    ui.label(match rating.as_str() {
                        "great" => egui::RichText::new("Great").color(ui.visuals().hyperlink_color),
                        "not-good" => {
                            egui::RichText::new("Not good").color(ui.visuals().error_fg_color)
                        }
                        _ => egui::RichText::new("Okay"),
                    });
                    let number = r["conversation_number"].as_i64();
                    ui.label(number.map(|n| n.to_string()).unwrap_or_else(|| "—".into()));
                    ui.label(str_field(r, "customer"));
                    ui.label(str_field(r, "comments"));
                    ui.end_row();
                }
            });
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
            egui::ComboBox::from_id_salt("k5-report-pick")
                .selected_text(format!("{} — {}", current.family.label(), current.name))
                .width(340.0)
                .show_ui(ui, |ui| {
                    for (i, spec) in specs.iter().enumerate() {
                        ui.selectable_value(
                            &mut self.hs_pick,
                            i,
                            format!("{} — {}", spec.family.label(), spec.name),
                        );
                    }
                });
            if ui
                .add(egui::Button::new(egui::RichText::new(
                    "Fetch from Help Scout",
                )))
                .on_hover_text(
                    "GET the report for the selected range. A 403 answer means the plan does \
                     not include the Reports API and is recorded as such.",
                )
                .clicked()
            {
                let start = spp_core::business_hours::iso_days_ago(self.days);
                let end = spp_core::business_hours::now_iso_millis();
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

/// Extra constructor used by the reports page.
impl DashboardScope {
    /// Scope with only the day range set.
    #[must_use]
    pub fn days(days: i64) -> Self {
        Self {
            days,
            mailbox_ids: Vec::new(),
            channel: None,
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
}
