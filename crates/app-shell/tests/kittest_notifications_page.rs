//! RS-20 — DS notifications goldens (wave D).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/notifications-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `NotificationsPage`: the DS-C-09 tab rail, the
//! header's mark-all-read text act, the notification cards with the real
//! unread state dot, the raw type badge, the relative time (raw UTC in the
//! hover title) and the pushed-right read/unread act, and the lab's quirk
//! receipt) over a seeded mirror; the side-by-side review against the lab
//! is the G3 evidence pack.
//!
//! The goldens seed the fixture rows at whole-day offsets from the run
//! moment, so the relative labels ("Yesterday", "2d ago", "3d ago") are
//! stable across the test window and the diff-locked rule holds without a
//! wall-clock seam; the hour/minute branches are covered by the page's
//! deterministic formatter tests. The interaction golden switches to the
//! Mentions tab (the clean-audit-130 surface — the toggles read the
//! mentions list, not the All window).
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_notifications_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::notifications::NotificationsPage;

/// Harness state: the REAL page + the seeded database it reads.
struct NotificationsPageState {
    installed: bool,
    page: NotificationsPage,
    db: DbHandle,
}

/// Bootstrap an in-memory database and seed the notifications fixture (the
/// lab's six-row shape, `me` = Ada/user 5): two unread rows (an info
/// customer_replied and an error sla_breach — the two badge tones), a read
/// mention, and three read fillers — created_at at whole-day offsets so
/// the relative labels are calendar-stable.
fn seeded_page() -> (NotificationsPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    // `me` = Ada (user 5) — the FK target of target_user_id and the user
    // the mentions list reads.
    conn.execute(
        "INSERT INTO users (id, remote_id, first_name, last_name, email, mention)
         VALUES (5, 505, 'Ada', 'Lovelace', 'ada@example.com', 'ada')",
        [],
    )
    .expect("user");
    spp_core::settings::set_string(&conn, "me_remote_id", "505").expect("me");
    let row = |id: i64, ntype: &str, sev: &str, title: &str, body: &str, read: bool, ago: i64| {
        conn.execute(
            "INSERT INTO notifications (id, type, severity, title, body, target_user_id, dedup_key, created_at, read_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 5, ?6, ?7,
                     CASE WHEN ?8 = 1 THEN ?7 ELSE NULL END)",
            rusqlite::params![
                id,
                ntype,
                sev,
                title,
                body,
                format!("seed-{id}"),
                spp_core::business_hours::iso_days_ago(ago),
                read
            ],
        )
        .expect("notification");
    };
    row(
        1,
        "customer_replied",
        "info",
        "Customer replied on #501 Export to CSV fails on large files",
        "Is there an ETA on the fix?",
        false,
        0,
    );
    row(
        2,
        "sla_breach",
        "error",
        "SLA breached on #505 Refund for duplicate charge",
        "",
        false,
        0,
    );
    row(
        3,
        "mentioned",
        "info",
        "Ada mentioned you on #490 Billing address will not save",
        "",
        true,
        1,
    );
    row(
        4,
        "campaign_reply",
        "info",
        "Campaign reply on Spring push",
        "",
        true,
        2,
    );
    row(
        5,
        "sync_failure",
        "error",
        "Sync failed on the inbox mirror",
        "",
        true,
        2,
    );
    row(
        6,
        "mentioned",
        "info",
        "Grace mentioned you on #477 Refund window",
        "",
        true,
        3,
    );
    (NotificationsPage::new(), DbHandle::new(conn))
}

// ---------------------------------------------------------------------------
// The six goldens + the Mentions-tab interaction
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light and lock it
/// as `{name}.png` under `tests/snapshots/`.
fn render_notifications_page(dark: bool, width: f32, name: &str) {
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
            NotificationsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_notifications_page_1440_light() {
    render_notifications_page(false, 1440.0, "r3_notifications_page_1440_light");
}

#[test]
fn r3_notifications_page_1024_light() {
    render_notifications_page(false, 1024.0, "r3_notifications_page_1024_light");
}

#[test]
fn r3_notifications_page_800_light() {
    render_notifications_page(false, 800.0, "r3_notifications_page_800_light");
}

#[test]
fn r3_notifications_page_1440_dark() {
    render_notifications_page(true, 1440.0, "r3_notifications_page_1440_dark");
}

#[test]
fn r3_notifications_page_1024_dark() {
    render_notifications_page(true, 1024.0, "r3_notifications_page_1024_dark");
}

#[test]
fn r3_notifications_page_800_dark() {
    render_notifications_page(true, 800.0, "r3_notifications_page_800_dark");
}

/// The Mentions tab: the clean-audit-130 surface (the tab renders the
/// mentions list, not the All window) with the same card anatomy.
/// Snapshot the tab state.
#[test]
fn r3_notifications_mentions_tab_light() {
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
            NotificationsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Mentions")
        .next()
        .expect("the tab")
        .click();
    h.run_ok();
    h.run_steps(2); // settle the tab frame
    assert!(
        h.get_all_by_label("unread").next().is_some(),
        "the read mention rows must carry their unread act"
    );
    h.snapshot("r3_notifications_mentions_tab_light");
}
