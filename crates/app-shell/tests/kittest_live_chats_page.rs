#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::NavRequest;
use spp_shell::pages::live_chats::LiveChatsPage;

struct LiveChatsPageState {
    installed: bool,
    page: LiveChatsPage,
    db: DbHandle,
}

fn seeded_page() -> (LiveChatsPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");

    conn.execute(
        "INSERT INTO accounts (remote_id, plan, raw_json, last_seen_at)
         VALUES (5, 'owner', '{}', '2026-02-16T10:00:00.000Z')",
        [],
    )
    .expect("seed the connected user");

    ingest(
        &conn,
        "beacon.chat.created",
        serde_json::json!({
            "id": "chat-1", "mailboxId": 3, "state": "open",
            "customer": {"id": 7, "first": "Kim", "last": "One"},
            "createdAt": "2026-02-16T09:39:00.000Z",
            "events": [
                {"id": "m-1", "type": "beacon.chat.customer.replied", "author": "Kim One",
                 "text": "help the export is broken", "createdAt": "2026-02-16T09:39:10.000Z"},
                {"id": "m-2", "type": "beacon.chat.agent.replied", "author": "Ada",
                 "text": "Hi Kim — I'm on it. Can you open the export screen and tell me the row count?",
                 "createdAt": "2026-02-16T09:41:00.000Z"}
            ]
        }),
    );
    ingest(
        &conn,
        "beacon.chat.assigned",
        serde_json::json!({
            "id": "chat-2", "mailboxId": 3, "state": "open",
            "assignee": {"id": 5, "first": "Ada"},
            "createdAt": "2026-02-16T08:12:00.000Z"
        }),
    );
    ingest(
        &conn,
        "beacon.chat.ended",
        serde_json::json!({
            "id": "chat-3", "mailboxId": 3, "state": "ended",
            "customer": {"id": 8, "first": "Grace", "last": "N"},
            "conversationId": 499,
            "createdAt": "2026-02-15T17:31:00.000Z",
            "endedAt": "2026-02-15T17:33:00.000Z",
            "events": [
                {"id": "m-3", "type": "beacon.chat.customer.replied", "author": "Grace N",
                 "text": "the beacon wasn't loading at all", "createdAt": "2026-02-15T17:31:00.000Z"},
                {"id": "m-4", "type": "beacon.chat.agent.replied", "author": "Ada",
                 "text": "Should be fixed now — hard refresh and the loader is back.",
                 "createdAt": "2026-02-15T17:32:40.000Z"},
                {"id": "m-5", "type": "beacon.chat.customer.replied", "author": "Grace N",
                 "text": "thanks, that fixed it", "createdAt": "2026-02-15T17:33:00.000Z"}
            ]
        }),
    );

    seed_conversation(
        &conn,
        7,
        499,
        51234,
        "Beacon widget wouldn't load",
        "closed",
        "2026-02-15T17:30:00.000Z",
    );

    let mut page = LiveChatsPage::new();
    page.select_chat("chat-1");
    (page, DbHandle::new(conn))
}

fn ingest(conn: &rusqlite::Connection, event: &str, payload: serde_json::Value) {
    spp_core::chats::ingest_event(conn, event, &payload).expect("ingest chat event");
}

fn seed_conversation(
    conn: &rusqlite::Connection,
    id: i64,
    remote_id: i64,
    number: i64,
    subject: &str,
    status: &str,
    at: &str,
) {
    conn.execute(
        "INSERT INTO conversations (id, remote_id, number, subject, preview, status, type,
             priority, created_at, updated_at, closed_at, first_customer_message_at,
             local_created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'chat', 'none', ?7, ?7, ?7, ?7, ?7)",
        rusqlite::params![
            id,
            remote_id,
            number,
            subject,
            format!("{subject} preview"),
            status,
            at
        ],
    )
    .expect("conversation");
}

fn render_live_chats_page(dark: bool, width: f32, name: &str) {
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
                design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db));
            },
            LiveChatsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_live_chats_page_1440_light() {
    render_live_chats_page(false, 1440.0, "r3_live_chats_page_1440_light");
}

#[test]
fn r3_live_chats_page_1024_light() {
    render_live_chats_page(false, 1024.0, "r3_live_chats_page_1024_light");
}

#[test]
fn r3_live_chats_page_800_light() {
    render_live_chats_page(false, 800.0, "r3_live_chats_page_800_light");
}

#[test]
fn r3_live_chats_page_1440_dark() {
    render_live_chats_page(true, 1440.0, "r3_live_chats_page_1440_dark");
}

#[test]
fn r3_live_chats_page_1024_dark() {
    render_live_chats_page(true, 1024.0, "r3_live_chats_page_1024_dark");
}

#[test]
fn r3_live_chats_page_800_dark() {
    render_live_chats_page(true, 800.0, "r3_live_chats_page_800_dark");
}

#[test]
fn r3_live_chats_tab_switch_light() {
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
                design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db));
            },
            LiveChatsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_by_label("Ended chats").click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.query_by_label("chat with Grace N — ended").is_some(),
        "the ended view must list the ended chat"
    );
    h.snapshot("r3_live_chats_tab_switch_light");
}

struct LiveChatsNavState {
    installed: bool,
    page: LiveChatsPage,
    db: DbHandle,
    nav: Option<NavRequest>,
}

#[test]
fn audp2_live_chats_conversation_link_light() {
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
                if let Some(nav) = design::host_standard_page(ui, |ui| s.page.ui(ui, &s.db)).inner {
                    s.nav = Some(nav);
                }
            },
            LiveChatsNavState {
                installed: false,
                page,
                db,
                nav: None,
            },
        );
    h.run_ok();
    h.get_by_label("Ended chats").click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.query_by_label("chat with Grace N — ended").is_some(),
        "the ended view lists the ended chat"
    );
    h.get_by_label("chat with Grace N — ended").scroll_to_me();
    h.run_ok();
    h.run_steps(2);
    h.get_by_label("chat with Grace N — ended").click();
    h.run_ok();
    assert!(
        h.query_by_label("became conversation #51234").is_some(),
        "the conversation link renders when the mirror row exists"
    );
    assert!(
        h.query_by_label("customer 8 not in the mirror yet")
            .is_some(),
        "an unsynced customer renders the honest fallback"
    );
    h.get_by_label("became conversation #51234").click();
    h.run_ok();
    h.run_steps(2);
    assert_eq!(
        h.state().nav,
        Some(NavRequest::OpenConversation(7)),
        "the conversation link opens the conversation in the Inbox"
    );
    h.run_steps(2);
    h.snapshot("audp2_live_chats_conversation_link_light");
}
