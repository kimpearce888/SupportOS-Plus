use std::sync::Arc;

use eframe::egui;
use rusqlite::Connection;
use spp_core::db::DbHandle;
use spp_core::sync_engine::SyncEngine;

use crate::design::{self, ButtonVariant};
use crate::icons::Icon;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CheckpointRow {
    pub resource: String,
    pub last_success_at: Option<String>,
    pub records_processed: i64,
    pub records_failed: i64,
    pub last_error: Option<String>,
    pub retry_count: i64,
    pub status: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RunRow {
    pub id: i64,
    pub kind: String,
    pub state: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub resources_done: i64,
    pub resources_total: i64,
    pub records_processed: i64,
    pub errors: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct WebhookQueue {
    pub total: i64,
    pub pending: i64,
    pub processed: i64,
    pub failed: i64,
    pub ignored: i64,
    pub duplicates: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WebhookHealthSnapshot {
    pub verdict: String,
    pub detail: String,
    pub checked_at: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SyncStatus {
    pub state: String,
    pub last_success: Option<String>,
    pub checkpoints: Vec<CheckpointRow>,
    pub runs: Vec<RunRow>,
    pub webhooks: WebhookQueue,
    pub webhook_health: Option<WebhookHealthSnapshot>,
}

#[must_use]
pub(crate) fn load_status(conn: &Connection) -> SyncStatus {
    let state = spp_core::sync_engine::get_state(conn);
    let last_success = spp_core::sync_engine::last_successful_sync(conn);

    let checkpoints = conn
        .prepare(
            "SELECT resource, last_success_at, records_processed, records_failed,
                    last_error, retry_count, status
               FROM sync_checkpoints ORDER BY resource",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(CheckpointRow {
                    resource: r.get(0)?,
                    last_success_at: r.get(1)?,
                    records_processed: r.get(2)?,
                    records_failed: r.get(3)?,
                    last_error: r.get(4)?,
                    retry_count: r.get(5)?,
                    status: r.get(6)?,
                })
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
        })
        .unwrap_or_default();

    let runs = conn
        .prepare(
            "SELECT id, kind, state, started_at, finished_at, resources_done,
                    resources_total, records_processed, errors
               FROM sync_runs ORDER BY id DESC LIMIT 10",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(RunRow {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    state: r.get(2)?,
                    started_at: r.get(3)?,
                    finished_at: r.get(4)?,
                    resources_done: r.get(5)?,
                    resources_total: r.get(6)?,
                    records_processed: r.get(7)?,
                    errors: r.get(8)?,
                })
            })
            .map(|rows| rows.filter_map(Result::ok).collect())
        })
        .unwrap_or_default();

    let webhooks = conn
        .query_row(
            "SELECT COUNT(*),
                    SUM(CASE WHEN processing_state = 'pending' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN processing_state = 'processed' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN processing_state = 'failed' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN processing_state = 'ignored' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN processing_state = 'duplicate' THEN 1 ELSE 0 END)
               FROM webhook_events",
            [],
            |r| {
                Ok(WebhookQueue {
                    total: r.get(0)?,
                    pending: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    processed: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    failed: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    ignored: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    duplicates: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                })
            },
        )
        .unwrap_or_default();

    let webhook_health = spp_core::webhook_health::load_outcome(conn)
        .ok()
        .flatten()
        .map(|o| WebhookHealthSnapshot {
            verdict: o.verdict,
            detail: o.detail,
            checked_at: o.checked_at,
        });

    SyncStatus {
        state,
        last_success,
        checkpoints,
        runs,
        webhooks,
        webhook_health,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncAction {
    Initial,
    Incremental,
    Reconcile,
    Cancel,
}

impl SyncAction {
    fn label(self) -> &'static str {
        match self {
            Self::Initial => "Run initial sync",
            Self::Incremental => "Run incremental sync",
            Self::Reconcile => "Run reconcile",
            Self::Cancel => "Cancel running sync",
        }
    }
}

