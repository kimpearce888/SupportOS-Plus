#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::sync_health::SyncHealthPage;

struct SyncHealthPageState {
    installed: bool,
    page: SyncHealthPage,
    db: DbHandle,
}

fn seeded_page() -> (SyncHealthPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    assert!(
        spp_core::business_hours::pin_now_for_kittest(
            spp_core::business_hours::KITTEST_PINNED_NOW_ISO
        ),
        "the shot clock must pin to the golden anchor"
    );
    spp_core::sync_engine::set_state(&conn, "LIVE");
    for (resource, status, ok, fail, retries, last) in [
        (
            "conversations",
            "ok",
            1204,
            0,
            0,
            Some("2026-10-27 04:31:12"),
        ),
        (
            "customers",
            "retrying",
            210,
            3,
            2,
            Some("2026-10-26 22:10:03"),
        ),
        ("ratings", "failed", 0, 12, 8, None),
    ] {
        conn.execute(
            "INSERT INTO sync_checkpoints (resource, last_success_at, records_processed, records_failed, last_error, retry_count, status)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)",
            rusqlite::params![resource, last, ok, fail, retries, status],
        )
        .expect("checkpoint");
    }
    for (id, kind, state, done, total, records, errors) in [
        (1, "initial", "LIVE", 22, 22, 1204, 0),
        (2, "incremental", "CATCHING_UP", 3, 22, 33, 2),
    ] {
        conn.execute(
            "INSERT INTO sync_runs (id, kind, state, started_at, finished_at, resources_done, resources_total, records_processed, errors)
             VALUES (?1, ?2, ?3, '2026-10-27 04:30:00', '2026-10-27 04:31:12', ?4, ?5, ?6, ?7)",
            rusqlite::params![id, kind, state, done, total, records, errors],
        )
        .expect("run");
    }
    for (n, state) in ["pending", "processed", "processed", "failed", "duplicate"]
        .iter()
        .enumerate()
    {
        conn.execute(
            "INSERT INTO webhook_events (event_id, event_hash, event_type, received_at, payload, processing_state)
             VALUES (?1, ?2, 'conversation.Updated', '2026-10-27 05:00:00', '{}', ?3)",
            rusqlite::params![format!("evt-{n}"), format!("hash-{n}"), state],
        )
        .expect("webhook event");
    }
    (SyncHealthPage::new(), DbHandle::new(conn))
}

fn render_sync_health_page(dark: bool, width: f32, name: &str) {
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
                design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db, None));
            },
            SyncHealthPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_sync_health_page_1440_light() {
    render_sync_health_page(false, 1440.0, "r3_sync_health_page_1440_light");
}

#[test]
fn r3_sync_health_page_1024_light() {
    render_sync_health_page(false, 1024.0, "r3_sync_health_page_1024_light");
}

#[test]
fn r3_sync_health_page_800_light() {
    render_sync_health_page(false, 800.0, "r3_sync_health_page_800_light");
}

#[test]
fn r3_sync_health_page_1440_dark() {
    render_sync_health_page(true, 1440.0, "r3_sync_health_page_1440_dark");
}

#[test]
fn r3_sync_health_page_1024_dark() {
    render_sync_health_page(true, 1024.0, "r3_sync_health_page_1024_dark");
}

#[test]
fn r3_sync_health_page_800_dark() {
    render_sync_health_page(true, 800.0, "r3_sync_health_page_800_dark");
}

#[test]
fn r3_sync_health_run_action_light() {
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
                design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db, None));
            },
            SyncHealthPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Run initial sync")
        .next()
        .expect("the primary act")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Run incremental sync").next().is_some(),
        "the action toolbar must stay after the click"
    );
    h.snapshot("r3_sync_health_run_action_light");
}
