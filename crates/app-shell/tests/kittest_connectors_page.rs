#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::connectors::ConnectorsPage;

struct ConnectorsPageState {
    installed: bool,
    page: ConnectorsPage,
    db: DbHandle,
}

fn seeded_page() -> (ConnectorsPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    let mk = |name: &str, kind: &str, url: &str, file: &str| {
        let config = serde_json::json!({ "kind": kind, "file": file, "url": url });
        spp_core::connectors::create(
            &conn,
            name,
            kind,
            &config,
            &serde_json::json!({}),
            "manual",
            3600,
            false,
        )
        .expect("connector")
    };
    mk("Billing export", "csv", "", "billing.csv");
    mk("Status page", "http", "https://status.example.test", "");
    mk(
        "Changelog feed",
        "http",
        "https://changelog.example.test",
        "",
    );
    conn.execute(
        "UPDATE connectors SET health = 'ok', last_sync_at = '2026-10-27 05:01:41'
         WHERE name = 'Billing export'",
        [],
    )
    .expect("health");
    conn.execute(
        "UPDATE connectors SET health = 'stale', last_sync_at = '2026-10-25 05:01:44'
         WHERE name = 'Status page'",
        [],
    )
    .expect("health");
    conn.execute(
        "UPDATE connectors SET health = 'error', last_sync_at = NULL WHERE name = 'Changelog feed'",
        [],
    )
    .expect("health");
    for (key, data) in [
        ("A-100", r#"{"sku":"A-100","qty":42}"#),
        ("A-114", r#"{"sku":"A-114","qty":3}"#),
    ] {
        conn.execute(
            "INSERT INTO connector_rows (connector_id, row_key, data, fetched_at)
             VALUES ((SELECT id FROM connectors WHERE name = 'Billing export'), ?1, ?2,
                     '2026-10-27 05:01:41')",
            rusqlite::params![key, data],
        )
        .expect("connector row");
    }
    (ConnectorsPage::new(), DbHandle::new(conn))
}

fn render_connectors_page(dark: bool, width: f32, name: &str) {
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
            ConnectorsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Billing export")
        .next()
        .expect("the Billing export chip")
        .click();
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_connectors_page_1440_light() {
    render_connectors_page(false, 1440.0, "r3_connectors_page_1440_light");
}

#[test]
fn r3_connectors_page_1024_light() {
    render_connectors_page(false, 1024.0, "r3_connectors_page_1024_light");
}

#[test]
fn r3_connectors_page_800_light() {
    render_connectors_page(false, 800.0, "r3_connectors_page_800_light");
}

#[test]
fn r3_connectors_page_1440_dark() {
    render_connectors_page(true, 1440.0, "r3_connectors_page_1440_dark");
}

#[test]
fn r3_connectors_page_1024_dark() {
    render_connectors_page(true, 1024.0, "r3_connectors_page_1024_dark");
}

#[test]
fn r3_connectors_page_800_dark() {
    render_connectors_page(true, 800.0, "r3_connectors_page_800_dark");
}

#[test]
fn r3_connectors_new_connector_light() {
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
            ConnectorsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("New Connector")
        .next()
        .expect("the New Connector act")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Create").next().is_some(),
        "the New Connector form must show the create act"
    );
    h.snapshot("r3_connectors_new_connector_light");
}
