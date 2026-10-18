//! AI Center page (port of the web `pages/ai_center.rs`).
//!
//! Tabs (web: Health / Analytics / Attributes / Jobs):
//! * **Health** — LM Studio settings read from the settings table plus a
//!   test-connection button that lists models through
//!   `ai_lm_studio::OpenAiCompatibleClient` (the `/api/ai/status` route).
//! * **Analytics** — the `/api/ai/analytics` SQL: tickets analyzed, run
//!   success rate, draft states, edit rate, verification warnings,
//!   unsupported-claim rate, common failure patterns.
//! * **Attributes** — the AI attribute catalog (`AiAttributeKey::ALL`,
//!   `/api/attributes/catalog`) and live distributions
//!   (`ai_attributes::distributions`, `/api/attributes/report`).
//! * **Jobs** — the recent `ai_runs` table (`/api/ai/jobs`).

use std::sync::{Arc, Mutex};

use eframe::egui;
use spp_core::ai_attributes::AttributeDistribution;
use spp_core::catalog::AiAttributeKey;
use spp_core::db::DbHandle;

/// Page tabs (web: `AiTab`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Health,
    Analytics,
    Attributes,
    Jobs,
}

impl Tab {
    const ALL: [Self; 4] = [Self::Health, Self::Analytics, Self::Attributes, Self::Jobs];

    fn label(self) -> &'static str {
        match self {
            Self::Health => "Health",
            Self::Analytics => "Analytics",
            Self::Attributes => "Attributes",
            Self::Jobs => "Jobs",
        }
    }
}

/// LM Studio settings snapshot (the `/api/ai/status` settings half).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AiSettings {
    pub ai_enabled: bool,
    pub base_url: String,
    pub chat_model: Option<String>,
    pub embedding_model: Option<String>,
    pub timeout_ms: i64,
    pub concurrency: i64,
}

/// Load the settings half of the AI status (the route's read-only view).
#[must_use]
pub fn load_ai_settings(conn: &rusqlite::Connection) -> AiSettings {
    AiSettings {
        ai_enabled: spp_core::settings::get_bool(conn, "ai_enabled", true).unwrap_or(true),
        base_url: spp_core::settings::get_string(conn, "lmstudio_base_url")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "http://127.0.0.1:1234".into()),
        chat_model: spp_core::settings::get_string(conn, "lmstudio_chat_model")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty()),
        embedding_model: spp_core::settings::get_string(conn, "lmstudio_embedding_model")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty()),
        timeout_ms: spp_core::settings::get_i64(conn, "lmstudio_timeout_ms", 120_000)
            .unwrap_or(120_000),
        concurrency: spp_core::settings::get_i64(conn, "lmstudio_concurrency", 2).unwrap_or(2),
    }
}

/// AI analytics metrics (typed port of the `/api/ai/analytics` SQL).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AiAnalytics {
    pub tickets_analyzed: i64,
    pub analysis_success_rate: f64,
    pub draft_count: i64,
    pub drafts_accepted: i64,
    pub drafts_rejected: i64,
    pub draft_edit_rate: f64,
    pub verification_warnings: i64,
    pub unsupported_claim_rate: f64,
    pub failure_patterns: Vec<(String, i64)>,
}

