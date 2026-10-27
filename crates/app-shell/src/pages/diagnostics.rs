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
//! * **Audit trail** — `audit_log` via [`spp_core::audit::list_audit`]
//!   (clean-audit item 68): who changed what, when, with what before/
//!   after state and remote outcome — the record the outbound lane,
//!   the people store, the copilot and the side-thread writers keep,
//!   previously written but never surfaced anywhere.
//! * **Sync reports** — the recent `sync_runs` rows (kind, state,
//!   duration, records, errors) so a run's outcome is visible without
//!   reading logs.

use eframe::egui;
use spp_core::db::DbHandle;

use crate::design::{self, BadgeTone, ButtonVariant};
use crate::pages::{ConfirmRequest, NavRequest};

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

/// One audit-trail row as the Diagnostics page shows it (clean-audit
/// item 68 — the `audit_log` record, previously never surfaced).
#[derive(Debug, Clone)]
struct AuditRow {
    id: i64,
    timestamp: String,
    actor: String,
    action: String,
    conversation_id: Option<i64>,
    before_state: Option<String>,
    after_state: Option<String>,
    remote_operation: Option<String>,
    remote_result: Option<String>,
    ai_involvement: bool,
}

impl AuditRow {
    /// Shape a `list_audit` JSON row for display (payloads render
    /// compactly; non-JSON payloads arrive as JSON strings and show
    /// verbatim).
    fn from_json(v: &serde_json::Value) -> Self {
        let compact = |key: &str| -> Option<String> {
            v.get(key).and_then(|p| match p {
                serde_json::Value::Null => None,
                other => Some(other.to_string()),
            })
        };
        Self {
            id: v.get("id").and_then(serde_json::Value::as_i64).unwrap_or(0),
            timestamp: v
                .get("timestamp")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            actor: v
                .get("actor")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            action: v
                .get("action")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            conversation_id: v.get("conversation_id").and_then(serde_json::Value::as_i64),
            before_state: compact("before_state"),
            after_state: compact("after_state"),
            remote_operation: v
                .get("remote_operation")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            remote_result: compact("remote_result"),
            ai_involvement: v
                .get("ai_involvement")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        }
    }
}

/// The interactive Diagnostics page state.
pub struct DiagnosticsPage {
    events: Option<Vec<spp_core::audit::DiagEvent>>,
    runs: Option<Vec<SyncRunRow>>,
    audit: Option<Vec<AuditRow>>,
    filter: LevelFilter,
    message: Option<String>,
    /// Clean-audit item 132: a failed clear reported "event log cleared
    /// (0 rows removed)" — the error surfaces here now.
    error: Option<String>,
}

