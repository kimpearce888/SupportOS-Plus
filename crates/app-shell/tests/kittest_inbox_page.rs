#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use spp_core::db::DbHandle;
use spp_core::event_bus::EventBus;
use spp_shell::design;
use spp_shell::pages::inbox::InboxPage;

struct InboxPageState {
    installed: bool,
    page: InboxPage,
    db: DbHandle,
    bus: EventBus,
}

fn seeded_page() -> (InboxPage, DbHandle, EventBus) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    seed_users(&conn);
    seed_mailboxes(&conn);
    seed_customers(&conn);
    seed_conversation(
        &conn,
        1,
        "Export broken",
        "active",
        "email",
        1,
        5,
        9,
        "2026-02-16T10:10:00.000Z",
    );
    seed_conversation(
        &conn,
        2,
        "Billing address update",
        "pending",
        "email",
        2,
        6,
        10,
        "2026-02-16T09:52:00.000Z",
    );
    seed_conversation(
        &conn,
        3,
        "Chat: login loop",
        "active",
        "chat",
        1,
        0,
        9,
        "2026-02-16T09:31:00.000Z",
    );
    seed_conversation(
        &conn,
        4,
        "Refund for duplicate charge",
        "active",
        "email",
        2,
        5,
        10,
        "2026-02-16T09:05:00.000Z",
    );
    seed_conversation(
        &conn,
        5,
        "Data export format question",
        "closed",
        "email",
        1,
        6,
        9,
        "2026-02-15T18:44:00.000Z",
    );
    seed_conversation(
        &conn,
        6,
        "Mobile app crashes on start",
        "pending",
        "chat",
        1,
        5,
        9,
        "2026-02-14T08:12:00.000Z",
    );
    for (id, at) in [
        (2i64, "2026-02-16T09:50:00.000Z"),
        (3, "2026-02-16T09:29:00.000Z"),
        (4, "2026-02-16T09:03:00.000Z"),
        (5, "2026-02-15T18:40:00.000Z"),
        (6, "2026-02-14T08:10:00.000Z"),
    ] {
        seed_thread(
            &conn,
            id,
            "customer_message",
            "customer",
            None,
            Some(9),
            "Opening message from the customer.",
            at,
        );
    }
    seed_thread(
        &conn,
        1,
        "customer_message",
        "customer",
        None,
        Some(9),
        "The export button does nothing on the dashboard — I click Export and nothing happens.",
        "2026-02-16T09:41:00.000Z",
    );
    seed_thread(
        &conn,
        1,
        "reply",
        "user",
        Some(5),
        None,
        "Thanks for the report — reproducing now. Could you share the report id?",
        "2026-02-16T09:45:00.000Z",
    );
    seed_thread(
        &conn,
        1,
        "note",
        "user",
        Some(5),
        None,
        "Likely the v2 export path — flagged for the data track.",
        "2026-02-16T09:46:00.000Z",
    );
    seed_thread(
        &conn,
        1,
        "ai_draft",
        "Support AI",
        None,
        None,
        "Draft: this matches the known export regression fixed in v1.8.4.",
        "2026-02-16T09:47:00.000Z",
    );

    let db = DbHandle::new(conn);
    let mut page = InboxPage::new();
    page.select(&db, 1);
    (page, db, EventBus::new(8))
}

fn seed_users(conn: &rusqlite::Connection) {
    conn.execute(
        "INSERT INTO users (id, remote_id, first_name, last_name, email, mention)
         VALUES (5, 505, 'Ada', 'Lovelace', 'ada@example.com', 'ada'),
                (6, 506, 'Grace', 'Hopper', 'grace@example.com', NULL)",
        [],
    )
    .expect("users");
}

fn seed_mailboxes(conn: &rusqlite::Connection) {
    conn.execute(
        "INSERT INTO mailboxes (id, remote_id, name, email)
         VALUES (1, 101, 'Support', 'support@example.com'),
                (2, 102, 'Billing', 'billing@example.com')",
        [],
    )
    .expect("mailboxes");
}

fn seed_customers(conn: &rusqlite::Connection) {
    conn.execute(
        "INSERT INTO customers (id, remote_id, first_name, last_name, email)
         VALUES (9, 909, 'Kim', 'Pearce', 'kim@example.com'),
                (10, 910, 'Ada', 'Customer', 'adac@example.com')",
        [],
    )
    .expect("customers");
    conn.execute(
        "INSERT INTO customer_emails (customer_id, value, type)
         VALUES (9, 'kim@example.com', 'work'),
                (10, 'adac@example.com', 'work')",
        [],
    )
    .expect("customer emails");
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
    at: &str,
) {
    conn.execute(
        "INSERT INTO conversations (id, remote_id, number, subject, preview, status, type,
             mailbox_local_id, assignee_local_id, customer_local_id, priority,
             created_at, updated_at, closed_at, first_customer_message_at, local_created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                 CASE WHEN ?9 = 0 THEN NULL ELSE ?9 END,
                 ?10, 'none', ?11, ?11,
                 CASE WHEN ?6 = 'closed' THEN ?11 ELSE NULL END,
                 ?11, ?11)",
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
            at,
        ],
    )
    .expect("conversation");
}

#[allow(clippy::too_many_arguments)]
fn seed_thread(
    conn: &rusqlite::Connection,
    conversation_id: i64,
    kind: &str,
    from_type: &str,
    created_by_user_id: Option<i64>,
    created_by_customer_id: Option<i64>,
    body: &str,
    at: &str,
) {
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, body_text, from_type, state,
             created_by_user_id, created_by_customer_id, created_at)
         VALUES (?1, ?2, ?3, ?4, 'published', ?5, ?6, ?7)",
        rusqlite::params![
            conversation_id,
            kind,
            body,
            from_type,
            created_by_user_id,
            created_by_customer_id,
            at,
        ],
    )
    .expect("thread");
}

fn render_inbox_page(dark: bool, width: f32, name: &str) {
    let (page, db, bus) = seeded_page();
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
                s.page.ui(ui, &s.db, &s.bus, None);
            },
            InboxPageState {
                installed: false,
                page,
                db,
                bus,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_inbox_page_1440_light() {
    render_inbox_page(false, 1440.0, "r3_inbox_page_1440_light");
}

#[test]
fn r3_inbox_page_1024_light() {
    render_inbox_page(false, 1024.0, "r3_inbox_page_1024_light");
}

#[test]
fn r3_inbox_page_800_light() {
    render_inbox_page(false, 800.0, "r3_inbox_page_800_light");
}

#[test]
fn r3_inbox_page_1440_dark() {
    render_inbox_page(true, 1440.0, "r3_inbox_page_1440_dark");
}

#[test]
fn r3_inbox_page_1024_dark() {
    render_inbox_page(true, 1024.0, "r3_inbox_page_1024_dark");
}

#[test]
fn r3_inbox_page_800_dark() {
    render_inbox_page(true, 800.0, "r3_inbox_page_800_dark");
}
