use eframe::egui;
use rusqlite::Connection;
use spp_core::db::DbHandle;

use super::{ConfirmRequest, NavRequest, str_field};
use crate::design::{self, BadgeTone, ButtonVariant, CalloutRole};
use crate::icons::Icon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Alerts,
    Clusters,
    Gaps,
    Known,
}

impl Tab {
    const ALL: [Self; 4] = [Self::Alerts, Self::Clusters, Self::Gaps, Self::Known];

    fn label(self) -> &'static str {
        match self {
            Self::Alerts => "Alerts",
            Self::Clusters => "Clusters",
            Self::Gaps => "Gaps",
            Self::Known => "Known issues",
        }
    }

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|t| *t == self)
            .unwrap_or_default()
    }

    fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or(Self::Alerts)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ClusterRow {
    pub title: String,
    pub summary: Option<String>,
    pub trend: Option<String>,
    pub conversation_count: i64,
    pub customer_count: Option<i64>,
    pub sample_ids: Vec<(i64, i64)>,
}

#[must_use]
pub(crate) fn load_clusters(conn: &Connection) -> Vec<ClusterRow> {
    conn.prepare(
        "SELECT id, title, summary, trend, conversation_count, customer_count
           FROM issue_clusters ORDER BY conversation_count DESC",
    )
    .and_then(|mut stmt| {
        stmt.query_map([], |r| {
            let cluster_id: i64 = r.get(0)?;
            let sample_ids: Vec<(i64, i64)> = conn
                .prepare(
                    "SELECT c.id, c.number FROM issue_cluster_conversations icc
                      JOIN conversations c ON c.id = icc.conversation_id
                      WHERE icc.cluster_id = ?1 ORDER BY c.number LIMIT 5",
                )
                .and_then(|mut s| {
                    s.query_map(rusqlite::params![cluster_id], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                    })
                    .map(|rows| rows.filter_map(Result::ok).collect())
                })
                .unwrap_or_default();
            Ok(ClusterRow {
                title: r.get(1)?,
                summary: r.get(2)?,
                trend: r.get(3)?,
                conversation_count: r.get(4)?,
                customer_count: r.get(5)?,
                sample_ids,
            })
        })
        .map(|rows| rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

#[derive(Debug, Clone, Default)]
pub(crate) struct KnownIssueRow {
    pub id: i64,
    pub title: String,
    pub symptoms: Option<String>,
    pub status: String,
    pub conversation_count: i64,
    pub workaround: Option<String>,
}

#[must_use]
pub(crate) fn load_known(conn: &Connection) -> Vec<KnownIssueRow> {
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

pub(crate) fn load_known_links(conn: &Connection, known_issue_id: i64) -> Vec<(i64, i64)> {
    conn.prepare(
        "SELECT kil.conversation_id, c.number
           FROM known_issue_links kil
           JOIN conversations c ON c.id = kil.conversation_id
          WHERE kil.known_issue_id = ?1
          ORDER BY c.number",
    )
    .and_then(|mut stmt| {
        stmt.query_map(rusqlite::params![known_issue_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        })
        .map(|rows| rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

pub(crate) fn create_known_issue(
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
             SELECT ?1, c.id, 'human' FROM conversations c WHERE c.id = ?2",
            rusqlite::params![id, conv_id],
        )?;
    }
    spp_core::intelligence_features::refresh_known_issue_counts(&tx, id)?;
    tx.commit()?;
    Ok(id)
}

pub struct IssueRadarPage {
    tab: Tab,
    radar: Vec<serde_json::Value>,
    sla: Option<spp_core::sla::SlaAlerts>,
    clusters: Vec<ClusterRow>,
    known: Vec<KnownIssueRow>,
    gaps: serde_json::Value,
    show_new: bool,
    new_title: String,
    new_symptoms: String,
    new_workaround: String,
    new_status: String,
    new_conversation_ids: String,
    error: Option<String>,
    notice: Option<String>,
    failed_ai_jobs: i64,
    reload_at: Option<std::time::Instant>,
    edit_known: Option<(i64, String, String, String)>,
    link_known_target: Option<i64>,
    link_conversation_id: String,
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
            gaps: serde_json::Value::Null,
            show_new: false,
            new_title: String::new(),
            new_symptoms: String::new(),
            new_workaround: String::new(),
            new_status: "investigating".into(),
            new_conversation_ids: String::new(),
            error: None,
            notice: None,
            failed_ai_jobs: 0,
            reload_at: None,
            edit_known: None,
            link_known_target: None,
            link_conversation_id: String::new(),
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.radar.clear();
        self.sla = None;
        self.clusters.clear();
        self.known.clear();
        self.gaps = serde_json::Value::Null;
        self.reload_at = None;
    }

    fn reload(&mut self, db: &DbHandle) {
        let (radar, sla, clusters, known, gaps, failed_ai_jobs) = db.with_read(|conn| {
            let failed_ai_jobs: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM jobs WHERE queue = 'ai' AND status = 'failed'",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            (
                spp_core::analytics::issue_radar_impl(conn),
                spp_core::sla::sla_alerts(conn).ok(),
                load_clusters(conn),
                load_known(conn),
                spp_core::quality::gap_report(conn),
                failed_ai_jobs,
            )
        });
        self.radar = radar;
        self.sla = sla;
        self.clusters = clusters;
        self.known = known;
        self.gaps = gaps;
        self.failed_ai_jobs = failed_ai_jobs;
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.sla.is_none() && self.reload_at.is_none_or(|t| t.elapsed().as_secs() >= 1) {
            self.reload_at = Some(std::time::Instant::now());
            self.reload(db);
        }
    }

    fn create(&mut self, db: &DbHandle) {
        let title = self.new_title.trim().to_string();
        if title.is_empty() || title.chars().count() > 300 {
            self.error = Some("Title is required (max 300 characters).".into());
            return;
        }
        let mut parsed: Vec<i64> = Vec::new();
        for id in self
            .new_conversation_ids
            .split(',')
            .filter_map(|id| id.trim().parse::<i64>().ok().filter(|v| *v > 0))
        {
            if !parsed.contains(&id) {
                parsed.push(id);
            }
        }
        let existing: std::collections::HashSet<i64> = db.with_read(|conn| {
            parsed
                .iter()
                .filter(|id| {
                    conn.query_row(
                        "SELECT 1 FROM conversations WHERE id = ?1",
                        rusqlite::params![id],
                        |_| Ok(()),
                    )
                    .is_ok()
                })
                .copied()
                .collect()
        });
        let conversation_ids: Vec<i64> = parsed
            .iter()
            .copied()
            .filter(|id| existing.contains(id))
            .collect();
        let skipped: Vec<String> = parsed
            .iter()
            .copied()
            .filter(|id| !existing.contains(id))
            .map(|id| id.to_string())
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
                if skipped.is_empty() {
                    self.notice = None;
                } else {
                    let linked = conversation_ids.len();
                    self.notice = Some(format!(
                        "known issue created; linked {} conversation{} — unknown id{} \
                         skipped: {}",
                        linked,
                        if linked == 1 { "" } else { "s" },
                        if skipped.len() == 1 { "" } else { "s" },
                        skipped.join(", ")
                    ));
                }
                self.reload(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub(crate) fn open_known_tab(&mut self) {
        self.tab = Tab::Known;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        let mut nav: Option<NavRequest> = None;
        self.ensure_loaded(db);

        design::page_header(
            ui,
            "Issue Radar",
            Some("AI-derived alerts, clusters, and known issues"),
            |ui| {
                if crate::design::button(ui, "Cluster now", crate::design::ButtonVariant::Outline)
                    .on_hover_text(
                        "group the last 14 days of conversations into issue clusters \
                     through the Classify role (needs a routed AI endpoint)",
                    )
                    .clicked()
                {
                    let feature_on = db.with_read(|conn| {
                        spp_core::ai_hub::ai_feature_enabled(conn, "issue_clustering")
                    });
                    if !feature_on {
                        self.error = Some(
                            "issue clustering is disabled — enable it in AI Center \
                             (Features & kill switches)."
                                .into(),
                        );
                    } else {
                        match db.with_write(|conn| {
                            spp_core::jobs::enqueue_on(conn, "ai", "cluster_issues", "{}", 3, 2)
                        }) {
                            Ok(_) => {
                                self.error = None;
                                self.notice = Some(
                                    "clustering queued — reload clusters in a few seconds \
                                     to see what landed."
                                        .into(),
                                );
                            }
                            Err(e) => {
                                self.notice = None;
                                self.error = Some(format!("could not queue clustering: {e}"));
                            }
                        }
                    }
                }
                if crate::design::button(
                    ui,
                    "Reload clusters",
                    crate::design::ButtonVariant::Outline,
                )
                .clicked()
                {
                    self.reload(db);
                    self.notice = None;
                }
                if crate::design::button(ui, "Detect gaps now", crate::design::ButtonVariant::Outline)
                    .on_hover_text(
                        "rebuild friction findings and 'asked often, documented nowhere' gap candidates \
                         from the last 90 days (deterministic, no model needed)",
                    )
                    .clicked()
                {
                    let feature_on = db.with_read(|conn| {
                        spp_core::ai_hub::ai_feature_enabled(conn, "gap_detection")
                    });
                    if !feature_on {
                        self.error = Some(
                            "gap detection is disabled — enable it in AI Center \
                             (Features & kill switches)."
                                .into(),
                        );
                    } else {
                        match db
                            .with_write(|conn| {
                                spp_core::jobs::enqueue_on(
                                    conn,
                                    "ai",
                                    "rebuild_gaps",
                                    "{}",
                                    3,
                                    2,
                                )
                            })
                        {
                            Ok(_) => {
                                self.error = None;
                                self.notice = Some(
                                    "gap detection queued — reload in a few seconds to see what landed."
                                        .into(),
                                );
                            }
                            Err(e) => {
                                self.notice = None;
                                self.error = Some(format!("could not queue gap detection: {e}"));
                            }
                        }
                    }
                }
            },
        );
        if self.failed_ai_jobs > 0 {
            design::error_text(
                ui,
                &format!(
                    "{} failed AI job{} — check AI Center / diagnostics for the error",
                    self.failed_ai_jobs,
                    if self.failed_ai_jobs == 1 { "" } else { "s" }
                ),
            );
        }
        if let Some(note) = &self.notice {
            crate::design::hint_text(ui, note);
        }

        let labels: Vec<&str> = Tab::ALL.iter().map(|t| t.label()).collect();
        if let Some(i) = design::tabs(ui, &labels, self.tab.index()) {
            self.tab = Tab::from_index(i);
        }
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }

        match self.tab {
            Tab::Alerts => self.alerts_ui(ui, &mut nav),
            Tab::Clusters => self.clusters_ui(ui, &mut nav),
            Tab::Gaps => self.gaps_ui(ui, db, &mut nav),
            Tab::Known => self.known_ui(ui, db, &mut nav),
        }
        nav
    }

    fn gaps_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, nav: &mut Option<NavRequest>) {
        let report = self.gaps.clone();
        if report.is_null() {
            ui.spinner();
            return;
        }
        let totals = report.get("totals").cloned().unwrap_or_default();
        ui.horizontal(|ui| {
            design::badge(
                ui,
                &format!(
                    "{} open",
                    totals
                        .get("candidates")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0)
                ),
                BadgeTone::Warning,
            );
            design::badge(
                ui,
                &format!(
                    "{} approved",
                    totals.get("approved").and_then(|v| v.as_i64()).unwrap_or(0)
                ),
                BadgeTone::Success,
            );
            design::badge(
                ui,
                &format!(
                    "{} rejected",
                    totals.get("rejected").and_then(|v| v.as_i64()).unwrap_or(0)
                ),
                BadgeTone::Neutral,
            );
        });
        let empty = report
            .get("kinds")
            .and_then(|k| k.as_array())
            .map(|kinds| {
                kinds.iter().all(|k| {
                    k.get("candidates")
                        .and_then(|c| c.as_array())
                        .is_none_or(Vec::is_empty)
                })
            })
            .unwrap_or(true);
        if empty {
            design::empty_state(
                ui,
                Icon::Target,
                "No gap candidates yet",
                "Run 'Detect gaps now' — repeated unresolved patterns from resolved and reopened conversations land here with counts and examples.",
            );
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("gap-candidates")
            .show(ui, |ui| {
                if let Some(notes) = report.get("notes").and_then(|n| n.as_array()) {
                    for note in notes {
                        design::hint_text(ui, note.as_str().unwrap_or_default());
                    }
                    ui.add_space(design::scale::space::CLUSTER);
                }
                if let Some(kinds) = report.get("kinds").and_then(|k| k.as_array()) {
                    for kind in kinds {
                        let label = kind.get("label").and_then(|l| l.as_str()).unwrap_or("gaps");
                        let candidates = kind
                            .get("candidates")
                            .and_then(|c| c.as_array())
                            .cloned()
                            .unwrap_or_default();
                        if candidates.is_empty() {
                            continue;
                        }
                        design::card_titled(ui, label, None, |ui| {
                            for (n, candidate) in candidates.iter().enumerate() {
                                if n > 0 {
                                    design::divider(ui);
                                }
                                self.gap_candidate_ui(ui, db, candidate, &mut *nav);
                            }
                        });
                        ui.add_space(design::scale::space::SP2);
                    }
                }
            });
    }

    fn gap_candidate_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        candidate: &serde_json::Value,
        nav: &mut Option<NavRequest>,
    ) {
        let id = candidate.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let query = candidate
            .get("query_text")
            .and_then(|q| q.as_str())
            .unwrap_or("(unknown topic)");
        let count = candidate
            .get("occurrence_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let status = candidate
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("open");
        let explanation = candidate
            .get("explanation")
            .and_then(|e| e.as_str())
            .or_else(|| {
                candidate
                    .get("detail")
                    .and_then(|d| d.get("explanation"))
                    .and_then(|e| e.as_str())
            })
            .unwrap_or_default();
        let t = design::tokens(ui);
        ui.horizontal(|ui| {
            design::badge(ui, &format!("{count}×"), BadgeTone::Warning);
            ui.label(
                egui::RichText::new(query)
                    .size(design::scale::text::BODY)
                    .strong()
                    .color(t.foreground),
            );
            let tone = match status {
                "approved" => BadgeTone::Success,
                "rejected" => BadgeTone::Neutral,
                _ => BadgeTone::Info,
            };
            design::badge(ui, status, tone);
        });
        if !explanation.is_empty() {
            design::hint_text(ui, explanation);
        }
        let examples: Vec<i64> = candidate
            .get("evidence_conversation_ids")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_i64()).take(5).collect())
            .unwrap_or_default();
        if !examples.is_empty() {
            let numbers: Vec<(i64, i64)> = db.with_read(|conn| {
                examples
                    .iter()
                    .filter_map(|id| {
                        conn.query_row(
                            "SELECT number FROM conversations WHERE id = ?1",
                            rusqlite::params![id],
                            |r| r.get::<_, i64>(0),
                        )
                        .ok()
                        .map(|n| (*id, n))
                    })
                    .collect()
            });
            if !numbers.is_empty() {
                ui.horizontal(|ui| {
                    design::hint_text(ui, "examples:");
                    for (local_id, number) in numbers {
                        let label = ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("#{number}"))
                                    .size(design::scale::text::SMALL)
                                    .color(design::tokens(ui).primary),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if label.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if label.clicked() && nav.is_none() {
                            *nav = Some(NavRequest::OpenConversation(local_id));
                        }
                    }
                });
            }
        }
        if status == "open" {
            ui.horizontal(|ui| {
                if design::button(ui, "Approve", ButtonVariant::Outline).clicked() {
                    let decided = db
                        .with_write(|conn| {
                            spp_core::quality::decide_gap(conn, id, "approved", None, None)
                        })
                        .is_some();
                    if decided {
                        self.notice =
                            Some("candidate approved — decisions survive rebuilds.".into());
                        self.reload(db);
                    } else {
                        self.error = Some("approve failed — see Diagnostics.".into());
                    }
                }
                let reject_id = format!("gap-reject-{id}");
                if design::armed_danger_button(ui, &reject_id, "Reject", "Confirm Reject?") {
                    let decided = db
                        .with_write(|conn| {
                            spp_core::quality::decide_gap(conn, id, "rejected", None, None)
                        })
                        .is_some();
                    if decided {
                        self.notice = Some("candidate rejected.".into());
                        self.reload(db);
                    } else {
                        self.error = Some("reject failed — see Diagnostics.".into());
                    }
                }
            });
        }
    }

    fn alerts_ui(&mut self, ui: &mut egui::Ui, nav: &mut Option<NavRequest>) {
        let radar = self.radar.clone();
        design::card_titled(
            ui,
            &format!("{} radar alert(s)", radar.len()),
            Some("AI-derived from local conversation data"),
            |ui| {
                if radar.is_empty() {
                    design::empty_state(
                        ui,
                        Icon::Target,
                        "No alerts",
                        "No AI-derived alerts for the current data.",
                    );
                } else {
                    for alert in &radar {
                        let title = str_field(alert, "title");
                        let severity = str_field(alert, "severity");
                        let count = alert
                            .get("conversation_ids")
                            .and_then(|v| v.as_array())
                            .map_or(0, Vec::len);
                        let tone = if severity == "critical" {
                            BadgeTone::Destructive
                        } else {
                            BadgeTone::Warning
                        };
                        ui.horizontal(|ui| {
                            design::badge(ui, &severity, tone);
                            ui.label(
                                egui::RichText::new(format!("{title} · {count} conversations"))
                                    .size(design::scale::text::CONTROL)
                                    .color(design::tokens(ui).foreground),
                            );
                            if let Some(conv_id) = alert
                                .get("conversation_ids")
                                .and_then(|v| v.as_array())
                                .and_then(|a| a.first())
                                .and_then(|v| v.as_i64())
                                && nav.is_none()
                                && design::button(ui, "Open", ButtonVariant::Ghost)
                                    .on_hover_text("open the first conversation behind this alert")
                                    .clicked()
                            {
                                *nav = Some(NavRequest::OpenConversation(conv_id));
                            }
                        });
                        ui.add_space(design::scale::space::SP2);
                    }
                }
            },
        );
        ui.add_space(design::scale::space::SP2);
        let sla = self.sla.clone();
        let Some(sla) = sla else {
            ui.spinner();
            return;
        };
        design::card_titled(
            ui,
            &format!(
                "SLA: {} breached · {} at risk",
                sla.total_breached, sla.total_at_risk
            ),
            None,
            |ui| {
                if sla.alerts.is_empty() {
                    design::empty_state(
                        ui,
                        Icon::Clock,
                        "No SLA alerts",
                        "No conversations are breaching or at risk right now.",
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("issue-radar-sla-scroll")
                        .show(ui, |ui| {
                            for alert in &sla.alerts {
                                let state = alert.state.as_str();
                                let line = format!(
                                    "#{} {} · {} · {}",
                                    alert.number,
                                    alert.subject.clone().unwrap_or_default(),
                                    alert.mailbox_name,
                                    state
                                );
                                if state == "breached" {
                                    design::callout(ui, CalloutRole::Err, &line);
                                } else {
                                    design::callout(ui, CalloutRole::Warn, &line);
                                }
                                ui.add_space(design::scale::space::SP1);
                                if nav.is_none() {
                                    let open = ui.add(
                                        egui::Label::new(
                                            egui::RichText::new("open →")
                                                .size(design::scale::text::SMALL)
                                                .color(design::tokens(ui).primary),
                                        )
                                        .sense(egui::Sense::click()),
                                    );
                                    if open.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if open.clicked() {
                                        *nav = Some(NavRequest::OpenConversation(
                                            alert.conversation_id,
                                        ));
                                    }
                                }
                                ui.add_space(design::scale::space::SP2);
                            }
                        });
                }
            },
        );
    }

    fn clusters_ui(&mut self, ui: &mut egui::Ui, nav: &mut Option<NavRequest>) {
        let clusters = self.clusters.clone();
        design::section_title(ui, &format!("{} cluster(s)", clusters.len()));
        if clusters.is_empty() {
            design::empty_state(
                ui,
                Icon::Boxes,
                "No clusters yet",
                "Clusters appear once enough related conversations accumulate.",
            );
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for cluster in &clusters {
                design::card_titled(ui, &cluster.title, None, |ui| {
                    if let Some(trend) = &cluster.trend {
                        let tone = if trend == "rising" || trend == "new" {
                            BadgeTone::Warning
                        } else {
                            BadgeTone::Neutral
                        };
                        design::badge(ui, trend, tone);
                    }
                    design::hint_text(
                        ui,
                        &format!(
                            "{} conversations · {} customers",
                            cluster.conversation_count,
                            cluster.customer_count.unwrap_or(0)
                        ),
                    );
                    if let Some(summary) = &cluster.summary {
                        design::hint_text(ui, summary.as_str());
                    }
                    if !cluster.sample_ids.is_empty() {
                        ui.horizontal(|ui| {
                            design::hint_text(ui, "members:");
                            for (local_id, number) in &cluster.sample_ids {
                                let label = ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!("#{number}"))
                                            .size(design::scale::text::SMALL)
                                            .color(design::tokens(ui).primary),
                                    )
                                    .sense(egui::Sense::click()),
                                );
                                if label.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if label.clicked() && nav.is_none() {
                                    *nav = Some(NavRequest::OpenConversation(*local_id));
                                }
                            }
                        });
                    }
                });
                ui.add_space(design::scale::space::SP2);
            }
        });
    }

    fn known_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, nav: &mut Option<NavRequest>) {
        ui.horizontal(|ui| {
            if design::button(ui, "New known issue", ButtonVariant::Primary).clicked() {
                self.show_new = !self.show_new;
            }
        });
        if self.show_new {
            self.new_known_ui(ui, db);
            design::divider(ui);
        }
        let known = self.known.clone();
        design::section_title(ui, &format!("{} known issue(s)", known.len()));
        if known.is_empty() {
            design::empty_state(
                ui,
                Icon::AlertCircle,
                "No known issues",
                "Create one to track recurring problems and their workarounds.",
            );
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for issue in &known {
                design::card_titled(ui, &issue.title, None, |ui| {
                    let t = design::tokens(ui);
                    ui.horizontal(|ui| {
                        design::badge(ui, issue.status.as_str(), BadgeTone::Neutral);
                        ui.label(
                            egui::RichText::new(format!(
                                "{} conversations",
                                issue.conversation_count
                            ))
                            .size(design::scale::text::META_HALF)
                            .color(t.muted_foreground),
                        );
                    });
                    if let Some(symptoms) = &issue.symptoms {
                        design::hint_text(ui, symptoms.as_str());
                    }
                    if let Some(workaround) = &issue.workaround {
                        ui.label(
                            egui::RichText::new(format!("workaround: {workaround}"))
                                .size(design::scale::text::CONTROL)
                                .color(t.foreground),
                        );
                    }
                    ui.add_space(design::scale::space::SP1);
                    ui.horizontal(|ui| {
                        design::field_label(ui, "Status");
                        let statuses = spp_core::intelligence_features::KNOWN_ISSUE_STATUSES;
                        let active = statuses
                            .iter()
                            .position(|s| *s == issue.status)
                            .unwrap_or(usize::MAX);
                        if let Some(i) = design::segmented(
                            ui,
                            &format!("known-status-{}", issue.id),
                            &statuses,
                            active,
                        ) && statuses[i] != issue.status
                        {
                            self.set_known_status(db, issue.id, statuses[i]);
                        }
                    });
                    ui.add_space(design::scale::space::SP1);
                    let links = db.with_read(|conn| load_known_links(conn, issue.id));
                    if !links.is_empty() {
                        ui.horizontal(|ui| {
                            design::hint_text(ui, "evidence:");
                            for (conv_id, number) in &links {
                                let chip = ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!("#{number}"))
                                            .size(design::scale::text::SMALL)
                                            .color(design::tokens(ui).primary),
                                    )
                                    .sense(egui::Sense::click()),
                                );
                                if chip.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if chip.clicked() && nav.is_none() {
                                    *nav = Some(NavRequest::OpenConversation(*conv_id));
                                }
                                if design::chip(ui, false, "×")
                                    .on_hover_text("unlink this conversation")
                                    .clicked()
                                {
                                    self.unlink_known(db, issue.id, *conv_id);
                                }
                            }
                        });
                    }
                    if self.link_known_target == Some(issue.id) {
                        ui.horizontal(|ui| {
                            design::field_label(ui, "Link conversation");
                            ui.add_sized(
                                [120.0, 28.0],
                                egui::TextEdit::singleline(&mut self.link_conversation_id),
                            )
                            .on_hover_text("local conversation id");
                            if design::button(ui, "Link", ButtonVariant::Secondary).clicked() {
                                self.link_known(db, issue.id);
                            }
                            if design::button(ui, "Done", ButtonVariant::Ghost).clicked() {
                                self.link_known_target = None;
                                self.link_conversation_id.clear();
                            }
                        });
                    }
                    ui.add_space(design::scale::space::SP1);
                    ui.horizontal(|ui| {
                        if design::button(ui, "Edit", ButtonVariant::Secondary).clicked() {
                            self.edit_known = Some((
                                issue.id,
                                issue.title.clone(),
                                issue.symptoms.clone().unwrap_or_default(),
                                issue.workaround.clone().unwrap_or_default(),
                            ));
                        }
                        if self.link_known_target.is_none()
                            && design::button(ui, "Link…", ButtonVariant::Ghost).clicked()
                        {
                            self.link_known_target = Some(issue.id);
                            self.link_conversation_id.clear();
                        }
                        if design::button(ui, "Delete", ButtonVariant::Ghost)
                            .on_hover_text("delete this known issue and its links")
                            .clicked()
                            && nav.is_none()
                        {
                            *nav = Some(NavRequest::Confirm(ConfirmRequest::delete_known_issue(
                                issue.id,
                                issue.title.as_str(),
                            )));
                        }
                    });
                });
                ui.add_space(design::scale::space::SP2);
            }
        });
        if let Some((id, mut title, mut symptoms, mut workaround)) = self.edit_known.clone() {
            design::card_titled(ui, "Edit known issue", None, |ui| {
                design::form_grid(ui, "radar-edit-known", |ui| {
                    design::field_label(ui, "Title");
                    ui.add_sized([260.0, 28.0], egui::TextEdit::singleline(&mut title));
                    ui.end_row();
                    design::field_label(ui, "Symptoms");
                    ui.add(egui::TextEdit::multiline(&mut symptoms).desired_width(f32::INFINITY));
                    ui.end_row();
                    design::field_label(ui, "Workaround");
                    ui.add(egui::TextEdit::multiline(&mut workaround).desired_width(f32::INFINITY));
                    ui.end_row();
                });
                ui.add_space(design::scale::space::SP1);
                ui.horizontal(|ui| {
                    if design::button(ui, "Save Changes", ButtonVariant::Primary).clicked() {
                        self.save_known_edit(db, id, &title, &symptoms, &workaround);
                    }
                    if design::button(ui, "Cancel", ButtonVariant::Ghost).clicked() {
                        self.edit_known = None;
                    }
                });
            });
        }
    }

    fn set_known_status(&mut self, db: &DbHandle, id: i64, status: &str) {
        match db.with_write(|conn| {
            spp_core::intelligence_features::set_known_issue_status(conn, id, status)
        }) {
            Ok(true) => {
                self.notice = Some(format!("status set to {status}."));
                self.reload(db);
            }
            Ok(false) => self.error = Some("known issue not found.".into()),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn save_known_edit(
        &mut self,
        db: &DbHandle,
        id: i64,
        title: &str,
        symptoms: &str,
        workaround: &str,
    ) {
        match db.with_write(|conn| {
            spp_core::intelligence_features::set_known_issue_text(
                conn,
                id,
                Some(title),
                Some(symptoms),
                Some(workaround),
            )
        }) {
            Ok(true) => {
                self.notice = Some("known issue updated.".into());
                self.edit_known = None;
                self.reload(db);
            }
            Ok(false) => self.error = Some("known issue not found.".into()),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn link_known(&mut self, db: &DbHandle, id: i64) {
        let Ok(conv_id) = self.link_conversation_id.trim().parse::<i64>() else {
            self.error = Some("conversation id must be a number.".into());
            return;
        };
        match db.with_write(|conn| {
            spp_core::intelligence_features::link_known_issue_conversation(conn, id, conv_id)
        }) {
            Ok(true) => {
                self.notice = Some("conversation linked as evidence.".into());
                self.link_conversation_id.clear();
                self.reload(db);
            }
            Ok(false) => {
                self.error = Some("link refused — check the conversation id exists.".into())
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn unlink_known(&mut self, db: &DbHandle, id: i64, conversation_id: i64) {
        match db.with_write(|conn| {
            spp_core::intelligence_features::unlink_known_issue_conversation(
                conn,
                id,
                conversation_id,
            )
        }) {
            Ok(true) => {
                self.notice = Some("conversation unlinked.".into());
                self.reload(db);
            }
            Ok(false) => self.error = Some("no such link.".into()),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn new_known_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card_titled(ui, "New known issue", None, |ui| {
            design::form_grid(ui, "radar-new-known", |ui| {
                design::field_label(ui, "Title");
                ui.add_sized(
                    [260.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_title),
                );
                ui.end_row();
                design::field_label(ui, "Status");
                let status_label = self.new_status.clone();
                egui::ComboBox::from_id_salt("radar-new-status")
                    .selected_text(status_label.clone())
                    .show_ui(ui, |ui| {
                        for status in ["investigating", "identified", "fix_in_progress", "resolved"]
                        {
                            ui.selectable_value(&mut self.new_status, status.into(), status);
                        }
                    })
                    .response
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            ui.is_enabled(),
                            format!("status: {status_label}"),
                        )
                    });
                ui.end_row();
                design::field_label(ui, "Symptoms");
                ui.add(
                    egui::TextEdit::multiline(&mut self.new_symptoms).desired_width(f32::INFINITY),
                );
                ui.end_row();
                design::field_label(ui, "Workaround");
                ui.add(
                    egui::TextEdit::multiline(&mut self.new_workaround)
                        .desired_width(f32::INFINITY),
                );
                ui.end_row();
                design::field_label(ui, "Conversation ids");
                ui.add_sized(
                    [260.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_conversation_ids),
                )
                .on_hover_text("comma-separated local conversation ids");
                ui.end_row();
            });
            ui.add_space(design::scale::space::SP1);
            if design::button(ui, "Create", ButtonVariant::Primary).clicked() {
                self.create(db);
            }
        });
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
        db.with_write(|conn| {
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

#[cfg(test)]
mod batch27_audit81_tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn dangling_ids_gain_no_links_and_do_not_count() {
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
        db.with_write(|conn| {
            create_known_issue(
                conn,
                "Login loop",
                None,
                None,
                "investigating",
                &[1, 99, 100],
            )
            .expect("known issue")
        });

        let links: Vec<i64> = db.with_read(|conn| {
            conn.prepare("SELECT conversation_id FROM known_issue_links ORDER BY conversation_id")
                .expect("prepare")
                .query_map([], |r| r.get(0))
                .expect("query")
                .filter_map(Result::ok)
                .collect()
        });
        assert_eq!(
            links,
            vec![1],
            "only the existing conversation gained a link"
        );

        let known = db.with_read(load_known);
        assert_eq!(known.len(), 1, "the known issue loads");
        assert_eq!(known[0].conversation_count, 1, "dangling ids never counted");
    }

    #[test]
    fn pre_fix_dangling_links_no_longer_count() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO known_issues (name, status, title, provenance)
                 VALUES ('Legacy', 'identified', 'Legacy', 'human_local')",
                [],
            )
            .expect("known");
            let kid = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO known_issue_links (known_issue_id, conversation_id, link_type)
                 VALUES (?1, 99, 'human')",
                rusqlite::params![kid],
            )
            .expect("dangling link");
            spp_core::intelligence_features::refresh_known_issue_counts(conn, kid)
                .expect("refresh");
        });
        let known = db.with_read(load_known);
        assert_eq!(
            known[0].conversation_count, 0,
            "the refresh counts only links whose conversation exists"
        );
    }

    #[test]
    fn page_create_reports_skipped_unknown_ids() {
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
        let mut page = IssueRadarPage::new();
        page.new_title = "Login loop".into();
        page.new_conversation_ids = "1, 7, 1".into();
        page.create(&db);

        assert!(page.error.is_none(), "create failed: {:?}", page.error);
        let notice = page.notice.clone().expect("the skipped id is reported");
        assert!(notice.contains("7"), "notice lists id 7: {notice}");
        assert!(notice.contains("1 conversation"), "linked one: {notice}");

        let links: i64 = db.with_read(|conn| {
            conn.query_row("SELECT COUNT(*) FROM known_issue_links", [], |r| r.get(0))
                .expect("links")
        });
        assert_eq!(links, 1, "dedupe + existence check both applied");
    }
}

#[cfg(test)]
mod audp2_tests {
    use super::*;
    use crate::testsupport::fixture;

    fn seeded() -> (spp_core::db::DbHandle, i64) {
        let db = fixture(vec![("conv", 1, "Login loop", "active", "email", 1, 5, 9)]);
        let known_id: i64 = db.with_write(|conn| {
            let id = create_known_issue(
                conn,
                "Login loop",
                Some("users stuck at SSO"),
                None,
                "investigating",
                &[],
            )
            .expect("known");
            spp_core::intelligence_features::link_known_issue_conversation(conn, id, 1)
                .expect("link");
            id
        });
        (db, known_id)
    }

    #[test]
    fn audp2_known_issue_lifecycle_round_trips() {
        let (db, known_id) = seeded();
        let mut page = IssueRadarPage::new();
        page.reload(&db);

        page.set_known_status(&db, known_id, "fix_in_progress");
        assert!(page.error.is_none(), "status write: {:?}", page.error);
        let status: String = db.with_read(|conn| {
            conn.query_row(
                "SELECT status FROM known_issues WHERE id = ?1",
                rusqlite::params![known_id],
                |r| r.get(0),
            )
            .expect("row")
        });
        assert_eq!(status, "fix_in_progress");

        page.set_known_status(&db, known_id, "magic");
        assert!(page.error.is_some(), "the vocabulary is closed");

        page.save_known_edit(&db, known_id, "Login loop v2", "updated symptoms", "");
        let (name, symptoms, workaround): (String, Option<String>, Option<String>) =
            db.with_read(|conn| {
                conn.query_row(
                    "SELECT name, symptoms, workaround FROM known_issues WHERE id = ?1",
                    rusqlite::params![known_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .expect("row")
            });
        assert_eq!(name, "Login loop v2");
        assert_eq!(symptoms.as_deref(), Some("updated symptoms"));
        assert_eq!(workaround, None, "an empty save clears");

        page.unlink_known(&db, known_id, 1);
        let links: i64 = db.with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM known_issue_links WHERE known_issue_id = ?1",
                rusqlite::params![known_id],
                |r| r.get(0),
            )
            .expect("count")
        });
        assert_eq!(links, 0);
        page.link_conversation_id = "1".into();
        page.link_known(&db, known_id);
        let links: i64 = db.with_read(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM known_issue_links WHERE known_issue_id = ?1",
                rusqlite::params![known_id],
                |r| r.get(0),
            )
            .expect("count")
        });
        assert_eq!(links, 1);

        let deleted = db
            .with_write(|conn| spp_core::intelligence_features::delete_known_issue(conn, known_id));
        assert!(deleted.unwrap_or(false));
        let remaining: i64 = db
            .with_read(|conn| conn.query_row("SELECT COUNT(*) FROM known_issues", [], |r| r.get(0)))
            .expect("count");
        assert_eq!(remaining, 0);
    }

    #[test]
    fn audp2_known_loader_carries_the_row_id() {
        let (db, known_id) = seeded();
        let known = db.with_read(load_known);
        assert_eq!(known.len(), 1);
        assert_eq!(known[0].id, known_id, "rows are addressable now");
    }

    #[test]
    fn audp2_gap_examples_resolve_to_clickable_pairs() {
        let (db, known_id) = seeded();
        let links = db.with_read(|conn| load_known_links(conn, known_id));
        assert_eq!(links, vec![(1, 501)], "the fixture conversation resolves");
    }
}
