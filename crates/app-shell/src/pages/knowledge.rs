use std::path::{Path, PathBuf};

use eframe::egui;
use rusqlite::Connection;
use serde_json::Value;
use spp_core::db::DbHandle;
use spp_core::sync_engine::SyncEngine;

use super::{ConfirmRequest, NavRequest, i64_field, str_field};
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

const VISIBILITIES: [(&str, &str); 2] = [
    ("internal_only", "internal-only (never in customer drafts)"),
    (
        "customer_safe",
        "customer-safe (may support customer drafts)",
    ),
];

pub(crate) fn run_file_import(
    db: &DbHandle,
    data_dir: &Path,
    file: &str,
) -> spp_core::Result<usize> {
    db.with_write(|conn| {
        let results =
            spp_core::knowledge_store::import_file(conn, data_dir, file, None, "internal_only")?;
        post_import_side_effects(conn, &results, "file");
        Ok(results.len())
    })
}

pub(crate) fn run_manual_import(
    db: &DbHandle,
    source_name: &str,
    title: &str,
    content: &str,
    visibility: &str,
) -> spp_core::Result<usize> {
    if title.trim().is_empty() || content.trim().is_empty() {
        return Err(spp_core::error::Error::Validation(
            "A title and content are required.".to_string(),
        ));
    }
    if !VISIBILITIES.iter().any(|(v, _)| *v == visibility) {
        return Err(spp_core::error::Error::Validation(format!(
            "visibility must be {} or {}.",
            VISIBILITIES[0].0, VISIBILITIES[1].0
        )));
    }
    let source = if source_name.trim().is_empty() {
        "Manual import"
    } else {
        source_name.trim()
    };
    let documents = vec![serde_json::json!({
        "title": title,
        "content": content,
        "format": "markdown",
    })];
    db.with_write(|conn| {
        let results =
            spp_core::knowledge_store::import_manual(conn, source, &documents, visibility)?;
        post_import_side_effects(conn, &results, "manual");
        Ok(results.len())
    })
}

fn post_import_side_effects(conn: &Connection, results: &[Value], mode: &str) {
    let _ = spp_core::jobs::enqueue_on(conn, "embeddings", "embed_knowledge_chunks", "{}", 4, 2);
    let after = serde_json::json!({
        "imported": results.len(),
        "mode": mode,
    });
    let _ = spp_core::jobs::audit_entry(
        conn,
        "user",
        "knowledge_imported",
        None,
        None,
        Some(&after.to_string()),
        None,
        None,
        false,
        None,
        None,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Documents,
    Sources,
    Freshness,
    HelpCenter,
    Drafts,
}

impl Tab {
    const ALL: [Self; 5] = [
        Self::Documents,
        Self::Sources,
        Self::Freshness,
        Self::HelpCenter,
        Self::Drafts,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Documents => "Documents",
            Self::Sources => "Sources",
            Self::Freshness => "Freshness",
            Self::HelpCenter => "Help center",
            Self::Drafts => "Article drafts",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DocsStats {
    pub collections: i64,
    pub articles: i64,
    pub published: i64,
    pub draft: i64,
    pub internal: i64,
    pub views: i64,
    pub last_synced: Option<String>,
}

#[must_use]
pub(crate) fn docs_stats(conn: &Connection) -> DocsStats {
    conn.query_row(
        "SELECT (SELECT COUNT(*) FROM docs_collections),
                (SELECT COUNT(*) FROM docs_articles),
                (SELECT COUNT(*) FROM docs_articles WHERE status = 'published'),
                (SELECT COUNT(*) FROM docs_articles WHERE status = 'draft'),
                (SELECT COUNT(*) FROM docs_articles WHERE status = 'internal'),
                (SELECT COALESCE(SUM(views), 0) FROM docs_articles),
                (SELECT MAX(remote_updated_at) FROM docs_articles)",
        [],
        |r| {
            Ok(DocsStats {
                collections: r.get(0)?,
                articles: r.get(1)?,
                published: r.get(2)?,
                draft: r.get(3)?,
                internal: r.get(4)?,
                views: r.get(5)?,
                last_synced: r.get(6)?,
            })
        },
    )
    .unwrap_or_default()
}

#[must_use]
pub fn docs_collections(conn: &Connection) -> Vec<(i64, String)> {
    conn.prepare("SELECT id, name FROM docs_collections ORDER BY name")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DocArticle {
    pub id: i64,
    pub collection_name: Option<String>,
    pub name: String,
    pub status: String,
    pub preview: String,
    pub words: i64,
    pub views: i64,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DocsFilter {
    pub collection: Option<i64>,
    pub status: Option<String>,
    pub query: Option<String>,
}

const ARTICLE_SELECT: &str = "SELECT a.id,
       c.name, a.name, COALESCE(a.status, 'draft'), a.preview,
       COALESCE(a.words, 0), COALESCE(a.views, 0), a.remote_updated_at
  FROM docs_articles a
  LEFT JOIN docs_collections c ON c.id = a.collection_local_id";

fn article_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<DocArticle> {
    Ok(DocArticle {
        id: r.get(0)?,
        collection_name: r.get(1)?,
        name: r.get(2)?,
        status: r.get(3)?,
        preview: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
        words: r.get(5)?,
        views: r.get(6)?,
        updated_at: r.get(7)?,
    })
}

#[must_use]
pub(crate) fn fts_article_ids(conn: &Connection, q: &str) -> Vec<i64> {
    let tokens = spp_core::search::tokenize(q, 2, 8);
    if tokens.is_empty() {
        return Vec::new();
    }
    let pattern = tokens
        .iter()
        .map(|t| format!("\"{t}\"*"))
        .collect::<Vec<_>>()
        .join(" ");
    conn.prepare("SELECT article_id FROM docs_fts WHERE docs_fts MATCH ?1 ORDER BY rank LIMIT 40")
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![pattern], |r| r.get::<_, i64>(0))
                .map(|rows| {
                    let mut seen = std::collections::HashSet::new();
                    rows.filter_map(|r| r.ok())
                        .filter(|id| seen.insert(*id))
                        .collect()
                })
        })
        .unwrap_or_default()
}

#[must_use]
pub(crate) fn list_articles(conn: &Connection, filter: &DocsFilter) -> Vec<DocArticle> {
    let fts_ids = filter
        .query
        .as_deref()
        .filter(|q| !q.trim().is_empty())
        .map(|q| fts_article_ids(conn, q))
        .unwrap_or_default();
    if filter
        .query
        .as_deref()
        .is_some_and(|q| !q.trim().is_empty())
        && fts_ids.is_empty()
    {
        return Vec::new();
    }

    let mut where_parts: Vec<String> = Vec::new();
    let mut binds: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(collection) = filter.collection {
        where_parts.push(format!("a.collection_local_id = ?{}", binds.len() + 1));
        binds.push(collection.into());
    }
    if let Some(status) = &filter.status {
        where_parts.push(format!("a.status = ?{}", binds.len() + 1));
        binds.push(status.clone().into());
    }
    if !fts_ids.is_empty() {
        let list = fts_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        where_parts.push(format!("a.id IN ({list})"));
    }
    let where_sql = if where_parts.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_parts.join(" AND "))
    };
    let sql = format!("{ARTICLE_SELECT} {where_sql} ORDER BY a.name LIMIT 200");
    conn.prepare(&sql)
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params_from_iter(binds), article_from_row)
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

