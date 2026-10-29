//! RS-19 — DS operations-console goldens (wave C).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/operations-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `OperationsPage`: the DS-C-09 underline tab
//! rail, the Tiles tab's severity-banded DS-C-04 metric tiles with the
//! lab's 'open →' drill line, the Workload table with the lab's
//! Load/Pressure label fix + quirk receipt, and the Library's saved-reply
//! CRUD + workflow surface with its companion quirk) over a seeded mirror;
//! the side-by-side review against the lab is the G3 evidence pack.
//!
//! Determinism note: the operations fixture seeds fixed day offsets with
//! pinned times (the snapshot's windows are now-relative), so tile counts
//! are stable across runs.
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_operations_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::operations::OperationsPage;

/// Harness state: the REAL page + the seeded database it reads.
struct OperationsPageState {
    installed: bool,
    page: OperationsPage,
    db: DbHandle,
}

// ---------------------------------------------------------------------------
// Deterministic fixture (fixed day offsets with pinned times)
// ---------------------------------------------------------------------------

/// `days_ago` days before the seed moment, at a pinned UTC time-of-day.
fn stamp(days_ago: i64, time: &str) -> String {
    let date = spp_core::business_hours::iso_days_ago(days_ago)
        .split_once('T')
        .expect("iso timestamp")
        .0
        .to_string();
    format!("{date}T{time}Z")
}

/// Bootstrap an in-memory database (all migrations) and seed the operations
/// fixture: two agents (the workload table's Ada/Grace rows), one mailbox,
/// a spread of conversations across statuses/assignees (the unassigned /
/// urgent / waiting tiles), one saved reply and one manual workflow (the
/// Library's rows).
fn seeded_page() -> (OperationsPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");

    conn.execute(
        "INSERT INTO users (id, remote_id, first_name, last_name, email, mention)
         VALUES (5, 505, 'Ada', 'Lovelace', 'ada@example.com', 'ada'),
                (6, 506, 'Grace', 'Hopper', 'grace@example.com', NULL)",
        [],
    )
    .expect("users");
    conn.execute(
        "INSERT INTO mailboxes (id, remote_id, name, email)
         VALUES (1, 101, 'Support', 'support@example.com')",
        [],
    )
    .expect("mailboxes");
    conn.execute(
        "INSERT INTO customers (id, remote_id, first_name, last_name, email)
         VALUES (9, 909, 'Kim', 'Pearce', 'kim@example.com'),
                (10, 910, 'Ada', 'Customer', 'adac@example.com')",
        [],
    )
    .expect("customers");

    seed_conversation(
        &conn,
        1,
        "Bug in export",
        "active",
        "email",
        1,
        5,
        9,
        &stamp(2, "09:00:00.000"),
    );
    seed_conversation(
        &conn,
        2,
        "Urgent outage",
        "active",
        "chat",
        1,
        6,
        10,
        &stamp(1, "10:00:00.000"),
    );
    // The unassigned tile's count: an active conversation with no assignee.
    seed_conversation(
        &conn,
        3,
        "Orphan request",
        "active",
        "email",
        1,
        0,
        9,
        &stamp(1, "08:00:00.000"),
    );
    seed_conversation(
        &conn,
        4,
        "Old closed one",
        "closed",
        "email",
        1,
        5,
        10,
        &stamp(10, "09:00:00.000"),
    );
    // Customer messages make the waiting/waited tiles read from real rows.
    seed_thread(
        &conn,
        3,
        "customer_message",
        "still no answer",
        &stamp(1, "08:05:00.000"),
    );

    // The Library tab's rows: one saved reply + one manual workflow.
    db_seed_library(&conn);

    (OperationsPage::new(), DbHandle::new(conn))
}

fn db_seed_library(conn: &rusqlite::Connection) {
    conn.execute(
        "INSERT INTO saved_replies (remote_id, mailbox_local_id, name, preview, text)
         VALUES (11, 1, 'Refund path', '…here is how refunds are handled…',
                 '<p>here is how refunds are handled</p>')",
        [],
    )
    .expect("saved reply");
    conn.execute(
        "INSERT INTO workflows (remote_id, mailbox_local_id, name, type, status, sort_order)
         VALUES (21, 1, 'Escalate', 'manual', 'active', 1)",
        [],
    )
    .expect("workflow");
}

