#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
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
                // AUD-V1 (register V1): the golden renders through the
                // PRODUCTION host wrapper — CentralPanel + fill + the
                // SP5/14 inset app/mod.rs applies — so the drift gate
                // finally gates the look the user sees. Bare-ui renders
                // certified a synthetic composition.
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(design::tokens(ui).background)
                            .inner_margin(egui::Margin::symmetric(
                                design::scale::space::SP5 as i8,
                                14,
                            )),
                    )
                    .show(ui, |ui| {
                        s.page.ui(ui, &s.db, &s.bus, None);
                    });
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

// UXE-2-3: the r3 fixtures' 4-entry thread never overflows the reader,
// so the scroll-active state (thread clipped at the fold, cards that
// must NOT reflow when the bar rides the edge — egui 0.37's default
// floating bar allocates zero width) is pinned by no golden. This
// fixture seeds a 16-entry thread (2026-02-15 block + the standard
// 2026-02-16 block, one day divider) so the fold falls inside the
// 02-15 block: the lock proves the reader cards keep the composer
// card's exact width at the fold instead of shrinking by a scrollbar
// gutter.
fn seeded_page_overflow() -> (InboxPage, DbHandle, EventBus) {
    let (page, db, bus) = seeded_page();
    // 12 entries across 2026-02-15 (each round = 4 kinds, 3 rounds) so the
    // thread overflows the 900px viewport with the fold inside this block;
    // the 02-16 block (incl. the note card) follows below the fold.
    let mut entries: Vec<(&str, &str, String)> = Vec::new();
    for round in 0..3 {
        for (kind, from) in [
            ("customer_message", "customer"),
            ("reply", "user"),
            ("note", "user"),
            ("ai_draft", "Support AI"),
        ] {
            let n = entries.len();
            entries.push((
                kind,
                from,
                format!("2026-02-15T{:02}:{:02}:00.000Z", 8 + round, n * 3),
            ));
        }
    }
    db.with_write(|conn| {
        for (i, (kind, from, at)) in entries.iter().enumerate() {
            conn.execute(
                "INSERT INTO conversation_threads (conversation_id, type, body_text, from_type,
                     state, created_by_user_id, created_by_customer_id, created_at)
                 VALUES (1, ?1, ?2, ?3, 'published',
                     CASE WHEN ?3 = 'user' THEN 5 ELSE NULL END,
                     CASE WHEN ?3 = 'customer' THEN 9 ELSE NULL END, ?4)",
                rusqlite::params![
                    kind,
                    format!("Overflow entry {}: {} from {}.", i + 1, kind, from),
                    from,
                    at,
                ],
            )
            .expect("overflow thread");
        }
    });
    (page, db, bus)
}

