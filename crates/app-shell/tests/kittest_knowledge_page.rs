//! RS-18 — DS knowledge-page goldens (wave B).
//!
//! The lab reference lives at `mockups/design-lab/goldens/v4/{light,dark}/knowledge-*.png`;
//! egui renders will not pixel-match that HTML lab — these goldens lock the
//! EGUI page state (the real `KnowledgePage`: the PageHeader with the
//! Docs-sync composite, the DS-C-09 underline tab rail (the lab's five
//! labels), the Documents table with its review/verify/delete acts over the
//! watch-folder import panel, and the tab block stacks) over a seeded
//! mirror; the side-by-side review against the lab is the G3 evidence pack.
//!
//! Determinism note: the knowledge seeds are fixed stamps (the knowledge
//! store carries no wall-clock windows), so every tab renders identically
//! on every run. The Freshness tab renders no flag cards by design — the
//! freshness_report() emits no `flags` key (the lab's empty-state quirk,
//! receipted there and kept here).
//!
//! The interaction golden exercises the DS-C-09 rail's real state change:
//! a `Help center` click lands on the help-center surface (the statGrid,
//! the widget_info'd collection/status combos with their apply acts, the
//! tracked-gaps card and the articles table with the read act).
//!
//! Run under a display (Xvfb on display :99 in this lab):
//! `cargo test -p supportos-plus-app-shell --features kittest-ui --test kittest_knowledge_page`
//! Regenerate the baselines with `UPDATE_SNAPSHOTS=1`; a clean re-run must
//! then pass byte-identical (the repo's diff-locked-twice rule).
#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::knowledge::KnowledgePage;

/// Harness state: the REAL page + the seeded database it reads.
struct KnowledgePageState {
    installed: bool,
    page: KnowledgePage,
    db: DbHandle,
}

// ---------------------------------------------------------------------------
// Deterministic fixture (fixed stamps)
// ---------------------------------------------------------------------------

/// Bootstrap an in-memory database (all migrations) and seed the knowledge
/// fixture: one local source with two chunked documents, one help-center
/// collection with a published + a draft article, one tracked zero-hit gap
/// + one stale freshness row (the Help center gaps card) and one open gap
/// candidate + one drafted article (the Drafts queue).
fn seeded_page() -> (KnowledgePage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");

    // One local source + two documents with chunk rows (the Documents
    // table's title/source/visibility/chunks columns).
    conn.execute(
        "INSERT INTO knowledge_sources (id, name, kind, visibility, created_at)
         VALUES (1, 'local docs', 'local_file', 'internal_only', '2026-01-10T08:00:00.000Z')",
        [],
    )
    .expect("source");
    conn.execute(
        "INSERT INTO knowledge_documents (id, source_id, title, visibility, version, content,
             format, created_at, updated_at)
         VALUES (1, 1, 'Export guide', 'public', 1, 'How to export data safely from the mirror.',
                 'markdown', '2026-01-11T09:00:00.000Z', '2026-01-11T09:00:00.000Z'),
                (2, 1, 'Billing FAQ', 'internal_only', 1, 'Billing answers for the support desk.',
                 'markdown', '2026-01-12T09:00:00.000Z', '2026-01-12T09:00:00.000Z')",
        [],
    )
    .expect("documents");
    for (doc, idx) in [(1, 0), (1, 1), (2, 0)] {
        conn.execute(
            "INSERT INTO knowledge_chunks (document_id, chunk_index, content, fts_indexed)
             VALUES (?1, ?2, 'chunk body', 0)",
            rusqlite::params![doc, idx],
        )
        .expect("chunk");
    }

    // The help center: one collection, one published + one draft article.
    conn.execute(
        "INSERT INTO docs_collections (id, remote_id, name, slug)
         VALUES (1, 11, 'Guides', 'guides')",
        [],
    )
    .expect("collection");
    conn.execute(
        "INSERT INTO docs_articles (collection_local_id, number, name, status, text, preview,
             views, words, remote_created_at, remote_updated_at)
         VALUES (1, 1, 'Export your data', 'published', 'Open the export page…',
                 'Open the export page…', 41, 120, '2026-01-10T00:00:00.000Z',
                 '2026-01-10T00:00:00.000Z'),
                (1, 2, 'Billing plans', 'draft', 'Plans and limits…', 'Plans and limits…',
                 3, 90, '2026-01-11T00:00:00.000Z', '2026-01-11T00:00:00.000Z')",
        [],
    )
    .expect("articles");

    // Item 150: the tracked gaps — one zero-hit search + one stale doc.
    conn.execute(
        "INSERT INTO knowledge_gaps (query_text, hit_count, last_seen_at)
         VALUES ('bulk rename workspaces', 0, '2026-01-12T10:00:00.000Z')",
        [],
    )
    .expect("gap");
    conn.execute(
        "INSERT INTO knowledge_doc_freshness (doc_id, last_synced_at, content_hash, stale_at)
         VALUES (2, '2026-01-05T00:00:00.000Z', 'hash-2', '2026-01-06T00:00:00.000Z')",
        [],
    )
    .expect("freshness row");

    // K2: one open gap candidate (the gap combo) + one drafted article
    // (the review queue row with its awaiting-review badge and Open act).
    conn.execute(
        "INSERT INTO knowledge_gap_candidates (kind, query_text, occurrence_count, status,
             created_at, updated_at)
         VALUES ('repeated_question_uncovered', 'how do I bulk rename workspaces', 7, 'open',
                 '2026-01-12T07:00:00.000Z', '2026-01-12T09:00:00.000Z')",
        [],
    )
    .expect("gap candidate");
    conn.execute(
        "INSERT INTO article_drafts (candidate_id, title, body, citations, status, created_at)
         VALUES (1, 'Bulk rename workspaces', 'Draft body of the bulk-rename article.', '[]',
                 'drafted', '2026-01-12T11:00:00.000Z')",
        [],
    )
    .expect("draft");

    (KnowledgePage::new(), DbHandle::new(conn))
}