pub(crate) fn spawn_sync_action(
    db: &DbHandle,
    engine: &Arc<SyncEngine>,
    action: SyncAction,
) -> bool {
    if action != SyncAction::Cancel && engine.is_running() {
        return false;
    }
    let engine = Arc::clone(engine);
    let db = db.clone();
    let log_db = db.clone();
    let spawned = std::thread::Builder::new()
        .name("spp-sync-action".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let bus = engine.bus.clone();
            let result = runtime
                .map_err(|e| spp_core::Error::Config(e.to_string()))
                .and_then(|rt| {
                    rt.block_on(async move {
                        match action {
                            SyncAction::Initial => engine
                                .initial_sync(spp_core::events::SyncTrigger::Manual)
                                .await
                                .map(|_| "Initial sync completed.".to_string()),
                            SyncAction::Incremental => engine
                                .incremental_sync(spp_core::events::SyncTrigger::Manual)
                                .await
                                .map(|_| "Incremental sync completed.".to_string()),
                            SyncAction::Reconcile => engine
                                .reconcile()
                                .await
                                .map(|_| "Reconcile completed.".to_string()),
                            SyncAction::Cancel => {
                                engine.request_cancellation();
                                Ok("Cancellation requested.".to_string())
                            }
                        }
                    })
                });
            if let Err(e) = result {
                db.with_write(|conn| {
                    let _ = spp_core::jobs::log_error(
                        conn,
                        "sync",
                        &format!("{} failed: {e}", action_label_static(action)),
                    );
                });
                if let Some(bus) = &bus {
                    spp_core::event_bus::notify_error(
                        bus,
                        "error",
                        &format!("{} failed: {e}", action_label_static(action)),
                        "sync",
                        None,
                    );
                }
            }
        });
    if spawned.is_err() {
        log_db.with_write(|conn| {
            let _ =
                spp_core::jobs::log_error(conn, "sync", "failed to spawn the sync action thread");
        });
        return false;
    }
    true
}

pub(crate) fn spawn_docs_sync(db: &DbHandle, engine: &Arc<SyncEngine>) {
    if engine.is_running() {
        return;
    }
    let engine = Arc::clone(engine);
    let db = db.clone();
    let log_db = db.clone();
    let spawned = std::thread::Builder::new()
        .name("spp-docs-sync".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let bus = engine.bus.clone();
            let result = runtime
                .map_err(|e| spp_core::Error::Config(e.to_string()))
                .and_then(|rt| {
                    rt.block_on(async move {
                        engine
                            .sync_docs_only(spp_core::events::SyncTrigger::Manual)
                            .await
                            .map(|_| ())
                    })
                });
            if let Err(e) = result {
                db.with_write(|conn| {
                    let _ = spp_core::jobs::log_error(
                        conn,
                        "docs",
                        &format!("Help Center sync failed: {e}"),
                    );
                });
                if let Some(bus) = &bus {
                    spp_core::event_bus::notify_error(
                        bus,
                        "error",
                        &format!("Help Center sync failed: {e}"),
                        "docs",
                        None,
                    );
                }
            }
        });
    if spawned.is_err() {
        log_db.with_write(|conn| {
            let _ = spp_core::jobs::log_error(
                conn,
                "docs",
                "failed to spawn the Help Center sync thread",
            );
        });
    }
}

pub struct SyncHealthPage {
    status: Option<SyncStatus>,
    message: Option<String>,
    last_progress: Option<spp_core::events::SyncProgressEvent>,
}

