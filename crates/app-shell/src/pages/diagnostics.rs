//! Diagnostics page (1.2.0).
//!
//! Every error and report the app produces used to vanish into stdout
//! logs — the user saw "nothing is syncing" with no way to find out why.
//! This page surfaces the persisted record:
//!
//! * **Event log** — `application_errors` via
//!   [`spp_core::audit::list_recent_events`]: sync failures, automatic
//!   client-credentials sign-ins, OAuth callback outcomes, webhook
//!   rejections and job failures, written by the sync engine, the
//!   workers, the loopback listener and the settings page. Filter chips
//!   (all / errors / warnings / info), expandable per-row details, and
//!   a clear button.
//! * **Sync reports** — the recent `sync_runs` rows (kind, state,
//!   duration, records, errors) so a run's outcome is visible without
//!   reading logs.

use eframe::egui;
use spp_core::db::DbHandle;

use crate::design::{self, BadgeTone, ButtonVariant};

/// Which events the log table shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LevelFilter {
    All,
    Errors,
    Warnings,
    Info,
}

impl LevelFilter {
    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Errors => "errors",
            Self::Warnings => "warnings",
            Self::Info => "info",
        }
    }

    fn level(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Errors => Some("error"),
            Self::Warnings => Some("warn"),
            Self::Info => Some("info"),
        }
    }
}

/// One sync run row as the reports card shows it.
#[derive(Debug, Clone)]
struct SyncRunRow {
    id: i64,
    kind: String,
    state: String,
    started_at: String,
    finished_at: Option<String>,
    records_processed: i64,
    errors: i64,
}

/// The interactive Diagnostics page state.
pub struct DiagnosticsPage {
    events: Option<Vec<spp_core::audit::DiagEvent>>,
    runs: Option<Vec<SyncRunRow>>,
    filter: LevelFilter,
    message: Option<String>,
}

impl DiagnosticsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: None,
            runs: None,
            filter: LevelFilter::All,
            message: None,
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.events = None;
        self.runs = None;
    }

    fn reload(&mut self, db: &DbHandle) {
        let filter = self.filter;
        let events = db.with_read(|conn| {
            spp_core::audit::list_recent_events(conn, 200, filter.level()).unwrap_or_default()
        });
        let runs = db.with_read(load_sync_runs);
        self.events = Some(events);
        self.runs = Some(runs);
    }

    /// Delete every stored diagnostics event.
    fn clear_log(&mut self, db: &DbHandle) {
        let cleared = db.with_read(spp_core::audit::clear_events).unwrap_or(0);
        self.message = Some(format!("event log cleared ({cleared} rows removed)"));
        self.invalidate();
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.events.is_none() || self.runs.is_none() {
            self.reload(db);
        }

        design::page_header(
            ui,
            "Diagnostics",
            Some(
                "Every error and report the app records: sync failures, \
                 connection problems, webhook rejections and background job \
                 failures — with the outcome of every sync run.",
            ),
            |_| {},
        );

        if let Some(message) = &self.message {
            design::success_text(ui, message);
            self.message = None;
        }

        let events = self.events.clone().unwrap_or_default();
        let runs = self.runs.clone().unwrap_or_default();

        // --- Event log -------------------------------------------------
        let error_count = events.iter().filter(|e| e.level == "error").count();
        let warn_count = events.iter().filter(|e| e.level == "warn").count();
        let subtitle = format!(
            "{} event(s) shown — {} error(s), {} warning(s)",
            events.len(),
            error_count,
            warn_count
        );
        design::card_titled(ui, "Event log", Some(&subtitle), |ui| {
            ui.horizontal(|ui| {
                for filter in [
                    LevelFilter::All,
                    LevelFilter::Errors,
                    LevelFilter::Warnings,
                    LevelFilter::Info,
                ] {
                    let selected = self.filter == filter;
                    if ui.selectable_label(selected, filter.label()).clicked() {
                        self.filter = filter;
                        self.invalidate();
                    }
                }
                if design::button(ui, "refresh", ButtonVariant::Outline).clicked() {
                    self.invalidate();
                }
                if !events.is_empty()
                    && design::button(ui, "clear log", ButtonVariant::Destructive).clicked()
                {
                    self.clear_log(db);
                }
            });
            ui.add_space(6.0);
            if events.is_empty() {
                design::hint_text(
                    ui,
                    "No events recorded yet. Errors from syncing, connecting \
                         and background jobs will appear here.",
                );
                return;
            }
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .auto_shrink([false, false])
                .show_rows(ui, 26.0, events.len(), |ui, range| {
                    for event in &events[range] {
                        let tone = match event.level.as_str() {
                            "error" => BadgeTone::Destructive,
                            "warn" => BadgeTone::Warning,
                            _ => BadgeTone::Info,
                        };
                        ui.horizontal(|ui| {
                            design::badge(ui, &event.level, tone);
                            ui.monospace(egui::RichText::new(&event.timestamp).size(12.0).weak());
                            if !event.service.is_empty() {
                                design::badge(ui, &event.service, BadgeTone::Neutral);
                            }
                            ui.label(
                                egui::RichText::new(&event.message)
                                    .size(12.5)
                                    .color(design::tokens(ui).foreground),
                            );
                        });
                        if event.stack.is_some() || event.context.is_some() {
                            ui.indent(format!("evt-{}", event.id), |ui| {
                                if let Some(stack) = &event.stack {
                                    ui.monospace(egui::RichText::new(stack).size(11.0).weak());
                                }
                                if let Some(context) = &event.context {
                                    ui.monospace(
                                        egui::RichText::new(context.to_string()).size(11.0).weak(),
                                    );
                                }
                            });
                        }
                        ui.add_space(2.0);
                    }
                });
        });
        ui.add_space(10.0);

        // --- Sync reports ----------------------------------------------
        design::card_titled(
            ui,
            "Sync reports",
            Some("The outcome of every recent sync run (newest first)."),
            |ui| {
                if runs.is_empty() {
                    design::hint_text(
                        ui,
                        "No sync runs recorded yet. Runs start automatically once \
                         Help Scout credentials are configured.",
                    );
                    return;
                }
                design::table(
                    ui,
                    "diagnostics-runs",
                    &["run", "kind", "state", "records", "errors", "started"],
                    24.0,
                    |ui| {
                        for run in &runs {
                            ui.monospace(egui::RichText::new(run.id.to_string()).size(12.5));
                            ui.monospace(egui::RichText::new(run.kind.as_str()).size(12.5));
                            let tone = match run.state.as_str() {
                                "LIVE" => BadgeTone::Success,
                                "ERROR" => BadgeTone::Destructive,
                                _ => BadgeTone::Warning,
                            };
                            design::badge(ui, &run.state, tone);
                            design::td_num(ui, &run.records_processed.to_string());
                            design::td_num(ui, &run.errors.to_string());
                            ui.monospace(
                                egui::RichText::new(
                                    run.finished_at
                                        .clone()
                                        .unwrap_or_else(|| run.started_at.clone()),
                                )
                                .size(12.0)
                                .weak(),
                            );
                            ui.end_row();
                        }
                    },
                );
            },
        );
    }
}

