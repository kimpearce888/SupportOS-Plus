#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::reports::ReportsPage;

use chrono::TimeZone;

struct ReportsPageState {
    installed: bool,
    page: ReportsPage,
    db: DbHandle,
}

fn anchor() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc
        .with_ymd_and_hms(2026, 1, 15, 12, 0, 0)
        .single()
        .expect("fixed anchor")
}

fn stamp(days_ago: i64, time: &str) -> String {
    let date = spp_core::business_hours::iso_days_ago_at(days_ago, anchor())
        .split_once('T')
        .expect("iso timestamp")
        .0
        .to_string();
    format!("{date}T{time}Z")
}

fn seeded_page() -> (ReportsPage, DbHandle) {
    // UXG-1-9: the SLA tab renders wall-clock-dependent stats ("waiting
    // oldest (min)" ticks every real minute — the first SLA-tab golden
    // drifted +31 minutes between the update and diff runs). Pin the
    // kittest clock (the UXB-T1 mechanism) to the fixture's own anchor so
    // every now-read in the page — SLA waiting stats included — is
    // deterministic. The default-tab goldens derive their ranges from
    // `pin_today(anchor())` and don't read the clock, so they hold.
    assert!(
        spp_core::business_hours::pin_now_for_kittest(
            &spp_core::business_hours::now_iso_millis_at(anchor())
        ),
        "the kittest clock pin must take (first pin wins)"
    );
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

    conn.execute(
        "INSERT INTO friction_scores (conversation_id, effort_score, factors_json, created_at)
         VALUES (2, 7.5, '{\"message_count\":12,\"resolution_hours\":48.0,\"reopen_count\":1,\"escalated\":1}', ?1),
                (4, 2.0, '{\"message_count\":4,\"resolution_hours\":24.0,\"reopen_count\":0,\"escalated\":0}', ?1)",
        rusqlite::params![stamp(1, "13:00:00.000")],
    )
    .expect("friction scores");

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

    let mut page = ReportsPage::new();
    page.pin_today(anchor());
    (page, DbHandle::new(conn))
}

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
                // VF-4 (V1 completion): render through the shared
                // production host — the exact funnel app/mod.rs uses.
                design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db, None));
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

/// UXG-1-9: the 4K (3840 logical px) tier through the same
/// `content_column` + margin frame the shell's page funnel wraps every
/// standard page in — the swept data tables keep professional column
/// widths inside the centered 1800px column instead of stretching
/// seven-column lists across a 4K window. The optional tab opens a
/// reports tab so the swept table registers (SLA's seven-column list,
/// the Insights friction/graph tables) are locked, not just the default
/// Overview.
fn render_reports_page_capped(dark: bool, width: f32, tab: Option<&str>, name: &str) {
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
                // The same funnel the shell renders every standard page
                // in: full-width CentralPanel background → content_column
                // cap → margin frame.
                // VF-4 (V1 completion): the capped golden renders through
                // the shared production host (scroll + column + SP5/14) —
                // the old inline copy drifted with a literal 22 margin.
                design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db, None));
            },
            ReportsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    if let Some(tab) = tab {
        h.get_by_label(tab).click();
        h.run_ok();
        h.run_steps(2);
    }
    h.snapshot(name);
}

#[test]
fn uxg_reports_capped_3840_light() {
    render_reports_page_capped(false, 3840.0, Some("SLA"), "uxg_reports_capped_3840_light");
}

#[test]
fn uxg_reports_capped_2560_dark() {
    render_reports_page_capped(true, 2560.0, None, "uxg_reports_capped_2560_dark");
}

/// UXG-1-3: the swept SLA list at a standard width — the exact-column
/// data_table register (header strip, integer columns, span-the-card)
/// replaces the old natural-width table on the SLA tab.
#[test]
fn uxg_reports_sla_tab_1440_light() {
    render_reports_page_capped(false, 1440.0, Some("SLA"), "uxg_reports_sla_tab_1440_light");
}

/// UXG-1-3: the swept Insights tables (friction + graph nodes) at a
/// standard width.
#[test]
fn uxg_reports_insights_tab_1440_light() {
    render_reports_page_capped(
        false,
        1440.0,
        Some("Insights"),
        "uxg_reports_insights_tab_1440_light",
    );
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
                // VF-4 (V1 completion): render through the shared
                // production host — the exact funnel app/mod.rs uses.
                design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db, None));
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
    h.run_steps(2);
    assert!(
        h.query_by_label("Run report").is_some(),
        "the Builder tab must show the report definition's Run report act"
    );
    h.snapshot("r3_reports_builder_tab_light");
}