impl DiagnosticsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: None,
            runs: None,
            audit: None,
            filter: LevelFilter::All,
            message: None,
            error: None,
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.events = None;
        self.runs = None;
        self.audit = None;
    }

    fn reload(&mut self, db: &DbHandle) {
        let filter = self.filter;
        let events = db.with_read(|conn| {
            spp_core::audit::list_recent_events(conn, 200, filter.level()).unwrap_or_default()
        });
        let runs = db.with_read(load_sync_runs);
        // Clean-audit item 68: the audit trail (`audit_log`) surfaces
        // here — written all along by the outbound lane, the people
        // store, the copilot and the side-thread writers, but never
        // readable anywhere before.
        let audit = db.with_read(|conn| {
            spp_core::audit::list_audit(conn, None, 50)
                .unwrap_or_default()
                .iter()
                .map(AuditRow::from_json)
                .collect::<Vec<_>>()
        });
        self.events = Some(events);
        self.runs = Some(runs);
        self.audit = Some(audit);
    }

    /// Delete every stored diagnostics event. Runs on shell confirmation
    /// only (clean-audit item 140); clean-audit item 132 pins the writer
    /// lane and the surfaced error. Returns the core outcome for the
    /// shell's toast.
    pub fn clear_log(&mut self, db: &DbHandle) -> spp_core::Result<()> {
        // Clean-audit item 132: the DELETE ran on the reader lane (with
        // its pooled read-only connections) and `unwrap_or(0)` swallowed
        // every failure as a success with 0 rows. The write belongs on
        // the writer lane; the error surfaces instead of faking success.
        let result = db.with_write(|conn| spp_core::audit::clear_events(conn));
        match &result {
            Ok(cleared) => {
                self.error = None;
                self.message = Some(format!("event log cleared ({cleared} rows removed)"));
            }
            Err(e) => {
                self.message = None;
                self.error = Some(format!("could not clear the event log: {e}"));
            }
        }
        self.invalidate();
        result.map(|_| ())
    }

    /// Render the page. Returns a confirm request when the user asked to
    /// clear the event log (clean-audit item 140 — the clear runs only
    /// after the shell dialog confirms).
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        if self.events.is_none() || self.runs.is_none() || self.audit.is_none() {
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
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
            self.error = None;
        }

        let events = self.events.clone().unwrap_or_default();
        let runs = self.runs.clone().unwrap_or_default();
        let audit = self.audit.clone().unwrap_or_default();

        // --- Event log -------------------------------------------------
        let error_count = events.iter().filter(|e| e.level == "error").count();
        let warn_count = events.iter().filter(|e| e.level == "warn").count();
        let subtitle = format!(
            "{} event(s) shown — {} error(s), {} warning(s)",
            events.len(),
            error_count,
            warn_count
        );
        let mut clear_requested = false;
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
                    // Clean-audit item 140: the clear arms the shell dialog
                    // (with the row count it is about to destroy) instead of
                    // running unconfirmed. Deferred out of the card closure —
                    // a `return` here would only exit the closure.
                    clear_requested = true;
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
            // Audit fix (M24): the two Diagnostics ScrollAreas derived the
            // SAME auto id (both live in an unnamed card frame under the
            // same page scope), painting egui's "First use of ScrollArea
            // ID …" debug overlay across the event log. Give each a
            // unique, stable salt.
            egui::ScrollArea::vertical()
                .id_salt("diagnostics-event-log")
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

        // --- Audit trail ------------------------------------------------
        // Clean-audit item 68: the `audit_log` record surfaces — actor,
        // action, conversation, AI involvement, and the before/after
        // states plus remote outcome when the row carries them.
        let audit_subtitle = format!("{} audited change(s) shown — newest first", audit.len());
        design::card_titled(ui, "Audit trail", Some(&audit_subtitle), |ui| {
            if audit.is_empty() {
                design::hint_text(
                    ui,
                    "No audited changes yet. Outbound writes, people edits, \
                         copilot runs and side-thread actions record here.",
                );
                return;
            }
            design::table(
                ui,
                "diagnostics-audit",
                &["when", "actor", "action", "conversation", "ai"],
                24.0,
                |ui| {
                    for row in &audit {
                        ui.monospace(
                            egui::RichText::new(row.timestamp.as_str())
                                .size(12.0)
                                .weak(),
                        );
                        design::badge(ui, &row.actor, BadgeTone::Neutral);
                        ui.label(
                            egui::RichText::new(row.action.as_str())
                                .size(12.5)
                                .color(design::tokens(ui).foreground),
                        );
                        ui.monospace(
                            egui::RichText::new(
                                row.conversation_id
                                    .map_or_else(|| "—".to_string(), |cid| cid.to_string()),
                            )
                            .size(12.5),
                        );
                        let (tone, label) = if row.ai_involvement {
                            (BadgeTone::Info, "ai")
                        } else {
                            (BadgeTone::Neutral, "—")
                        };
                        design::badge(ui, label, tone);
                        ui.end_row();
                    }
                },
            );
            // The recorded states and remote outcomes (compact, muted).
            // Audit fix (M24): unique stable id — see the event-log note.
            egui::ScrollArea::vertical()
                .id_salt("diagnostics-audit-detail")
                .max_height(140.0)
                .auto_shrink([false, false])
                .show_rows(ui, 20.0, audit.len(), |ui, range| {
                    for row in &audit[range] {
                        let has_detail = row.before_state.is_some()
                            || row.after_state.is_some()
                            || row.remote_operation.is_some()
                            || row.remote_result.is_some();
                        if !has_detail {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            ui.monospace(
                                egui::RichText::new(format!("#{}", row.id))
                                    .size(11.0)
                                    .weak(),
                            );
                            if let Some(op) = &row.remote_operation {
                                design::badge(ui, op, BadgeTone::Neutral);
                            }
                            for (label, value) in [
                                ("before", &row.before_state),
                                ("after", &row.after_state),
                                ("remote", &row.remote_result),
                            ] {
                                if let Some(value) = value {
                                    ui.monospace(
                                        egui::RichText::new(format!("{label}: {value}"))
                                            .size(11.0)
                                            .weak(),
                                    );
                                }
                            }
                        });
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
        ui.add_space(10.0);

        // --- Outbound deliveries ---------------------------------------
        // The reply/note/status/assignment dispatch lane: what went out,
        // what is pending, and — most importantly — what failed and why.
        let (out_stats, failed_rows) = db.with_read(|conn| {
            let stats = spp_core::outbound::stats(conn);
            let mut stmt = conn
                .prepare(
                    "SELECT id, kind, updated_at, COALESCE(error, '') FROM outbound_jobs
                      WHERE status = 'failed' ORDER BY id DESC LIMIT 8",
                )
                .ok();
            let rows: Vec<(i64, String, String, String)> = match stmt {
                Some(ref mut s) => s
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
                    .unwrap_or_default(),
                None => Vec::new(),
            };
            (stats, rows)
        });
        let out_subtitle = format!(
            "{} delivered · {} queued · {} sending · {} failed",
            out_stats.confirmed, out_stats.queued, out_stats.sending, out_stats.failed
        );
        design::card_titled(ui, "Outbound deliveries", Some(&out_subtitle), |ui| {
            if failed_rows.is_empty() {
                design::hint_text(
                    ui,
                    "No failed deliveries. Replies, notes, status changes and \
                         assignments are dispatched to Help Scout within seconds.",
                );
            } else {
                design::table(
                    ui,
                    "diagnostics-outbound",
                    &["job", "kind", "at", "error"],
                    22.0,
                    |ui| {
                        for (id, kind, at, error) in &failed_rows {
                            ui.monospace(egui::RichText::new(id.to_string()).size(12.5));
                            ui.monospace(egui::RichText::new(kind.as_str()).size(12.5));
                            ui.monospace(egui::RichText::new(at.as_str()).size(12.0).weak());
                            ui.monospace(egui::RichText::new(error.as_str()).size(11.5).weak());
                            ui.end_row();
                        }
                    },
                );
            }
        });
        // Clean-audit item 140: the armed clear request leaves the page.
        if clear_requested {
            return Some(NavRequest::Confirm(ConfirmRequest::clear_diagnostics_log(
                events.len(),
            )));
        }
        None
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
        let _ = page.clear_log(&db);
        page.reload(&db);
        let events = page.events.clone().expect("events");
        assert!(events.is_empty());
        assert!(page.message.is_some());
        // Clean-audit item 132: the clear runs on the writer lane and a
        // success stays a success — no swallowed-error masquerade.
        assert!(page.error.is_none(), "{:?}", page.error);
        assert!(
            page.message
                .as_deref()
                .is_some_and(|m| m.contains("1 rows removed"))
        );
    }

    /// Clean-audit item 68: the audit trail (`audit_log`) surfaces on the
    /// Diagnostics page — the record was written all along but never had
    /// a reader in the shell.
    #[test]
    fn audit_trail_card_lists_recorded_changes() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            spp_core::audit::audit(
                conn,
                &spp_core::audit::AuditEntry::user("update_status")
                    .with_conversation_id(1)
                    .with_before_state(serde_json::json!({"status": "open"}))
                    .with_after_state(serde_json::json!({"status": "closed"})),
            )
            .expect("audit row");
            // A row with a plain-text (non-JSON) payload — must surface
            // verbatim, not as null (the dead-reader drop).
            conn.execute(
                "INSERT INTO audit_log (timestamp, actor, action, conversation_id,
                                         before_state, after_state, ai_involvement)
                 VALUES (datetime('now'), 'ai', 'draft_inserted', 2,
                         NULL, 'plain text payload', 1)",
                [],
            )
            .expect("plain row");
        });
        let mut page = DiagnosticsPage::new();
        page.reload(&db);
        let audit = page.audit.clone().expect("audit rows");
        assert_eq!(audit.len(), 2, "both audit rows are listed");
        // Newest first: the plain-text row was inserted last.
        assert_eq!(audit[0].actor, "ai");
        assert!(audit[0].ai_involvement, "the ai flag rides the row");
        assert_eq!(
            audit[0].after_state.as_deref(),
            Some("\"plain text payload\""),
            "non-JSON payloads surface verbatim"
        );
        assert_eq!(audit[1].action, "update_status");
        assert_eq!(audit[1].conversation_id, Some(1));
        assert_eq!(
            audit[1].before_state.as_deref(),
            Some("{\"status\":\"open\"}")
        );
    }
}
