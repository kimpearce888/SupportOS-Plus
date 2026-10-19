//! Knowledge & docs pages (port of the web `pages/knowledge.rs` and
//! `pages/docs.rs`).
//!
//! The knowledge tabs (documents / sources / freshness) read and act
//! through `spp_core::knowledge_store` directly. The help-center docs tab
//! ports the `/api/docs/*` route SQL as typed queries (collections, stats,
//! article list with collection/status filters, FTS search using the same
//! tokenizer the route uses, and the article reader). The import panel
//! ports the web `ImportKnowledgeModal`: paste-import plus the
//! `knowledge-import` watch folder, with the route's embeddings-job and
//! audit side effects — reaching this panel is also what makes the
//! pdf/docx parse sandbox live in the shipped binary.

use std::path::{Path, PathBuf};

use eframe::egui;
use rusqlite::Connection;
use serde_json::Value;
use spp_core::db::DbHandle;

use super::{i64_field, str_field};

/// Importable visibility values (route parity).
const VISIBILITIES: [(&str, &str); 2] = [
    ("internal_only", "internal-only (never in customer drafts)"),
    (
        "customer_safe",
        "customer-safe (may support customer drafts)",
    ),
];

/// Import one file from the `knowledge-import` watch folder (port of
/// `POST /api/knowledge/import-file`): the core import plus the route's
/// side effects — an embeddings backfill job and a `knowledge_imported`
/// audit entry.
///
/// # Errors
/// Propagates the core `import_file` failures (path outside the watch
/// folder, missing file, unsupported type, oversized input).
pub fn run_file_import(db: &DbHandle, data_dir: &Path, file: &str) -> spp_core::Result<usize> {
    db.with_write(|conn| {
        let results =
            spp_core::knowledge_store::import_file(conn, data_dir, file, None, "internal_only")?;
        post_import_side_effects(conn, &results, "file");
        Ok(results.len())
    })
}

/// Paste-import one Markdown document (port of `POST /api/knowledge/import`).
///
/// # Errors
/// `Validation` when the title/content is blank or the visibility is
/// unknown; otherwise the core `import_manual` failure.
pub fn run_manual_import(
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

/// Route-parity side effects after a successful import: enqueue the
/// embeddings backfill and write the audit row. Fire-and-forget exactly
/// like the original routes (`let _ = ...`).
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

/// Which tab is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Documents,
    Sources,
    Freshness,
    HelpCenter,
}

impl Tab {
    const ALL: [Self; 4] = [
        Self::Documents,
        Self::Sources,
        Self::Freshness,
        Self::HelpCenter,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Documents => "Documents",
            Self::Sources => "Sources",
            Self::Freshness => "Freshness",
            Self::HelpCenter => "Help center",
        }
    }
}

/// Help-center stats (port of the `/api/docs/stats` SQL).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocsStats {
    pub collections: i64,
    pub articles: i64,
    pub published: i64,
    pub draft: i64,
    pub internal: i64,
    pub views: i64,
    pub last_synced: Option<String>,
}

