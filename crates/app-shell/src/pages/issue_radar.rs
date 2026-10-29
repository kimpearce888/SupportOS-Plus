//! Issue Radar page (port of the web `pages/issue_radar.rs`).
//!
//! RS-18 (wave B) rebuild on **Design Contract v1.0 archetype DS-A-02** —
//! the dashboard/stack recipe (docs/audit/design-contract-v1.md §4):
//! PageHeader (the lab's three outline actions in the right cluster) →
//! DS-C-09 underline tabs → per-tab block stack of DS cards. The
//! conformed lab reference is `mockups/design-lab/pages-ds.js`
//! (`DS.definePage("issue-radar", …)`).
//!
//! Queries, state, and behavior are unchanged (decisions.md D6:
//! presentation only). **Alerts** still reads `issue_radar_impl`
//! (`/api/reports/issue-radar`) plus `sla::sla_alerts`
//! (`/api/issues/sla-alerts` — SLA rows render through the DS-C-15
//! callout roles: breached → Err, at-risk → Warn, exactly the lab's
//! mapping); **Clusters** still reads the issue-cluster table
//! (`/api/issues/clusters`) with conversation ids; **Gaps** still runs
//! the deterministic K1 pipeline (`quality::gap_report`) with the same
//! approve/reject writes — the reject act now follows the lab's armed
//! two-click form (DS-C-16/A-5 armed danger), approve stays one click;
//! **Known issues** still reads `/api/issues/known` plus the create form
//! (the create route's validation and inserts, in-process). The lab's
//! receipt-cited quirk about the raw severity tokens renders on the
//! Alerts tab (DS-C-15 quirk — data-layer quirks stay visible, DC C-1).

use eframe::egui;
use rusqlite::Connection;
use spp_core::db::DbHandle;

use super::str_field;
use crate::design::{self, BadgeTone, ButtonVariant, CalloutRole};
use crate::icons::Icon;

/// Page tabs.
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
/// Clean-audit item 81: conversation ids are validated against the
/// conversations table INSIDE the transaction — a dangling id gains no
/// link row (the pre-fix code inserted links for nonexistent
/// conversations, and `refresh_known_issue_counts` then counted them).
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
        // Item 81: the link materializes only when the conversation
        // exists — the INSERT selects from conversations itself, so a
        // dangling id inserts zero rows instead of a dead link.
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

