#![cfg(feature = "kittest-ui")]

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spp_core::db::DbHandle;
use spp_shell::design;
use spp_shell::pages::knowledge::KnowledgePage;

struct KnowledgePageState {
    installed: bool,
    page: KnowledgePage,
    db: DbHandle,
}

fn seeded_page() -> (KnowledgePage, DbHandle) {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");

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
    h.run_steps(2);
    assert!(
        h.get_all_by_label("Read").next().is_some(),
        "the Help center tab must show the articles' read act"
    );
    h.snapshot("r3_knowledge_help_center_tab_light");
}
