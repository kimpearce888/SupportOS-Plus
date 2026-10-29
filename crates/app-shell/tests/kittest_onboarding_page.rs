//! RS-20 — DS onboarding goldens (wave D).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/onboarding-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `OnboardingPage`: the Setup status card with
//! the lab's DS.row anatomy — state dot + strong key left, mono value
//! right — the Run-initial-sync primary + Mark-complete outline toolbar
//! and the conditional hints) over a seeded mirror; the side-by-side
//! review against the lab is the G3 evidence pack.
//!
//! The goldens lock the mid-onboarding state (a conversation present, no
//! credentials — the honest fresh-install shape, 1.2.0's empty-until-
//! connected rule); the interaction golden clicks "Mark complete" and
//! locks the completed state.
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_onboarding_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::onboarding::OnboardingPage;

/// Harness state: the REAL page + the seeded database it reads.
struct OnboardingPageState {
    installed: bool,
    page: OnboardingPage,
    db: DbHandle,
}

/// Bootstrap an in-memory database and seed the mid-onboarding fixture:
/// one conversation (the conversations dot turns ok), no credentials, no
/// sync activity (the honest fresh-install shape).
fn seeded_page() -> (OnboardingPage, DbHandle) {
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
    (OnboardingPage::new(), DbHandle::new(conn))
}

// ---------------------------------------------------------------------------
// The six goldens + the Mark-complete interaction
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light and lock it
/// as `{name}.png` under `tests/snapshots/`.
fn render_onboarding_page(dark: bool, width: f32, name: &str) {
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
                s.page.ui(ui, &s.db, None);
            },
            OnboardingPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_onboarding_page_1440_light() {
    render_onboarding_page(false, 1440.0, "r3_onboarding_page_1440_light");
}

#[test]
fn r3_onboarding_page_1024_light() {
    render_onboarding_page(false, 1024.0, "r3_onboarding_page_1024_light");
}

#[test]
fn r3_onboarding_page_800_light() {
    render_onboarding_page(false, 800.0, "r3_onboarding_page_800_light");
}

#[test]
fn r3_onboarding_page_1440_dark() {
    render_onboarding_page(true, 1440.0, "r3_onboarding_page_1440_dark");
}

#[test]
fn r3_onboarding_page_1024_dark() {
    render_onboarding_page(true, 1024.0, "r3_onboarding_page_1024_dark");
}

#[test]
fn r3_onboarding_page_800_dark() {
    render_onboarding_page(true, 800.0, "r3_onboarding_page_800_dark");
}

/// The "Mark complete" act: the first-run flag flips, the status card's
/// dot turns ok and the step reads complete. Snapshot the state.
#[test]
fn r3_onboarding_mark_complete_light() {
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
                s.page.ui(ui, &s.db, None);
            },
            OnboardingPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Mark complete")
        .next()
        .expect("the outline act")
        .click();
    h.run_ok();
    h.run_steps(2); // settle the reloaded status frame
    assert!(
        h.get_all_by_label("Run initial sync").next().is_some(),
        "the toolbar must stay after completing"
    );
    h.snapshot("r3_onboarding_mark_complete_light");
}
