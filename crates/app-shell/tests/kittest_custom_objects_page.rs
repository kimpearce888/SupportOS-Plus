//! RS-19 — DS custom-objects goldens (wave C).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/custom-objects-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `CustomObjectsPage`: the PageHeader with the
//! lab's counter pill beside the New Type primary, the Types rail with the
//! confirm-armed deletes, and the selected type's object pane with the
//! lab's section-title + counter row, the FilterBar search, the kv-row
//! object cards and the New object form) over a seeded mirror; the
//! side-by-side review against the lab is the G3 evidence pack.
//!
//! The page opens on its unselected state (HEAD behavior — the right pane
//! populates when the user picks a type), so the goldens click the first
//! type chip through the a11y tree before locking the populated pane,
//! exactly what a user's first visit does (the customers precedent).
//! The interaction golden opens the New Type builder (the lab's header
//! act) and locks the form card with its chips, inputs and create act.
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_custom_objects_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::custom_objects::FieldInput;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::custom_objects::CustomObjectsPage;

/// Harness state: the REAL page + the seeded database it reads.
struct CustomObjectsPageState {
    installed: bool,
    page: CustomObjectsPage,
    db: DbHandle,
}

/// Bootstrap an in-memory database and seed the custom-objects fixture:
/// the conformed lab fixture (`mockups/design-lab/data.js` →
/// `PAGES["custom-objects"]`) — a "Warranty Claims" type (sku text,
/// qty number, in_stock boolean, unit_cost number) holding two objects
/// whose properties mix strings with real JSON scalars, plus an empty
/// "Rmas" type so the rail shows a zero-count chip.
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
    let props = |pairs: &[(&str, serde_json::Value)]| {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect::<serde_json::Map<String, serde_json::Value>>()
    };
    spp_core::custom_objects::create_object(
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

// ---------------------------------------------------------------------------
// The six goldens + the New Type interaction
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light, pick the
/// first type (the user's first act; the lab's `sel: 0` state) and lock
/// the populated page as `{name}.png` under `tests/snapshots/`.
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

/// The lab's header act: clicking `New Type` opens the type-builder card
/// with its name/description inputs, the per-field key/label/type-chip
/// rows and the create act. Snapshot the form state.
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
    h.run_steps(2); // settle the form frame
    // The form opened: the create act is in the tree, and the a11y check
    // (still ON) validates the form's controls.
    assert!(
        h.get_all_by_label("create type").next().is_some(),
        "the New Type form must show the create type act"
    );
    h.snapshot("r3_custom_objects_new_type_light");
}
