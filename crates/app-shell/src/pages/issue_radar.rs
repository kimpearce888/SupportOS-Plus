//! Issue Radar page (port of the web `pages/issue_radar.rs`).
//!
//! Tabs (web: Alerts / Clusters / Known issues / SLA):
//! * **Alerts** — `issue_radar_impl` (`/api/reports/issue-radar`) plus
//!   `sla::sla_alerts` (`/api/issues/sla-alerts`).
//! * **Clusters** — the issue-cluster table (`/api/issues/clusters`) with
//!   conversation ids.
//! * **Known issues** — the known-issue list (`/api/issues/known`) plus the
//!   create form (the create route's validation and inserts, in-process).

use eframe::egui;
use rusqlite::Connection;
use spp_core::db::DbHandle;

use super::str_field;

/// Page tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Alerts,
    Clusters,
    Known,
}

impl Tab {
    const ALL: [Self; 3] = [Self::Alerts, Self::Clusters, Self::Known];

    fn label(self) -> &'static str {
        match self {
            Self::Alerts => "Alerts",
            Self::Clusters => "Clusters",
            Self::Known => "Known issues",
        }
    }
}

/// One issue cluster row (typed port of `/api/issues/clusters`).
#[derive(Debug, Clone, Default)]
pub struct ClusterRow {
    pub id: i64,
    pub title: String,
    pub summary: Option<String>,
    pub category: Option<String>,
    pub trend: Option<String>,
    pub conversation_count: i64,
    pub customer_count: Option<i64>,
    pub conversation_ids: Vec<i64>,
}