impl SyncHealthPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            status: None,
            message: None,
            last_progress: None,
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.status = None;
        self.message = None;
    }

    pub(crate) fn note_progress(&mut self, event: spp_core::events::SyncProgressEvent) {
        self.last_progress = Some(event);
        self.status = None;
    }

    fn reload(&mut self, db: &DbHandle) {
        self.status = Some(db.with_read(load_status));
    }

    pub(crate) fn run_sync_action(
        &mut self,
        db: &DbHandle,
        engine: &Arc<SyncEngine>,
        action: SyncAction,
    ) {
        if action != SyncAction::Cancel && engine.is_running() {
            self.message = Some("A sync is already running.".into());
            return;
        }
        if action == SyncAction::Cancel && !engine.is_running() {
            self.message = Some("No sync is running.".into());
            return;
        }
        if spawn_sync_action(db, engine, action) {
            self.message = Some(format!("{} (started in background)", action.label()));
        } else {
            self.message = Some("A sync is already running.".into());
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, engine: Option<&Arc<SyncEngine>>) {
        if self.status.is_none() {
            self.reload(db);
        }

        design::page_header(
            ui,
            "Sync Health",
            Some("engine state, checkpoints, and recent runs"),
            |_| {},
        );
        if let Some(p) = &self.last_progress {
            let stage_line = if p.total > 0 {
                format!(
                    "progress: {} · resource {}/{} · {} record(s) so far",
                    p.stage, p.done, p.total, p.processed
                )
            } else {
                format!("progress: {} · {} record(s) so far", p.stage, p.processed)
            };
            design::hint_text(ui, &stage_line);
        }
        let engine_running = engine.is_some_and(|e| e.is_running());
        if engine_running {
            design::hint_text(ui, "engine: running");
        }
        if let Some(message) = &self.message {
            design::success_text(ui, &format!("status: {message}"));
        }
        ui.add_space(8.0);

        let status = self.status.clone().unwrap_or_default();
        let state_tone = match status.state.as_str() {
            "LIVE" => crate::design::BadgeTone::Success,
            "ERROR" => crate::design::BadgeTone::Destructive,
            "NEW" => crate::design::BadgeTone::Neutral,
            "CATCHING_UP" => crate::design::BadgeTone::Info,
            _ => crate::design::BadgeTone::Warning,
        };
        let state_label = status.state.clone();
        // UX-20: the hero line carried a developer note ("never shows
        // relative time — forensic value"). Keep the exact timestamp (it is
        // the forensic record) but say it in user language, with the
        // relative age next to it so freshness is readable at a glance.
        let last_success = match status.last_success.as_deref() {
            Some(iso) => {
                let rel = crate::pages::relative_time(iso);
                if rel == iso {
                    iso.to_string()
                } else {
                    format!("{iso} · {rel}")
                }
            }
            None => "never synced".to_string(),
        };
        design::hero(
            ui,
            &status.state,
            &format!("Last success: {last_success}"),
            |ui| {
                design::badge(ui, &state_label, state_tone);
            },
        );
        design::card_titled(ui, "Engine", None, |ui| {
            // AUD-P2-20 (B7): the ignored bucket counts unknown event
            // types — accepted and stored, but honestly not processed.
            design::hint_text(
                ui,
                &format!(
                    "webhook queue: {} total · {} pending · {} processed · {} failed · {} ignored · {} duplicates",
                    status.webhooks.total,
                    status.webhooks.pending,
                    status.webhooks.processed,
                    status.webhooks.failed,
                    status.webhooks.ignored,
                    status.webhooks.duplicates
                ),
            );
            if let Some(health) = &status.webhook_health {
                design::hint_text(
                    ui,
                    &format!(
                        "webhook health: {} · checked {} · {}",
                        health.verdict, health.checked_at, health.detail
                    ),
                );
            }
        });
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            for action in [
                SyncAction::Initial,
                SyncAction::Incremental,
                SyncAction::Reconcile,
            ] {
                let variant = match action {
                    SyncAction::Initial => ButtonVariant::Primary,
                    _ => ButtonVariant::Outline,
                };
                if design::button(ui, action.label(), variant).clicked() {
                    match engine {
                        Some(engine) => {
                            self.run_sync_action(db, engine, action);
                            self.status = None;
                        }
                        None => {
                            self.message =
                                Some("The sync engine is not available in this build.".into());
                        }
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if design::button(ui, SyncAction::Cancel.label(), ButtonVariant::Destructive)
                    .clicked()
                {
                    match engine {
                        Some(engine) => {
                            self.run_sync_action(db, engine, SyncAction::Cancel);
                            self.status = None;
                        }
                        None => {
                            self.message =
                                Some("The sync engine is not available in this build.".into());
                        }
                    }
                }
            });
        });
        ui.add_space(8.0);

        design::card_titled_wide(ui, "Checkpoints", None, |ui| {
            if status.checkpoints.is_empty() {
                design::empty_state(
                    ui,
                    Icon::HeartPulse,
                    "(no checkpoints yet — run an initial sync)",
                    "The checkpoint table fills after the first successful sync.",
                );
            } else {
                // UXG-1-5: exact-column data_table (virtualized rows built
                // in) — the checkpoint list spans the card at every width;
                // the last column keeps its values beside its header.
                let checkpoints = &status.checkpoints;
                design::data_table(
                    ui,
                    "sync-checkpoint-table",
                    &[
                        "Resource",
                        "Status",
                        "Processed",
                        "Failed",
                        "Retries",
                        "Last Success",
                    ],
                    checkpoints.len(),
                    26.0,
                    |ui, i| {
                        let t = design::tokens(ui);
                        let cp = &checkpoints[i];
                        ui.monospace(
                            egui::RichText::new(cp.resource.as_str())
                                .size(13.0)
                                .color(t.foreground),
                        );
                        ui.label(
                            egui::RichText::new(cp.status.as_str())
                                .size(13.0)
                                .color(t.foreground),
                        );
                        design::td_num(ui, &cp.records_processed.to_string());
                        design::td_num(ui, &cp.records_failed.to_string());
                        design::td_num(ui, &cp.retry_count.to_string());
                        let last = cp.last_success_at.clone().unwrap_or_else(|| "—".into());
                        ui.monospace(egui::RichText::new(last.as_str()).size(12.5))
                            .on_hover_text(if last == "—" {
                                "This resource has never completed a sync.".to_string()
                            } else {
                                format!("{} ({})", last, crate::pages::relative_time(&last))
                            });
                    },
                );
            }
        });
        ui.add_space(8.0);

        design::card_titled_wide(ui, "Recent runs", None, |ui| {
            if status.runs.is_empty() {
                design::empty_state(
                    ui,
                    Icon::Activity,
                    "No runs recorded",
                    "Runs appear here after the first sync.",
                );
            } else {
                // UXG-1-5: exact-column data_table (virtualized rows built
                // in) — the runs list spans the card at every width.
                let runs = &status.runs;
                design::data_table(
                    ui,
                    "sync-run-table",
                    &[
                        "Id",
                        "Kind",
                        "State",
                        "Started",
                        "Resources",
                        "Records",
                        "Errors",
                    ],
                    runs.len(),
                    26.0,
                    |ui, i| {
                        let t = design::tokens(ui);
                        let run = &runs[i];
                        design::td_num(ui, &run.id.to_string());
                        ui.label(
                            egui::RichText::new(run.kind.as_str())
                                .size(13.0)
                                .color(t.foreground),
                        );
                        ui.label(
                            egui::RichText::new(run.state.as_str())
                                .size(13.0)
                                .color(t.foreground),
                        );
                        design::td_num(ui, run.started_at.as_str());
                        design::td_num(
                            ui,
                            &format!("{}/{}", run.resources_done, run.resources_total),
                        );
                        design::td_num(ui, &run.records_processed.to_string());
                        design::td_num(ui, &run.errors.to_string());
                    },
                );
            }
        });
        ui.add_space(8.0);
    }
}

