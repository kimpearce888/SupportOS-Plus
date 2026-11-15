use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::sla::SlaReport;

use super::{ConfirmRequest, NavRequest, i64_field, str_field};
use crate::chart;
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;
use crate::pages::dashboard::{DashboardData, DashboardScope, load_dashboard};
mod insights;
mod sla;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Overview,
    Sla,
    WhyContacting,
    TopQuestions,
    Quality,
    Radar,
    Digest,
    Happiness,
    HelpScout,
    Builder,
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

pub struct ReportsPage {
    pub sync_note: String,
    pending_confirm: Option<ConfirmRequest>,

    tab: Tab,
    days: i64,
    today: Option<chrono::DateTime<chrono::Utc>>,
    overview: Option<DashboardData>,
    sla: Option<SlaReport>,
    why_contacting: Vec<serde_json::Value>,
    top_questions: Vec<serde_json::Value>,
    radar: Vec<serde_json::Value>,
    quality: Option<serde_json::Value>,
    quality_rows: Vec<serde_json::Value>,
    digests: Vec<spp_core::digest::DigestRow>,
    digest_search: String,
    digest_pick: Option<i64>,
    happiness: Option<serde_json::Value>,
    hs_capability: spp_core::reports_api::ReportsCapability,
    hs_cached: Vec<(&'static str, &'static str, String, serde_json::Value)>,
    hs_pick: usize,
    tracked_breaches: u32,
    sla_cfg_pick: Option<i64>,
    sla_cfg_fr: String,
    sla_cfg_res: String,
    sla_cfg_note: Option<(bool, String)>,
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
    b_mailboxes: Vec<(i64, String)>,
    b_assignees: Vec<(i64, String)>,
    b_outcome: Option<std::result::Result<spp_core::reports::ReportRunOutcome, String>>,
    b_saved: Vec<spp_core::reports::SavedReport>,
    b_save_name: String,
    b_note: Option<(bool, String)>,
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
    pub(crate) fn sync_finished(&mut self, outcome: &str) {
        if !self.sync_note.is_empty() {
            self.sync_note = outcome.to_string();
        }
    }

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
            b_mailboxes: Vec::new(),
            b_assignees: Vec::new(),
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

    pub(crate) fn invalidate(&mut self) {
        self.overview = None;
        self.sla = None;
        self.why_contacting.clear();
        self.top_questions.clear();
        self.radar.clear();
        self.digests.clear();
        self.happiness = None;
        self.hs_capability = spp_core::reports_api::ReportsCapability::Unknown;
        self.hs_cached.clear();
        self.tracked_breaches = 0;
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
        self.b_mailboxes = db.with_read(crate::pages::dashboard::mailboxes);
        self.b_assignees = db.with_read(crate::pages::inbox::assignable_users);
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
                spp_core::intelligence_features::count_sla_breaches(conn).unwrap_or(0),
            )
        });
        self.digests = digests;
        self.happiness = happiness;
        self.hs_capability = capability;
        self.hs_cached = hs_cached;
        self.tracked_breaches = tracked;
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
                if crate::pages::sync_health::sync_now_button(ui, db, engine, &mut self.sync_note) {
                    self.invalidate();
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
        let labels: Vec<&str> = Tab::ALL.iter().map(|t| t.label()).collect();
        if let Some(clicked) = design::tabs(
            ui,
            &labels,
            Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0),
        ) && let Some(tab) = Tab::ALL.get(clicked)
        {
            self.tab = *tab;
        }
        ui.add_space(6.0);
        if let Some(err) = &self.error {
            design::error_note(ui, err);
        }

        let (why, questions) = (self.why_contacting.clone(), self.top_questions.clone());
        let mut nav: Option<NavRequest> = None;
        match self.tab {
            Tab::Overview => self.overview_ui(ui),
            Tab::Sla => self.sla_ui(ui, db),
            Tab::WhyContacting => Self::list_ui(ui, &why, "why contacting"),
            Tab::TopQuestions => Self::list_ui(ui, &questions, "top questions"),
            Tab::Radar => self.radar_ui(ui),
            Tab::Quality => self.quality_ui(ui),
            Tab::Digest => self.digest_ui(ui, db),
            Tab::Happiness => self.happiness_ui(ui, &mut nav),
            Tab::HelpScout => self.help_scout_ui(ui, db),
            Tab::Builder => self.builder_ui(ui, db),
            Tab::Insights => self.insights_ui(ui, db),
        }
        if let Some(request) = self.pending_confirm.take() {
            return Some(NavRequest::Confirm(request));
        }
        nav
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
        let gap = 10.0;
        let card_w = ((width - gap * (cols - 1) as f32) / cols as f32).floor();
        egui::Grid::new("reports-overview")
            .num_columns(cols)
            .spacing(egui::vec2(gap, gap))
            .show(ui, |ui| {
                for row in facts.chunks(cols) {
                    for (label, value) in row {
                        design::metric_card_w(ui, label, &value.to_string(), None, card_w);
                    }
                    ui.end_row();
                }
            });
        ui.add_space(4.0);
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
                chart::line_chart_axes(
                    ui,
                    &points,
                    "new conversations",
                    Some((
                        crate::pages::dashboard::iso_date_only(&data.range_from).as_ref(),
                        crate::pages::dashboard::iso_date_only(&data.range_to).as_ref(),
                    )),
                );
            }
        });
    }

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

    fn happiness_ui(&mut self, ui: &mut egui::Ui, nav: &mut Option<NavRequest>) {
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
        design::data_table(
            ui,
            "p1-happiness-list",
            &["Rating", "Conversation", "Customer", "Comment"],
            recent.len(),
            design::scale::CONTROL_H,
            |ui, i| {
                let r = &recent[i];
                let rating = str_field(r, "rating");
                if rating == "great" {
                    ui.label(egui::RichText::new("Great").color(ui.visuals().hyperlink_color));
                } else if rating == "not-good" {
                    design::badge(ui, "Not good", BadgeTone::Destructive);
                } else {
                    ui.label("Okay");
                }
                let number = r["conversation_number"].as_i64();
                let conversation_id = r["conversation_id"].as_i64();
                match (number, conversation_id) {
                    (Some(n), Some(id)) => {
                        let label = ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("#{n}"))
                                    .size(design::scale::text::SMALL)
                                    .color(design::tokens(ui).primary),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if label.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if label.clicked() && nav.is_none() {
                            *nav = Some(NavRequest::OpenConversation(id));
                        }
                    }
                    _ => design::td_num(
                        ui,
                        &number
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| design::MISSING_VALUE.into()),
                    ),
                }
                ui.label(str_field(r, "customer"));
                ui.label(str_field(r, "comments"));
            },
        );
    }

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

impl ReportsPage {
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
        self.overview = None;
    }
}

impl DashboardScope {
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
mod audp2_tests;
#[cfg(test)]
mod tests;

fn csv_contains_id(csv: &str, id: i64) -> bool {
    csv.split(',')
        .map(str::trim)
        .any(|s| s.parse::<i64>().map(|parsed| parsed == id).unwrap_or(false))
}

fn toggle_csv_id(csv: &str, id: i64) -> String {
    let mut parts: Vec<i64> = csv
        .split(',')
        .map(str::trim)
        .filter_map(|s| s.parse::<i64>().ok())
        .collect();
    if let Some(pos) = parts.iter().position(|p| *p == id) {
        parts.remove(pos);
    } else {
        parts.push(id);
    }
    parts
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(",")
}