/// Load the cluster list.
#[must_use]
pub fn load_clusters(conn: &Connection) -> Vec<ClusterRow> {
    conn.prepare(
        "SELECT id, title, summary, category, trend, conversation_count, customer_count
           FROM issue_clusters ORDER BY conversation_count DESC",
    )
    .and_then(|mut stmt| {
        stmt.query_map([], |r| {
            let id: i64 = r.get(0)?;
            let ids: Vec<i64> = conn
                .prepare(
                    "SELECT conversation_id FROM issue_cluster_conversations WHERE cluster_id = ?1",
                )?
                .query_map(rusqlite::params![id], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(ClusterRow {
                id,
                title: r.get(1)?,
                summary: r.get(2)?,
                category: r.get(3)?,
                trend: r.get(4)?,
                conversation_count: r.get(5)?,
                customer_count: r.get(6)?,
                conversation_ids: ids,
            })
        })
        .map(|rows| rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

/// One known-issue row (typed port of `/api/issues/known`).
#[derive(Debug, Clone, Default)]
pub struct KnownIssueRow {
    pub id: i64,
    pub title: String,
    pub symptoms: Option<String>,
    pub status: String,
    pub conversation_count: i64,
    pub workaround: Option<String>,
}

/// Load the known-issue list.
#[must_use]
pub fn load_known(conn: &Connection) -> Vec<KnownIssueRow> {
    conn.prepare(
        "SELECT id, COALESCE(title, name), symptoms, status,
                COALESCE(conversation_count, 0), workaround
           FROM known_issues ORDER BY last_seen_at DESC",
    )
    .and_then(|mut stmt| {
        stmt.query_map([], |r| {
            Ok(KnownIssueRow {
                id: r.get(0)?,
                title: r.get(1)?,
                symptoms: r.get(2)?,
                status: r.get(3)?,
                conversation_count: r.get(4)?,
                workaround: r.get(5)?,
            })
        })
        .map(|rows| rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

/// Create a known issue (the create route's insert + link + refresh
/// sequence, in-process).
///
/// # Errors
/// Propagates SQL failures.
pub fn create_known_issue(
    conn: &mut Connection,
    title: &str,
    symptoms: Option<&str>,
    workaround: Option<&str>,
    status: &str,
    conversation_ids: &[i64],
) -> spp_core::Result<i64> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO known_issues (name, status, description, title, symptoms, workaround, provenance)
         VALUES (?1, ?2, '', ?1, ?3, ?4, 'human_local')",
        rusqlite::params![title, status, symptoms, workaround],
    )?;
    let id = tx.last_insert_rowid();
    for conv_id in conversation_ids {
        tx.execute(
            "INSERT OR IGNORE INTO known_issue_links (known_issue_id, conversation_id, link_type)
             VALUES (?1, ?2, 'human')",
            rusqlite::params![id, conv_id],
        )?;
    }
    spp_core::intelligence_features::refresh_known_issue_counts(&tx, id)?;
    tx.commit()?;
    Ok(id)
}

/// The interactive issue radar page state.
pub struct IssueRadarPage {
    tab: Tab,
    radar: Vec<serde_json::Value>,
    sla: Option<spp_core::sla::SlaAlerts>,
    clusters: Vec<ClusterRow>,
    known: Vec<KnownIssueRow>,
    show_new: bool,
    new_title: String,
    new_symptoms: String,
    new_workaround: String,
    new_status: String,
    new_conversation_ids: String,
    error: Option<String>,
}

impl IssueRadarPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::Alerts,
            radar: Vec::new(),
            sla: None,
            clusters: Vec::new(),
            known: Vec::new(),
            show_new: false,
            new_title: String::new(),
            new_symptoms: String::new(),
            new_workaround: String::new(),
            new_status: "investigating".into(),
            new_conversation_ids: String::new(),
            error: None,
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.radar.clear();
        self.sla = None;
        self.clusters.clear();
        self.known.clear();
    }

    fn reload(&mut self, db: &DbHandle) {
        let (radar, sla, clusters, known) = db.with_read(|conn| {
            (
                spp_core::analytics::issue_radar_impl(conn),
                spp_core::sla::sla_alerts(conn).ok(),
                load_clusters(conn),
                load_known(conn),
            )
        });
        self.radar = radar;
        self.sla = sla;
        self.clusters = clusters;
        self.known = known;
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.sla.is_none() {
            self.reload(db);
        }
    }

    fn create(&mut self, db: &DbHandle) {
        let title = self.new_title.trim().to_string();
        if title.is_empty() || title.chars().count() > 300 {
            self.error = Some("Title is required (max 300 characters).".into());
            return;
        }
        let conversation_ids: Vec<i64> = self
            .new_conversation_ids
            .split(',')
            .filter_map(|id| id.trim().parse::<i64>().ok().filter(|v| *v > 0))
            .collect();
        let symptoms = optional(&self.new_symptoms);
        let workaround = optional(&self.new_workaround);
        let status = self.new_status.clone();
        let result = db.with_write(|conn| {
            create_known_issue(
                conn,
                &title,
                symptoms.as_deref(),
                workaround.as_deref(),
                &status,
                &conversation_ids,
            )
        });
        match result {
            Ok(_) => {
                self.show_new = false;
                self.new_title.clear();
                self.new_symptoms.clear();
                self.new_workaround.clear();
                self.new_conversation_ids.clear();
                self.reload(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        ui.heading("Issue Radar");
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

        match self.tab {
            Tab::Alerts => self.alerts_ui(ui),
            Tab::Clusters => self.clusters_ui(ui),
            Tab::Known => self.known_ui(ui, db),
        }
    }

    fn alerts_ui(&mut self, ui: &mut egui::Ui) {
        let radar = self.radar.clone();
        ui.strong(format!("{} radar alert(s)", radar.len()));
        if radar.is_empty() {
            ui.weak("(no alerts)");
        } else {
            for alert in &radar {
                let title = str_field(alert, "title");
                let severity = str_field(alert, "severity");
                let count = alert
                    .get("conversation_ids")
                    .and_then(|v| v.as_array())
                    .map_or(0, Vec::len);
                let color = if severity == "critical" {
                    egui::Color32::RED
                } else {
                    egui::Color32::YELLOW
                };
                ui.colored_label(
                    color,
                    format!("{title} · {count} conversations · {severity}"),
                );
            }
        }
        ui.separator();
        let sla = self.sla.clone();
        let Some(sla) = sla else {
            ui.spinner();
            return;
        };
        ui.strong(format!(
            "SLA: {} breached · {} at risk",
            sla.total_breached, sla.total_at_risk
        ));
        if sla.alerts.is_empty() {
            ui.weak("(no SLA alerts)");
            return;
        }
        egui::ScrollArea::vertical().show_rows(ui, 20.0, sla.alerts.len(), |ui, range| {
            for alert in &sla.alerts[range] {
                let state = alert.state.as_str();
                let color = if state == "breached" {
                    egui::Color32::RED
                } else {
                    egui::Color32::YELLOW
                };
                ui.colored_label(
                    color,
                    format!(
                        "#{} {} · {} · {}",
                        alert.number,
                        alert.subject.clone().unwrap_or_default(),
                        alert.mailbox_name,
                        state
                    ),
                );
            }
        });
    }

    fn clusters_ui(&mut self, ui: &mut egui::Ui) {
        let clusters = self.clusters.clone();
        ui.strong(format!("{} cluster(s)", clusters.len()));
        if clusters.is_empty() {
            ui.weak("(no clusters yet)");
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for cluster in &clusters {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(cluster.title.as_str());
                        if let Some(trend) = &cluster.trend {
                            ui.colored_label(
                                if trend == "rising" || trend == "new" {
                                    egui::Color32::YELLOW
                                } else {
                                    egui::Color32::GRAY
                                },
                                trend,
                            );
                        }
                        ui.weak(format!(
                            "{} conversations · {} customers",
                            cluster.conversation_count,
                            cluster.customer_count.unwrap_or(0)
                        ));
                    });
                    if let Some(summary) = &cluster.summary {
                        ui.weak(summary.as_str());
                    }
                });
            }
        });
    }

    fn known_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.horizontal(|ui| {
            if ui.button("New known issue").clicked() {
                self.show_new = !self.show_new;
            }
        });
        if self.show_new {
            self.new_known_ui(ui, db);
            ui.separator();
        }
        let known = self.known.clone();
        ui.strong(format!("{} known issue(s)", known.len()));
        if known.is_empty() {
            ui.weak("(no known issues)");
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for issue in &known {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(issue.title.as_str());
                        ui.weak(issue.status.as_str());
                        ui.monospace(format!("{} conversations", issue.conversation_count));
                    });
                    if let Some(symptoms) = &issue.symptoms {
                        ui.weak(symptoms.as_str());
                    }
                    if let Some(workaround) = &issue.workaround {
                        ui.label(format!("workaround: {workaround}"));
                    }
                });
            }
        });
    }

    fn new_known_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.strong("New known issue");
        egui::Grid::new("radar-new-known")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("title:");
                ui.text_edit_singleline(&mut self.new_title);
                ui.end_row();
                ui.label("status:");
                egui::ComboBox::from_id_salt("radar-new-status")
                    .selected_text(self.new_status.clone())
                    .show_ui(ui, |ui| {
                        for status in ["investigating", "identified", "fix_in_progress", "resolved"]
                        {
                            ui.selectable_value(&mut self.new_status, status.into(), status);
                        }
                    });
                ui.end_row();
                ui.label("symptoms:");
                ui.text_edit_multiline(&mut self.new_symptoms);
                ui.end_row();
                ui.label("workaround:");
                ui.text_edit_multiline(&mut self.new_workaround);
                ui.end_row();
                ui.label("conversation ids:");
                ui.text_edit_singleline(&mut self.new_conversation_ids)
                    .on_hover_text("comma-separated local conversation ids");
                ui.end_row();
            });
        if ui.button("create").clicked() {
            self.create(db);
        }
    }
}

