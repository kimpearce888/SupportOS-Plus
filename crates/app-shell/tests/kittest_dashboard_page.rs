#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::dashboard::DashboardPage;

use chrono::TimeZone;

struct DashboardPageState {
    installed: bool,
    page: DashboardPage,
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

    seed_thread(
        &conn,
        1,
        "customer_message",
        "help export broken",
        &stamp(2, "09:01:00.000"),
    );
    seed_thread(&conn, 1, "reply", "we are on it", &stamp(2, "09:32:00.000"));

    seed_rating(&conn, 1, "great", &stamp(1, "12:00:00.000"));
    seed_rating(&conn, 2, "okay", &stamp(1, "12:30:00.000"));
    seed_rating(&conn, 3, "not-good", &stamp(1, "08:30:00.000"));

    seed_tag(&conn, 1, "export-bug");
    seed_tag(&conn, 2, "login");

    let mut page = DashboardPage::new();
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

fn seed_rating(conn: &rusqlite::Connection, conversation_id: i64, rating: &str, at: &str) {
    conn.execute(
        "INSERT INTO ratings (conversation_id, rating, remote_created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![conversation_id, rating, at],
    )
    .expect("rating");
}

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
    assert!(
        h.query_by_label("export-bug").is_some(),
        "the by-tag card must list export-bug before the drill"
    );
    assert!(
        h.query_by_label("login").is_some(),
        "the by-tag card must list login before the drill"
    );
    h.get_by_label("Chat (Beacon)").click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.query_by_label("export-bug").is_none(),
        "the chat scope must drop the email-only tag"
    );
    assert!(
        h.query_by_label("login").is_some(),
        "the chat scope must keep the chat tag"
    );
    assert!(
        h.query_all_by_label("email").next().is_some(),
        "the by-channel card must ignore the channel chip (the labeled quirk)"
    );
    h.snapshot("r3_dashboard_scope_chip_light");
}
