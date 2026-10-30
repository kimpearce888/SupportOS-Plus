//! RS-4-8 — the copilot-open inbox states, golden-locked (the G4-rejection
//! fix wave's regression locks).
//!
//! The G4 rejection (2026-10-30) proved the copilot-open full-page state had
//! never been render-locked: `kittest_inbox_page` locks the copilot CLOSED,
//! the identity goldens lock the pane widgets at 720 px, and at a real
//! desktop width the reader's content painted straight over the copilot
//! panel (root cause + fix in the RS-4 batch record, `completed.md`). This
//! suite locks the fixed state class at the owner's live aspect (1625×828):
//! copilot OPEN (light + dark) and copilot CLOSED (light).
//!
//! The fixture mirrors the owner's live data shape — the case that broke:
//! Help Scout HTML in `body_text` (`<br><br>`), a long unbreakable URL in
//! the internal note, a blank-name customer row, a closed unassigned
//! conversation. Customer bodies use the canonical `customer` thread type
//! the analyzer reads, so the copilot pane renders its full surface
//! (Signals + Summary) deterministically.
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_inbox_copilot`
//! Regenerate with `UPDATE_SNAPSHOTS=1`; a clean re-run must pass
//! byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_core::eventing::EventBus;
use spp_shell::design;
use spp_shell::pages::inbox::InboxPage;

struct CopilotPageState {
    installed: bool,
    page: InboxPage,
    db: DbHandle,
    bus: EventBus,
}

fn seeded_page() -> (InboxPage, DbHandle, EventBus) {
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
         VALUES (1, 101, 'Support', 'support@example.com')",
        [],
    )
    .expect("mailboxes");
    conn.execute(
        "INSERT INTO customers (id, remote_id, first_name, last_name, email)
         VALUES (9, 909, 'Amy', 'Fraughton', 'amy@example.com'),
                (10, 910, '', '', 'judy@example.com')",
        [],
    )
    .expect("customers");
    conn.execute(
        "INSERT INTO customer_emails (customer_id, value, type)
         VALUES (9, 'amy@example.com', 'work'), (10, 'judy@example.com', 'work')",
        [],
    )
    .expect("customer emails");
    // Conv 1 = the owner's live ticket shape: closed, unassigned, HTML body.
    seed_conversation(
        &conn,
        1,
        "Re: Keep webinar page available",
        "closed",
        "email",
        1,
        0,
        9,
        "2026-10-28T10:26:00.000Z",
    );
    // Conv 2 = the blank-name customer row (RS-4-7's fallback case).
    seed_conversation(
        &conn,
        2,
        "Joinnow data erasure request - from Judy Mills",
        "closed",
        "email",
        1,
        0,
        10,
        "2026-07-22T11:15:00.000Z",
    );
    // Canonical `customer` type + HTML markup: the analyzer strips it
    // (RS-4-2), the reader renders it clean, the copilot Summary card fills.
    seed_thread(
        &conn,
        1,
        "customer",
        "customer",
        None,
        Some(9),
        "Hey there,<br><br>Thanks so much for all the help you've given me, you guys have been \
         awesome.<br><br>The export is broken, throws an error and fails every time, and the \
         webinar watch page sends people to a login form. How do I give them full access to \
         the watch page without a login?<br><br>Thank you!",
        "2026-10-16T14:09:00.000Z",
    );
    // Internal note with the long unbreakable URL (RS-4-3's break case).
    seed_thread(
        &conn,
        1,
        "note",
        "user",
        Some(5),
        None,
        "Technical Information\nSession Information\nCurrent Page URL\nhttps://stealthseminarapp.\
         com/?qa=2.216478160.73549554.1792024737-fb4c0a7b8d9e0f1a2b3c4d5e6f7a8b9c",
        "2026-10-16T14:09:00.000Z",
    );
    seed_thread(
        &conn,
        2,
        "customer",
        "customer",
        None,
        Some(10),
        "Please erase my Joinnow data.",
        "2026-07-22T11:14:00.000Z",
    );

    let db = DbHandle::new(conn);
    let mut page = InboxPage::new();
    page.select(&db, 1);
    (page, db, EventBus::new(8))
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

/// Render at `w`×`h`; `open_copilot` drives the REAL toggle chip (the same
/// path a user's click takes) before locking.
fn render_copilot_state(dark: bool, w: f32, h: f32, open_copilot: bool, name: &str) {
    let (page, db, bus) = seeded_page();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(w, h))
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
            CopilotPageState {
                installed: false,
                page,
                db,
                bus,
            },
        );
    harness.run_ok();
    if open_copilot {
        harness.get_by_label("Copilot").click();
        harness.run_ok();
        harness.run_ok();
    }
    harness.snapshot(name);
}

/// The owner's live window size with the copilot open — the state class
/// that was never locked and broke (light).
#[test]
fn r4_inbox_copilot_open_1625_light() {
    render_copilot_state(
        false,
        1625.0,
        828.0,
        true,
        "r4_inbox_copilot_open_1625_light",
    );
}

/// Same state class, dark theme.
#[test]
fn r4_inbox_copilot_open_1625_dark() {
    render_copilot_state(true, 1625.0, 828.0, true, "r4_inbox_copilot_open_1625_dark");
}

/// The copilot-closed state at the same aspect (reader-only regression
/// lock for the note-card token breaking).
#[test]
fn r4_inbox_copilot_closed_1625_light() {
    render_copilot_state(
        false,
        1625.0,
        828.0,
        false,
        "r4_inbox_copilot_closed_1625_light",
    );
}
