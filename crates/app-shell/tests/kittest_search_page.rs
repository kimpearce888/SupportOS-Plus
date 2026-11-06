#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::search::SearchPage;

struct SearchPageState {
    installed: bool,
    page: SearchPage,
    db: DbHandle,
}

fn seeded_page() -> (SearchPage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    conn.execute(
        "INSERT INTO users (id, remote_id, first_name, last_name, email, mention)
         VALUES (5, 505, 'Ada', 'Lovelace', 'ada@example.com', 'ada')",
        [],
    )
    .expect("user");
    conn.execute(
        "INSERT INTO mailboxes (id, remote_id, name, email)
         VALUES (1, 101, 'Support', 'support@example.com')",
        [],
    )
    .expect("mailbox");
    conn.execute(
        "INSERT INTO customers (id, remote_id, first_name, last_name, email)
         VALUES (9, 909, 'Kim', 'Pearce', 'kim@example.com')",
        [],
    )
    .expect("customer");
    conn.execute(
        "INSERT INTO conversations (id, remote_id, number, subject, preview, status, type,
             mailbox_local_id, assignee_local_id, customer_local_id, priority,
             created_at, updated_at, closed_at, first_customer_message_at, local_created_at)
         VALUES (1, 1001, 1, 'Export broken', 'the export button does nothing', 'active', 'email',
                 1, 5, 9, 'none', '2026-01-01T09:00:00Z', '2026-01-01T09:00:00Z', NULL,
                 '2026-01-01T09:00:00Z', '2026-01-01T09:00:00Z')",
        [],
    )
    .expect("conversation");
    conn.execute(
        "INSERT INTO conversations (id, remote_id, number, subject, preview, status, type,
             mailbox_local_id, assignee_local_id, customer_local_id, priority,
             created_at, updated_at, closed_at, first_customer_message_at, local_created_at)
         VALUES (2, 1002, 2, 'Refund requested', 'asking for a refund on the last invoice',
                 'active', 'email', 1, 5, 9, 'none', '2026-01-01T09:30:00Z',
                 '2026-01-01T09:30:00Z', NULL, '2026-01-01T09:30:00Z', '2026-01-01T09:30:00Z')",
        [],
    )
    .expect("refund conversation");
    conn.execute(
        "INSERT INTO knowledge_sources (id, name, kind) VALUES (1, 'guide', 'file')",
        [],
    )
    .expect("source");
    conn.execute(
        "INSERT INTO knowledge_documents (id, source_id, title, visibility)
         VALUES (1, 1, 'Reset password', 'customer_safe')",
        [],
    )
    .expect("document");
    conn.execute(
        "INSERT INTO knowledge_chunks (document_id, chunk_index, content)
         VALUES (1, 0, 'To reset your password click the link in the login screen')",
        [],
    )
    .expect("chunk");
    let _ = spp_core::search::rebuild_indexes(&conn);
    (SearchPage::new(), DbHandle::new(conn))
}

fn render_search_page(dark: bool, width: f32, name: &str) {
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
            SearchPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_search_page_1440_light() {
    render_search_page(false, 1440.0, "r3_search_page_1440_light");
}

#[test]
fn r3_search_page_1024_light() {
    render_search_page(false, 1024.0, "r3_search_page_1024_light");
}

#[test]
fn r3_search_page_800_light() {
    render_search_page(false, 800.0, "r3_search_page_800_light");
}

#[test]
fn r3_search_page_1440_dark() {
    render_search_page(true, 1440.0, "r3_search_page_1440_dark");
}

#[test]
fn r3_search_page_1024_dark() {
    render_search_page(true, 1024.0, "r3_search_page_1024_dark");
}

#[test]
fn r3_search_page_800_dark() {
    render_search_page(true, 800.0, "r3_search_page_800_dark");
}

#[test]
fn r3_search_export_hits_light() {
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
            SearchPageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    // UX-15: the empty state offers one-click example queries; clicking one
    // runs the search the way typing + Enter always did.
    h.get_all_by_label("refund")
        .next()
        .expect("an example-query chip")
        .click();
    h.run_ok();
    h.run_steps(2);
    assert!(
        h.get_all_by_label("#2 Refund requested").next().is_some(),
        "the example chip must run a real search and surface the seeded refund conversation"
    );
    h.snapshot("r3_search_export_hits_light");
}
