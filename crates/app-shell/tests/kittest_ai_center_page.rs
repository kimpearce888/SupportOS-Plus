#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::ai_center::AiCenterPage;

struct AiCenterPageState {
    installed: bool,
    page: AiCenterPage,
    db: DbHandle,
}

fn seeded_page() -> (AiCenterPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    (AiCenterPage::new(), DbHandle::new(conn))
}

fn render_ai_center_page(dark: bool, width: f32, name: &str) {
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
            AiCenterPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_ai_center_page_1440_light() {
    render_ai_center_page(false, 1440.0, "r3_ai_center_page_1440_light");
}

#[test]
fn r3_ai_center_page_1024_light() {
    render_ai_center_page(false, 1024.0, "r3_ai_center_page_1024_light");
}

#[test]
fn r3_ai_center_page_800_light() {
    render_ai_center_page(false, 800.0, "r3_ai_center_page_800_light");
}

#[test]
fn r3_ai_center_page_1440_dark() {
    render_ai_center_page(true, 1440.0, "r3_ai_center_page_1440_dark");
}

#[test]
fn r3_ai_center_page_1024_dark() {
    render_ai_center_page(true, 1024.0, "r3_ai_center_page_1024_dark");
}

#[test]
fn r3_ai_center_page_800_dark() {
    render_ai_center_page(true, 800.0, "r3_ai_center_page_800_dark");
}

#[test]
fn r3_ai_center_usage_tab_light() {
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
            AiCenterPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Usage")
        .next()
        .expect("the Usage tab")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Run golden set now").next().is_some(),
        "the Usage tab must show the golden-set act"
    );
    h.snapshot("r3_ai_center_usage_tab_light");
}
