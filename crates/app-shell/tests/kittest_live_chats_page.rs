#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
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

    let mut page = LiveChatsPage::new();
    page.select_chat("chat-1");
    (page, DbHandle::new(conn))
}

fn ingest(conn: &rusqlite::Connection, event: &str, payload: serde_json::Value) {
    spp_core::chats::ingest_event(conn, event, &payload).expect("ingest chat event");
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
                s.page.ui(ui, &s.db);
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
                s.page.ui(ui, &s.db);
            },
            LiveChatsPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_by_label("show ended chats").click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.query_by_label("chat with Grace N — ended").is_some(),
        "the ended view must list the ended chat"
    );
    h.snapshot("r3_live_chats_tab_switch_light");
}
