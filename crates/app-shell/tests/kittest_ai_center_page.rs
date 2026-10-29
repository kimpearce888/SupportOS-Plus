//! RS-19 — DS ai-center goldens (wave C).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/ai-center-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `AiCenterPage`: the DS-C-09 underline tab rail,
//! the Health tab's LM Studio settings / provider record / endpoint hub /
//! model-role routing cards with the lab's routing-name fix + quirk
//! receipt, the features grid + the kill switch, and the cost ledger) over
//! a fresh mirror the page seeds itself; the side-by-side review against
//! the lab is the G3 evidence pack.
//!
//! Determinism note: the fixture is a fresh in-memory database — the page
//! seeds its own LM Studio defaults, and no wall-clock text renders on the
//! Health tab (the cost ledger's day buckets are date-granular from seeded
//! runs; with none, the table is empty by design).
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_ai_center_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::ai_center::AiCenterPage;

/// Harness state: the REAL page + the seeded database it reads.
struct AiCenterPageState {
    installed: bool,
    page: AiCenterPage,
    db: DbHandle,
}

/// Fresh in-memory database (all migrations) — the page seeds its own
/// LM Studio defaults on first load.
fn seeded_page() -> (AiCenterPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    (AiCenterPage::new(), DbHandle::new(conn))
}

// ---------------------------------------------------------------------------
// The six goldens + the Usage-tab interaction
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light and lock it as
/// `{name}.png` under `tests/snapshots/`.
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

/// The DS-C-09 rail interaction: clicking the `Usage` tab through the a11y
/// tree lands on the per-feature usage surface with its golden-set eval
/// card. Snapshot the tab.
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
    h.run_steps(2); // settle the tab frame
    // The tab switched: the golden-set act is in the tree, and the a11y
    // check (still ON) validates the tab's controls.
    assert!(
        h.get_all_by_label("Run golden set now").next().is_some(),
        "the Usage tab must show the golden-set act"
    );
    h.snapshot("r3_ai_center_usage_tab_light");
}