/// One conversation row (created == updated == `created`; `assignee_id` 0
/// seeds the NULL assignee the unassigned predicates read).
#[allow(clippy::too_many_arguments)]
fn seed_conversation(
    conn: &rusqlite::Connection,
    id: i64,
    subject: &str,
    status: &str,
    channel: &str,
    mailbox_id: i64,
    assignee_id: i64,
    customer_id: i64,
    created: &str,
) {
    conn.execute(
        "INSERT INTO conversations (id, remote_id, number, subject, preview, status, type,
             mailbox_local_id, assignee_local_id, customer_local_id, priority,
             created_at, updated_at, closed_at, first_customer_message_at, local_created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                 CASE WHEN ?9 = 0 THEN NULL ELSE ?9 END,
                 ?10, 'none', ?11, ?11, NULL, ?11, ?11)",
        rusqlite::params![
            id,
            1000 + id,
            500 + id,
            subject,
            format!("{subject} preview"),
            status,
            channel,
            mailbox_id,
            assignee_id,
            customer_id,
            created,
        ],
    )
    .expect("conversation");
}

/// One published customer thread (the waiting-tile shape).
fn seed_thread(
    conn: &rusqlite::Connection,
    conversation_id: i64,
    kind: &str,
    body: &str,
    at: &str,
) {
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, body_text, from_type, state,
             created_by_customer_id, created_at)
         VALUES (?1, ?2, ?3, 'customer', 'published', 9, ?4)",
        rusqlite::params![conversation_id, kind, body, at],
    )
    .expect("thread");
}

// ---------------------------------------------------------------------------
// The six goldens + the Workload-tab interaction
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light and lock it as
/// `{name}.png` under `tests/snapshots/`.
fn render_operations_page(dark: bool, width: f32, name: &str) {
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
                // Freeze the wall-clock stamps before the paint; the first
                // frame loads the snapshot (the freeze is a no-op), so the
                // harness renders a settle frame before snapshotting.
                s.page.kittest_freeze_generated_at();
                s.page.ui(ui, &s.db);
            },
            OperationsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.run_ok(); // settle: the freeze now covers the loaded snapshot
    h.snapshot(name);
}

#[test]
fn r3_operations_page_1440_light() {
    render_operations_page(false, 1440.0, "r3_operations_page_1440_light");
}

#[test]
fn r3_operations_page_1024_light() {
    render_operations_page(false, 1024.0, "r3_operations_page_1024_light");
}

#[test]
fn r3_operations_page_800_light() {
    render_operations_page(false, 800.0, "r3_operations_page_800_light");
}

#[test]
fn r3_operations_page_1440_dark() {
    render_operations_page(true, 1440.0, "r3_operations_page_1440_dark");
}

#[test]
fn r3_operations_page_1024_dark() {
    render_operations_page(true, 1024.0, "r3_operations_page_1024_dark");
}

#[test]
fn r3_operations_page_800_dark() {
    render_operations_page(true, 800.0, "r3_operations_page_800_dark");
}

/// The DS-C-09 rail interaction: clicking the `Workload` tab through the
/// a11y tree lands on the Team Workload table (with the lab's Load/Pressure
/// label fix and its quirk receipt) and the capacity +/− acts. Snapshot the
/// tab.
#[test]
fn r3_operations_workload_tab_light() {
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
                // Same freeze as the golden renderer (see its comment).
                s.page.kittest_freeze_generated_at();
                s.page.ui(ui, &s.db);
            },
            OperationsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Workload")
        .next()
        .expect("the Workload tab")
        .click();
    h.run_ok(); // the workload tab loads its snapshot here
    h.run_ok(); // settle: the freeze now covers the loaded snapshot
    h.run_steps(2);
    // The tab switched: the capacity increase act is in the tree, and the
    // a11y check (still ON) validates the tab's controls.
    assert!(
        h.get_all_by_label("+").next().is_some(),
        "the Workload tab must show the capacity + act"
    );
    h.snapshot("r3_operations_workload_tab_light");
}
