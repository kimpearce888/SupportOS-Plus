//! RS-17 — DS-A-02 dashboard goldens (wave A).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/dashboard-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `DashboardPage`: PageHeader with the A-8 sync
//! composite, the scope card's three chip tool-rows, the DS-C-03 statGrid,
//! the per-day chart card, the DS-C-23 bar-row breakdowns, the DS-C-05
//! channel-metrics + mailbox-comparison tables, the AI-badged issue radar
//! and the closing by-channel quirk over a seeded mirror); the side-by-side
//! review against the lab is the G3 evidence pack.
//!
//! Determinism note: the dashboard's real data path bounds every query at
//! `now − days` (`spp_core::business_hours::iso_days_ago`), so the seeds
//! are fixed OFFSETS from the seed moment — day-granular dates with pinned
//! time-of-day — which keeps the metric values, breakdowns and daily point
//! count stable across runs. The one live-clock text is the chart card's
//! range subtitle (`{from} → {to} · n points`), which is HEAD's computed
//! behavior (unchanged per D6; the lab's hint is a static fixture — the
//! noted lab-vs-egui divergence) and drifts only across a UTC midnight.
//! (`spp_shell::testsupport` is `#[cfg(test)]`-gated and not visible to
//! integration tests anyway; the SQL below mirrors it.)
//!
//! The conformed lab dashboard has **no tile drill** (no DS-C-04 metric
//! tiles on this page — banded drill tiles live on the operations
//! console), so the interaction golden exercises the page's real state
//! change instead: a scope-chip click on the Channel tool-row, which
//! re-runs the dashboard's scope queries. The golden locks the drilled
//! state, including the by-channel quirk (the distribution card ignores
//! the channel chip — the labeled data-layer bug).
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_dashboard_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::dashboard::DashboardPage;

/// Harness state: the REAL page + the seeded database it reads.
struct DashboardPageState {
    installed: bool,
    page: DashboardPage,
    db: DbHandle,
}

// ---------------------------------------------------------------------------
// Deterministic fixture (fixed day offsets with pinned times)
// ---------------------------------------------------------------------------

/// `days_ago` days before the seed moment, at a pinned UTC time-of-day.
/// Day-granularity keeps every row inside the default 30-day range no
/// matter when the suite runs, while the pinned clock face makes the
/// minute-valued metrics (first response, resolution) deterministic.
fn stamp(days_ago: i64, time: &str) -> String {
    let date = spp_core::business_hours::iso_days_ago(days_ago)
        .split_once('T')
        .expect("iso timestamp")
        .0
        .to_string();
    format!("{date}T{time}Z")
}

/// Bootstrap an in-memory database (all migrations) and seed the dashboard
/// fixture through the same tables the sync pipeline fills. Two mailboxes
/// (Support/Billing), two agents (Ada/Grace — Tier 1/Tier 2), six
/// conversations across channels/statuses/days (two unassigned, one
/// 7d-stale → backlog), one published reply (first response 31 min), one
/// closed thread (resolution 2880 min), one rating of each kind and two
/// tags spread over the channels (the chip-drill golden flips the by-tag
/// card while the by-channel card stays — the quirk).
fn seeded_page() -> (DashboardPage, DbHandle) {
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
    conn.execute(
        "INSERT INTO teams (id, remote_id, name) VALUES (1, 11, 'Tier 1'), (2, 12, 'Tier 2')",
        [],
    )
    .expect("teams");
    conn.execute(
        "INSERT INTO team_members (team_id, user_id) VALUES (1, 5), (2, 6)",
        [],
    )
    .expect("team members");

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
        None,
    );
    seed_conversation(
        &conn,
        2,
        "Login broken",
        "closed",
        "chat",
        2,
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
        1,
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
        6,
        10,
        &stamp(1, "11:00:00.000"),
        None,
    );

    // Conv 1: the customer message + the one published reply — first
    // response 31 min, `replies sent` 1.
    seed_thread(
        &conn,
        1,
        "customer_message",
        "help export broken",
        &stamp(2, "09:01:00.000"),
    );
    seed_thread(&conn, 1, "reply", "we are on it", &stamp(2, "09:32:00.000"));

    // One rating of each kind, all inside the range.
    seed_rating(&conn, 1, "great", &stamp(1, "12:00:00.000"));
    seed_rating(&conn, 2, "okay", &stamp(1, "12:30:00.000"));
    seed_rating(&conn, 3, "not-good", &stamp(1, "08:30:00.000"));

    // Tags spread over the channels: the chip drill must drop `export-bug`
    // (email-only) while `login` (chat) survives.
    seed_tag(&conn, 1, "export-bug");
    seed_tag(&conn, 2, "login");

    (DashboardPage::new(), DbHandle::new(conn))
}

