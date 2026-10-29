//! RS-20 — DS diagnostics goldens (wave D).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/diagnostics-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `DiagnosticsPage`: the Event log card with the
//! DS-chip filter row and the pushed-right refresh/clear acts, the lab's
//! evRows (small muted timestamp, the DS-C-06 level dot, the service badge
//! + message), the Audit trail table with the AI-tone actor cells, the
//! Sync reports with the lab's SYNC-map tones and the Outbound deliveries
//! table) over a seeded mirror; the side-by-side review against the lab is
//! the G3 evidence pack.
//!
//! The seeded event timestamps are wall-clock (`datetime('now')` — the
//! core writer), so the goldens must not lock the timestamp strings at
//! pixel level across runs... they DO lock them: the seed writes fixed
//! stamps straight into `application_errors` (the writer caps at insert
//! time, the reader is a plain SELECT), keeping the diff-locked rule.
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_diagnostics_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::diagnostics::DiagnosticsPage;

/// Harness state: the REAL page + the seeded database it reads.
struct DiagnosticsPageState {
    installed: bool,
    page: DiagnosticsPage,
    db: DbHandle,
}

/// Bootstrap an in-memory database and seed the diagnostics fixture: three
/// events (error/warn/info — the two dot tones plus the plain one), two
/// audit rows (one human, one AI-involved with before/after detail), two
/// sync runs (LIVE + ERROR — the SYNC-map tones) and one failed outbound
/// job. Timestamps are fixed strings so the goldens diff-lock.
fn seeded_page() -> (DiagnosticsPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    for (level, service, msg, ctx) in [
        (
            "error",
            "outbound",
            "Outbound job 1194 failed: HTTP 400: type is required",
            Some(r#"{"kind":"create_reply"}"#),
        ),
        (
            "warn",
            "sync",
            "Sync checkpoint took 41s — the mirror is catching up",
            None,
        ),
        (
            "info",
            "listener",
            "Webhook accepted for conversation 501",
            None,
        ),
    ] {
        conn.execute(
            "INSERT INTO application_errors (timestamp, service, message, stack, context, level)
             VALUES ('2026-10-27 05:02:00', ?1, ?2, NULL, ?3, ?4)",
            rusqlite::params![service, msg, ctx, level],
        )
        .expect("event");
    }
    // Direct writes with FIXED timestamps — the audit writer stamps
    // datetime('now'), which would drift between the snapshot write and
    // the clean diff run and break the diff-locked rule.
    conn.execute(
        "INSERT INTO audit_log (timestamp, actor, action, conversation_id, before_state, after_state, ai_involvement)
         VALUES ('2026-10-27 05:01:41', 'ada@example.com', 'status changed', 501,
                 'active', 'pending', 0)",
        [],
    )
    .expect("audit row 1");
    conn.execute(
        "INSERT INTO audit_log (timestamp, actor, action, conversation_id, ai_involvement)
         VALUES ('2026-10-27 05:01:42', 'ai', 'reply drafted', 501, 1)",
        [],
    )
    .expect("audit row 2");
    for (id, kind, state, records, errors) in
        [(1, "initial", "LIVE", 1204, 0), (2, "incremental", "ERROR", 33, 2)]
    {
        conn.execute(
            "INSERT INTO sync_runs (id, kind, state, started_at, finished_at, records_processed, errors)
             VALUES (?1, ?2, ?3, '2026-10-27 04:30:00', '2026-10-27 04:31:12', ?4, ?5)",
            rusqlite::params![id, kind, state, records, errors],
        )
        .expect("sync run");
    }
    conn.execute(
        "INSERT INTO outbound_jobs (kind, status, payload, error, created_at, updated_at)
         VALUES ('create_reply', 'failed', '{}', 'HTTP 400: type is required',
                 '2026-10-27 05:01:41', '2026-10-27 05:02:00')",
        [],
    )
    .expect("outbound job");
    (DiagnosticsPage::new(), DbHandle::new(conn))
}

// ---------------------------------------------------------------------------
// The six goldens + the errors-filter interaction
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light and lock it
/// as `{name}.png` under `tests/snapshots/`.
fn render_diagnostics_page(dark: bool, width: f32, name: &str) {
    let (page, db) = seeded_page();
    let mut h = Harness::builder()
        .with_size(egui::vec2(width, 900.0))
        .with_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        })
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    design::install(ui.ctx());
                    s.installed = true;
                }
                s.page.ui(ui, &s.db);
            },
            DiagnosticsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_diagnostics_page_1440_light() {
    render_diagnostics_page(false, 1440.0, "r3_diagnostics_page_1440_light");
}

#[test]
fn r3_diagnostics_page_1024_light() {
    render_diagnostics_page(false, 1024.0, "r3_diagnostics_page_1024_light");
}

#[test]
fn r3_diagnostics_page_800_light() {
    render_diagnostics_page(false, 800.0, "r3_diagnostics_page_800_light");
}

#[test]
fn r3_diagnostics_page_1440_dark() {
    render_diagnostics_page(true, 1440.0, "r3_diagnostics_page_1440_dark");
}

#[test]
fn r3_diagnostics_page_1024_dark() {
    render_diagnostics_page(true, 1024.0, "r3_diagnostics_page_1024_dark");
}

#[test]
fn r3_diagnostics_page_800_dark() {
    render_diagnostics_page(true, 800.0, "r3_diagnostics_page_800_dark");
}

/// The lab's filter row: switching to the errors chip narrows the event
/// log to the error level (the same invalidate + refetch the user's click
/// runs). Snapshot the filtered state.
#[test]
fn r3_diagnostics_errors_filter_light() {
    let (page, db) = seeded_page();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    design::install(ui.ctx());
                    s.installed = true;
                }
                s.page.ui(ui, &s.db);
            },
            DiagnosticsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("errors").next().expect("the chip").click();
    h.run_ok();
    h.run_steps(2); // settle the refetch frame
    // StaticText labels are excluded from the default query filter, so
    // the content assertion rides the interactive surface: the filtered
    // log keeps its clear act (the snapshot locks the narrowed rows).
    assert!(
        h.get_all_by_label("clear log").next().is_some(),
        "the filtered log keeps its clear act"
    );
    h.snapshot("r3_diagnostics_errors_filter_light");
}