// ---------------------------------------------------------------------------
// The six goldens + the Help-center-tab interaction
// ---------------------------------------------------------------------------

/// Render the real page once at `width` × 900 in `dark`/light and lock it as
/// `{name}.png` under `tests/snapshots/`.
fn render_knowledge_page(dark: bool, width: f32, name: &str) {
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
            KnowledgePageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.snapshot(name);
}

#[test]
fn r3_knowledge_page_1440_light() {
    render_knowledge_page(false, 1440.0, "r3_knowledge_page_1440_light");
}

#[test]
fn r3_knowledge_page_1024_light() {
    render_knowledge_page(false, 1024.0, "r3_knowledge_page_1024_light");
}

#[test]
fn r3_knowledge_page_800_light() {
    render_knowledge_page(false, 800.0, "r3_knowledge_page_800_light");
}

#[test]
fn r3_knowledge_page_1440_dark() {
    render_knowledge_page(true, 1440.0, "r3_knowledge_page_1440_dark");
}

#[test]
fn r3_knowledge_page_1024_dark() {
    render_knowledge_page(true, 1024.0, "r3_knowledge_page_1024_dark");
}

#[test]
fn r3_knowledge_page_800_dark() {
    render_knowledge_page(true, 800.0, "r3_knowledge_page_800_dark");
}

/// The DS-C-09 rail interaction: clicking the `Help center` tab through
/// the a11y tree lands on the help-center surface — the statGrid, the
/// collection/status combos with their apply acts, the tracked-gaps card
/// and the articles table with the read act. Snapshot the tab.
#[test]
fn r3_knowledge_help_center_tab_light() {
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
            KnowledgePageState {
                installed: false,
                page,
                db,
            },
        );
    h.run_ok();
    h.get_all_by_label("Help center")
        .next()
        .expect("the Help center tab")
        .click();
    h.run_ok();
    h.run_steps(2); // settle the tab frame
    // The tab switched: the articles table's read act is in the tree, and
    // the a11y check (still ON) validates the tab's controls.
    assert!(
        h.get_all_by_label("read").next().is_some(),
        "the Help center tab must show the articles' read act"
    );
    h.snapshot("r3_knowledge_help_center_tab_light");
}
