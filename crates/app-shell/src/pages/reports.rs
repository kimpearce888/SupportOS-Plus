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
use crate::design::{self, BadgeTone};
use crate::icons::Icon;
use crate::pages::dashboard::{DashboardData, DashboardScope, load_dashboard};

/// Report tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Sla,
    WhyContacting,
    TopQuestions,
    Radar,
}

impl Tab {
    const ALL: [Self; 5] = [
        Self::Overview,
        Self::Sla,
        Self::WhyContacting,
        Self::TopQuestions,
        Self::Radar,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Sla => "SLA",
            Self::WhyContacting => "Why contacting",
            Self::TopQuestions => "Top questions",
            Self::Radar => "Issue radar",
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
    tab: Tab,
    days: i64,
    overview: Option<DashboardData>,
    sla: Option<SlaReport>,
    why_contacting: Vec<serde_json::Value>,
    top_questions: Vec<serde_json::Value>,
    radar: Vec<serde_json::Value>,
    error: Option<String>,
}

impl ReportsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::Overview,
            days: 30,
            overview: None,
            sla: None,
            why_contacting: Vec::new(),
            top_questions: Vec::new(),
            radar: Vec::new(),
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
    }

    fn reload(&mut self, db: &DbHandle) {
        let from = spp_core::business_hours::iso_days_ago(self.days);
        let to = spp_core::business_hours::now_iso_millis();
        let (overview, sla, why, questions, radar) = db.with_read(|conn| {
            (
                Some(load_dashboard(conn, &DashboardScope::days(self.days))),
                spp_core::sla::sla_report(conn, &from, &to, None).ok(),
                spp_core::analytics::why_contacting_impl(conn, self.days),
                spp_core::analytics::top_questions_impl(conn, self.days),
                spp_core::analytics::issue_radar_impl(conn),
            )
        });
        self.overview = overview;
        self.sla = sla;
        self.why_contacting = why;
        self.top_questions = questions;
        self.radar = radar;
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.overview.is_none() {
            self.reload(db);
        }
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
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
