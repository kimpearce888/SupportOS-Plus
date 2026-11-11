#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::SnapshotOptions;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::settings::SettingsPage;

struct SettingsPageState {
    installed: bool,
    page: SettingsPage,
    db: DbHandle,
}

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
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Save Credentials").next().is_some(),
        "the credentials form must carry its save act"
    );
    h.snapshot("r3_settings_helpscout_tab_light");
}

/// AUD-P1-8 (D2): the Vector index tab carries the user-facing semantic
/// re-index act — the reset the audit found missing everywhere (5 failed
/// attempts used to exclude a chunk permanently, with no way back).
#[test]
fn audp1_settings_vector_reindex_light() {
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
    h.get_all_by_label("Vector index")
        .next()
        .expect("the Vector index tab")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Rebuild vector index").next().is_some(),
        "the re-index act must be on the Vector index tab"
    );
    h.snapshot_options(
        "audp1_settings_vector_reindex_light",
        &SnapshotOptions::new().max_failed_pixels(32),
    );
}

/// AUD-P2-3-3 (D8): the Data retention card — the retention horizon's
/// first Settings surface. With the horizon armed (30 days) the card
/// carries the standing irreversible-deletion warning; at the default 0
/// it sits quiet. This golden locks the disclosure shape.
#[test]
fn audp2_3_settings_retention_horizon_light() {
    let (page, db) = seeded_page();
    // Arm the horizon BEFORE the page loads its GeneralSettings snapshot.
    db.with_write(|conn| spp_core::settings::set_i64(conn, "retention_days", 30))
        .expect("arm retention");
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
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Data retention").next().is_some(),
        "the Data retention card must live on the Sync & AI tab"
    );
    assert!(
        h.get_all_by_label("Retention horizon (days, 0 = keep everything)")
            .next()
            .is_some(),
        "the horizon control must be present"
    );
    h.snapshot_options(
        "audp2_3_settings_retention_horizon_light",
        &SnapshotOptions::new().max_failed_pixels(32),
    );
}