impl Default for DiagnosticsPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Load the recent sync runs (the sync-health queries, capped).
fn load_sync_runs(conn: &rusqlite::Connection) -> Vec<SyncRunRow> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, kind, state, started_at, finished_at, records_processed, errors \
         FROM sync_runs ORDER BY id DESC LIMIT 25",
    ) else {
        return Vec::new();
    };
    stmt.query_map([], |r| {
        Ok(SyncRunRow {
            id: r.get(0)?,
            kind: r.get(1)?,
            state: r.get(2)?,
            started_at: r.get(3)?,
            finished_at: r.get(4)?,
            records_processed: r.get(5)?,
            errors: r.get(6)?,
        })
    })
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn page_lists_recorded_events_and_sync_runs() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            spp_core::audit::log_event(
                conn,
                "error",
                "sync",
                "Sync resource conversations failed",
                None,
                Some(&serde_json::json!({ "resource": "conversations" })),
            )
            .expect("log");
            spp_core::audit::log_event(conn, "info", "sync", "Initial sync finished", None, None)
                .expect("log");
            spp_core::sync_engine::start_run(conn, "initial").expect("run");
        });

        let mut page = DiagnosticsPage::new();
        page.reload(&db);
        let events = page.events.clone().expect("events");
        assert_eq!(events.len(), 2, "both events must be listed");
        assert_eq!(events[0].message, "Initial sync finished", "newest first");
        let runs = page.runs.clone().expect("runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].kind, "initial");
    }

    #[test]
    fn filter_errors_only_hides_non_errors() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            spp_core::audit::log_event(conn, "info", "sync", "info event", None, None).expect("i");
            spp_core::audit::log_event(conn, "error", "sync", "bad thing", None, None).expect("e");
        });
        let mut page = DiagnosticsPage::new();
        page.filter = LevelFilter::Errors;
        page.reload(&db);
        let events = page.events.clone().expect("events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].level, "error");
        assert_eq!(events[0].message, "bad thing");
    }

    #[test]
    fn clear_log_empties_the_view() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            spp_core::audit::log_event(conn, "warn", "webhook", "w", None, None).expect("w");
        });
        let mut page = DiagnosticsPage::new();
        page.clear_log(&db);
        page.reload(&db);
        let events = page.events.clone().expect("events");
        assert!(events.is_empty());
        assert!(page.message.is_some());
    }
}