fn render_inbox_page_overflow(dark: bool, width: f32, name: &str) {
    let (page, db, bus) = seeded_page_overflow();
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
                // AUD-V1 (register V1): the golden renders through the
                // PRODUCTION host wrapper — CentralPanel + fill + the
                // SP5/14 inset app/mod.rs applies — so the drift gate
                // finally gates the look the user sees. Bare-ui renders
                // certified a synthetic composition.
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(design::tokens(ui).background)
                            .inner_margin(egui::Margin::symmetric(
                                design::scale::space::SP5 as i8,
                                14,
                            )),
                    )
                    .show(ui, |ui| {
                        s.page.ui(ui, &s.db, &s.bus, None);
                    });
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
fn r5_inbox_reader_overflow_800_light() {
    render_inbox_page_overflow(false, 800.0, "r5_inbox_reader_overflow_800_light");
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

// ---------------------------------------------------------------------------
// AUD-L1/V3: composition geometry invariants — the owner's complaint,
// turned into hard assertions on the LIVE panel rects at every monitor
// class. These are the tests the old battery never had: the goldens
// compared pixels, nothing asserted that panels respect each other's
// bounds. The rects come from InboxPage::panel_geometry (recorded each
// frame inside the panel closures).
//
// Contracts (register L1/V3):
//   G1  no sibling column overlaps (rail | list | reader | copilot)
//   G2  when the copilot is docked and a conversation is selected, the
//       copilot column ENDS at the composer's top edge — the sidebar
//       never runs beneath the reply strip
//   G3  the composer spans the reader+copilot zone (shared right edge
//       with the copilot, left edge with the reader)
//   G4  the reader never renders below READER_FLOOR while the copilot
//       is docked
//   G5  the composer height stays within its declared bounds
// ---------------------------------------------------------------------------

use std::sync::Arc;
use std::sync::Mutex;

use spp_shell::pages::inbox::InboxPanelGeometry;
use spp_shell::pages::inbox::inbox_geo;

const EPS: f32 = 0.75;

fn rects_disjoint(a: egui::Rect, b: egui::Rect) -> bool {
    let separate = a.max.x <= b.min.x + EPS
        || b.max.x <= a.min.x + EPS
        || a.max.y <= b.min.y + EPS
        || b.max.y <= a.min.y + EPS;
    let overlap_area = a.intersect(b).area();
    separate && overlap_area < 1.0
}

fn assert_composition(w: f32, g: &InboxPanelGeometry) {
    // G1: the columns never overlap each other.
    if let (Some(rail), Some(list)) = (g.rail, g.list) {
        assert!(rects_disjoint(rail, list), "{w}px: rail overlaps list");
    }
    if let (Some(list), Some(reader)) = (g.list, g.reader) {
        assert!(
            rects_disjoint(list, reader),
            "{w}px: list overlaps reader — list right {}, reader left {}",
            list.max.x,
            reader.min.x
        );
    }
    if let (Some(copilot), Some(reader)) = (g.copilot, g.reader) {
        assert!(
            rects_disjoint(copilot, reader),
            "{w}px: copilot overlaps reader"
        );
    }
    if let (Some(composer), Some(list)) = (g.composer, g.list) {
        assert!(
            rects_disjoint(composer, list),
            "{w}px: composer intrudes into the list column"
        );
    }
    // The reader always sits fully above the composer.
    if let (Some(reader), Some(composer)) = (g.reader, g.composer) {
        assert!(
            reader.max.y <= composer.min.y + EPS,
            "{w}px: reader runs beneath the composer (reader bottom {}, composer top {})",
            reader.max.y,
            composer.min.y
        );
    }
    if let (Some(copilot), Some(composer), Some(reader)) = (g.copilot, g.composer, g.reader) {
        // The recorded rects are the panels' INNER uis (post-inset); the
        // composition contracts compare the true PANEL edges, restoring
        // the insets first (SP3 copilot / SP2 composer / SP5 composer-x).
        let cop_bottom_edge = copilot.max.y + inbox_geo::COPILOT_INSET;
        let comp_top_edge = composer.min.y - inbox_geo::COMPOSER_TOP_INSET;
        // G2: the copilot column ENDS at the composer's top edge — the
        // exact register L1 demands (no sidebar beneath the reply).
        assert!(
            (cop_bottom_edge - comp_top_edge).abs() <= 1.5,
            "{w}px: L1 REGRESSION — the sidebar does not end at the reply's \
             top edge (copilot bottom edge {}, composer top edge {})",
            cop_bottom_edge,
            comp_top_edge
        );
        // G3: the composer spans the reader+copilot zone — both columns
        // share the SAME right panel edge.
        let comp_right_edge = composer.max.x + inbox_geo::COMPOSER_X_INSET;
        let cop_right_edge = copilot.max.x + inbox_geo::COPILOT_INSET;
        assert!(
            (comp_right_edge - cop_right_edge).abs() <= 1.5,
            "{w}px: composer does not span under the copilot (composer \
             right edge {}, copilot right edge {})",
            comp_right_edge,
            cop_right_edge
        );
        // G4: the reader keeps its floor while the copilot is docked.
        assert!(
            reader.width() >= inbox_geo::READER_FLOOR - EPS,
            "{w}px: reader collapsed below the floor ({}px)",
            reader.width()
        );
    }
    // G5: composer height bounds.
    if let Some(composer) = g.composer {
        assert!(
            composer.height() >= inbox_geo::COMPOSER_MIN_H - EPS,
            "{w}px: composer sliver ({})",
            composer.height()
        );
    }
    // When the copilot is docked, its PANEL width (inner + insets, the
    // separator stroke may eat a pixel) is the declared panel width.
    if let Some(copilot) = g.copilot {
        assert!(
            (copilot.width() + 2.0 * inbox_geo::COPILOT_INSET - inbox_geo::COPILOT_W).abs() <= 2.0,
            "{w}px: copilot panel width {} drifts from the declared {}",
            copilot.width() + 2.0 * inbox_geo::COPILOT_INSET,
            inbox_geo::COPILOT_W
        );
    }
}

fn run_geometry(width: f32, open_copilot: bool) -> InboxPanelGeometry {
    let (page, db, bus) = seeded_page();
    let geo_out: Arc<Mutex<InboxPanelGeometry>> = Arc::new(Mutex::new(Default::default()));
    let sink = geo_out.clone();
    let mut h = Harness::builder()
        .with_size(egui::vec2(width, 900.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    design::install(ui.ctx());
                    s.installed = true;
                }
                // The production host wrapper (AUD-V1).
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(design::tokens(ui).background)
                            .inner_margin(egui::Margin::symmetric(
                                design::scale::space::SP5 as i8,
                                14,
                            )),
                    )
                    .show(ui, |ui| {
                        s.page.ui(ui, &s.db, &s.bus, None);
                    });
                *sink.lock().unwrap() = s.page.panel_geometry();
            },
            InboxPageState {
                installed: false,
                page,
                db,
                bus,
            },
        );
    h.run_ok();
    if open_copilot {
        h.get_by_label("Copilot").click();
        // settle: the composer measurement + panel re-allocation need a
        // couple of frames after the copilot docks.
        h.run_ok();
        h.run_ok();
        h.run_ok();
    } else {
        h.run_ok();
        h.run_ok();
    }
    *geo_out.lock().unwrap()
}

#[test]
fn aud_geo_composition_copilot_closed_all_monitor_classes() {
    for width in [800.0, 1024.0, 1280.0, 1440.0, 1920.0, 2560.0] {
        let g = run_geometry(width, false);
        assert!(g.reader.is_some(), "{width}px: no reader rect");
        assert!(g.composer.is_some(), "{width}px: no composer rect");
        assert!(g.copilot.is_none(), "{width}px: copilot must not be docked");
        assert_composition(width, &g);
    }
}

#[test]
fn aud_geo_composition_copilot_docked_wide_monitors() {
    for width in [1440.0, 1625.0, 1920.0, 2560.0] {
        let g = run_geometry(width, true);
        assert!(g.copilot.is_some(), "{width}px: copilot should dock here");
        assert!(g.composer.is_some(), "{width}px: no composer rect");
        assert_composition(width, &g);
    }
}

#[test]
fn aud_geo_composition_copilot_narrow_small_monitors() {
    for width in [800.0, 1024.0] {
        let g = run_geometry(width, true);
        // Narrow mode: the copilot swaps in over the reader — no docked
        // copilot column, and the composer steps aside with the reader.
        assert!(g.copilot.is_none(), "{width}px: copilot must not dock");
        assert!(g.composer.is_none(), "{width}px: composer must step aside");
        assert!(g.reader.is_some(), "{width}px: no reader rect");
    }
}
