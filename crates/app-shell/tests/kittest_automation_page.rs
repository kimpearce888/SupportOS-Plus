//! RS-19 — DS automation goldens (wave C).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/automation-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `AutomationPage`: the header with the lab's
//! 'New Rule' primary, the Rules table with the enabled/disabled chips and
//! the confirm-armed deletes, the Pending approvals card with the lab's
//! human-readable-action fix (raw JSON collapsible, quirk receipt) and the
//! outline approve / danger reject pair) over a seeded mirror; the
//! side-by-side review against the lab is the G3 evidence pack.
//!
//! The interaction golden opens the New Rule form (the lab's header act)
//! and locks the form card with its chips, inputs and create act.
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_automation_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::automation::AutomationPage;

/// Harness state: the REAL page + the seeded database it reads.
struct AutomationPageState {
    installed: bool,
    page: AutomationPage,
    db: DbHandle,
}

/// Bootstrap an in-memory database and seed the automation fixture: the
/// acting user (the item-80 decision attribution), two rules (one manual
/// tag action, one approval-gated priority action) and one pending
/// approval whose proposed_action_json carries the legacy tagged-enum
/// shape the page parses for the human-readable badge.
fn seeded_page() -> (AutomationPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    conn.execute(
        "INSERT INTO users (id, remote_id, first_name, last_name, email, mention)
         VALUES (5, 505, 'Ada', 'Lovelace', 'ada@example.com', 'ada')",
        [],
    )
    .expect("users");
    conn.execute(
        "INSERT INTO automation_rules (name, trigger_json, action_json, enabled)
         VALUES ('Close stale solved', '{\"kind\":\"sla_risk\"}',
                 '{\"kind\":\"add_tag\",\"tag\":\"stale\"}', 1),
                ('Escalate urgent replies', '{\"kind\":\"tag_added\",\"tag\":\"urgent\"}',
                 '{\"kind\":\"set_priority\",\"priority\":\"urgent\"}', 1)",
        [],
    )
    .expect("rules");
    let rule_id: i64 = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO automation_approvals (rule_id, conversation_id, proposed_action_json)
         VALUES (?1, 2, '{\"kind\":\"set_priority\",\"priority\":\"urgent\"}')",
        rusqlite::params![rule_id],
    )
    .expect("approval");
    (AutomationPage::new(), DbHandle::new(conn))
}

// ---------------------------------------------------------------------------
// The six goldens + the New Rule interaction
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light and lock it as
/// `{name}.png` under `tests/snapshots/`.
fn render_automation_page(dark: bool, width: f32, name: &str) {
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
            AutomationPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_automation_page_1440_light() {
    render_automation_page(false, 1440.0, "r3_automation_page_1440_light");
}

#[test]
fn r3_automation_page_1024_light() {
    render_automation_page(false, 1024.0, "r3_automation_page_1024_light");
}

#[test]
fn r3_automation_page_800_light() {
    render_automation_page(false, 800.0, "r3_automation_page_800_light");
}

#[test]
fn r3_automation_page_1440_dark() {
    render_automation_page(true, 1440.0, "r3_automation_page_1440_dark");
}

#[test]
fn r3_automation_page_1024_dark() {
    render_automation_page(true, 1024.0, "r3_automation_page_1024_dark");
}

#[test]
fn r3_automation_page_800_dark() {
    render_automation_page(true, 800.0, "r3_automation_page_800_dark");
}

/// The lab's header act: clicking `New Rule` opens the rule form card with
/// its trigger/action chips, inputs and the Create Rule act. Snapshot the
/// form state.
#[test]
fn r3_automation_new_rule_light() {
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
            AutomationPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("New Rule")
        .next()
        .expect("the New Rule act")
        .click();
    h.run_ok();
    h.run_steps(2); // settle the form frame
    // The form opened: the create act is in the tree, and the a11y check
    // (still ON) validates the form's controls.
    assert!(
        h.get_all_by_label("Create Rule").next().is_some(),
        "the New Rule form must show the Create Rule act"
    );
    h.snapshot("r3_automation_new_rule_light");
}