impl Default for IssueRadarPage {
    fn default() -> Self {
        Self::new()
    }
}

fn optional(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn clusters_and_known_load_from_seeded_rows() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO issue_clusters (name, title, summary, category, trend, conversation_count, customer_count)
                 VALUES ('Login loop', 'Login loop', 'Users stuck at SSO', 'auth', 'rising', 4, 3)",
                [],
            )
            .expect("cluster");
            conn.execute(
                "INSERT INTO known_issues (name, status, title, symptoms, provenance)
                 VALUES ('Login loop', 'identified', 'Login loop', 'SSO redirect loop', 'human_local')",
                [],
            )
            .expect("known");
        });
        let clusters = db.with_read(load_clusters);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].title, "Login loop");
        assert_eq!(clusters[0].conversation_count, 4);

        let known = db.with_read(load_known);
        assert_eq!(known.len(), 1);
        assert_eq!(known[0].status, "identified");
        assert_eq!(known[0].conversation_count, 0);
    }

    #[test]
    fn create_known_issue_links_conversations_and_counts() {
        let db = fixture(vec![(
            "conv",
            1,
            "Login broken",
            "active",
            "email",
            1,
            5,
            9,
        )]);
        let id = db.with_write(|conn| {
            create_known_issue(
                conn,
                "Login loop",
                Some("SSO redirect loop"),
                Some("Clear cookies"),
                "investigating",
                &[1],
            )
            .expect("known issue")
        });
        let known = db.with_read(load_known);
        assert_eq!(known.len(), 1);
        assert_eq!(known[0].id, id);
        assert_eq!(known[0].title, "Login loop");
        assert_eq!(
            known[0].conversation_count, 1,
            "refresh_known_issue_counts ran"
        );
        let links: i64 = db.with_read(|conn| {
            conn.query_row("SELECT COUNT(*) FROM known_issue_links", [], |r| r.get(0))
                .expect("links")
        });
        assert_eq!(links, 1);
    }

    #[test]
    fn page_create_validates_the_title() {
        let db = fixture(Vec::new());
        let mut page = IssueRadarPage::new();
        page.create(&db);
        let error = page.error.clone().expect("title required");
        assert!(error.contains("Title"));
    }

    #[test]
    fn radar_and_sla_load_on_a_fresh_database() {
        let db = fixture(Vec::new());
        let mut page = IssueRadarPage::new();
        page.reload(&db);
        assert!(page.radar.is_empty());
        assert!(page.sla.is_some());
        assert!(page.clusters.is_empty());
        assert!(page.known.is_empty());
    }

    #[test]
    fn known_issue_optional_text_helper() {
        assert_eq!(optional("  hi  ").as_deref(), Some("hi"));
        assert_eq!(optional("   "), None);
    }
}