impl Default for SyncHealthPage {
    fn default() -> Self {
        Self::new()
    }
}

fn action_label_static(action: SyncAction) -> &'static str {
    action.label()
}

#[cfg(test)]
mod tests {
    use super::super::{i64_field, opt_str_field, str_field};
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn status_loads_from_seeded_checkpoints_and_runs() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            spp_core::sync_engine::start_run(conn, "initial").expect("run");
        });
        let status = db.with_read(load_status);
        assert_eq!(status.state, "NEW", "the unsynced default state");
        assert_eq!(status.runs.len(), 1);
        assert_eq!(status.runs[0].kind, "initial");
        assert_eq!(status.webhooks.total, 0);
        assert!(status.checkpoints.is_empty());
        assert!(status.last_success.is_none());
    }

    #[test]
    fn webhook_queue_counters_count_states() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            for (state, hash, event_id) in [
                ("pending", "h1", Some("a".to_string())),
                ("processed", "h2", Some("b".to_string())),
                ("processed", "h3", Some("c".to_string())),
                ("failed", "h4", Some("d".to_string())),
                ("duplicate", "h5", None),
            ] {
                conn.execute(
                    "INSERT INTO webhook_events (event_hash, event_id, event_type, payload, received_at, processing_state, attempts)
                     VALUES (?1, ?2, 'convo.created', '{}', strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?3, 0)",
                    rusqlite::params![hash, event_id, state],
                )
                .expect("webhook event");
            }
        });
        let status = db.with_read(load_status);
        assert_eq!(status.webhooks.total, 5);
        assert_eq!(status.webhooks.pending, 1);
        assert_eq!(status.webhooks.processed, 2);
        assert_eq!(status.webhooks.failed, 1);
        assert_eq!(status.webhooks.duplicates, 1);
    }

    #[test]
    fn json_field_helpers_read_checkpoint_shaped_rows() {
        let v = serde_json::json!({
            "state": "idle",
            "last_success": null,
            "total": 5,
        });
        assert_eq!(str_field(&v, "state"), "idle");
        assert_eq!(opt_str_field(&v, "last_success"), None);
        assert_eq!(i64_field(&v, "total"), 5);
    }

    #[test]
    fn webhook_health_outcome_surfaces_in_the_status_model() {
        let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        spp_core::bootstrap::apply_all(&mut conn).expect("migrations");
        assert!(
            load_status(&conn).webhook_health.is_none(),
            "no check has run yet"
        );
        let outcome = spp_core::webhook_health::not_configured_outcome("probe: no relay URL");
        spp_core::webhook_health::store_outcome(&conn, &outcome).expect("store outcome");
        let status = load_status(&conn);
        let health = status.webhook_health.expect("outcome surfaced");
        assert_eq!(health.verdict, "not_configured");
        assert_eq!(health.detail, "probe: no relay URL");
    }
}
