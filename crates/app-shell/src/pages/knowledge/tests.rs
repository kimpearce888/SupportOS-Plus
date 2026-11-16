use super::*;
use crate::testsupport::{empty_db, seed_doc_article};

fn seeded_docs() -> DbHandle {
    let db = empty_db();
    db.with_write(|conn| {
        seed_doc_article(
            conn,
            1,
            "Export guide",
            "published",
            "How to export data safely",
        );
        seed_doc_article(
            conn,
            1,
            "Import guide",
            "draft",
            "How to import data safely",
        );
    });
    db
}

#[test]
fn docs_stats_count_collections_and_statuses() {
    let db = seeded_docs();
    let stats = db.with_read(docs_stats);
    assert_eq!(stats.collections, 1);
    assert_eq!(stats.articles, 2);
    assert_eq!(stats.published, 1);
    assert_eq!(stats.draft, 1);
    assert_eq!(stats.internal, 0);
}

#[test]
fn list_articles_filters_by_status_and_collection() {
    let db = seeded_docs();
    db.with_read(|conn| {
        let all = list_articles(conn, &DocsFilter::default());
        assert_eq!(all.len(), 2);

        let published = list_articles(
            conn,
            &DocsFilter {
                status: Some("published".into()),
                ..DocsFilter::default()
            },
        );
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].name, "Export guide");

        let none = list_articles(
            conn,
            &DocsFilter {
                collection: Some(999),
                ..DocsFilter::default()
            },
        );
        assert!(none.is_empty());
    });
}

#[test]
fn fts_search_finds_articles_by_token() {
    let db = seeded_docs();
    db.with_read(|conn| {
        let hits = fts_article_ids(conn, "export");
        assert_eq!(hits.len(), 1);
        let articles = list_articles(
            conn,
            &DocsFilter {
                query: Some("export".into()),
                ..DocsFilter::default()
            },
        );
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].name, "Export guide");

        assert!(fts_article_ids(conn, "zzzz-nothing").is_empty());
        assert!(fts_article_ids(conn, "e").is_empty());
    });
}

#[test]
fn article_detail_returns_summary_and_text() {
    let db = seeded_docs();
    db.with_read(|conn| {
        let (article, text) = article_detail(conn, 1).expect("article");
        assert_eq!(article.name, "Export guide");
        assert!(text.contains("export data"));
        assert!(article_detail(conn, 999).is_none());
    });
}

#[test]
fn article_body_renders_the_reported_html_shape_without_syntax() {
    let spans = spp_core::richtext::html_to_spans(
        "<p>The Mailbox API allows 400 requests per minute per token. If you \
         exceed it, wait for the <code>Retry-After</code> seconds given in the \
         response. Persistent 429s with low traffic usually mean a shared IP \
         pool - contact support to have your bucket reset.</p>",
    );
    let blocks = article_blocks(&spans);
    assert_eq!(blocks.len(), 1, "one paragraph, one block");
    let (gap, block) = &blocks[0];
    assert_eq!(*gap, 0, "the first block carries no leading gap");
    let text: String = block.iter().map(|s| s.text.as_str()).collect();
    assert!(
        !text.contains('<'),
        "no raw syntax may survive the reader path: {text:?}"
    );
    assert!(text.contains("Retry-After"));
    assert!(text.contains("bucket reset"));
    assert!(
        block
            .iter()
            .any(|s| s.style.code && s.text == "Retry-After"),
        "the inline code run keeps its monospace style"
    );
}

#[test]
fn article_blocks_split_paragraphs_headings_lists_and_code() {
    let spans = spp_core::richtext::html_to_spans(
        "<h2>Setup</h2><p>First paragraph.</p><p>Second paragraph.</p>\
         <ul><li>one</li><li>two</li></ul><pre>cargo run\n--release</pre>",
    );
    let blocks = article_blocks(&spans);
    assert_eq!(
        blocks.len(),
        6,
        "heading, two paragraphs, two list items, one code block"
    );
    assert_eq!(blocks[0].1[0].style.heading, Some(2));
    assert_eq!(blocks[0].0, 0, "the leading block has no gap");
    assert_eq!(blocks[1].0, 2, "paragraph gaps carry two newlines");
    assert_eq!(blocks[2].0, 2);
    assert!(
        blocks[3].1[0].text.starts_with("\u{2022} "),
        "the bullet is baked into the list item text"
    );
    assert_eq!(blocks[4].0, 1, "list items ride soft breaks");
    assert!(
        blocks[5]
            .1
            .iter()
            .any(|s| s.style.code && s.text.contains('\n')),
        "the code block keeps its internal line break"
    );
    for (_, block) in &blocks {
        for span in block.iter() {
            assert!(!span.text.contains('<'), "no raw syntax: {:?}", span.text);
        }
    }
}