#[must_use]
pub(crate) fn article_detail(conn: &Connection, id: i64) -> Option<(DocArticle, String)> {
    let sql = format!("{ARTICLE_SELECT} WHERE a.id = ?1");
    let summary = conn
        .query_row(&sql, rusqlite::params![id], article_from_row)
        .ok()?;
    let text: Option<String> = conn
        .query_row(
            "SELECT text FROM docs_articles WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    Some((summary, text.unwrap_or_default()))
}

#[derive(Debug, Default)]
pub(crate) struct ImportPanel {
    title: String,
    content: String,
    source_name: String,
    visibility: String,
    files: Option<Vec<String>>,
    import_dir: Option<String>,
}

impl ImportPanel {
    fn new() -> Self {
        Self {
            visibility: "internal_only".to_string(),
            ..Self::default()
        }
    }
}

/// AUD-P2-15 (C11): the local-document edit form state — prefilled from
/// `get_document` (title, visibility, full content; the list rows only
/// carry a 200-char preview), saved through `update_document`.
#[derive(Debug, Clone)]
struct DocEditPanel {
    id: i64,
    title: String,
    content: String,
    visibility: String,
    /// Set when the stored visibility is not one of the two pickable
    /// values and the form fell back to a pickable one — the edit card
    /// carries the note so the fallback is never silent.
    visibility_note: Option<String>,
}

pub struct KnowledgePage {
    tab: Tab,
    documents: Vec<Value>,
    loaded: bool,
    sources: Vec<Value>,
    freshness: Option<Value>,
    stats: DocsStats,
    collections: Vec<(i64, String)>,
    articles: Vec<DocArticle>,
    filter: DocsFilter,
    search_edit: String,
    collection_pick: Option<i64>,
    status_pick: Option<String>,
    reader: Option<i64>,
    reader_text: Option<(DocArticle, String)>,
    /// AUD-P2-15 (C11): the open document record for the Documents-tab
    /// reader (`get_document` payload — full content plus the chunk
    /// counts), replacing the bare list as the record view.
    doc_reader: Option<Value>,
    /// AUD-P2-15 (C11): deep-link latch — the shell's `go_to_documents`
    /// call carries no db handle, so the id waits here and documents_ui
    /// resolves it into `doc_reader` on the next render.
    pending_doc: Option<i64>,
    /// AUD-P2-15 (C11): the in-place document edit form (the connectors
    /// edit idiom), riding above the list or the reader.
    doc_edit: Option<DocEditPanel>,
    status_message: Option<String>,
    error: Option<String>,
    data_dir: Option<PathBuf>,
    import: ImportPanel,
    drafts: Vec<spp_core::knowledge_store::ArticleDraftRow>,
    gaps: Vec<(i64, String)>,
    draft_pick: Option<i64>,
    gap_pick: Option<i64>,
    publish_collection_pick: Option<i64>,
    query_gaps: Vec<(String, i64)>,
    stale_docs_tracked: u32,
    /// UX-18: one-shot flag — the empty-state CTA force-expands the import
    /// disclosure on its next render. The user's own toggles live in egui's
    /// persistent state and are never overridden.
    import_force_open: bool,
}

impl KnowledgePage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::Documents,
            documents: Vec::new(),
            loaded: false,
            sources: Vec::new(),
            freshness: None,
            stats: DocsStats::default(),
            collections: Vec::new(),
            articles: Vec::new(),
            filter: DocsFilter::default(),
            search_edit: String::new(),
            collection_pick: None,
            status_pick: None,
            reader: None,
            reader_text: None,
            doc_reader: None,
            pending_doc: None,
            doc_edit: None,
            status_message: None,
            error: None,
            data_dir: None,
            import: ImportPanel::new(),
            drafts: Vec::new(),
            gaps: Vec::new(),
            draft_pick: None,
            gap_pick: None,
            publish_collection_pick: None,
            query_gaps: Vec::new(),
            stale_docs_tracked: 0,
            import_force_open: false,
        }
    }

    pub(crate) fn set_data_dir(&mut self, data_dir: PathBuf) {
        self.data_dir = Some(data_dir);
    }

    fn reload_importable(&mut self) {
        let Some(dir) = self.data_dir.clone() else {
            return;
        };
        let v = spp_core::knowledge_store::importable_files(&dir);
        self.import.files = v.get("files").and_then(Value::as_array).map(|a| {
            a.iter()
                .filter_map(|f| f.as_str().map(str::to_string))
                .collect()
        });
        self.import.import_dir = v.get("dir").and_then(Value::as_str).map(str::to_string);
    }

    pub(crate) fn import_file_action(&mut self, db: &DbHandle, file: &str) {
        let Some(dir) = self.data_dir.clone() else {
            self.error = Some("Import is unavailable (no data directory).".to_string());
            return;
        };
        match run_file_import(db, &dir, file) {
            Ok(n) => {
                self.status_message = Some(format!("Imported {n} document(s) from file."));
                // AU2-5: refetch instead of clearing under a true `loaded` latch
                // (the table would otherwise blank until the next bus event).
                self.reload_all(db);
            }
            Err(e) => self.error = Some(format!("Import failed: {e}")),
        }
        self.reload_importable();
    }

    pub(crate) fn manual_import_action(&mut self, db: &DbHandle) {
        match run_manual_import(
            db,
            &self.import.source_name,
            &self.import.title,
            &self.import.content,
            &self.import.visibility,
        ) {
            Ok(n) => {
                self.status_message = Some(format!("Imported {n} document(s)."));
                self.import.title.clear();
                self.import.content.clear();
                // AU2-5: refetch instead of clearing under a true `loaded` latch.
                self.reload_all(db);
            }
            Err(e) => self.error = Some(format!("Import failed: {e}")),
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.loaded = false;
        self.documents.clear();
        self.sources.clear();
        self.freshness = None;
        self.articles.clear();
        self.stats = DocsStats::default();
        self.drafts.clear();
        self.gaps.clear();
    }

    fn reload_all(&mut self, db: &DbHandle) {
        let (documents, sources, freshness, stats, collections, articles) = db.with_read(|conn| {
            (
                spp_core::knowledge_store::list_documents(conn, None),
                spp_core::knowledge_store::list_sources(conn),
                Some(spp_core::knowledge_store::freshness_report(conn)),
                docs_stats(conn),
                docs_collections(conn),
                list_articles(conn, &self.filter),
            )
        });
        self.documents = documents;
        self.sources = sources;
        self.freshness = freshness;
        self.stats = stats;
        self.collections = collections;
        self.articles = articles;
        self.loaded = true;
    }

    fn reload_docs(&mut self, db: &DbHandle) {
        let (articles, query_gaps, stale) = db.with_read(|conn| {
            (
                list_articles(conn, &self.filter),
                spp_core::intelligence_features::list_knowledge_gaps(conn, 10).unwrap_or_default(),
                spp_core::intelligence_features::count_stale_docs(conn).unwrap_or(0),
            )
        });
        self.articles = articles;
        self.query_gaps = query_gaps;
        self.stale_docs_tracked = stale;
    }

    pub(crate) fn mark_reviewed(&mut self, db: &DbHandle, id: i64) {
        // AU2-5: this is an UPDATE — it belongs on the serialized write lane,
        // not on a pooled reader connection (SQLITE_BUSY-prone otherwise).
        let ok = db.with_write(|conn| spp_core::knowledge_store::mark_reviewed(conn, id));
        self.status_message = Some(match ok {
            Ok(true) => format!("Document {id} marked reviewed."),
            Ok(false) => format!("Document {id} not found."),
            Err(e) => format!("Could not mark reviewed: {e}"),
        });
        // AU2-5: explicit refetch — clearing the vectors while `loaded` stays
        // true blanked the table until the next unrelated bus event.
        self.reload_all(db);
    }

    pub(crate) fn mark_verified(&mut self, db: &DbHandle, id: i64) {
        // AU2-5: write lane (see mark_reviewed).
        let ok = db.with_write(|conn| spp_core::knowledge_store::mark_verified(conn, id));
        self.status_message = Some(match ok {
            Ok(true) => format!("Document {id} marked verified."),
            Ok(false) => format!("Document {id} not found."),
            Err(e) => format!("Could not mark verified: {e}"),
        });
        self.reload_all(db);
    }

    pub(crate) fn delete_document(&mut self, db: &DbHandle, id: i64) -> spp_core::Result<()> {
        db.with_write(|conn| {
            let removed = spp_core::knowledge_store::delete_document(conn, id)?;
            if removed {
                let _ = spp_core::jobs::enqueue_on(
                    conn,
                    "embeddings",
                    "embed_knowledge_chunks",
                    "{}",
                    4,
                    2,
                );
            }
            Ok::<_, spp_core::error::Error>(removed)
        })?;
        self.status_message = Some(format!("Document {id} deleted."));
        self.reload_all(db);
        Ok(())
    }

    /// AUD-P2-15 (C11): per-document re-ingest — the scoped re-embed the
    /// audit found missing (delete + re-import was the only refresh path,
    /// and the Settings re-index resets the WHOLE corpus). Resets this
    /// document's chunks — the permanently-excluded 5-attempt rows too —
    /// and re-queues the embedding pass.
    pub(crate) fn reindex_document_action(&mut self, db: &DbHandle, id: i64) {
        match db.with_write(|conn| spp_core::knowledge_store::reindex_document(conn, id)) {
            Ok(0) => {
                self.status_message = Some(format!("Document {id} has no chunks to re-index yet."));
            }
            Ok(n) => {
                self.status_message = Some(format!("{n} chunks re-queued for embedding."));
            }
            Err(e) => self.error = Some(format!("Could not re-index document {id}: {e}")),
        }
        self.refresh_doc_reader(db);
        self.reload_all(db);
    }

    /// AUD-P2-15 (C11): refresh the open reader record in place (its
    /// chunk counts move after a re-index or a save).
    fn refresh_doc_reader(&mut self, db: &DbHandle) {
        let Some(id) = self.doc_reader.as_ref().map(|d| i64_field(d, "id")) else {
            return;
        };
        self.doc_reader = db.with_read(|conn| spp_core::knowledge_store::get_document(conn, id));
    }

    /// AUD-P2-15 (C11): open the edit form prefilled from the stored
    /// record (title, visibility, full content). A stored visibility
    /// outside the two pickable values falls back to a pickable one with
    /// a note in the card — the chips can only pick the two.
    fn start_doc_edit(&mut self, db: &DbHandle, id: i64) {
        let Some(record) = db.with_read(|conn| spp_core::knowledge_store::get_document(conn, id))
        else {
            self.error = Some(format!("Document {id} was not found."));
            return;
        };
        let stored = str_field(&record, "visibility");
        let (visibility, visibility_note) = if VISIBILITIES.iter().any(|(v, _)| *v == stored) {
            (stored, None)
        } else {
            (
                VISIBILITIES[0].0.to_string(),
                Some(format!(
                    "the stored visibility “{stored}” is not one of the pickable values — \
                     saving applies the selected one"
                )),
            )
        };
        self.doc_edit = Some(DocEditPanel {
            id,
            title: str_field(&record, "title"),
            content: str_field(&record, "content"),
            visibility,
            visibility_note,
        });
        self.error = None;
    }

    /// AUD-P2-15 (C11): save the edit form through `update_document` —
    /// re-chunks and refreshes the FTS index in the same write. The
    /// upsert inside keys on (source_id, title), so a RENAME is applied
    /// to the row first; otherwise the save would insert a sibling
    /// document under the new title instead of editing this one. A
    /// visibility-only edit rides the upsert's no-change early return,
    /// so it is applied directly (FTS rows kept in step).
    fn save_doc_edit(&mut self, db: &DbHandle) {
        let Some(edit) = self.doc_edit.clone() else {
            return;
        };
        let title = edit.title.trim().to_string();
        if title.is_empty() {
            self.error = Some("A title is required.".to_string());
            return;
        }
        if !VISIBILITIES.iter().any(|(v, _)| *v == edit.visibility) {
            self.error = Some(format!(
                "Visibility must be {} or {}.",
                VISIBILITIES[0].0, VISIBILITIES[1].0
            ));
            return;
        }
        let result = db.with_write(|conn| {
            let record =
                spp_core::knowledge_store::get_document(conn, edit.id).ok_or_else(|| {
                    spp_core::error::Error::Validation("document not found".to_string())
                })?;
            if str_field(&record, "title") != title {
                conn.execute(
                    "UPDATE knowledge_documents SET title = ?1, updated_at = datetime('now')
                      WHERE id = ?2",
                    rusqlite::params![title, edit.id],
                )?;
                conn.execute(
                    "UPDATE fts_knowledge SET title = ?1 WHERE document_id = ?2",
                    rusqlite::params![title, edit.id],
                )?;
            }
            let (version, changed) = spp_core::knowledge_store::update_document(
                conn,
                edit.id,
                &title,
                &edit.content,
                &edit.visibility,
            )?;
            if str_field(&record, "visibility") != edit.visibility {
                conn.execute(
                    "UPDATE knowledge_documents
                        SET visibility = ?1, updated_at = datetime('now') WHERE id = ?2",
                    rusqlite::params![edit.visibility, edit.id],
                )?;
                conn.execute(
                    "UPDATE fts_knowledge SET visibility = ?1 WHERE document_id = ?2",
                    rusqlite::params![edit.visibility, edit.id],
                )?;
            }
            Ok::<_, spp_core::error::Error>((version, changed))
        });
        match result {
            Ok((version, changed)) => {
                self.doc_edit = None;
                self.status_message = Some(if changed {
                    format!(
                        "Document saved — version {version}, chunks and search index refreshed."
                    )
                } else {
                    format!("Document saved — no content change, version stays at {version}.")
                });
                self.refresh_doc_reader(db);
                self.reload_all(db);
            }
            Err(e) => self.error = Some(format!("Could not save the document: {e}")),
        }
    }

    fn import_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let mut file_action: Option<String> = None;
        let mut manual_submit = false;
        let mut refresh = false;
        // UX-18: the disclosure starts expanded on a first-run empty
        // Documents tab (default_open only seeds egui's persistent state,
        // so later user toggles always win), and the empty-state CTA can
        // force it back open exactly once per click.
        let header_id = ui.make_persistent_id(egui::IdSalt::new("Import documents"));
        if self.import_force_open {
            let mut state =
                egui::containers::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    header_id,
                    self.documents.is_empty(),
                );
            state.set_open(true);
            state.store(ui.ctx());
            self.import_force_open = false;
        }
        egui::CollapsingHeader::new("Import documents")
            .default_open(self.documents.is_empty())
            .show(ui, |ui| {
                if self.import.files.is_none() {
                    self.reload_importable();
                }
                design::section_title(ui, "Import from the knowledge-import folder");
                let dir = self
                    .import
                    .import_dir
                    .clone()
                    .unwrap_or_else(|| "<data dir>/knowledge-import".to_string());
                design::hint_text(
                    ui,
                    &format!(
                        "Copy documents into {dir} (supported: .md .txt .csv .json .html .pdf .docx). \
                         For safety, file imports must live inside that folder."
                    ),
                );
                if design::button_with_icon(ui, Icon::Refresh, "Refresh", ButtonVariant::Outline)
                    .clicked()
                {
                    refresh = true;
                }
                let files = self.import.files.clone().unwrap_or_default();
                if files.is_empty() {
                    design::hint_text(ui, "No importable files in the folder");
                }
                for file in files {
                    ui.horizontal(|ui| {
                        ui.monospace(
                            egui::RichText::new(file.clone())
                                .color(design::tokens(ui).foreground),
                        );
                        if design::button(ui, "Import", ButtonVariant::Primary).clicked() {
                            file_action = Some(file);
                        }
                    });
                }
                design::divider(ui);
                design::section_title(ui, "Paste a document");
                design::form_grid(ui, "knowledge-import-form", |ui| {
                    let title_label = design::field_label(ui, "Title");
                    ui.add_sized(
                        [260.0, 28.0],
                        egui::TextEdit::singleline(&mut self.import.title),
                    )
                    .labelled_by(title_label.id);
                    ui.end_row();
                    design::field_label(ui, "Visibility");
                    ui.horizontal(|ui| {
                        for (value, label) in VISIBILITIES {
                            let selected = self.import.visibility == value;
                            if design::chip(ui, selected, label).clicked() {
                                self.import.visibility = value.to_string();
                            }
                        }
                    });
                    ui.end_row();
                    let source_label = design::field_label(ui, "Source name");
                    ui.add_sized(
                        [260.0, 28.0],
                        egui::TextEdit::singleline(&mut self.import.source_name),
                    )
                    .labelled_by(source_label.id);
                    ui.end_row();
                    let content_label = design::field_label(ui, "Content (Markdown)");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.import.content)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    )
                    .labelled_by(content_label.id);
                    ui.end_row();
                });
                let ready =
                    !self.import.title.trim().is_empty() && !self.import.content.trim().is_empty();
                if ready {
                    if design::button(ui, "Import document", ButtonVariant::Primary).clicked() {
                        manual_submit = true;
                    }
                } else {
                    let _ = ui.add_enabled(false, egui::Button::new("Import document"));
                }
            });
        if refresh {
            self.reload_importable();
        }
        if let Some(file) = file_action {
            self.import_file_action(db, &file);
        }
        if manual_submit {
            self.manual_import_action(db);
        }
    }

    pub(crate) fn open_article(&mut self, db: &DbHandle, id: i64) {
        self.reader = Some(id);
        self.reader_text = db.with_read(|conn| article_detail(conn, id));
    }

    /// Deep link `/knowledge?doc={id}`: land on the Documents tab with that
    /// document's reader open — AUD-P2-15 (C11): the id used to be discarded
    /// here (the old comment admitted there was no per-document reader on
    /// that tab). The shell calls this without a db handle, so the id
    /// latches and `documents_ui` resolves it into the reader on the next
    /// render.
    pub(crate) fn go_to_documents(&mut self, id: i64) {
        self.tab = Tab::Documents;
        self.pending_doc = Some(id);
        self.doc_reader = None;
        self.doc_edit = None;
        self.reader = None;
        self.reader_text = None;
    }

    /// AUD-P2-15 (C11): open the per-document reader on the Documents tab
    /// — the row "Read" act and the palette deep link both land here.
    /// Loads the full record; the list rows only carry a preview.
    pub(crate) fn open_document(&mut self, db: &DbHandle, id: i64) {
        self.doc_reader = db.with_read(|conn| spp_core::knowledge_store::get_document(conn, id));
        self.doc_edit = None;
    }

    /// AUD-P2-15 (C11): the reader's back act — return to the list (the
    /// list stays loaded behind the reader).
    pub(crate) fn close_document(&mut self) {
        self.doc_reader = None;
    }

    /// Deep link `/knowledge/help-center/{id}`: land on the Help Center tab with
    /// the article reader open.
    pub(crate) fn open_help_article(&mut self, db: &DbHandle, id: i64) {
        self.tab = Tab::HelpCenter;
        self.open_article(db, id);
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        engine: Option<&std::sync::Arc<SyncEngine>>,
    ) -> Option<NavRequest> {
        design::page_header(
            ui,
            "Knowledge",
            Some("Documents, sources, freshness, and the Help Center"),
            |ui| {
                if design::button_with_icon(
                    ui,
                    Icon::Refresh,
                    "Sync Help Center now",
                    ButtonVariant::Outline,
                )
                .on_hover_text("pull Help Scout Docs collections and articles (v1 API)")
                .clicked()
                    && let Some(engine) = engine
                {
                    crate::pages::sync_health::spawn_docs_sync(db, engine);
                    self.status_message = Some("Help Center sync started…".into());
                }
            },
        );
        let docs_key_missing = db.with_read(|conn| {
            spp_core::settings::get_secret(conn, spp_core::helpscout_real::SECRET_DOCS_API_KEY)
                .ok()
                .flatten()
                .is_none_or(|v| v.is_empty())
        }) && std::env::var("HELPSCOUT_DOCS_API_KEY")
            .map(|v| v.trim().is_empty())
            .unwrap_or(true);
        if docs_key_missing {
            design::hint_text(
                ui,
                "Help Center sync needs a Docs API key (Help Scout Profile → Security & Access \
                 → Docs API). Add it in Settings → Help Scout, then press \"Sync Help Center now\".",
            );
        }
        let labels: Vec<&str> = Tab::ALL.iter().map(|t| t.label()).collect();
        if let Some(i) = design::tabs(
            ui,
            &labels,
            Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0),
        ) && let Some(tab) = Tab::ALL.get(i)
        {
            self.tab = *tab;
        }
        if let Some(msg) = &self.status_message {
            design::success_text(ui, msg);
        }
        if let Some(err) = &self.error {
            design::error_text(ui, err);
        }

        match self.tab {
            Tab::Documents => self.documents_ui(ui, db),
            Tab::Sources => {
                self.sources_ui(ui, db);
                None
            }
            Tab::Freshness => {
                self.freshness_ui(ui, db);
                None
            }
            Tab::HelpCenter => {
                self.help_center_ui(ui, db);
                None
            }
            Tab::Drafts => {
                self.drafts_ui(ui, db);
                None
            }
        }
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if !self.loaded {
            self.reload_all(db);
        }
    }

    fn documents_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        self.ensure_loaded(db);
        // AUD-P2-15 (C11): resolve the deep-link latch — `go_to_documents`
        // carries no db handle, so the id waits for this render to open
        // the reader (an unknown id falls through to the list, as before).
        if let Some(id) = self.pending_doc.take() {
            self.open_document(db, id);
        }
        // AUD-P2-15 (C11): the edit card rides above whichever surface is
        // below it — the list, or the reader it was opened from.
        if self.doc_edit.is_some() {
            self.doc_edit_ui(ui, db);
            ui.add_space(8.0);
        }
        // AUD-P2-15 (C11): the per-document reader replaces the list — the
        // same UXH-1 reading surface the Help Center reader rides.
        if self.doc_reader.is_some() {
            self.doc_reader_ui(ui, db);
            return None;
        }
        design::hint_text(
            ui,
            "Local knowledge documents used by search and AI answers",
        );
        self.import_ui(ui, db);
        ui.add_space(8.0);
        if self.documents.is_empty() {
            // UX-18: the empty state carries the import action instead of
            // pointing at a collapsed disclosure 70px above it.
            if design::empty_state_cta(
                ui,
                Icon::BookOpen,
                "No documents yet",
                "Import a file, paste text, or sync the Help Center — imported \
                 documents power search and AI answers.",
                "Import Documents",
            ) {
                self.import_force_open = true;
            }
            return None;
        }
        let mut action: Option<(i64, String, &str)> = None;
        // AUD-P2-15 (C11): the row acts — Read (the reader), Edit (the
        // in-place form), Re-index (the scoped re-embed) — joined the
        // review lifecycle buttons. The cell wraps so six acts stay
        // inside the Actions column at every width.
        let mut read: Option<i64> = None;
        let mut edit: Option<i64> = None;
        let mut reindex: Option<i64> = None;
        // UXG-1-4: exact-column data_table (virtualized rows built in) —
        // the document list spans the content column at every width.
        let documents = &self.documents;
        design::data_table(
            ui,
            "knowledge-documents",
            &["Title", "Source", "Visibility", "Chunks", "Actions"],
            documents.len(),
            26.0,
            |ui, i| {
                let t = design::tokens(ui);
                let doc = &documents[i];
                let id = i64_field(doc, "id");
                ui.label(
                    egui::RichText::new(str_field(doc, "title"))
                        .size(13.0)
                        .color(t.foreground),
                );
                ui.label(
                    egui::RichText::new(format!("#{}", i64_field(doc, "source_id")))
                        .size(13.0)
                        .color(t.foreground),
                );
                ui.label(
                    egui::RichText::new(str_field(doc, "visibility"))
                        .size(13.0)
                        .color(t.foreground),
                );
                design::td_num(ui, &i64_field(doc, "chunk_count").to_string());
                ui.horizontal_wrapped(|ui| {
                    if design::button(ui, "Read", ButtonVariant::Outline)
                        .on_hover_text("open this document in the reader")
                        .clicked()
                    {
                        read = Some(id);
                    }
                    if design::button(ui, "Edit", ButtonVariant::Outline)
                        .on_hover_text("edit title, visibility and content in place")
                        .clicked()
                    {
                        edit = Some(id);
                    }
                    if design::button(ui, "Re-index", ButtonVariant::Outline)
                        .on_hover_text("re-embed this document's chunks — resets failed attempts")
                        .clicked()
                    {
                        reindex = Some(id);
                    }
                    if design::button(ui, "Mark Reviewed", ButtonVariant::Outline).clicked() {
                        action = Some((id, str_field(doc, "title"), "reviewed"));
                    }
                    if design::button(ui, "Verify", ButtonVariant::Outline).clicked() {
                        action = Some((id, str_field(doc, "title"), "verify"));
                    }
                    if design::button(ui, "Delete", ButtonVariant::Destructive).clicked() {
                        action = Some((id, str_field(doc, "title"), "delete"));
                    }
                });
            },
        );
        if let Some(id) = read {
            self.open_document(db, id);
        }
        if let Some(id) = edit {
            self.start_doc_edit(db, id);
        }
        if let Some(id) = reindex {
            self.reindex_document_action(db, id);
        }
        if let Some((id, title, what)) = action {
            match what {
                "reviewed" => self.mark_reviewed(db, id),
                "verify" => self.mark_verified(db, id),
                _ => {
                    return Some(NavRequest::Confirm(ConfirmRequest::delete_knowledge_doc(
                        id, &title,
                    )));
                }
            }
        }
        None
    }

    fn sources_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);
        if self.sources.is_empty() {
            design::empty_state(
                ui,
                Icon::BookOpen,
                "No sources yet",
                "Sources appear when documents are imported.",
            );
            return;
        }
        // UXG-1-4: exact-column data_table in a full-width titled card —
        // the sources list spans the card at every width.
        let sources = &self.sources;
        design::card_titled_wide(ui, "Sources", None, |ui| {
            design::data_table(
                ui,
                "knowledge-sources",
                &["Name", "Kind", "Visibility", "Documents", "Created"],
                sources.len(),
                26.0,
                |ui, i| {
                    let t = design::tokens(ui);
                    let source = &sources[i];
                    ui.label(
                        egui::RichText::new(str_field(source, "name"))
                            .size(13.0)
                            .color(t.foreground),
                    );
                    ui.label(
                        egui::RichText::new(str_field(source, "kind"))
                            .size(13.0)
                            .color(t.foreground),
                    );
                    ui.label(
                        egui::RichText::new(str_field(source, "visibility"))
                            .size(13.0)
                            .color(t.foreground),
                    );
                    design::td_num(ui, &i64_field(source, "document_count").to_string());
                    ui.label(
                        egui::RichText::new(str_field(source, "created_at"))
                            .size(13.0)
                            .color(t.muted_foreground),
                    );
                },
            );
        });
    }

    fn reload_drafts(&mut self, db: &DbHandle) {
        let (drafts, gaps) = db.with_read(|conn| {
            let gaps: Vec<(i64, String)> = conn
                .prepare(
                    "SELECT id, query_text FROM knowledge_gap_candidates
                      WHERE status = 'open' ORDER BY occurrence_count DESC, id DESC LIMIT 50",
                )
                .and_then(|mut stmt| {
                    stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                        .map(|rows| rows.filter_map(|r| r.ok()).collect())
                })
                .unwrap_or_default();
            (
                spp_core::knowledge_store::list_article_drafts(conn, None),
                gaps,
            )
        });
        self.drafts = drafts;
        self.gaps = gaps;
    }

    fn drafts_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.drafts.is_empty() && self.gaps.is_empty() {
            self.reload_drafts(db);
        }
        design::hint_text(
            ui,
            "AI drafts knowledge articles from real resolutions; a human review is the only \
             path to publishing. Approved drafts publish to Help Scout through the Docs API.",
        );
        ui.add_space(8.0);

        if !self.gaps.is_empty() {
            design::card_titled(ui, "Draft an article from a gap", None, |ui| {
                let gap_label = self
                    .gap_pick
                    .and_then(|id| {
                        self.gaps
                            .iter()
                            .find(|(gid, _)| *gid == id)
                            .map(|(_, q)| q.chars().take(60).collect::<String>())
                    })
                    .unwrap_or_else(|| "Select a gap…".into());
                egui::ComboBox::from_id_salt("k2-gap-pick")
                    .selected_text(gap_label.clone())
                    .width(ui.available_width() - 140.0)
                    .show_ui(ui, |ui| {
                        for (id, question) in &self.gaps {
                            let label: String = question.chars().take(80).collect();
                            ui.selectable_value(&mut self.gap_pick, Some(*id), label);
                        }
                    })
                    .response
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            ui.is_enabled(),
                            format!("gap: {gap_label}"),
                        )
                    });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(self.gap_pick.is_some(), egui::Button::new("Draft article"))
                        .on_hover_text(
                            "The AI grounds the article in the best resolutions of the gap's \
                             evidence conversations, with citations. It lands here for review.",
                        )
                        .clicked()
                        && let Some(candidate) = self.gap_pick
                    {
                        let payload = format!(r#"{{"candidateId": {candidate}}}"#);
                        let enqueued = db.with_write(|conn| {
                            spp_core::jobs::enqueue_on(conn, "ai", "draft_article", &payload, 3, 2)
                        });
                        match enqueued {
                            Ok(_) => {
                                self.status_message = Some(
                                    "Article draft requested — it appears in the queue \
                                         when the model finishes."
                                        .into(),
                                );
                            }
                            Err(e) => self.error = Some(format!("Could not queue the draft: {e}")),
                        }
                    }
                });
            });
            ui.add_space(10.0);
        }

        if self.drafts.is_empty() {
            design::empty_state(
                ui,
                Icon::BookOpen,
                "no drafts yet",
                "Drafted articles wait here for a human decision.",
            );
            return;
        }

        let mut reloaded = false;
        for draft in &self.drafts {
            let selected = self.draft_pick == Some(draft.id);
            let status_chip = match draft.status.as_str() {
                "approved" => "approved",
                "rejected" => "rejected",
                _ => "awaiting review",
            };
            let publish_chip = match draft.publish_state.as_str() {
                "verified" => "published + verified",
                "published" => "published",
                "draft_upserted" => "draft in Help Scout",
                "publish_failed" => "publish failed",
                _ => "",
            };
            let row = egui::Frame::default()
                .fill(if selected {
                    ui.visuals().faint_bg_color
                } else {
                    egui::Color32::TRANSPARENT
                })
                .inner_margin(egui::Margin::same(6));
            row.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} — {}", draft.id, draft.title)).strong(),
                    );
                    match draft.status.as_str() {
                        "approved" => {
                            design::badge(ui, status_chip, BadgeTone::Success);
                        }
                        "rejected" => {
                            design::badge(ui, status_chip, BadgeTone::Destructive);
                        }
                        _ => {
                            design::badge(ui, status_chip, BadgeTone::Neutral);
                        }
                    }
                    if !publish_chip.is_empty() {
                        let tone = if draft.publish_state == "publish_failed" {
                            BadgeTone::Destructive
                        } else if matches!(draft.publish_state.as_str(), "published" | "verified") {
                            BadgeTone::Success
                        } else {
                            BadgeTone::Info
                        };
                        design::badge(ui, publish_chip, tone);
                    }
                    if design::button(ui, "Open", ButtonVariant::Outline).clicked() {
                        self.draft_pick = Some(draft.id);
                    }
                });
            });
        }
        ui.add_space(8.0);

        let Some(pick) = self.draft_pick else {
            return;
        };
        let Some(draft) = self.drafts.iter().find(|d| d.id == pick).cloned() else {
            return;
        };
        ui.separator();
        design::card_titled(
            ui,
            &draft.title,
            Some("AI-drafted — review before anything else"),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("k2-draft-body")
                    .max_height(320.0)
                    .show(ui, |ui| {
                        ui.label(&draft.body);
                    });
                if let Some(err) = &draft.publish_error {
                    ui.add_space(4.0);
                    design::error_text(ui, err);
                }
                ui.add_space(6.0);
                let citations = draft
                    .citations
                    .as_array()
                    .map(|a| a.to_vec())
                    .unwrap_or_default();
                if !citations.is_empty() {
                    ui.collapsing(
                        egui::RichText::new(format!("Citations ({})", citations.len())).small(),
                        |ui| {
                            for (i, c) in citations.iter().enumerate() {
                                let number = c["number"].as_i64().unwrap_or_default();
                                let subject = c["subject"].as_str().unwrap_or("(no subject)");
                                let excerpt = c["excerpt"].as_str().unwrap_or("");
                                ui.label(
                                    egui::RichText::new(format!(
                                        "[^{}] Conversation #{number} — {subject}",
                                        i + 1
                                    ))
                                    .small()
                                    .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(
                                        excerpt.chars().take(220).collect::<String>(),
                                    )
                                    .small()
                                    .color(ui.visuals().weak_text_color()),
                                );
                                ui.add_space(4.0);
                            }
                        },
                    );
                }
                ui.add_space(6.0);
                if draft.status == "drafted" {
                    ui.horizontal(|ui| {
                        if design::button_with_icon(
                            ui,
                            Icon::Check,
                            "Approve",
                            ButtonVariant::Primary,
                        )
                        .on_hover_text(
                            "Approving unlocks the publish path — it never publishes by itself.",
                        )
                        .clicked()
                        {
                            let decided = db.with_write(|conn| {
                                spp_core::knowledge_store::decide_article_draft(
                                    conn, draft.id, "approved", None,
                                )
                            });
                            match decided {
                                Some(_) => {
                                    self.status_message = Some(
                                        "Draft approved. Publishing needs the explicit publish \
                                     action below."
                                            .into(),
                                    );
                                    reloaded = true;
                                }
                                None => self.error = Some("The draft was already decided.".into()),
                            }
                        }
                        if design::button(ui, "Reject", ButtonVariant::Outline).clicked() {
                            let decided = db.with_write(|conn| {
                                spp_core::knowledge_store::decide_article_draft(
                                    conn, draft.id, "rejected", None,
                                )
                            });
                            match decided {
                                Some(_) => {
                                    self.status_message = Some("Draft rejected.".into());
                                    reloaded = true;
                                }
                                None => self.error = Some("The draft was already decided.".into()),
                            }
                        }
                    });
                } else if draft.status == "approved" {
                    ui.separator();
                    ui.label(egui::RichText::new("Publish to Help Scout (Docs API)").strong());
                    if self.publish_collection_pick.is_none() && !self.collections.is_empty() {
                        self.publish_collection_pick = self.collections.first().map(|(id, _)| *id);
                    }
                    if self.collections.is_empty() {
                        design::hint_text(
                            ui,
                            "No Help Center collections are synced yet - sync the Help Center \
                         (top right) or configure a Docs API key in Settings first.",
                        );
                    } else {
                        let collection_label = self
                            .publish_collection_pick
                            .and_then(|id| {
                                self.collections
                                    .iter()
                                    .find(|(cid, _)| *cid == id)
                                    .map(|(_, name)| name.clone())
                            })
                            .unwrap_or_else(|| "Select collection…".into());
                        egui::ComboBox::from_id_salt("k2-publish-collection")
                            .selected_text(collection_label.clone())
                            .show_ui(ui, |ui| {
                                for (id, name) in &self.collections {
                                    ui.selectable_value(
                                        &mut self.publish_collection_pick,
                                        Some(*id),
                                        name.clone(),
                                    );
                                }
                            })
                            .response
                            .widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::Button,
                                    ui.is_enabled(),
                                    format!("collection: {collection_label}"),
                                )
                            });
                        ui.add_space(4.0);
                        if ui
                            .add_enabled(
                                draft.publish_state != "verified",
                                egui::Button::new("Publish to Help Center"),
                            )
                            .on_hover_text(
                                "Upserts the draft into the Docs API, publishes the approved text, \
                             then verifies by re-syncing the article here.",
                            )
                            .clicked()
                            && let Some(collection) = self.publish_collection_pick
                        {
                            let payload = serde_json::json!({
                                "draftId": draft.id,
                                "collectionLocalId": collection,
                            })
                            .to_string();
                            let enqueued = db.with_write(|conn| {
                                spp_core::jobs::enqueue(conn, "publish_article_draft", &payload)
                            });
                            match enqueued {
                                Ok(_) => {
                                    self.status_message = Some(
                                        "Publishing — upsert draft, publish, verify re-sync…"
                                            .into(),
                                    );
                                    reloaded = true;
                                }
                                Err(e) => {
                                    self.error = Some(format!("Could not queue the publish: {e}"))
                                }
                            }
                        }
                    }
                }
            },
        );
        if reloaded {
            self.reload_drafts(db);
        }
    }

    fn freshness_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.freshness.is_none() {
            self.reload_all(db);
        }
        let Some(report) = self.freshness.clone() else {
            return;
        };
        design::hint_text(
            ui,
            "Documents by review state (stale / unreviewed / unverified / low usage)",
        );
        ui.add_space(8.0);
        for flag in ["stale", "unreviewed_long", "unverified", "low_usage"] {
            let Some(rows) = report
                .get("flags")
                .and_then(|f| f.get(flag))
                .and_then(Value::as_array)
            else {
                continue;
            };
            design::card_titled(ui, &format!("{flag} ({})", rows.len()), None, |ui| {
                let t = design::tokens(ui);
                for row in rows.iter().take(20) {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {}",
                            str_field(row, "title"),
                            str_field(row, "updated_at")
                        ))
                        .size(13.0)
                        .color(t.foreground),
                    );
                }
            });
            ui.add_space(8.0);
        }
    }

    fn help_center_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.articles.is_empty() && self.reader.is_none() {
            self.reload_all(db);
        }

        egui::Grid::new("docs-stats")
            .num_columns(3)
            .spacing(egui::vec2(10.0, 10.0))
            .show(ui, |ui| {
                design::metric_card(ui, "collections", &self.stats.collections.to_string(), None);
                design::metric_card(
                    ui,
                    "articles",
                    &self.stats.articles.to_string(),
                    Some(&format!(
                        "published {} · draft {} · internal {}",
                        self.stats.published, self.stats.draft, self.stats.internal
                    )),
                );
                design::metric_card(ui, "views", &self.stats.views.to_string(), None);
            });
        design::divider(ui);

        if let Some(id) = self.reader {
            self.reader_ui(ui, db, id);
            return;
        }

        ui.horizontal(|ui| {
            design::field_label(ui, "Collection");
            let collection_label = self
                .collection_pick
                .and_then(|id| {
                    self.collections
                        .iter()
                        .find(|(cid, _)| *cid == id)
                        .map(|(_, name)| name.clone())
                })
                .unwrap_or_else(|| "All".into());
            egui::ComboBox::from_id_salt("docs-collection")
                .selected_text(collection_label.clone())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.collection_pick, None, "All");
                    for (id, name) in self.collections.clone() {
                        ui.selectable_value(&mut self.collection_pick, Some(id), name);
                    }
                })
                .response
                .widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        ui.is_enabled(),
                        format!("collection: {collection_label}"),
                    )
                });
            design::field_label(ui, "Status");
            let status_label = self.status_pick.clone().unwrap_or_else(|| "Any".into());
            egui::ComboBox::from_id_salt("docs-status")
                .selected_text(status_label.clone())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.status_pick, None, "Any");
                    ui.selectable_value(
                        &mut self.status_pick,
                        Some("published".into()),
                        "published",
                    );
                    ui.selectable_value(&mut self.status_pick, Some("draft".into()), "draft");
                    ui.selectable_value(&mut self.status_pick, Some("internal".into()), "internal");
                })
                .response
                .widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        ui.is_enabled(),
                        format!("status: {status_label}"),
                    )
                });
            if design::button(ui, "Apply", ButtonVariant::Outline).clicked() {
                self.filter.collection = self.collection_pick;
                self.filter.status = self.status_pick.clone();
                self.filter.query = None;
                self.search_edit.clear();
                self.reload_docs(db);
            }
        });
        ui.horizontal(|ui| {
            let mut search = self.search_edit.clone();
            let response = ui.add_sized(
                [320.0, 28.0],
                egui::TextEdit::singleline(&mut search).hint_text("Search help center…"),
            );
            if response.changed() {
                self.search_edit = search;
            }
            if design::button(ui, "Search", ButtonVariant::Primary).clicked() {
                let trimmed = self.search_edit.trim();
                self.filter.query = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                self.filter.collection = self.collection_pick;
                self.filter.status = self.status_pick.clone();
                self.reload_docs(db);
                if self.filter.query.is_some()
                    && self.articles.is_empty()
                    && let Some(q) = self.filter.query.clone()
                {
                    let recorded = db
                        .with_write(|conn| {
                            spp_core::intelligence_features::record_knowledge_gap(conn, &q)
                        })
                        .is_ok();
                    self.status_message = Some(if recorded {
                        format!("No hits — “{q}” was tracked as a knowledge gap.")
                    } else {
                        "Search returned no hits.".into()
                    });
                    self.reload_docs(db);
                }
            }
        });
        design::divider(ui);

        if !self.query_gaps.is_empty() || self.stale_docs_tracked > 0 {
            design::card_titled(
                ui,
                "Knowledge gaps (tracked)",
                Some(
                    "zero-hit searches recorded as gaps; stale count from the tracked freshness rows",
                ),
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if self.query_gaps.is_empty() {
                            design::hint_text(ui, "No zero-hit searches recorded");
                        }
                        for (q, hits) in &self.query_gaps {
                            design::badge(ui, &format!("{q} ×{hits}"), BadgeTone::Warning);
                        }
                    });
                    if self.stale_docs_tracked > 0 {
                        ui.add_space(4.0);
                        design::badge(
                            ui,
                            &format!(
                                "{} stale doc(s) tracked — re-ingest to refresh",
                                self.stale_docs_tracked
                            ),
                            BadgeTone::Warning,
                        );
                    }
                },
            );
            ui.add_space(8.0);
        }

        if self.articles.is_empty() {
            design::empty_state(
                ui,
                Icon::BookOpen,
                "No articles match",
                "Adjust the collection, status, or search terms.",
            );
            return;
        }
        let mut open: Option<i64> = None;
        // UXG-1-4: exact-column data_table (virtualized rows built in) —
        // the article list spans the content column at every width.
        let articles = &self.articles;
        design::data_table(
            ui,
            "docs-articles",
            &["Article", "Collection", "Status", "Views", ""],
            articles.len(),
            26.0,
            |ui, i| {
                let t = design::tokens(ui);
                let article = &articles[i];
                ui.label(
                    egui::RichText::new(article.name.as_str())
                        .size(13.0)
                        .strong()
                        .color(t.foreground),
                );
                ui.label(
                    egui::RichText::new(article.collection_name.as_deref().unwrap_or("—"))
                        .size(13.0)
                        .color(t.foreground),
                );
                let tone = match article.status.as_str() {
                    "published" => BadgeTone::Success,
                    "draft" => BadgeTone::Neutral,
                    "internal" => BadgeTone::Info,
                    _ => BadgeTone::Neutral,
                };
                design::badge(ui, article.status.as_str(), tone);
                design::td_num(ui, &article.views.to_string());
                if design::button(ui, "Read", ButtonVariant::Outline).clicked() {
                    open = Some(article.id);
                }
            },
        );
        if let Some(id) = open {
            self.open_article(db, id);
        }
    }

    fn reader_ui(&mut self, ui: &mut egui::Ui, _db: &DbHandle, id: i64) {
        // UXH-1: the article reader rides the shared reading measure —
        // the same READER_MEASURE column the inbox reader thread and
        // composer hold — so back, title, meta and body read as one
        // centered article surface instead of lines stretching across the
        // content column on 2K/4K/8K displays.
        let back = design::centered_column(ui, design::scale::READER_MEASURE, |ui| {
            let back = design::button(ui, "← back to list", ButtonVariant::Ghost).clicked();
            let Some((article, text)) = self.reader_text.clone() else {
                design::hint_text(ui, &format!("Article {id} not found."));
                return back;
            };
            design::section_title(ui, article.name.as_str());
            design::hint_text(
                ui,
                &format!(
                    "{} · {} · updated {}",
                    article.collection_name.as_deref().unwrap_or("—"),
                    article.status,
                    article.updated_at.as_deref().unwrap_or("—")
                ),
            );
            design::divider(ui);
            egui::ScrollArea::vertical()
                .id_salt("docs-reader")
                .show(ui, |ui| {
                    article_body_ui(ui, &text);
                });
            back
        })
        .inner;
        if back {
            self.reader = None;
            self.reader_text = None;
        }
    }

    /// AUD-P2-15 (C11): the Documents-tab document reader — the UXH-1
    /// reading surface (centered READER_MEASURE column, the sanitized
    /// span pipeline from `article_body_ui`) pointed at the stored local
    /// document instead of a synced article: full content, the chunk
    /// embedding state as the meta line, and the edit / re-index acts
    /// one click away. Plain-text bodies degrade to one readable block.
    fn doc_reader_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let doc = self.doc_reader.clone();
        let mut edit: Option<i64> = None;
        let mut reindex: Option<i64> = None;
        let back = design::centered_column(ui, design::scale::READER_MEASURE, |ui| {
            let back = design::button(ui, "← back to list", ButtonVariant::Ghost).clicked();
            let Some(doc) = &doc else {
                design::hint_text(ui, "This document is no longer available.");
                return back;
            };
            let id = i64_field(doc, "id");
            design::section_title(ui, &str_field(doc, "title"));
            let mut meta = format!(
                "{} · v{} · updated {}",
                str_field(doc, "visibility"),
                i64_field(doc, "version"),
                str_field(doc, "updated_at")
            );
            let chunks = i64_field(doc, "chunk_count");
            if chunks > 0 {
                meta.push_str(&format!(
                    " · {}/{} chunks indexed",
                    i64_field(doc, "indexed_count"),
                    chunks
                ));
                let exhausted = i64_field(doc, "exhausted_count");
                if exhausted > 0 {
                    meta.push_str(&format!(
                        " · {exhausted} chunk(s) gave up after 5 failed attempts"
                    ));
                }
            }
            design::hint_text(ui, &meta);
            design::divider(ui);
            egui::ScrollArea::vertical()
                .id_salt("docs-document-reader")
                .show(ui, |ui| {
                    article_body_ui(ui, &str_field(doc, "content"));
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if design::button(ui, "Edit", ButtonVariant::Outline)
                    .on_hover_text("edit title, visibility and content in place")
                    .clicked()
                {
                    edit = Some(id);
                }
                if design::button(ui, "Re-index", ButtonVariant::Outline)
                    .on_hover_text("re-embed this document's chunks — resets failed attempts")
                    .clicked()
                {
                    reindex = Some(id);
                }
            });
            back
        })
        .inner;
        if back {
            self.close_document();
        }
        if let Some(id) = edit {
            self.start_doc_edit(db, id);
        }
        if let Some(id) = reindex {
            self.reindex_document_action(db, id);
        }
    }

    /// AUD-P2-15 (C11): the in-place document edit card — the connectors
    /// edit idiom (prefilled form + "Save Changes"/Cancel) riding above
    /// the list or the reader it was opened from. Saving runs
    /// `update_document`, which re-chunks and refreshes the search index
    /// in the same write.
    fn doc_edit_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let Some(mut form) = self.doc_edit.clone() else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        design::card_titled_wide(
            ui,
            "Edit document",
            Some("saving re-chunks the document and refreshes the search index"),
            |ui| {
                design::form_grid(ui, "knowledge-doc-edit-form", |ui| {
                    let title_label = design::field_label(ui, "Title");
                    ui.add_sized([260.0, 28.0], egui::TextEdit::singleline(&mut form.title))
                        .labelled_by(title_label.id);
                    ui.end_row();
                    design::field_label(ui, "Visibility");
                    ui.horizontal(|ui| {
                        for (value, label) in VISIBILITIES {
                            if design::chip(ui, form.visibility == value, label).clicked() {
                                form.visibility = value.to_string();
                            }
                        }
                    });
                    ui.end_row();
                    let content_label = design::field_label(ui, "Content");
                    ui.add(
                        egui::TextEdit::multiline(&mut form.content)
                            .desired_rows(8)
                            .desired_width(f32::INFINITY),
                    )
                    .labelled_by(content_label.id);
                    ui.end_row();
                });
                if let Some(note) = &form.visibility_note {
                    design::hint_text(ui, note);
                    ui.add_space(4.0);
                }
                let ready = !form.title.trim().is_empty() && !form.content.trim().is_empty();
                ui.horizontal(|ui| {
                    if ready {
                        if design::button(ui, "Save Changes", ButtonVariant::Primary).clicked() {
                            save = true;
                        }
                    } else {
                        let _ = ui.add_enabled(false, egui::Button::new("Save Changes"));
                    }
                    if design::button(ui, "Cancel", ButtonVariant::Outline).clicked() {
                        cancel = true;
                    }
                });
            },
        );
        self.doc_edit = Some(form);
        if cancel {
            self.doc_edit = None;
        }
        if save {
            self.save_doc_edit(db);
        }
    }
}