/// One conversation row (created == updated == `created`; `closed` pins
/// `closed_at`; `assignee_id` 0 seeds the NULL assignee the unassigned /
/// backlog predicates read).
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

/// One rating at a fixed `remote_created_at` (the range-bound column).
fn seed_rating(conn: &rusqlite::Connection, conversation_id: i64, rating: &str, at: &str) {
    conn.execute(
        "INSERT INTO ratings (conversation_id, rating, remote_created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![conversation_id, rating, at],
    )
    .expect("rating");
}

/// A tag attached to a conversation (mirrors testsupport's helper).
fn seed_tag(conn: &rusqlite::Connection, conversation_id: i64, name: &str) {
    let tag_id: i64 = conn
        .query_row(
            "INSERT INTO tags (remote_id, name, slug) VALUES ((SELECT COALESCE(MAX(remote_id), 0) + 1 FROM tags), ?1, ?1) RETURNING id",
            rusqlite::params![name],
            |r| r.get(0),
        )
        .expect("tag insert");
    conn.execute(
        "INSERT INTO conversation_tags (conversation_id, tag_id) VALUES (?1, ?2)",
        rusqlite::params![conversation_id, tag_id],
    )
    .expect("conversation tag");
}

// ---------------------------------------------------------------------------
// The six goldens + the scope-chip drill
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light and lock it as
/// `{name}.png` under `tests/snapshots/`.
fn render_dashboard_page(dark: bool, width: f32, name: &str) {
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
            DashboardPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_dashboard_page_1440_light() {
    render_dashboard_page(false, 1440.0, "r3_dashboard_page_1440_light");
}

#[test]
fn r3_dashboard_page_1024_light() {
    render_dashboard_page(false, 1024.0, "r3_dashboard_page_1024_light");
}

#[test]
fn r3_dashboard_page_800_light() {
    render_dashboard_page(false, 800.0, "r3_dashboard_page_800_light");
}

#[test]
fn r3_dashboard_page_1440_dark() {
    render_dashboard_page(true, 1440.0, "r3_dashboard_page_1440_dark");
}

#[test]
fn r3_dashboard_page_1024_dark() {
    render_dashboard_page(true, 1024.0, "r3_dashboard_page_1024_dark");
}

#[test]
fn r3_dashboard_page_800_dark() {
    render_dashboard_page(true, 800.0, "r3_dashboard_page_800_dark");
}

/// The page's real drill: the Channel tool-row's "Chat (Beacon)" chip sets
/// `scope.channel` and re-runs the dashboard's scope queries (same handler
/// as HEAD). The drill re-filters the by-tag card (`export-bug` — email
/// only — drops out; `login` survives) while the by-channel distribution
/// card deliberately stays (the labeled data-layer quirk: it ignores the
/// channel chip). Snapshot the drilled state.
#[test]
fn r3_dashboard_scope_chip_light() {
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
            DashboardPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    // Both tags are visible over the unfiltered scope.
    assert!(
        h.query_by_label("export-bug").is_some(),
        "the by-tag card must list export-bug before the drill"
    );
    assert!(
        h.query_by_label("login").is_some(),
        "the by-tag card must list login before the drill"
    );
    // Click the chat channel chip through the accessibility tree.
    h.get_by_label("Chat (Beacon)").click();
    h.run_ok();
    h.run_steps(2); // settle the re-queried frame
    // The drill re-ran the scope queries: the email-only tag is gone…
    assert!(
        h.query_by_label("export-bug").is_none(),
        "the chat scope must drop the email-only tag"
    );
    // …the chat tag survives…
    assert!(
        h.query_by_label("login").is_some(),
        "the chat scope must keep the chat tag"
    );
    // …and the by-channel distribution ignores the chip (the labeled
    // quirk): the email row is still on the page. (The label appears more
    // than once in the a11y tree — bar row + mailbox stat — so the
    // multi-match query is the correct probe.)
    assert!(
        h.query_all_by_label("email").next().is_some(),
        "the by-channel card must ignore the channel chip (the labeled quirk)"
    );
    h.snapshot("r3_dashboard_scope_chip_light");
}
