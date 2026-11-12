#![cfg(feature = "kittest-ui")]

// UXG-1-9: the 8K (7680 logical px) width-discipline probes. The wide
// goldens live in the page suites (dashboard 2560/3840 and reports 3840
// through `content_column`, the inbox reader at 2560); a 7680px PNG would
// be multi-megabyte dead weight in the repo, so the 8K tier is proven by
// exact pixel probes instead: the content-column primitive holds at
// CONTENT_MAX centered, the reading measure holds at READER_MEASURE
// centered, and the real inbox page at 7680 keeps its reader thread
// inside the reading measure (uncapped it would stretch ~6700px).

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_core::event_bus::EventBus;
use spp_shell::design;
use spp_shell::pages::inbox::InboxPage;

const W_8K: f32 = 7680.0;

/// The content-column primitive at 8K: marker widgets pin the column's
/// exact left and right edges — the column must hold at CONTENT_MAX and
/// center (the width discipline every standard page rides through the
/// shell funnel).
#[test]
fn uxg_content_column_cap_7680() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(W_8K, 900.0))
        .build_ui(|ui| {
            design::install(ui.ctx());
            design::content_column(ui, |ui| {
                ui.label("cap-left");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    ui.label("cap-right");
                });
            });
        });
    h.run_ok();
    let gutter = ((W_8K - design::scale::CONTENT_MAX) / 2.0).floor();
    let left = h.get_by_label("cap-left").rect();
    let right = h.get_by_label("cap-right").rect();
    assert!(
        (left.min.x - gutter).abs() <= 1.0,
        "the capped column must start at the centered gutter {}: got {}",
        gutter,
        left.min.x
    );
    assert!(
        (right.max.x - (gutter + design::scale::CONTENT_MAX)).abs() <= 1.0,
        "the capped column must end at gutter + CONTENT_MAX ({}): got {}",
        gutter + design::scale::CONTENT_MAX,
        right.max.x
    );
}

/// The reading-measure primitive at 8K: same exact-edge proof at
/// READER_MEASURE — the cap the inbox reader thread and composer ride.
#[test]
fn uxg_reader_measure_cap_7680() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(W_8K, 900.0))
        .build_ui(|ui| {
            design::install(ui.ctx());
            design::centered_column(ui, design::scale::READER_MEASURE, |ui| {
                ui.label("measure-left");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    ui.label("measure-right");
                });
            });
        });
    h.run_ok();
    let gutter = ((W_8K - design::scale::READER_MEASURE) / 2.0).floor();
    let left = h.get_by_label("measure-left").rect();
    let right = h.get_by_label("measure-right").rect();
    assert!(
        (left.min.x - gutter).abs() <= 1.0,
        "the reading column must start at the centered gutter {}: got {}",
        gutter,
        left.min.x
    );
    assert!(
        (right.max.x - (gutter + design::scale::READER_MEASURE)).abs() <= 1.0,
        "the reading column must end at gutter + READER_MEASURE ({}): got {}",
        gutter + design::scale::READER_MEASURE,
        right.max.x
    );
}

struct InboxState {
    installed: bool,
    page: InboxPage,
    db: DbHandle,
    bus: EventBus,
}

fn seeded_reader() -> (InboxPage, DbHandle, EventBus) {
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
         VALUES (9, 909, 'Amy', 'Fraughton', 'amy@example.com')",
        [],
    )
    .expect("customers");
    conn.execute(
        "INSERT INTO customer_emails (customer_id, value, type)
         VALUES (9, 'amy@example.com', 'work')",
        [],
    )
    .expect("customer emails");
    conn.execute(
        "INSERT INTO conversations (id, remote_id, number, subject, preview, status, type,
             mailbox_local_id, assignee_local_id, customer_local_id, priority,
             created_at, updated_at, closed_at, first_customer_message_at, local_created_at)
         VALUES (1, 1001, 501, 'Wide display check', 'Wide display check preview', 'closed',
                 'email', 1, NULL, 9, 'none', '2026-10-28T10:26:00.000Z', '2026-10-28T10:26:00.000Z',
                 '2026-10-28T10:26:00.000Z', '2026-10-28T10:26:00.000Z', '2026-10-28T10:26:00.000Z')",
        [],
    )
    .expect("conversation");
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, body_text, from_type, state,
             created_by_user_id, created_by_customer_id, created_at)
         VALUES (1, 'customer', 'Does the reading column hold its measure on an 8K display?',
                 'customer', 'published', NULL, 9, '2026-10-16T14:09:00.000Z')",
        [],
    )
    .expect("thread");

    let db = DbHandle::new(conn);
    let mut page = InboxPage::new();
    page.select(&db, 1);
    (page, db, EventBus::new(8))
}

/// The real inbox at 8K: the reader pane spans ~6900px after the rail and
/// the clamped list — the subject line (a truncate label that fills its
/// column) must hold inside READER_MEASURE instead of stretching across
/// the pane, and the reading column must sit centered in it.
#[test]
fn uxg_inbox_reader_capped_7680() {
    let (page, db, bus) = seeded_reader();
    let mut h = Harness::builder()
        .with_size(egui::vec2(W_8K, 900.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            move |ui, s| {
                if !s.installed {
                    design::install(ui.ctx());
                    s.installed = true;
                }
                // VF-4 (V1 completion): the 8K geometry check runs
                // through the shared inbox production host, like the
                // goldens.
                design::host_inbox_page(ui, |ui| {
                    s.page.ui(ui, &s.db, &s.bus, None);
                });
            },
            InboxState {
                installed: false,
                page,
                db,
                bus,
            },
        );
    h.run_ok();
    h.run_ok();
    // The subject text matches two nodes (the list row and the reader
    // head); the reader's copy is the one inside the reader pane (x past
    // the rail + clamped list, i.e. > 1000px on an 8K window).
    let reader_subject = h
        .get_all_by_label("#501  Wide display check")
        .find(|node| node.rect().min.x > 1000.0)
        .expect("the reader head's subject copy")
        .rect();
    assert!(
        reader_subject.width() <= design::scale::READER_MEASURE,
        "the reader subject must hold inside the {}px reading measure at 8K, got {}",
        design::scale::READER_MEASURE,
        reader_subject.width()
    );
    // The reader pane's content starts ≈ 168 (rail) + 560 (clamped list)
    // + 22 (margin) = 750px; a centered 860px column in the ~6908px pane
    // puts the subject well past 3000px — a left-hugging (uncapped)
    // reader would sit at ≈ 790px.
    assert!(
        reader_subject.min.x > 3000.0,
        "the reading column must center in the 8K reader pane, subject starts at {}",
        reader_subject.min.x
    );
}