/// The interactive issue radar page state.
pub struct IssueRadarPage {
    tab: Tab,
    radar: Vec<serde_json::Value>,
    sla: Option<spp_core::sla::SlaAlerts>,
    clusters: Vec<ClusterRow>,
    known: Vec<KnownIssueRow>,
    /// K1: the gap-detection report (deterministic candidates).
    gaps: serde_json::Value,
    show_new: bool,
    new_title: String,
    new_symptoms: String,
    new_workaround: String,
    new_status: String,
    new_conversation_ids: String,
    error: Option<String>,
    /// A2: outcome note for the manual clustering trigger.
    notice: Option<String>,
    /// FIND-008: failed `ai` jobs used to land only in the jobs table with
    /// no signal on this page; the header badges the count instead.
    failed_ai_jobs: i64,
    /// Audit fix (M21): stamp of the last full reload attempt — if any
    /// read errors, `sla` stays None and the whole radar (five analytics
    /// queries) used to re-run on the UI thread every rendered frame.
    reload_at: Option<std::time::Instant>,
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
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.radar.clear();
        self.sla = None;
        self.clusters.clear();
        self.known.clear();
        self.gaps = serde_json::Value::Null;
        // Audit fix (M21): an explicit invalidation reloads next frame.
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
                // K1: the deterministic gap pipeline's current report.
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
        // Audit fix (M21): reload at most once per second while a read
        // keeps failing (a None `sla` used to trigger the full radar
        // reload every rendered frame).
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
        // Clean-audit item 81: parse, dedupe (order-preserving) and
        // validate the ids against the conversations table — dangling
        // ids are reported to the user instead of silently gaining
        // link rows that counted toward the issue.
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
                    self.notice = Some(format!(
                        "known issue created; linked {} conversation(s) — unknown id(s) \
                         skipped: {}",
                        conversation_ids.len(),
                        skipped.join(", ")
                    ));
                }
                self.reload(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        design::page_header(
            ui,
            "Issue Radar",
            Some("AI-derived alerts, clusters, and known issues"),
            |ui| {
                // A2: the clustering pipeline is real now — one click runs
                // it on demand (a daily maintenance trigger also exists),
                // and a reload refetches whatever has landed.
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
                        // FIND-008: the enqueue result used to be discarded
                        // (`let _ = …`) — surface success and failure alike.
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
                // K1: the deterministic gap pipeline on demand — the daily
                // maintenance trigger also enqueues it.
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
                        // FIND-008: surface the enqueue result here too.
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
        // FIND-008: failed `ai` jobs used to be visible only in the jobs
        // table — badge the count here so failures surface on the page.
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

        // DS-C-09 Tabs (the lab's DS.tab rail replaces the chip row; the
        // tab labels and set are unchanged).
        let labels: Vec<&str> = Tab::ALL.iter().map(|t| t.label()).collect();
        if let Some(i) = design::tabs(ui, &labels, self.tab.index()) {
            self.tab = Tab::from_index(i);
        }
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }

        match self.tab {
            Tab::Alerts => self.alerts_ui(ui),
            Tab::Clusters => self.clusters_ui(ui),
            Tab::Gaps => self.gaps_ui(ui, db),
            Tab::Known => self.known_ui(ui, db),
        }
    }

    /// K1: the Gaps tab — "asked often, documented nowhere" topics with
    /// counts and examples, plus friction-derived kinds, all deterministic.
    /// Every candidate carries approve/reject (a human decides; approving
    /// marks the candidate only — nothing is published automatically).
    fn gaps_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let report = self.gaps.clone();
        if report.is_null() {
            ui.spinner();
            return;
        }
        let totals = report.get("totals").cloned().unwrap_or_default();
        // The lab's ds-chip-row of decision totals.
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
                "(no gap candidates yet)",
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
                                self.gap_candidate_ui(ui, db, candidate);
                            }
                        });
                        ui.add_space(design::scale::space::SP2);
                    }
                }
            });
    }

    /// K1: one gap candidate row — the question/topic, its occurrence count,
    /// example conversations, the deterministic explanation, and the
    /// approve/reject decision buttons (the lab's approve is one click;
    /// reject arms first — DS-C-16/A-5).
    fn gap_candidate_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        candidate: &serde_json::Value,
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
        // The lab's ds-tool-row: count badge, strong query, status badge.
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
        // Example conversations: local ids -> numbers for display (the
        // lab's single "examples:" hint line).
        let examples: Vec<i64> = candidate
            .get("evidence_conversation_ids")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_i64()).take(5).collect())
            .unwrap_or_default();
        if !examples.is_empty() {
            let numbers: Vec<String> = db.with_read(|conn| {
                examples
                    .iter()
                    .filter_map(|id| {
                        conn.query_row(
                            "SELECT number FROM conversations WHERE id = ?1",
                            rusqlite::params![id],
                            |r| r.get::<_, i64>(0),
                        )
                        .ok()
                    })
                    .map(|n| format!("#{n}"))
                    .collect()
            });
            design::hint_text(ui, &format!("examples: {}", numbers.join(" · ")));
        }
        if status == "open" {
            ui.horizontal(|ui| {
                if design::button(ui, "approve", ButtonVariant::Outline).clicked() {
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
                // A-5: the destructive act arms first — one click shows
                // "confirm reject?", the second click (or Esc elsewhere)
                // completes it. The decision write is unchanged. The id is
                // the candidate's own — it is unique in the report.
                let reject_id = format!("gap-reject-{id}");
                if design::armed_danger_button(ui, &reject_id, "reject", "confirm reject?") {
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

    fn alerts_ui(&mut self, ui: &mut egui::Ui) {
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
                        "(no alerts)",
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
                        });
                        ui.add_space(design::scale::space::SP1);
                    }
                }
            },
        );
        // DS-C-15 quirk (DC C-1): the raw severity tokens are the engine's
        // own strings, and non-critical keeps the Warning tone — the lab
        // keeps that honesty note visible on this tab.
        design::quirk(
            ui,
            "Raw severity tokens (critical/warning/info) — the engine's own \
             strings; info keeps the Warning tone exactly as the current build.",
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
                        "(no SLA alerts)",
                        "No conversations are breaching or at risk right now.",
                    );
                } else {
                    egui::ScrollArea::vertical().show_rows(
                        ui,
                        26.0,
                        sla.alerts.len(),
                        |ui, range| {
                            for alert in &sla.alerts[range] {
                                let state = alert.state.as_str();
                                let line = format!(
                                    "#{} {} · {} · {}",
                                    alert.number,
                                    alert.subject.clone().unwrap_or_default(),
                                    alert.mailbox_name,
                                    state
                                );
                                // The lab's DS.callout roles: breached →
                                // err, at-risk → warn.
                                if state == "breached" {
                                    design::callout(ui, CalloutRole::Err, &line);
                                } else {
                                    design::callout(ui, CalloutRole::Warn, &line);
                                }
                                ui.add_space(design::scale::space::SP1);
                            }
                        },
                    );
                }
            },
        );
    }

    fn clusters_ui(&mut self, ui: &mut egui::Ui) {
        let clusters = self.clusters.clone();
        design::section_title(ui, &format!("{} cluster(s)", clusters.len()));
        if clusters.is_empty() {
            design::empty_state(
                ui,
                Icon::Boxes,
                "(no clusters yet)",
                "Clusters appear once enough related conversations accumulate.",
            );
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for cluster in &clusters {
                // The lab's card: title in the card head, then the trend
                // badge, the counts line and the summary as the body.
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
                });
                ui.add_space(design::scale::space::SP2);
            }
        });
    }

    fn known_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
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
                "(no known issues)",
                "Create one to track recurring problems and their workarounds.",
            );
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for issue in &known {
                // The lab's card: title in the head, then the status badge,
                // the conversation count, the symptoms and the workaround.
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
                });
                ui.add_space(design::scale::space::SP2);
            }
        });
    }

    fn new_known_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card_titled(ui, "New known issue", None, |ui| {
            design::form_grid(ui, "radar-new-known", |ui| {
                design::field_label(ui, "title:");
                ui.add_sized(
                    [260.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_title),
                );
                ui.end_row();
                design::field_label(ui, "status:");
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
                        // The visible "status:" label is a separate widget,
                        // so the combo button carries the name itself or
                        // kittest's a11y check trips on an unlabeled node.
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            ui.is_enabled(),
                            format!("status: {status_label}"),
                        )
                    });
                ui.end_row();
                design::field_label(ui, "symptoms:");
                ui.add(
                    egui::TextEdit::multiline(&mut self.new_symptoms).desired_width(f32::INFINITY),
                );
                ui.end_row();
                design::field_label(ui, "workaround:");
                ui.add(
                    egui::TextEdit::multiline(&mut self.new_workaround)
                        .desired_width(f32::INFINITY),
                );
                ui.end_row();
                design::field_label(ui, "conversation ids:");
                ui.add_sized(
                    [260.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_conversation_ids),
                )
                .on_hover_text("comma-separated local conversation ids");
                ui.end_row();
            });
            ui.add_space(design::scale::space::SP1);
            if design::button(ui, "create", ButtonVariant::Primary).clicked() {
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

/// Clean-audit item 81: known-issue links accepted dangling conversation
/// ids and counted them — a link to a nonexistent conversation gained a
/// row and inflated `conversation_count`. Links now materialize only
/// for existing conversations, counts join the conversations table, and
/// the create form tells the user which ids were skipped.
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
        let id = db.with_write(|conn| {
            create_known_issue(
                conn,
                "Login loop",
                None,
                None,
                "investigating",
                &[1, 99, 100], // 99/100 do not exist
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
        assert_eq!(known[0].id, id);
        assert_eq!(known[0].conversation_count, 1, "dangling ids never counted");
    }

    #[test]
    fn pre_fix_dangling_links_no_longer_count() {
        let db = fixture(Vec::new());
        // A pre-fix database: a known issue with two link rows, one of
        // them dangling (conversation 99 never existed).
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
        // 1 exists; 7 does not; 1 repeated must dedupe.
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
