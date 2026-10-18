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
                spp_core::http::routes::analytics::why_contacting_impl(conn, self.days),
                spp_core::http::routes::analytics::top_questions_impl(conn, self.days),
                spp_core::http::routes::analytics::issue_radar_impl(conn),
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

        ui.heading("Reports");
        ui.horizontal(|ui| {
            ui.label("range:");
            for (d, label) in crate::pages::dashboard::RANGES {
                if ui.selectable_label(self.days == d, label).clicked() {
                    self.days = d;
                    self.reload(db);
                }
            }
        });
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
        });
        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("error: {err}"));
        }
        ui.separator();

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
        egui::Grid::new("reports-overview")
            .num_columns(4)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                for (label, value) in [
                    ("New", data.new_conversations),
                    ("Closed", data.closed_conversations),
                    ("Replies sent", data.replies_sent),
                    ("Backlog", data.backlog),
                    ("Great ratings", data.ratings_great),
                    ("Okay ratings", data.ratings_okay),
                    ("Not-good ratings", data.ratings_not_good),
                    ("Unassigned", data.unassigned),
                ] {
                    ui.strong(label);
                    ui.monospace(value.to_string());
                }
            });
        ui.separator();

        ui.strong("New conversations per day");
        let points: Vec<chart::Point> = crate::pages::dashboard::daily_new_points(data);
        if points.is_empty() {
            ui.weak("(no data in range)");
            return;
        }
        chart::line_chart(ui, "reports-daily-new", &points, "new conversations");
    }

    fn sla_ui(&mut self, ui: &mut egui::Ui) {
        let Some(report) = &self.sla else {
            ui.weak("(no SLA report)");
            return;
        };
        if !report.unconfigured_mailboxes.is_empty() {
            ui.weak(format!(
                "unconfigured mailboxes: {}",
                report.unconfigured_mailboxes.join(", ")
            ));
            ui.separator();
        }

        let bars = sla_first_response_bars(report);
        ui.strong("First response avg (business minutes)");
        chart::bar_chart(ui, "reports-sla-bars", &bars, "first response avg");
        ui.separator();

        egui::ScrollArea::vertical().show_rows(ui, 22.0, report.mailboxes.len(), |ui, range| {
            egui::Grid::new("reports-sla-table")
                .num_columns(7)
                .striped(true)
                .show(ui, |ui| {
                    for header in [
                        "mailbox",
                        "in range",
                        "fr met/missed",
                        "fr avg (min)",
                        "res met/missed",
                        "res avg (min)",
                        "waiting oldest (min)",
                    ] {
                        ui.strong(header);
                    }
                    ui.end_row();
                    for row in &report.mailboxes[range] {
                        ui.label(row.mailbox_name.as_str());
                        ui.monospace(row.conversations_in_range.to_string());
                        ui.monospace(format!(
                            "{}/{}",
                            row.first_response.met, row.first_response.missed
                        ));
                        ui.monospace(
                            row.first_response
                                .avg_business_min
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "—".into()),
                        );
                        ui.monospace(format!("{}/{}", row.resolution.met, row.resolution.missed));
                        ui.monospace(
                            row.resolution
                                .avg_business_min
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "—".into()),
                        );
                        ui.monospace(
                            row.waiting
                                .oldest_business_min
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "—".into()),
                        );
                        ui.end_row();
                    }
                });
        });
    }

    fn radar_ui(&mut self, ui: &mut egui::Ui) {
        if self.radar.is_empty() {
            ui.weak("(no alerts)");
            return;
        }
        egui::ScrollArea::vertical().show_rows(ui, 20.0, self.radar.len(), |ui, range| {
            for alert in &self.radar[range] {
                let title = str_field(alert, "title");
                let severity = str_field(alert, "severity");
                let count = alert
                    .get("conversation_ids")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                let color = if severity == "high" {
                    egui::Color32::RED
                } else {
                    egui::Color32::YELLOW
                };
                ui.colored_label(
                    color,
                    format!("{title} · {count} conversations · {severity}"),
                );
            }
        });
    }

    fn list_ui(ui: &mut egui::Ui, rows: &[serde_json::Value], title: &str) {
        ui.strong(title);
        if rows.is_empty() {
            ui.weak("(no data)");
            return;
        }
        egui::ScrollArea::vertical().show_rows(ui, 20.0, rows.len(), |ui, range| {
            for row in &rows[range] {
                let question = str_field(row, "question");
                let label = if question.is_empty() {
                    str_field(row, "category")
                } else {
                    question
                };
                let count = i64_field(row, "count");
                if count > 0 {
                    ui.label(format!("{} · {}", label, count));
                } else {
                    ui.label(label.as_str());
                }
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