#[test]
fn article_blocks_plain_text_body_is_one_readable_block() {
    let spans = spp_core::richtext::html_to_spans("Just a plain article body.");
    let blocks = article_blocks(&spans);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].0, 0);
    let text: String = blocks[0].1.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(text, "Just a plain article body.");
}

#[test]
fn article_blocks_empty_body_has_no_blocks() {
    assert!(article_blocks(&spp_core::richtext::html_to_spans("")).is_empty());
    assert!(article_blocks(&spp_core::richtext::html_to_spans("   \n\t ")).is_empty());
}

#[test]
fn knowledge_document_actions_round_trip() {
    let db = empty_db();
    let doc_id = db.with_write(|conn| {
        let source_id =
            spp_core::knowledge_store::get_or_create_source(conn, "Handbook", "manual", "internal")
                .expect("source");
        spp_core::knowledge_store::upsert_document(
            conn,
            source_id,
            "Troubleshooting logins",
            "Check the SSO configuration first.",
            "private",
            "text",
        )
        .expect("upsert")
        .0
    });

    let mut page = KnowledgePage::new();
    page.reload_all(&db);
    assert_eq!(page.documents.len(), 1);
    assert_eq!(page.sources.len(), 1);

    page.mark_reviewed(&db, doc_id);
    page.reload_all(&db);
    assert!(page.freshness.is_some());

    let _ = page.delete_document(&db, doc_id);
    page.reload_all(&db);
    assert!(page.documents.is_empty());
}

fn seed_knowledge_doc(db: &DbHandle, title: &str, content: &str, visibility: &str) -> i64 {
    db.with_write(|conn| {
        let source_id =
            spp_core::knowledge_store::get_or_create_source(conn, "Handbook", "manual", "internal")
                .expect("source");
        spp_core::knowledge_store::upsert_document(
            conn, source_id, title, content, visibility, "markdown",
        )
        .expect("upsert")
        .0
    })
}

#[test]
fn audp2_document_reader_round_trip() {
    let db = empty_db();
    let doc_id = seed_knowledge_doc(&db, "Reader guide", "Read me end to end.", "internal_only");
    let mut page = KnowledgePage::new();
    page.reload_all(&db);
    assert!(page.doc_reader.is_none());

    page.open_document(&db, doc_id);
    let reader = page.doc_reader.clone().expect("the reader state loads");
    assert_eq!(i64_field(&reader, "id"), doc_id);
    assert_eq!(str_field(&reader, "title"), "Reader guide");
    assert_eq!(str_field(&reader, "content"), "Read me end to end.");
    assert_eq!(i64_field(&reader, "chunk_count"), 1);

    page.close_document();
    assert!(page.doc_reader.is_none());
    assert_eq!(page.documents.len(), 1);

    page.go_to_documents(doc_id);
    assert_eq!(page.tab, Tab::Documents);
    assert_eq!(page.pending_doc, Some(doc_id));
    assert!(page.doc_reader.is_none());
    if let Some(id) = page.pending_doc.take() {
        page.open_document(&db, id);
    }
    let reader = page
        .doc_reader
        .clone()
        .expect("the deep link opens the reader");
    assert_eq!(i64_field(&reader, "id"), doc_id);
    assert_eq!(str_field(&reader, "content"), "Read me end to end.");

    page.go_to_documents(999);
    if let Some(id) = page.pending_doc.take() {
        page.open_document(&db, id);
    }
    assert!(page.doc_reader.is_none());
}

