//! RS-20 — DS settings goldens (wave D).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/settings-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `SettingsPage`: the DS-C-09 tab rail below the
//! header, the Synchronization form grid (the Drag values + combos with
//! their item-138/item-25 hover notes), the AI behaviour checkbox grid
//! with the lab's safe-defaults sub, the About & updates card and the
//! save act) over a seeded mirror; the side-by-side review against the lab
//! is the G3 evidence pack.
//!
//! The interaction golden switches to the Help Scout tab — the heaviest
//! a11y surface (the credentials form's four inputs bound to their visible
//! labels, the connection card, the doctor) — under the a11y check.
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_settings_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::SnapshotOptions;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::settings::SettingsPage;

/// Harness state: the REAL page + the seeded database it reads.
struct SettingsPageState {
    installed: bool,
    page: SettingsPage,
    db: DbHandle,
}

/// Bootstrap an in-memory database and seed the settings fixture: one
/// mailbox (the business-hours rows join it) + one user; the settings
/// table keeps its defaults (the honest unconfigured state).
fn seeded_page() -> (SettingsPage, DbHandle) {
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
    (SettingsPage::new(), DbHandle::new(conn))
}

// ---------------------------------------------------------------------------
// The six goldens + the Help Scout tab interaction
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light and lock it
/// as `{name}.png` under `tests/snapshots/` (the General tab, the page's
/// default).
fn render_settings_page(dark: bool, width: f32, name: &str) {
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
            SettingsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    // Registered tolerance (RS-21, G4 package): the runner's lavapipe
    // (mesa 26.0.8) anti-aliases a handful of pixels differently from the
    // baseline rasterizer — measured at 28 px out of ~1.3M on the General
    // tab, the page with the only Drag widgets and the densest input grid;
    // every other suite (22/23, 145 tests) is byte-exact on the same
    // runner, and this suite is byte-exact under the baseline (the
    // diff-locked-twice rule). max_failed_pixels is kittest's documented
    // last-resort budget — the guidance is the smallest value that passes
    // and a re-check on every snapshot update; 32 sits just above the
    // observed 28 px and remains far below what any real regression on
    // this page could produce (a moved border, a reflowed row or a
    // re-rendered glyph is hundreds of pixels). Scoped to the page
    // goldens only — the Help-Scout interaction golden keeps the default
    // zero-tolerance lock (it is byte-exact on the runner).
    h.snapshot_options(name, &SnapshotOptions::new().max_failed_pixels(32));
}

#[test]
fn r3_settings_page_1440_light() {
    render_settings_page(false, 1440.0, "r3_settings_page_1440_light");
}

#[test]
fn r3_settings_page_1024_light() {
    render_settings_page(false, 1024.0, "r3_settings_page_1024_light");
}

#[test]
fn r3_settings_page_800_light() {
    render_settings_page(false, 800.0, "r3_settings_page_800_light");
}

#[test]
fn r3_settings_page_1440_dark() {
    render_settings_page(true, 1440.0, "r3_settings_page_1440_dark");
}

#[test]
fn r3_settings_page_1024_dark() {
    render_settings_page(true, 1024.0, "r3_settings_page_1024_dark");
}

#[test]
fn r3_settings_page_800_dark() {
    render_settings_page(true, 800.0, "r3_settings_page_800_dark");
}

/// The Help Scout tab: the loopback-listener hint, the connection card
/// (credentials stored badge + redirect uri), the Connection Doctor, the
/// credentials form (four inputs bound to their visible labels — the a11y
/// check validates the bindings), the webhook relay card and the danger
/// zone. Snapshot the tab state.
#[test]
fn r3_settings_helpscout_tab_light() {
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
            SettingsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Help Scout")
        .next()
        .expect("the tab")
        .click();
    h.run_ok();
    h.run_steps(2); // settle the tab frame
    assert!(
        h.get_all_by_label("save credentials").next().is_some(),
        "the credentials form must carry its save act"
    );
    h.snapshot("r3_settings_helpscout_tab_light");
}
