//! RS-18 — DS reports-page goldens (wave B).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/reports-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `ReportsPage`: the PageHeader with the range
//! chips + sync composite, the DS-C-09 grouped tab rail (the lab's three
//! groups over the same eleven labels), the Overview statGrid + per-day
//! chart card, and the lab's block stacks behind the other tabs) over a
//! seeded mirror; the side-by-side review against the lab is the G3
//! evidence pack.
//!
//! Determinism note: the reports page bounds every query at
//! `now − days` (`spp_core::business_hours::iso_days_ago`), so the seeds
//! are fixed OFFSETS from the seed moment — day-granular dates with pinned
//! time-of-day — which keeps the metric values and the daily point count
//! stable across runs. The chart card's range subtitle drifts only across
//! a UTC midnight (HEAD's computed behavior, unchanged per D6).
//!
//! The interaction golden exercises the DS-C-09 rail's real state change:
//! a `Builder` click lands on the report-builder surface (definition form
//! with the widget_info'd metric/dimension combos, the saved-report row
//! with its outline load + danger delete, and the run hint).
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_reports_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::reports::ReportsPage;

/// Harness state: the REAL page + the seeded database it reads.
struct ReportsPageState {
    installed: bool,
    page: ReportsPage,
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

/// Bootstrap an in-memory database (all migrations) and seed the reports
/// fixture: two mailboxes, two customers, six conversations across
/// channels/statuses/days, published customer/reply threads (deterministic
/// first-response and resolution minutes under a pinned 24/7 business-hours
/// window), one rating of each kind with comments, friction scores, one
/// saved report row and one graph node — the rows the page's reload reads
/// on first paint.
fn seeded_page() -> (ReportsPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");

    conn.execute(
        "INSERT INTO users (id, remote_id, first_name, last_name, email, mention)
         VALUES (5, 505, 'Ada', 'Lovelace', 'ada@example.com', 'ada')",
        [],
    )
    .expect("users");
    conn.execute(
        "INSERT INTO mailboxes (id, remote_id, name, email)
         VALUES (1, 101, 'Support', 'support@example.com'),
                (2, 102, 'Billing', 'billing@example.com')",
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
        "closed",
        "email",
        1,
        5,
        9,
        &stamp(2, "09:00:00.000"),
        Some(&stamp(2, "11:00:00.000")),
    );
    seed_conversation(
        &conn,
        2,
        "Login broken",
        "closed",
        "chat",
        1,
        5,
        10,
        &stamp(3, "10:00:00.000"),
        Some(&stamp(1, "10:00:00.000")),
    );
    seed_conversation(
        &conn,
        3,
        "Billing question",
        "pending",
        "email",
        2,
        0,
        9,
        &stamp(1, "08:00:00.000"),
        None,
    );
    seed_conversation(
        &conn,
        4,
        "Old one",
        "closed",
        "email",
        1,
        5,
        9,
        &stamp(10, "09:00:00.000"),
        Some(&stamp(9, "09:00:00.000")),
    );
    seed_conversation(
        &conn,
        5,
        "Stale chat",
        "active",
        "chat",
        2,
        0,
        10,
        &stamp(10, "09:00:00.000"),
        None,
    );
    seed_conversation(
        &conn,
        6,
        "API question",
        "active",
        "email",
        1,
        5,
        10,
        &stamp(1, "11:00:00.000"),
        None,
    );

    // Customer messages + replies: the SLA per-mailbox first-response rows
    // (the DS-C-23 bar list) and the QA/deterministic surfaces read these.
    seed_thread(
        &conn,
        1,
        "customer_message",
        "help export broken",
        &stamp(2, "09:01:00.000"),
    );
    seed_thread(&conn, 1, "reply", "we are on it", &stamp(2, "09:32:00.000"));
    seed_thread(
        &conn,
        2,
        "customer_message",
        "cannot log in",
        &stamp(3, "10:05:00.000"),
    );
    seed_thread(&conn, 2, "reply", "reset sent", &stamp(3, "10:50:00.000"));
    seed_thread(
        &conn,
        3,
        "customer_message",
        "invoice copy please",
        &stamp(1, "08:10:00.000"),
    );

    // One rating of each kind, all inside the range, with comments (the
    // Happiness table renders the Great link and the Not-good badge).
    seed_rating(
        &conn,
        1,
        "great",
        "fast and precise",
        &stamp(1, "12:00:00.000"),
    );
    seed_rating(&conn, 2, "okay", "did the job", &stamp(1, "12:30:00.000"));
    seed_rating(
        &conn,
        3,
        "not-good",
        "still stuck",
        &stamp(1, "08:30:00.000"),
    );

    // Friction scores (the Insights effort table + the health-facts count).
    conn.execute(
        "INSERT INTO friction_scores (conversation_id, effort_score, factors_json, created_at)
         VALUES (2, 7.5, '{\"message_count\":12,\"resolution_hours\":48.0,\"reopen_count\":1,\"escalated\":1}', ?1),
                (4, 2.0, '{\"message_count\":4,\"resolution_hours\":24.0,\"reopen_count\":0,\"escalated\":0}', ?1)",
        rusqlite::params![stamp(1, "13:00:00.000")],
    )
    .expect("friction scores");

    // One saved report (the Builder tab's saved row: outline load + danger
    // delete with the shell confirm arm) and one graph node (the Insights
    // node table's inspect act).
    conn.execute(
        "INSERT INTO report_definitions (name, config, created_at, updated_at)
         VALUES ('Weekly volume', '{}', ?1, ?1)",
        rusqlite::params![stamp(1, "14:00:00.000")],
    )
    .expect("saved report");
    conn.execute(
        "INSERT INTO graph_nodes (kind, entity_id, label, properties_json, created_at)
         VALUES ('customer', 9, 'Kim Pearce', NULL, ?1)",
        rusqlite::params![stamp(1, "15:00:00.000")],
    )
    .expect("graph node");

    (ReportsPage::new(), DbHandle::new(conn))
}

/// One conversation row (created == updated == `created`; `closed` pins
/// `closed_at`; `assignee_id` 0 seeds the NULL assignee).
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
    closed: Option<&str>,
) {
    conn.execute(
        "INSERT INTO conversations (id, remote_id, number, subject, preview, status, type,
             mailbox_local_id, assignee_local_id, customer_local_id, priority,
             created_at, updated_at, closed_at, first_customer_message_at, local_created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                 CASE WHEN ?9 = 0 THEN NULL ELSE ?9 END,
                 ?10, 'none', ?11, ?11, ?12, ?11, ?11)",
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
            closed,
        ],
    )
    .expect("conversation");
}