#[test]
fn audp2_document_edit_round_trips_through_update_document() {
    let db = empty_db();
    let doc_id = seed_knowledge_doc(
        &db,
        "Troubleshooting logins",
        "Check the SSO configuration first.",
        "internal_only",
    );
    let mut page = KnowledgePage::new();
    page.reload_all(&db);

    page.start_doc_edit(&db, doc_id);
    let form = page.doc_edit.clone().expect("the edit form opens");
    assert_eq!(form.id, doc_id);
    assert_eq!(form.title, "Troubleshooting logins");
    assert_eq!(form.content, "Check the SSO configuration first.");
    assert_eq!(form.visibility, "internal_only");
    page.doc_edit = None;

    page.start_doc_edit(&db, doc_id);
    if let Some(form) = page.doc_edit.as_mut() {
        form.title = "Troubleshooting SSO logins".into();
        form.content = "Check the SSO configuration first, then the audit log.".into();
        form.visibility = "customer_safe".into();
    }
    page.save_doc_edit(&db);
    assert!(page.doc_edit.is_none(), "saving closes the form");
    assert!(page.error.is_none(), "{:?}", page.error);
    page.reload_all(&db);
    assert_eq!(
        page.documents.len(),
        1,
        "the rename edits the row, no sibling"
    );
    assert_eq!(
        str_field(&page.documents[0], "title"),
        "Troubleshooting SSO logins"
    );
    assert_eq!(str_field(&page.documents[0], "visibility"), "customer_safe");

    let stored = db
        .with_read(|conn| spp_core::knowledge_store::get_document(conn, doc_id))
        .expect("the document survives the edit");
    assert_eq!(str_field(&stored, "title"), "Troubleshooting SSO logins");
    assert_eq!(
        str_field(&stored, "content"),
        "Check the SSO configuration first, then the audit log."
    );
    assert_eq!(
        i64_field(&stored, "version"),
        2,
        "a content change bumps the version"
    );
    assert_eq!(i64_field(&stored, "chunk_count"), 1, "the edit re-chunked");

    page.start_doc_edit(&db, doc_id);
    if let Some(form) = page.doc_edit.as_mut() {
        form.visibility = "internal_only".into();
    }
    page.save_doc_edit(&db);
    assert!(page.error.is_none(), "{:?}", page.error);
    let (visibility, version, fts_visibility) = db.with_read(|conn| {
        let stored = spp_core::knowledge_store::get_document(conn, doc_id)
            .expect("the document survives the visibility edit");
        let fts: String = conn
            .query_row(
                "SELECT visibility FROM fts_knowledge WHERE document_id = ?1",
                rusqlite::params![doc_id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        (
            str_field(&stored, "visibility"),
            i64_field(&stored, "version"),
            fts,
        )
    });
    assert_eq!(visibility, "internal_only");
    assert_eq!(version, 2, "no content change, no version bump");
    assert_eq!(fts_visibility, "internal_only", "the FTS rows follow");
}

#[test]
fn audp2_reindex_resets_exhausted_chunks_and_requeues_embedding() {
    let db = empty_db();
    let doc_id = seed_knowledge_doc(
        &db,
        "Embedding laggard",
        "A body whose chunk failed to embed.",
        "internal_only",
    );
    db.with_write(|conn| {
        conn.execute(
            "UPDATE knowledge_chunks
                SET embedding_state = 'failed', embedding_attempts = 5
              WHERE document_id = ?1",
            rusqlite::params![doc_id],
        )
        .expect("exhaust the chunk");
    });

    let mut page = KnowledgePage::new();
    page.reload_all(&db);
    page.open_document(&db, doc_id);
    assert_eq!(
        i64_field(
            page.doc_reader.as_ref().expect("the reader is open"),
            "exhausted_count"
        ),
        1
    );

    page.reindex_document_action(&db, doc_id);
    assert!(
        page.status_message
            .as_deref()
            .is_some_and(|m| m.contains("re-queued")),
        "the re-ingest reports the reset: {:?}",
        page.status_message
    );
    assert!(page.error.is_none(), "{:?}", page.error);
    assert_eq!(
        i64_field(
            page.doc_reader.as_ref().expect("the reader stays open"),
            "exhausted_count"
        ),
        0,
        "the open reader refreshes with the reset"
    );

    let (state, attempts, job) = db.with_read(|conn| {
        let (state, attempts): (String, i64) = conn
            .query_row(
                "SELECT embedding_state, embedding_attempts FROM knowledge_chunks
                  WHERE document_id = ?1",
                rusqlite::params![doc_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("the chunk survives the reset");
        let job: String = conn
            .query_row(
                "SELECT type FROM jobs WHERE queue = 'embeddings' ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or_default();
        (state, attempts, job)
    });
    assert_eq!(state, "not_indexed", "the exhausted chunk is re-armed");
    assert_eq!(attempts, 0, "the failed-attempt count resets");
    assert_eq!(
        job, "embed_knowledge_chunks",
        "the embedding pass is re-queued"
    );
}

fn temp_data_dir(label: &str) -> crate::testsupport::TempDir {
    let dir = crate::testsupport::TempDir::new(&format!("spp-knowledge-import-{label}"));
    std::fs::create_dir_all(dir.join("knowledge-import")).expect("temp data dir");
    dir
}
fn write_import_file(dir: &Path, name: &str, body: &str) {
    std::fs::write(dir.join("knowledge-import").join(name), body).expect("seed file");
}

#[test]
fn file_import_roundtrip_enqueues_embeddings_and_audits() {
    let db = empty_db();
    let dir = temp_data_dir("roundtrip");
    write_import_file(&dir, "note.md", "# Runbook\n\nRestart the worker.");

    let count = run_file_import(&db, &dir, "note.md").expect("import succeeds");
    assert_eq!(count, 1);

    let documents = db.with_read(|conn| spp_core::knowledge_store::list_documents(conn, None));
    assert_eq!(documents.len(), 1);
    assert_eq!(str_field(&documents[0], "title"), "note");
    assert_eq!(str_field(&documents[0], "visibility"), "internal_only");

    let (job, audit) = db.with_read(|conn| {
        let job = conn
            .query_row(
                "SELECT type FROM jobs WHERE queue = 'embeddings' ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_default();
        let audit = conn
            .query_row(
                "SELECT actor, action FROM audit_log ORDER BY rowid DESC LIMIT 1",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .unwrap_or_default();
        (job, audit)
    });
    assert_eq!(job, "embed_knowledge_chunks");
    assert_eq!(
        audit,
        ("user".to_string(), "knowledge_imported".to_string())
    );
}

#[test]
fn file_import_rejects_paths_outside_the_watch_folder() {
    let db = empty_db();
    let dir = temp_data_dir("escape");

    let err = run_file_import(&db, &dir, "../escape.md").expect_err("escape must be rejected");
    assert!(err.to_string().contains("knowledge-import"));

    let documents = db.with_read(|conn| spp_core::knowledge_store::list_documents(conn, None));
    assert!(documents.is_empty());
}

#[test]
fn manual_import_validates_and_roundtrips() {
    let db = empty_db();
    let dir = temp_data_dir("manual");

    assert!(run_manual_import(&db, "", "", "content", "internal_only").is_err());
    assert!(run_manual_import(&db, "", "t", " ", "internal_only").is_err());
    assert!(run_manual_import(&db, "", "t", "c", "secret").is_err());

    let count = run_manual_import(
        &db,
        "  ",
        "Escalation guide",
        "First, stay calm.",
        "customer_safe",
    )
    .expect("manual import succeeds");
    assert_eq!(count, 1);

    let documents = db.with_read(|conn| spp_core::knowledge_store::list_documents(conn, None));
    assert_eq!(documents.len(), 1);
    assert_eq!(str_field(&documents[0], "title"), "Escalation guide");
    assert_eq!(str_field(&documents[0], "visibility"), "customer_safe");
    let sources = db.with_read(spp_core::knowledge_store::list_sources);
    let names: Vec<String> = sources.iter().map(|s| str_field(s, "name")).collect();
    assert!(names.iter().any(|n| n == "Manual import"), "{names:?}");
    let _ = dir;
}

#[test]
fn import_panel_lists_only_importable_files() {
    let dir = temp_data_dir("listing");
    write_import_file(&dir, "alpha.md", "a");
    write_import_file(&dir, "zeta.txt", "z");
    std::fs::write(dir.join("knowledge-import").join("virus.exe"), b"x").expect("seed");

    let mut page = KnowledgePage::new();
    page.set_data_dir(dir.path().to_path_buf());
    page.reload_importable();

    let files = page.import.files.clone().expect("files loaded");
    assert_eq!(files, vec!["alpha.md".to_string(), "zeta.txt".to_string()]);
    assert!(
        page.import
            .import_dir
            .as_deref()
            .is_some_and(|d| d.contains("knowledge-import"))
    );
}

#[test]
fn import_file_action_updates_page_state() {
    let db = empty_db();
    let dir = temp_data_dir("action");
    write_import_file(&dir, "note.md", "hello");

    let mut page = KnowledgePage::new();
    page.set_data_dir(dir.path().to_path_buf());
    page.reload_all(&db);
    assert!(page.documents.is_empty());

    page.import_file_action(&db, "note.md");
    assert_eq!(
        page.status_message.as_deref(),
        Some("Imported 1 document(s) from file.")
    );
    assert!(page.error.is_none());
    page.reload_all(&db);
    assert_eq!(page.documents.len(), 1);

    page.import_file_action(&db, "../escape.md");
    assert!(page.error.is_some_and(|e| e.contains("knowledge-import")));
}

#[test]
fn ensure_loaded_latches_and_invalidate_refetches() {
    let db = empty_db();
    let mut page = KnowledgePage::new();
    assert!(!page.loaded);
    page.ensure_loaded(&db);
    assert!(page.loaded);
    assert!(page.documents.is_empty(), "no documents seeded yet");
    assert!(page.sources.is_empty(), "a legitimately empty mirror");

    page.error = Some("sentinel".into());
    page.ensure_loaded(&db);
    assert_eq!(
        page.error.as_deref(),
        Some("sentinel"),
        "latch holds: no reload ran (pre-fix documents_ui and sources_ui each re-ran reload_all every frame on an empty mirror)"
    );

    db.with_write(|conn| {
        let source = spp_core::knowledge_store::get_or_create_source(
            conn,
            "Latch source",
            "manual",
            "internal_only",
        )
        .expect("source");
        spp_core::knowledge_store::upsert_document(
            conn,
            source,
            "Latch doc",
            "Latch body",
            "internal_only",
            "markdown",
        )
        .expect("document");
    });
    page.invalidate();
    assert!(!page.loaded);
    page.ensure_loaded(&db);
    assert!(page.loaded);
    assert_eq!(page.documents.len(), 1, "invalidate refetches");
}