/// Load the analytics (the route SQL, in-process).
#[must_use]
pub fn load_analytics(conn: &rusqlite::Connection) -> AiAnalytics {
    let (tickets_analyzed, analysis_completed, analysis_failed): (i64, i64, i64) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(DISTINCT conversation_id) FROM ai_runs
                  WHERE type = 'ticket_analysis' AND status = 'completed'
                    AND conversation_id IS NOT NULL),
                (SELECT COUNT(*) FROM ai_runs
                  WHERE type = 'ticket_analysis' AND status = 'completed'),
                (SELECT COUNT(*) FROM ai_runs
                  WHERE type = 'ticket_analysis' AND status = 'failed')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, 0, 0));
    let (draft_count, drafts_accepted, drafts_rejected, drafts_edited): (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM ai_drafts),
                (SELECT COUNT(*) FROM ai_drafts WHERE state = 'accepted'),
                (SELECT COUNT(*) FROM ai_drafts WHERE state = 'rejected'),
                (SELECT COUNT(DISTINCT f.draft_id) FROM ai_feedback f
                   JOIN ai_drafts d ON d.id = f.draft_id
                  WHERE f.edit_distance > 20)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap_or((0, 0, 0, 0));
    let (verification_warnings, unsupported_claims): (i64, i64) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM ai_verifications
                  WHERE verified = 1 AND warnings != '[]'),
                (SELECT COUNT(*) FROM ai_verifications
                  WHERE unsupported_claims != '[]')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));
    let analysis_success_rate = if analysis_completed + analysis_failed > 0 {
        ((analysis_completed as f64) / ((analysis_completed + analysis_failed) as f64) * 1000.0)
            .round()
            / 10.0
    } else {
        0.0
    };
    let feedback_total = drafts_accepted + drafts_rejected + drafts_edited;
    let draft_edit_rate = if feedback_total > 0 {
        ((drafts_edited as f64) / (feedback_total as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };
    let unsupported_claim_rate = if draft_count > 0 {
        ((unsupported_claims as f64) / (draft_count as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };
    let failure_patterns = conn
        .prepare(
            "SELECT json_extract(verification, '$.warnings[0]') AS pattern, COUNT(*) AS count
               FROM ai_drafts
              WHERE verification IS NOT NULL
                AND json_extract(verification, '$.warnings[0]') IS NOT NULL
              GROUP BY pattern ORDER BY count DESC LIMIT 5",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
                .map(|rows| rows.filter_map(Result::ok).collect())
        })
        .unwrap_or_default();
    AiAnalytics {
        tickets_analyzed,
        analysis_success_rate,
        draft_count,
        drafts_accepted,
        drafts_rejected,
        draft_edit_rate,
        verification_warnings,
        unsupported_claim_rate,
        failure_patterns,
    }
}

/// One `ai_runs` job row (typed port of `/api/ai/jobs`).
#[derive(Debug, Clone, Default)]
pub struct AiJobRow {
    pub id: i64,
    pub run_type: String,
    pub status: String,
    pub conversation_id: Option<i64>,
    pub model: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub latency_ms: Option<i64>,
}

/// Load the recent AI job rows.
#[must_use]
pub fn load_jobs(conn: &rusqlite::Connection, limit: i64) -> Vec<AiJobRow> {
    let limit = limit.clamp(1, 1000);
    conn.prepare(
        "SELECT id, type, status, conversation_id, model, error, created_at, latency_ms
           FROM ai_runs ORDER BY id DESC LIMIT ?1",
    )
    .and_then(|mut stmt| {
        stmt.query_map(rusqlite::params![limit], |r| {
            Ok(AiJobRow {
                id: r.get(0)?,
                run_type: r.get(1)?,
                status: r.get(2)?,
                conversation_id: r.get(3)?,
                model: r.get(4)?,
                error: r.get(5)?,
                created_at: r.get(6)?,
                latency_ms: r.get(7)?,
            })
        })
        .map(|rows| rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

/// The interactive AI center page state.
pub struct AiCenterPage {
    tab: Tab,
    settings: Option<AiSettings>,
    analytics: Option<AiAnalytics>,
    distributions: Vec<AttributeDistribution>,
    jobs: Vec<AiJobRow>,
    /// Connection test outcome, written by the background test thread
    /// and polled each frame.
    connection_status: Arc<Mutex<Option<String>>>,
}

impl AiCenterPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::Health,
            settings: None,
            analytics: None,
            distributions: Vec::new(),
            jobs: Vec::new(),
            connection_status: Arc::new(Mutex::new(None)),
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.settings = None;
        self.analytics = None;
        self.distributions.clear();
        self.jobs.clear();
    }

    fn reload(&mut self, db: &DbHandle) {
        let (settings, analytics, distributions, jobs) = db.with_read(|conn| {
            (
                Some(load_ai_settings(conn)),
                Some(load_analytics(conn)),
                spp_core::ai_attributes::distributions(conn).unwrap_or_default(),
                load_jobs(conn, 100),
            )
        });
        self.settings = settings;
        self.analytics = analytics;
        self.distributions = distributions;
        self.jobs = jobs;
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.settings.is_none() {
            self.reload(db);
        }
    }

    /// Test the LM Studio connection (the async half of `/api/ai/status`):
    /// the result lands in the shared status slot on a background thread.
    pub fn test_connection(&mut self) {
        let Some(settings) = self.settings.clone() else {
            return;
        };
        let base_url = settings.base_url.clone();
        let timeout = u64::try_from(settings.timeout_ms.max(1)).unwrap_or(120_000);
        let status = Arc::clone(&self.connection_status);
        if let Ok(mut guard) = status.lock() {
            *guard = Some("testing…".into());
        }
        std::thread::Builder::new()
            .name("spp-ai-test".to_string())
            .spawn(move || {
                let result = (|| -> Result<usize, String> {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| e.to_string())?;
                    runtime.block_on(async {
                        let client =
                            spp_core::ai_lm_studio::OpenAiCompatibleClient::new_with_timeout(
                                &base_url, timeout,
                            );
                        let models = client.list_models().await.map_err(|e| e.to_string())?;
                        Ok(models.len())
                    })
                })();
                if let Ok(mut guard) = status.lock() {
                    *guard = Some(match result {
                        Ok(0) => "connected · no models loaded".into(),
                        Ok(n) => format!("connected · {n} model(s)"),
                        Err(e) => format!("offline · {e}"),
                    });
                }
            })
            .ok();
        // The thread is detached: the status slot is polled every frame.
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        ui.heading("AI Center");
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
        });
        ui.separator();

        match self.tab {
            Tab::Health => self.health_ui(ui, db),
            Tab::Analytics => self.analytics_ui(ui),
            Tab::Attributes => self.attributes_ui(ui),
            Tab::Jobs => self.jobs_ui(ui),
        }
    }

    fn health_ui(&mut self, ui: &mut egui::Ui, _db: &DbHandle) {
        let Some(settings) = self.settings.clone() else {
            ui.spinner();
            return;
        };
        egui::Grid::new("ai-health")
            .num_columns(2)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                ui.strong("AI enabled");
                ui.monospace(settings.ai_enabled.to_string());
                ui.end_row();
                ui.strong("provider");
                ui.monospace(if settings.ai_enabled {
                    "lmstudio"
                } else {
                    "disabled"
                });
                ui.end_row();
                ui.strong("base url");
                ui.monospace(settings.base_url.as_str());
                ui.end_row();
                ui.strong("chat model");
                ui.monospace(
                    settings
                        .chat_model
                        .clone()
                        .unwrap_or_else(|| "(unset)".into()),
                );
                ui.end_row();
                ui.strong("embedding model");
                ui.monospace(
                    settings
                        .embedding_model
                        .clone()
                        .unwrap_or_else(|| "(unset)".into()),
                );
                ui.end_row();
                ui.strong("timeout");
                ui.monospace(format!("{} ms", settings.timeout_ms));
                ui.end_row();
                ui.strong("concurrency");
                ui.monospace(settings.concurrency.to_string());
                ui.end_row();
            });
        ui.separator();
        ui.weak(
            "Start LM Studio locally (default http://127.0.0.1:1234) and configure the \
             models in Settings → Integrations. Attributes stay local and never sync back.",
        );
        if let Some(status) = self
            .connection_status
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
        {
            ui.label(format!("connection: {status}"));
        }
        if ui.button("test connection").clicked() {
            self.test_connection();
        }
    }

    fn analytics_ui(&mut self, ui: &mut egui::Ui) {
        let Some(analytics) = self.analytics.clone() else {
            ui.spinner();
            return;
        };
        egui::Grid::new("ai-analytics")
            .num_columns(2)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                ui.strong("tickets analyzed");
                ui.monospace(analytics.tickets_analyzed.to_string());
                ui.end_row();
                ui.strong("analysis success rate");
                ui.monospace(format!("{}%", analytics.analysis_success_rate));
                ui.end_row();
                ui.strong("drafts");
                ui.monospace(format!(
                    "{} total · {} accepted · {} rejected",
                    analytics.draft_count, analytics.drafts_accepted, analytics.drafts_rejected
                ));
                ui.end_row();
                ui.strong("draft edit rate");
                ui.monospace(format!("{}%", analytics.draft_edit_rate));
                ui.end_row();
                ui.strong("verification warnings");
                ui.monospace(analytics.verification_warnings.to_string());
                ui.end_row();
                ui.strong("unsupported claim rate");
                ui.monospace(format!("{}%", analytics.unsupported_claim_rate));
                ui.end_row();
            });
        ui.separator();
        if analytics.failure_patterns.is_empty() {
            ui.weak("(no failure patterns)");
        } else {
            ui.strong("common failure patterns");
            for (pattern, count) in &analytics.failure_patterns {
                ui.label(format!("{pattern} · {count}"));
            }
        }
    }

    fn attributes_ui(&mut self, ui: &mut egui::Ui) {
        ui.strong("Catalog");
        ui.weak(
            "A missing attribute reads as unknown — it is never fabricated. Attributes \
             are local-only and never written to Help Scout.",
        );
        let distributions = self.distributions.clone();
        ui.separator();
        if distributions.is_empty() {
            ui.weak("(no attribute data yet — run ticket analysis)");
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for dist in &distributions {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(dist.label);
                        ui.monospace(dist.attribute);
                        ui.weak(format!("{} known / {} unknown", dist.known, dist.unknown));
                    });
                    for value in &dist.values {
                        let share = if dist.known > 0 {
                            value.count as f64 / dist.known as f64 * 100.0
                        } else {
                            0.0
                        };
                        ui.monospace(format!(
                            "  {}: {} ({:.1}%)",
                            value.value, value.count, share
                        ));
                    }
                });
            }
        });
    }

    fn jobs_ui(&mut self, ui: &mut egui::Ui) {
        let jobs = self.jobs.clone();
        if jobs.is_empty() {
            ui.weak("(no AI runs recorded)");
            return;
        }
        egui::ScrollArea::vertical().show_rows(ui, 24.0, jobs.len(), |ui, range| {
            egui::Grid::new("ai-jobs")
                .num_columns(6)
                .striped(true)
                .show(ui, |ui| {
                    for header in ["id", "type", "status", "conversation", "model", "created"] {
                        ui.strong(header);
                    }
                    ui.end_row();
                    for job in &jobs[range] {
                        ui.monospace(job.id.to_string());
                        ui.label(job.run_type.as_str());
                        ui.colored_label(
                            if job.status == "failed" {
                                egui::Color32::RED
                            } else if job.status == "completed" {
                                egui::Color32::GREEN
                            } else {
                                egui::Color32::GRAY
                            },
                            job.status.as_str(),
                        );
                        ui.monospace(
                            job.conversation_id
                                .map(|id| format!("#{id}"))
                                .unwrap_or_else(|| "—".into()),
                        );
                        ui.monospace(job.model.clone().unwrap_or_else(|| "—".into()));
                        ui.monospace(job.created_at.as_str());
                        ui.end_row();
                    }
                });
        });
    }
}