/// UXH-1: split the sanitized span stream into rendering blocks at its
/// newline separators. `html_to_spans` emits a plain all-newline span
/// between blocks — `\n` for soft breaks and list items, `\n\n` for
/// paragraph gaps — so the reader can lay each paragraph, heading, list
/// item or code block out as its own flowing unit. Each block is
/// `(gap_before, spans)`; the gap counts the newlines of the separator
/// that preceded it (0 for the first block).
fn article_blocks(spans: &[spp_core::richtext::Span]) -> Vec<(usize, &[spp_core::richtext::Span])> {
    let mut blocks: Vec<(usize, &[spp_core::richtext::Span])> = Vec::new();
    let mut start = 0usize;
    let mut gap = 0usize;
    for (i, span) in spans.iter().enumerate() {
        let separator = span.style == spp_core::richtext::SpanStyle::default()
            && span.text.chars().all(|c| c == '\n');
        if separator {
            if i > start {
                blocks.push((gap, &spans[start..i]));
                gap = 0;
            }
            gap = gap.max(span.text.chars().count());
            start = i + 1;
        }
    }
    if spans.len() > start {
        blocks.push((gap, &spans[start..]));
    }
    blocks
}

/// UXH-1: render the synced Help Scout article body as a readable
/// article. The stored body is HTML; it goes through the same sanitizer +
/// span pipeline the inbox reader uses for mail bodies (the raw source is
/// never shown), then lays out block by block — paragraphs, headings,
/// bulleted and numbered list items, and monospace code blocks — with a
/// paragraph rhythm between blocks. Plain-text bodies degrade to a single
/// readable block.
fn article_body_ui(ui: &mut egui::Ui, body: &str) {
    let spans = spp_core::richtext::html_to_spans(body);
    let blocks = article_blocks(&spans);
    if blocks.is_empty() {
        design::hint_text(ui, "This article has no body text yet.");
        return;
    }
    let mut first = true;
    for (gap, spans) in blocks {
        if !first && gap >= 2 {
            ui.add_space(design::scale::space::SP3);
        }
        first = false;
        article_block_ui(ui, spans);
    }
}

