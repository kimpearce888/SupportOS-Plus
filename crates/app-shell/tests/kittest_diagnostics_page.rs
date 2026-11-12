#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::diagnostics::DiagnosticsPage;

struct DiagnosticsPageState {
    installed: bool,
    page: DiagnosticsPage,
    db: DbHandle,
}

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
    for (id, kind, state, records, errors) in [
        (1, "initial", "LIVE", 1204, 0),
        (2, "incremental", "ERROR", 33, 2),
    ] {
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
                // VF-4 (V1 completion): render through the shared
                // production host — the exact funnel app/mod.rs uses.
                design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db));
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
                // VF-4 (V1 completion): render through the shared
                // production host — the exact funnel app/mod.rs uses.
                design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db));
            },
            DiagnosticsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    // UXG-1-5: the old exact-label query matched the runs table's header
    // cell ("Errors") — a non-interactive label, so the click was a no-op
    // and the golden silently captured the UNfiltered log. The chips carry
    // their counts ("Errors 36"); the contains-query lands on the chip,
    // which actually filters.
    h.get_all_by_label_contains("Errors")
        .next()
        .expect("the Errors filter chip")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Clear Log").next().is_some(),
        "the filtered log keeps its clear act"
    );
    h.snapshot("r3_diagnostics_errors_filter_light");
}
