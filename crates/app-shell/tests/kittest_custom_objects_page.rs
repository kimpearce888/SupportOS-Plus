#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::custom_objects::{FieldInput, LinkInput};
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::custom_objects::CustomObjectsPage;

struct CustomObjectsPageState {
    installed: bool,
    page: CustomObjectsPage,
    db: DbHandle,
}

fn seeded_page() -> (CustomObjectsPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    let (type_id, _) = spp_core::custom_objects::create_type(
        &conn,
        "Warranty Claims",
        Some("Warranty claim records linked from conversations"),
        &[
            FieldInput {
                key: "sku".into(),
                label: "SKU".into(),
                field_type: "text".into(),
                required: false,
                options: None,
            },
            FieldInput {
                key: "qty".into(),
                label: "Qty".into(),
                field_type: "number".into(),
                required: false,
                options: None,
            },
            FieldInput {
                key: "in_stock".into(),
                label: "In stock".into(),
                field_type: "boolean".into(),
                required: false,
                options: None,
            },
            FieldInput {
                key: "unit_cost".into(),
                label: "Unit cost".into(),
                field_type: "number".into(),
                required: false,
                options: None,
            },
        ],
    )
    .expect("type");
    let _rmas = spp_core::custom_objects::create_type(&conn, "Rmas", None, &[]).expect("rmas");
    // AUD-P2-11 (C3): a seeded customer + one real link on the first
    // object so the links row renders with a resolved label.
    conn.execute(
        "INSERT INTO customers (id, remote_id, first_name, last_name, email)
         VALUES (77, 7077, 'Kim', 'Pearce', 'kim@example.com')",
        [],
    )
    .expect("customer");
    let props = |pairs: &[(&str, serde_json::Value)]| {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect::<serde_json::Map<String, serde_json::Value>>()
    };
    let broken_hinge_id = spp_core::custom_objects::create_object(
        &conn,
        type_id,
        "Broken hinge",
        &props(&[
            ("sku", serde_json::json!("A-100")),
            ("qty", serde_json::json!(42)),
            ("in_stock", serde_json::json!(true)),
            ("unit_cost", serde_json::json!(1.5)),
        ]),
        &[],
    )
    .expect("object 1");
    // AUD-P2-11 (C3): one link on the first object — the links row's
    // chip renders the resolved customer label.
    spp_core::custom_objects::add_link(
        &conn,
        broken_hinge_id,
        &LinkInput {
            target_kind: "customer".into(),
            target_local_id: 77,
            note: Some("warranty sender".into()),
        },
    )
    .expect("link seeded");
    spp_core::custom_objects::create_object(
        &conn,
        type_id,
        "Loose panel",
        &props(&[
            ("sku", serde_json::json!("A-114")),
            ("qty", serde_json::json!(3)),
            ("in_stock", serde_json::json!(false)),
        ]),
        &[],
    )
    .expect("object 2");
    (CustomObjectsPage::new(), DbHandle::new(conn))
}

fn render_custom_objects_page(dark: bool, width: f32, name: &str) {
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
            CustomObjectsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Warranty Claims (2)")
        .next()
        .expect("the Warranty Claims type chip")
        .click();
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_custom_objects_page_1440_light() {
    render_custom_objects_page(false, 1440.0, "r3_custom_objects_page_1440_light");
}

#[test]
fn r3_custom_objects_page_1024_light() {
    render_custom_objects_page(false, 1024.0, "r3_custom_objects_page_1024_light");
}

#[test]
fn r3_custom_objects_page_800_light() {
    render_custom_objects_page(false, 800.0, "r3_custom_objects_page_800_light");
}

#[test]
fn r3_custom_objects_page_1440_dark() {
    render_custom_objects_page(true, 1440.0, "r3_custom_objects_page_1440_dark");
}

#[test]
fn r3_custom_objects_page_1024_dark() {
    render_custom_objects_page(true, 1024.0, "r3_custom_objects_page_1024_dark");
}

#[test]
fn r3_custom_objects_page_800_dark() {
    render_custom_objects_page(true, 800.0, "r3_custom_objects_page_800_dark");
}

#[test]
fn r3_custom_objects_new_type_light() {
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
            CustomObjectsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("New Type")
        .next()
        .expect("the New Type act")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Create Type").next().is_some(),
        "the New Type form must show the create type act"
    );
    h.snapshot("r3_custom_objects_new_type_light");
}

// AUD-P2-11 (C3): the links row (removable chip + add-link mini form)
// must be visible on the object cards once a type is selected.
#[test]
fn audp2_custom_objects_links_light() {
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
            CustomObjectsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Warranty Claims (2)")
        .next()
        .expect("the Warranty Claims type chip")
        .click();
    h.run_ok();
    assert!(
        h.get_all_by_label("customer · Kim Pearce (warranty sender)")
            .next()
            .is_some(),
        "the seeded link must render as a removable chip with its resolved label"
    );
    h.snapshot("audp2_custom_objects_links_light");
}
