#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::support_graph::SupportGraphPage;

struct SupportGraphPageState {
    installed: bool,
    page: SupportGraphPage,
    db: DbHandle,
}

fn seeded_page() -> (SupportGraphPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    conn.execute(
        "INSERT INTO users (id, remote_id, first_name, last_name, email, mention)
         VALUES (5, 505, 'Ada', 'Lovelace', 'ada@example.com', 'ada')",
        [],
    )
    .expect("user");
    conn.execute(
        "INSERT INTO mailboxes (id, remote_id, name, email)
         VALUES (1, 101, 'Support', 'support@example.com')",
        [],
    )
    .expect("mailbox");
    conn.execute(
        "INSERT INTO customers (id, remote_id, first_name, last_name, email)
         VALUES (9, 909, 'Kim', 'Pearce', 'kim@example.com')",
        [],
    )
    .expect("customer");
    conn.execute(
        "INSERT INTO conversations (id, remote_id, number, subject, preview, status, type,
             mailbox_local_id, assignee_local_id, customer_local_id, priority,
             created_at, updated_at, closed_at, first_customer_message_at, local_created_at)
         VALUES (1, 1001, 501, 'Export broken', 'the export button does nothing', 'active', 'email',
                 1, 5, 9, 'none', '2026-01-01T09:00:00Z', '2026-01-01T09:00:00Z', NULL,
                 '2026-01-01T09:00:00Z', '2026-01-01T09:00:00Z')",
        [],
    )
    .expect("conversation");
    let mut page = SupportGraphPage::new();
    let db = DbHandle::new(conn);
    page.kittest_seed_search(&db, "Kim");
    (page, db)
}

fn render_support_graph_page(dark: bool, width: f32, name: &str) {
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
            SupportGraphPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("graph node customer: Kim Pearce")
        .next()
        .expect("the customer result row")
        .click();
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_support_graph_page_1440_light() {
    render_support_graph_page(false, 1440.0, "r3_support_graph_page_1440_light");
}

#[test]
fn r3_support_graph_page_1024_light() {
    render_support_graph_page(false, 1024.0, "r3_support_graph_page_1024_light");
}

#[test]
fn r3_support_graph_page_800_light() {
    render_support_graph_page(false, 800.0, "r3_support_graph_page_800_light");
}

#[test]
fn r3_support_graph_page_1440_dark() {
    render_support_graph_page(true, 1440.0, "r3_support_graph_page_1440_dark");
}

#[test]
fn r3_support_graph_page_1024_dark() {
    render_support_graph_page(true, 1024.0, "r3_support_graph_page_1024_dark");
}

#[test]
fn r3_support_graph_page_800_dark() {
    render_support_graph_page(true, 800.0, "r3_support_graph_page_800_dark");
}

#[test]
fn r3_support_graph_neighborhood_light() {
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
            SupportGraphPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("graph node customer: Kim Pearce")
        .next()
        .expect("the customer result row")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Neighborhood").next().is_some(),
        "the Neighborhood card must render after the pick"
    );
    h.snapshot("r3_support_graph_neighborhood_light");
}
