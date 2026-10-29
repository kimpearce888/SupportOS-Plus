//! RS-20 — DS support-graph goldens (wave D).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/support-graph-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `SupportGraphPage`: the header with the stats
//! subtitle and the lab's nodes counter pill, the search toolbar, the
//! ds-sel-row results (kind + label head line, sublabel beneath, selected
//! fill + 2px primary inset) and the Neighborhood card with the DS.row
//! edges) over a seeded mirror; the side-by-side review against the lab is
//! the G3 evidence pack.
//!
//! The goldens match the lab's fixture state (`sel: 1` — results visible,
//! one picked): the query is prefilled and the results preloaded through
//! the page's real search, then the goldens click the customer row through
//! the a11y tree (the user's first act) before locking the populated page
//! with its Neighborhood card.
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_support_graph_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::support_graph::SupportGraphPage;

/// Harness state: the REAL page + the seeded database it reads.
struct SupportGraphPageState {
    installed: bool,
    page: SupportGraphPage,
    db: DbHandle,
}

/// Bootstrap an in-memory database and seed the graph fixture: Ada (the
/// assignee) + Kim Pearce (customer 9) + one conversation assigned to her
/// — the smallest graph with all three node kinds and one edge. The
/// returned page has the query "Kim" prefilled and the results preloaded
/// through the page's real search (the same path a submitted search runs).
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

// ---------------------------------------------------------------------------
// The six goldens + the neighborhood interaction
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light, pick the
/// customer row (the user's first act; the lab's `sel` state — the
/// Neighborhood card appears) and lock the populated page as `{name}.png`
/// under `tests/snapshots/`.
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

/// The Neighborhood card: the center (customer #9) head line, the hint
/// label and the DS.row edges (relation + far-side label strong left, the
/// origin mono-muted right). Snapshot the neighborhood state isolated.
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
    h.run_steps(2); // settle the neighborhood frame
    assert!(
        h.get_all_by_label("Neighborhood").next().is_some(),
        "the Neighborhood card must render after the pick"
    );
    h.snapshot("r3_support_graph_neighborhood_light");
}