/// UXH-1: one block of the article body — a run of styled spans that
/// belong to the same line group, flowing inline and wrapping as one
/// unit. The span styling mirrors the inbox message reader (the same
/// pipeline produced both), with heading sizes tuned for the article
/// register: the article title is section_title (15 strong), so body
/// headings step down h1/h2 → LEAD strong, h3+ → BODY strong — every
/// heading outranks the 13.5px body line beside it.
fn article_block_ui(ui: &mut egui::Ui, spans: &[spp_core::richtext::Span]) {
    ui.horizontal_wrapped(|ui| {
        for span in spans {
            let mut rich =
                egui::RichText::new(spp_core::richtext::break_long_tokens(&span.text, 40));
            let style = &span.style;
            if style.bold {
                rich = rich.strong();
            }
            if style.italic {
                rich = rich.italics();
            }
            if style.underline {
                rich = rich.underline();
            }
            if style.strikethrough {
                rich = rich.strikethrough();
            }
            if style.code {
                rich = rich.monospace();
            }
            if style.quote {
                rich = rich.weak();
            }
            if let Some(level) = style.heading {
                let size = if level <= 2 {
                    design::scale::text::LEAD
                } else {
                    design::scale::text::BODY
                };
                rich = rich.size(size).strong();
            }
            if let Some(href) = &style.link {
                ui.hyperlink_to(rich, href.clone());
            } else {
                ui.label(rich);
            }
        }
    });
}