/// Load help-center stats.
#[must_use]
pub fn docs_stats(conn: &Connection) -> DocsStats {
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

/// Help-center collections: `(id, name)`.
#[must_use]
pub fn docs_collections(conn: &Connection) -> Vec<(i64, String)> {
    conn.prepare("SELECT id, name FROM docs_collections ORDER BY name")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

/// Help-center article summary row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocArticle {
    pub id: i64,
    pub collection_name: Option<String>,
    pub name: String,
    pub status: String,
    pub preview: String,
    pub words: i64,
    pub views: i64,
    pub updated_at: Option<String>,
}

/// Search filter for the article list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocsFilter {
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

/// Article ids matching an FTS query (port of the route's `fts_doc_ranks`):
/// tokenize with the core tokenizer, quote each token as a prefix term,
/// return ids in rank order.
#[must_use]
pub fn fts_article_ids(conn: &Connection, q: &str) -> Vec<i64> {
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

/// List articles with the filter applied (FTS query narrows by id set).
#[must_use]
pub fn list_articles(conn: &Connection, filter: &DocsFilter) -> Vec<DocArticle> {
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

/// Full article (summary + body text) for the reader.
#[must_use]
pub fn article_detail(conn: &Connection, id: i64) -> Option<(DocArticle, String)> {
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

/// Import-panel state (port of the web `ImportKnowledgeModal`).
#[derive(Debug, Default)]
pub struct ImportPanel {
    /// Paste-import fields.
    title: String,
    content: String,
    /// Defaults to the route's "Manual import" when left blank.
    source_name: String,
    /// One of the [`VISIBILITIES`] values.
    visibility: String,
    /// Watch-folder listing; `None` until the panel is first opened
    /// (the modal fetched `/api/knowledge/importable` on open).
    files: Option<Vec<String>>,
    /// The watch-folder path, for display.
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

/// The interactive knowledge page state.
pub struct KnowledgePage {
    tab: Tab,
    documents: Vec<Value>,
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
    status_message: Option<String>,
    error: Option<String>,
    data_dir: Option<PathBuf>,
    import: ImportPanel,
}

impl KnowledgePage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::Documents,
            documents: Vec::new(),
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
            status_message: None,
            error: None,
            data_dir: None,
            import: ImportPanel::new(),
        }
    }

    /// Install the data directory (the import panel needs it for the
    /// `knowledge-import` watch folder). Mirrors the other path-consuming
    /// pages (`settings.set_backups_dir`, `connectors.set_data_dir`).
    pub fn set_data_dir(&mut self, data_dir: PathBuf) {
        self.data_dir = Some(data_dir);
    }

    /// (Re-)load the watch-folder listing (the modal's on-open fetch).
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

    /// Import one file from the watch folder and refresh the page state.
    pub fn import_file_action(&mut self, db: &DbHandle, file: &str) {
        let Some(dir) = self.data_dir.clone() else {
            self.error = Some("Import is unavailable (no data directory).".to_string());
            return;
        };
        match run_file_import(db, &dir, file) {
            Ok(n) => {
                self.status_message = Some(format!("Imported {n} document(s) from file."));
                self.documents.clear();
                self.sources.clear();
            }
            Err(e) => self.error = Some(format!("Import failed: {e}")),
        }
        self.reload_importable();
    }

    /// Paste-import the edited document and refresh the page state.
    pub fn manual_import_action(&mut self, db: &DbHandle) {
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
                self.documents.clear();
                self.sources.clear();
            }
            Err(e) => self.error = Some(format!("Import failed: {e}")),
        }
    }

    /// Live-update invalidation (incremental sync with rows → docs reload).
    pub fn invalidate(&mut self) {
        self.documents.clear();
        self.sources.clear();
        self.freshness = None;
        self.articles.clear();
        self.stats = DocsStats::default();
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
    }

    fn reload_docs(&mut self, db: &DbHandle) {
        self.articles = db.with_read(|conn| list_articles(conn, &self.filter));
    }

    /// Mark a knowledge document reviewed/verified (freshness actions).
    pub fn mark_reviewed(&mut self, db: &DbHandle, id: i64) {
        let ok = db.with_read(|conn| spp_core::knowledge_store::mark_reviewed(conn, id));
        self.status_message = Some(match ok {
            Ok(true) => format!("Document {id} marked reviewed."),
            Ok(false) => format!("Document {id} not found."),
            Err(e) => format!("Could not mark reviewed: {e}"),
        });
        self.documents.clear();
    }

    /// Mark a knowledge document verified (freshness actions).
    pub fn mark_verified(&mut self, db: &DbHandle, id: i64) {
        let ok = db.with_read(|conn| spp_core::knowledge_store::mark_verified(conn, id));
        self.status_message = Some(match ok {
            Ok(true) => format!("Document {id} marked verified."),
            Ok(false) => format!("Document {id} not found."),
            Err(e) => format!("Could not mark verified: {e}"),
        });
        self.documents.clear();
    }

    /// Delete a knowledge document.
    pub fn delete_document(&mut self, db: &DbHandle, id: i64) {
        let deleted = db
            .with_write(|conn| spp_core::knowledge_store::delete_document(conn, id))
            .unwrap_or(false);
        self.status_message = Some(if deleted {
            format!("Document {id} deleted.")
        } else {
            format!("Document {id} not found.")
        });
        self.documents.clear();
    }

    /// The import panel (web `ImportKnowledgeModal`): paste-import plus
    /// the watch folder. Deferred actions keep the immediate-mode borrows
    /// sound.
    fn import_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let mut file_action: Option<String> = None;
        let mut manual_submit = false;
        let mut refresh = false;
        egui::CollapsingHeader::new("Import documents")
            .default_open(false)
            .show(ui, |ui| {
                if self.import.files.is_none() {
                    self.reload_importable();
                }
                ui.strong("Import from the knowledge-import folder");
                let dir = self
                    .import
                    .import_dir
                    .clone()
                    .unwrap_or_else(|| "<data dir>/knowledge-import".to_string());
                ui.weak(format!(
                    "Copy documents into {dir} (supported: .md .txt .csv .json .html .pdf .docx). \
                     For safety, file imports must live inside that folder."
                ));
                if ui.button("Refresh").clicked() {
                    refresh = true;
                }
                let files = self.import.files.clone().unwrap_or_default();
                if files.is_empty() {
                    ui.weak("(no importable files in the folder)");
                }
                for file in files {
                    ui.horizontal(|ui| {
                        ui.monospace(file.clone());
                        if ui.button("Import").clicked() {
                            file_action = Some(file);
                        }
                    });
                }
                ui.separator();
                ui.strong("Paste a document");
                egui::Grid::new("knowledge-import-form")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Title");
                        ui.text_edit_singleline(&mut self.import.title);
                        ui.end_row();
                        ui.label("Visibility");
                        ui.horizontal(|ui| {
                            for (value, label) in VISIBILITIES {
                                let selected = self.import.visibility == value;
                                if ui.selectable_label(selected, label).clicked() {
                                    self.import.visibility = value.to_string();
                                }
                            }
                        });
                        ui.end_row();
                        ui.label("Source name");
                        ui.text_edit_singleline(&mut self.import.source_name);
                        ui.end_row();
                        ui.label("Content (Markdown)");
                        ui.add(
                            egui::TextEdit::multiline(&mut self.import.content)
                                .desired_rows(4)
                                .desired_width(f32::INFINITY),
                        );
                        ui.end_row();
                    });
                let ready =
                    !self.import.title.trim().is_empty() && !self.import.content.trim().is_empty();
                if ui
                    .add_enabled(ready, egui::Button::new("Import document"))
                    .clicked()
                {
                    manual_submit = true;
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

    /// Open the help-center article reader.
    pub fn open_article(&mut self, db: &DbHandle, id: i64) {
        self.reader = Some(id);
        self.reader_text = db.with_read(|conn| article_detail(conn, id));
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.heading("Knowledge");
        ui.separator();

        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
        });
        if let Some(msg) = &self.status_message {
            ui.colored_label(egui::Color32::GREEN, msg.clone());
        }
        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, err.clone());
        }
        ui.separator();

        match self.tab {
            Tab::Documents => self.documents_ui(ui, db),
            Tab::Sources => self.sources_ui(ui, db),
            Tab::Freshness => self.freshness_ui(ui, db),
            Tab::HelpCenter => self.help_center_ui(ui, db),
        }
    }

    fn documents_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.documents.is_empty() {
            self.reload_all(db);
        }
        ui.weak("Local knowledge documents used by search and AI answers");
        self.import_ui(ui, db);
        if self.documents.is_empty() {
            ui.weak("(no documents)");
            return;
        }
        // Actions are deferred out of the borrow of `self.documents`.
        let mut action: Option<(i64, &str)> = None;
        egui::ScrollArea::vertical().show_rows(ui, 24.0, self.documents.len(), |ui, range| {
            egui::Grid::new("knowledge-documents")
                .num_columns(5)
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("title");
                    ui.strong("source");
                    ui.strong("visibility");
                    ui.strong("chunks");
                    ui.strong("actions");
                    ui.end_row();
                    for doc in self.documents[range].iter() {
                        let id = i64_field(doc, "id");
                        ui.label(str_field(doc, "title"));
                        ui.label(format!("#{}", i64_field(doc, "source_id")));
                        ui.label(str_field(doc, "visibility"));
                        ui.monospace(i64_field(doc, "chunk_count").to_string());
                        ui.horizontal(|ui| {
                            if ui.button("reviewed").clicked() {
                                action = Some((id, "reviewed"));
                            }
                            if ui.button("verify").clicked() {
                                action = Some((id, "verify"));
                            }
                            if ui.button("delete").clicked() {
                                action = Some((id, "delete"));
                            }
                        });
                        ui.end_row();
                    }
                });
        });
        if let Some((id, what)) = action {
            match what {
                "reviewed" => self.mark_reviewed(db, id),
                "verify" => self.mark_verified(db, id),
                _ => self.delete_document(db, id),
            }
        }
    }

    fn sources_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.sources.is_empty() {
            self.reload_all(db);
        }
        if self.sources.is_empty() {
            ui.weak("(no sources)");
            return;
        }
        egui::Grid::new("knowledge-sources")
            .num_columns(5)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("name");
                ui.strong("kind");
                ui.strong("visibility");
                ui.strong("documents");
                ui.strong("created");
                ui.end_row();
                for source in &self.sources {
                    ui.label(str_field(source, "name"));
                    ui.label(str_field(source, "kind"));
                    ui.label(str_field(source, "visibility"));
                    ui.monospace(i64_field(source, "document_count").to_string());
                    ui.label(str_field(source, "created_at"));
                    ui.end_row();
                }
            });
    }

    fn freshness_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.freshness.is_none() {
            self.reload_all(db);
        }
        let Some(report) = self.freshness.clone() else {
            return;
        };
        ui.weak("Documents by review state (stale / unreviewed / unverified / low usage)");
        for flag in ["stale", "unreviewed_long", "unverified", "low_usage"] {
            let Some(rows) = report
                .get("flags")
                .and_then(|f| f.get(flag))
                .and_then(Value::as_array)
            else {
                continue;
            };
            ui.strong(format!("{flag} ({})", rows.len()));
            for row in rows.iter().take(20) {
                ui.label(format!(
                    "  {} · {}",
                    str_field(row, "title"),
                    str_field(row, "updated_at")
                ));
            }
            ui.add_space(4.0);
        }
    }

    fn help_center_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.articles.is_empty() && self.reader.is_none() {
            self.reload_all(db);
        }

        ui.monospace(format!(
            "collections: {} · articles: {} (published {}, draft {}, internal {}) · views: {}",
            self.stats.collections,
            self.stats.articles,
            self.stats.published,
            self.stats.draft,
            self.stats.internal,
            self.stats.views
        ));
        ui.separator();

        if let Some(id) = self.reader {
            self.reader_ui(ui, db, id);
            return;
        }

        ui.horizontal(|ui| {
            ui.label("collection:");
            egui::ComboBox::from_id_salt("docs-collection")
                .selected_text(
                    self.collection_pick
                        .and_then(|id| {
                            self.collections
                                .iter()
                                .find(|(cid, _)| *cid == id)
                                .map(|(_, name)| name.clone())
                        })
                        .unwrap_or_else(|| "All".into()),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.collection_pick, None, "All");
                    for (id, name) in self.collections.clone() {
                        ui.selectable_value(&mut self.collection_pick, Some(id), name);
                    }
                });
            ui.label("status:");
            egui::ComboBox::from_id_salt("docs-status")
                .selected_text(self.status_pick.clone().unwrap_or_else(|| "Any".into()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.status_pick, None, "Any");
                    ui.selectable_value(
                        &mut self.status_pick,
                        Some("published".into()),
                        "published",
                    );
                    ui.selectable_value(&mut self.status_pick, Some("draft".into()), "draft");
                    ui.selectable_value(&mut self.status_pick, Some("internal".into()), "internal");
                });
            if ui.button("apply").clicked() {
                self.filter.collection = self.collection_pick;
                self.filter.status = self.status_pick.clone();
                self.filter.query = None;
                self.search_edit.clear();
                self.reload_docs(db);
            }
        });
        ui.horizontal(|ui| {
            let mut search = self.search_edit.clone();
            let response =
                ui.add(egui::TextEdit::singleline(&mut search).hint_text("Search help center…"));
            if response.changed() {
                self.search_edit = search;
            }
            if ui.button("search").clicked() {
                let trimmed = self.search_edit.trim();
                self.filter.query = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                self.filter.collection = self.collection_pick;
                self.filter.status = self.status_pick.clone();
                self.reload_docs(db);
            }
        });
        ui.separator();

        if self.articles.is_empty() {
            ui.weak("(no articles match)");
            return;
        }
        let mut open: Option<i64> = None;
        egui::ScrollArea::vertical().show_rows(ui, 26.0, self.articles.len(), |ui, range| {
            egui::Grid::new("docs-articles")
                .num_columns(5)
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("article");
                    ui.strong("collection");
                    ui.strong("status");
                    ui.strong("views");
                    ui.strong("");
                    ui.end_row();
                    for article in self.articles[range].iter().cloned() {
                        ui.strong(article.name.as_str());
                        ui.label(article.collection_name.as_deref().unwrap_or("—"));
                        ui.label(article.status.as_str());
                        ui.monospace(article.views.to_string());
                        if ui.button("read").clicked() {
                            open = Some(article.id);
                        }
                        ui.end_row();
                    }
                });
        });
        if let Some(id) = open {
            self.open_article(db, id);
        }
    }

    fn reader_ui(&mut self, ui: &mut egui::Ui, _db: &DbHandle, id: i64) {
        if ui.button("← back to list").clicked() {
            self.reader = None;
            self.reader_text = None;
            return;
        }
        let Some((article, text)) = self.reader_text.clone() else {
            ui.weak(format!("Article {id} not found."));
            return;
        };
        ui.strong(article.name.as_str());
        ui.weak(format!(
            "{} · {} · updated {}",
            article.collection_name.as_deref().unwrap_or("—"),
            article.status,
            article.updated_at.as_deref().unwrap_or("—")
        ));
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("docs-reader")
            .show(ui, |ui| {
                ui.label(egui::RichText::new(text).monospace().size(14.0));
            });
    }
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

            // No match → empty list, not an error.
            assert!(fts_article_ids(conn, "zzzz-nothing").is_empty());
            // Too-short tokens are dropped by the tokenizer (min 2 chars).
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
        // Freshness report now classifies the reviewed document.
        assert!(page.freshness.is_some());

        page.delete_document(&db, doc_id);
        page.reload_all(&db);
        assert!(page.documents.is_empty());
    }

    // --- Import panel (web ImportKnowledgeModal port) ---

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

        // Route-parity side effects: embeddings backfill job + audit row.
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

        // Route parity: blank title/content rejected; visibility validated.
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
        // Blank source name fell back to the route default.
        let sources = db.with_read(spp_core::knowledge_store::list_sources);
        let names: Vec<String> = sources.iter().map(|s| str_field(s, "name")).collect();
        assert!(names.iter().any(|n| n == "Manual import"), "{names:?}");
        let _ = dir; // temp dir unused beyond scoping
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

        // Error path surfaces as the page error, not a crash.
        page.import_file_action(&db, "../escape.md");
        assert!(page.error.is_some_and(|e| e.contains("knowledge-import")));
    }
}
