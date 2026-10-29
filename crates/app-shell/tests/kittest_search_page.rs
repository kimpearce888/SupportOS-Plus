//! RS-20 — DS search goldens (wave D).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/search-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `SearchPage`: the toolbar's input + one
//! primary apply + the text clear act, the scope chip row, the lab's
//! empty state with the Try-"export" CTA, and the hit cards with the
//! scope badge + mono strong title + why chip row + snippet) over a
//! seeded mirror; the side-by-side review against the lab is the G3
//! evidence pack.
//!
//! The goldens lock the lab's first state (`q: ""`, `ran: false` — the
//! empty state); the interaction golden clicks the Try-"export" CTA (the
//! lab's se-demo act) and locks the populated hits state over the real
//! FTS engine.
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_search_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::search::SearchPage;

/// Harness state: the REAL page + the seeded database it reads.
struct SearchPageState {
    installed: bool,
    page: SearchPage,
    db: DbHandle,
}

/// Bootstrap an in-memory database and seed the search fixture: one
/// mailbox/customer/conversation ("Export broken" — the query the lab's
/// CTA runs) and one knowledge document, with the FTS indexes rebuilt
/// (direct writes bypass the sync path that maintains them, exactly like
/// the page's own unit tests).
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

// ---------------------------------------------------------------------------
// The six goldens + the Try-"export" interaction
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light and lock it
/// as `{name}.png` under `tests/snapshots/` (the lab's first state: the
/// empty state with the Try-"export" CTA).
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

/// The lab's se-demo act: the empty state's Try-"export" CTA fills the
/// query and runs the real FTS search — the hits state with the count
/// hint, the honest mode note and the hit cards. Snapshot it.
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
    h.get_all_by_label("Try \"export\"")
        .next()
        .expect("the Try-export CTA")
        .click();
    h.run_ok();
    h.run_steps(2); // settle the results frame
    assert!(
        h.get_all_by_label("#1 Export broken").next().is_some(),
        "the FTS engine must surface the seeded conversation (hit titles ride the number-subject shape)"
    );
    h.snapshot("r3_search_export_hits_light");
}