impl Default for KnowledgePage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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

    // UXH-1: the reader must lay the synced Help Scout HTML body out as
    // readable rich text — never the raw source the owner saw.

    #[test]
    fn article_body_renders_the_reported_html_shape_without_syntax() {
        // The owner-reported body: a <p> paragraph with an inline <code> run.
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
        // Bodies that were never HTML (manual seeds) degrade to a single
        // proportional-text block — no tags to strip, nothing to split.
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
            let source_id = spp_core::knowledge_store::get_or_create_source(
                conn, "Handbook", "manual", "internal",
            )
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

    // AUD-P2-15 (C11): the Documents-tab reader / edit / re-ingest — the
    // three dead ends the audit found (no per-document view, no scoped
    // re-embed, delete + re-import as the only edit path).

    fn seed_knowledge_doc(db: &DbHandle, title: &str, content: &str, visibility: &str) -> i64 {
        db.with_write(|conn| {
            let source_id = spp_core::knowledge_store::get_or_create_source(
                conn, "Handbook", "manual", "internal",
            )
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
        let doc_id =
            seed_knowledge_doc(&db, "Reader guide", "Read me end to end.", "internal_only");
        let mut page = KnowledgePage::new();
        page.reload_all(&db);
        assert!(page.doc_reader.is_none());

        // open loads the full stored record — the list row only carries
        // the 200-char preview, the reader carries the content.
        page.open_document(&db, doc_id);
        let reader = page.doc_reader.clone().expect("the reader state loads");
        assert_eq!(i64_field(&reader, "id"), doc_id);
        assert_eq!(str_field(&reader, "title"), "Reader guide");
        assert_eq!(str_field(&reader, "content"), "Read me end to end.");
        assert_eq!(i64_field(&reader, "chunk_count"), 1);

        // back clears the reader — the list stays loaded behind it
        page.close_document();
        assert!(page.doc_reader.is_none());
        assert_eq!(page.documents.len(), 1);

        // the deep link latches the id (the shell call has no db handle)
        // and resolves into the reader on the Documents tab
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

        // an unknown id falls through to the list, not a broken reader
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

        // the edit form opens prefilled with the full stored record
        page.start_doc_edit(&db, doc_id);
        let form = page.doc_edit.clone().expect("the edit form opens");
        assert_eq!(form.id, doc_id);
        assert_eq!(form.title, "Troubleshooting logins");
        assert_eq!(form.content, "Check the SSO configuration first.");
        assert_eq!(form.visibility, "internal_only");
        page.doc_edit = None; // the Cancel act clears without writing

        // title + content + visibility change: the title must EDIT the row
        // (not insert a sibling under the new title), the content change
        // must bump the version, and the re-chunk must run
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

        // a visibility-only save applies the chips without a version bump
        // (the checksum-early-return path) and keeps the FTS rows in step
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
        // the permanently-excluded state: 5 failed attempts
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

    fn temp_data_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "spp-knowledge-import-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
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
        page.set_data_dir(dir.clone());
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
        page.set_data_dir(dir);
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
}