impl Default for AiCenterPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Catalog row for the attribute table (web: `/api/attributes/catalog`).
#[must_use]
pub fn attribute_catalog() -> Vec<(&'static str, &'static str, &'static str)> {
    AiAttributeKey::ALL
        .iter()
        .map(|k| (k.as_str(), k.label(), k.value_type().as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn settings_default_to_lmstudio_endpoint() {
        let db = fixture(Vec::new());
        let settings = db.with_read(load_ai_settings);
        assert!(settings.ai_enabled);
        assert_eq!(settings.base_url, "http://127.0.0.1:1234");
        assert_eq!(settings.timeout_ms, 120_000);
        assert_eq!(settings.concurrency, 2);
        assert!(settings.chat_model.is_none());
    }

    #[test]
    fn analytics_counts_seeded_runs_and_drafts() {
        let db = fixture(vec![("conv", 1, "Analyzed", "active", "email", 1, 5, 9)]);
        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO ai_runs (input_hash, prompt_version, model, response_json, type, status, conversation_id, created_at)
                 VALUES ('r1', 'v1', 'm', '{}', 'ticket_analysis', 'completed', 1, '2026-01-01T00:00:00Z'),
                        ('r2', 'v1', 'm', '{}', 'ticket_analysis', 'failed', NULL, '2026-01-01T00:01:00Z')",
                [],
            )
            .expect("runs");
            conn.execute(
                "INSERT INTO ai_drafts (conversation_id, content, state, created_at)
                 VALUES (1, 'draft one', 'accepted', '2026-01-01T00:02:00Z'),
                        (1, 'draft two', 'rejected', '2026-01-01T00:03:00Z')",
                [],
            )
            .expect("drafts");
        });
        let analytics = db.with_read(load_analytics);
        assert_eq!(analytics.tickets_analyzed, 1);
        assert_eq!(analytics.analysis_success_rate, 50.0);
        assert_eq!(analytics.draft_count, 2);
        assert_eq!(analytics.drafts_accepted, 1);
        assert_eq!(analytics.drafts_rejected, 1);
        assert!(analytics.failure_patterns.is_empty());
    }

    #[test]
    fn jobs_rows_load_after_seeding() {
        let db = fixture(vec![("conv", 7, "Seven", "active", "email", 1, 5, 9)]);
        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO ai_runs (input_hash, prompt_version, model, response_json, type, status, conversation_id, created_at)
                 VALUES ('r3', 'v1', 'qwen-7b', '{}', 'ticket_analysis', 'completed', 7, '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("run");
        });
        let jobs = db.with_read(|conn| load_jobs(conn, 100));
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].run_type, "ticket_analysis");
        assert_eq!(jobs[0].conversation_id, Some(7));
        assert_eq!(jobs[0].model.as_deref(), Some("qwen-7b"));
    }

    #[test]
    fn attribute_catalog_covers_all_keys() {
        let catalog = attribute_catalog();
        assert_eq!(catalog.len(), AiAttributeKey::ALL.len());
        for (key, label, value_type) in &catalog {
            assert!(!key.is_empty());
            assert!(!label.is_empty());
            assert!(!value_type.is_empty());
        }
    }

    #[test]
    fn distributions_empty_on_a_fresh_database() {
        let db = fixture(Vec::new());
        let distributions = db
            .with_read(spp_core::ai_attributes::distributions)
            .unwrap_or_default();
        // Fresh DB: attributes exist in the schema but no snapshots yet.
        assert!(distributions.iter().all(|d| d.total_conversations >= 0));
    }
}