/// One published thread entry (the shape the replies-sent and first-response
/// queries read).
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

/// One rating with its comment at a fixed `remote_created_at`.
fn seed_rating(
    conn: &rusqlite::Connection,
    conversation_id: i64,
    rating: &str,
    comments: &str,
    at: &str,
) {
    conn.execute(
        "INSERT INTO ratings (conversation_id, rating, comments, remote_created_at)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![conversation_id, rating, comments, at],
    )
    .expect("rating");
}

// ---------------------------------------------------------------------------
// The six goldens + the Builder-tab interaction
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light and lock it as
/// `{name}.png` under `tests/snapshots/`.
fn render_reports_page(dark: bool, width: f32, name: &str) {
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
            ReportsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_reports_page_1440_light() {
    render_reports_page(false, 1440.0, "r3_reports_page_1440_light");
}

#[test]
fn r3_reports_page_1024_light() {
    render_reports_page(false, 1024.0, "r3_reports_page_1024_light");
}

#[test]
fn r3_reports_page_800_light() {
    render_reports_page(false, 800.0, "r3_reports_page_800_light");
}

#[test]
fn r3_reports_page_1440_dark() {
    render_reports_page(true, 1440.0, "r3_reports_page_1440_dark");
}

#[test]
fn r3_reports_page_1024_dark() {
    render_reports_page(true, 1024.0, "r3_reports_page_1024_dark");
}

#[test]
fn r3_reports_page_800_dark() {
    render_reports_page(true, 800.0, "r3_reports_page_800_dark");
}

/// The DS-C-09 rail interaction: clicking the `Builder` tab through the
/// a11y tree lands on the report-builder surface — the definition form
/// (with its widget_info'd metric/dimension combos), the saved-report row
/// (outline load + danger delete) and the run hint. Snapshot the tab.
#[test]
fn r3_reports_builder_tab_light() {
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
            ReportsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Builder")
        .next()
        .expect("the Builder tab")
        .click();
    h.run_ok();
    h.run_steps(2); // settle the tab frame
    // The tab switched: the builder's primary act is in the tree, and the
    // a11y check (still ON) validates the form's controls.
    assert!(
        h.query_by_label("Run report").is_some(),
        "the Builder tab must show the report definition's Run report act"
    );
    // Registered tolerance (RS-4-9, G4-rejection fix wave): the builder
    // tab's date-range inputs default to the rolling 30-day window from
    // the wall clock, so the two date glyphs drift at every midnight
    // (~85 px; first observed 2026-10-30, suite byte-exact on 10-29).
    // Same pre-existing dashboard-class determinism defect, same
    // registered last-resort budget; the proper clock seam is RS-4-11.
    h.snapshot_options(
        "r3_reports_builder_tab_light",
        &egui_kittest::SnapshotOptions::new().max_failed_pixels(120),
    );
}
