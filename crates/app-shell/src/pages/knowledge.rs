//! Knowledge & docs pages (port of the web `pages/knowledge.rs` and
//! `pages/docs.rs`).
//!
//! Presentational pass (shadcn redesign): this page renders through the
//! shared design system in `crate::design`; the queries, state, and
//! behavior are unchanged.
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
use spp_core::sync_engine::SyncEngine;

use super::{ConfirmRequest, NavRequest, i64_field, str_field};
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

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
    // K2: the AI article-draft review queue.
    drafts: Vec<spp_core::knowledge_store::ArticleDraftRow>,
    gaps: Vec<(i64, String)>,
    draft_pick: Option<i64>,
    gap_pick: Option<i64>,
    publish_collection_pick: Option<i64>,
    // Item 150: the tracked knowledge gaps (zero-hit searches) and the
    // tracked stale-doc count from the intelligence freshness sweep.
    query_gaps: Vec<(String, i64)>,
    stale_docs_tracked: u32,
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
            drafts: Vec::new(),
            gaps: Vec::new(),
            draft_pick: None,
            gap_pick: None,
            publish_collection_pick: None,
            query_gaps: Vec::new(),
            stale_docs_tracked: 0,
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
    }

    fn reload_docs(&mut self, db: &DbHandle) {
        let (articles, query_gaps, stale) = db.with_read(|conn| {
            (
                list_articles(conn, &self.filter),
                // Item 150: the tracked zero-hit queries, best-hit order.
                spp_core::intelligence_features::list_knowledge_gaps(conn, 10).unwrap_or_default(),
                spp_core::intelligence_features::count_stale_docs(conn).unwrap_or(0),
            )
        });
        self.articles = articles;
        self.query_gaps = query_gaps;
        self.stale_docs_tracked = stale;
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

    /// Delete a knowledge document (runs on shell confirmation only —
    /// clean-audit item 140). Returns the core outcome so the shell's
    /// toast reports the truth; the pre-fix `unwrap_or(false)` swallow
    /// is gone with it.
    pub fn delete_document(&mut self, db: &DbHandle, id: i64) -> spp_core::Result<()> {
        db.with_write(|conn| {
            let removed = spp_core::knowledge_store::delete_document(conn, id)?;
            if removed {
                // Clean-audit item 60: the delete hard-deletes the
                // document's chunk rows, orphaning their vector points
                // (which keep matching with their stale text snapshots).
                // Nothing is pending to embed, so the orphan sweep rides
                // an explicit embed pass — fire-and-forget like the
                // import side effects.
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
        self.documents.clear();
        Ok(())
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
                    design::hint_text(ui, "(no importable files in the folder)");
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
                    design::field_label(ui, "Title");
                    ui.add_sized(
                        [260.0, 28.0],
                        egui::TextEdit::singleline(&mut self.import.title),
                    );
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
                    design::field_label(ui, "Source name");
                    ui.add_sized(
                        [260.0, 28.0],
                        egui::TextEdit::singleline(&mut self.import.source_name),
                    );
                    ui.end_row();
                    design::field_label(ui, "Content (Markdown)");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.import.content)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
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

    /// Open the help-center article reader.
    pub fn open_article(&mut self, db: &DbHandle, id: i64) {
        self.reader = Some(id);
        self.reader_text = db.with_read(|conn| article_detail(conn, id));
    }

    /// Render the page. `engine` powers the "Sync Help Center now"
    /// action (a real Docs API v1 pull — before 1.3.0 the page had no
    /// way to fetch and the /v2/ endpoint paths it used did not exist).
    /// Returns a confirm request when the user asked to delete a
    /// document (clean-audit item 140 — the delete runs only after the
    /// shell dialog confirms).
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        engine: Option<&std::sync::Arc<SyncEngine>>,
    ) -> Option<NavRequest> {
        design::page_header(
            ui,
            "Knowledge",
            Some("local documents, sources, freshness, and the help center"),
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
        // Missing Docs API key = the Help Center tab can never fill.
        // Say so, and how to fix it, instead of an unexplained empty tab.
        // Audit (C2 residue): this used to read the plain `docs_api_key`
        // setting — a key nothing ever writes (the real one lives in the
        // encrypted envelope under `helpscout_docs_api_key`, with the
        // HELPSCOUT_DOCS_API_KEY env var as the provider's fallback) — so
        // the banner showed even while sync was working. Ask the same
        // question the provider's resolve() does: envelope first, then env.
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
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                if design::chip(ui, self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
        });
        if let Some(msg) = &self.status_message {
            design::success_text(ui, msg);
        }
        if let Some(err) = &self.error {
            design::error_text(ui, err);
        }
        design::divider(ui);

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

    /// The Documents tab. Returns a confirm request when the user asked
    /// to delete a document (clean-audit item 140).
    fn documents_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        if self.documents.is_empty() {
            self.reload_all(db);
        }
        design::hint_text(
            ui,
            "Local knowledge documents used by search and AI answers",
        );
        self.import_ui(ui, db);
        ui.add_space(8.0);
        if self.documents.is_empty() {
            design::empty_state(
                ui,
                Icon::BookOpen,
                "(no documents)",
                "Import a document to get started.",
            );
            return None;
        }
        // Actions are deferred out of the borrow of `self.documents`.
        let mut action: Option<(i64, String, &str)> = None;
        egui::ScrollArea::vertical().show_rows(ui, 24.0, self.documents.len(), |ui, range| {
            design::table(
                ui,
                "knowledge-documents",
                &["title", "source", "visibility", "chunks", "actions"],
                26.0,
                |ui| {
                    let t = design::tokens(ui);
                    for doc in self.documents[range].iter() {
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
                        ui.horizontal(|ui| {
                            if design::button(ui, "reviewed", ButtonVariant::Outline).clicked() {
                                action = Some((id, str_field(doc, "title"), "reviewed"));
                            }
                            if design::button(ui, "verify", ButtonVariant::Outline).clicked() {
                                action = Some((id, str_field(doc, "title"), "verify"));
                            }
                            if design::button(ui, "delete", ButtonVariant::Destructive).clicked() {
                                action = Some((id, str_field(doc, "title"), "delete"));
                            }
                        });
                        ui.end_row();
                    }
                },
            );
        });
        if let Some((id, title, what)) = action {
            match what {
                "reviewed" => self.mark_reviewed(db, id),
                "verify" => self.mark_verified(db, id),
                // Clean-audit item 140: arming the dialog replaces the
                // unconfirmed delete.
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
        if self.sources.is_empty() {
            self.reload_all(db);
        }
        if self.sources.is_empty() {
            design::empty_state(
                ui,
                Icon::BookOpen,
                "(no sources)",
                "Sources appear when documents are imported.",
            );
            return;
        }
        design::card(ui, |ui| {
            design::table(
                ui,
                "knowledge-sources",
                &["name", "kind", "visibility", "documents", "created"],
                26.0,
                |ui| {
                    let t = design::tokens(ui);
                    for source in &self.sources {
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
                        ui.end_row();
                    }
                },
            );
        });
    }

    /// K2: load the review queue and the requestable open gaps.
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

    /// K2: the AI article-draft review queue — the human gate between the
    /// gap pipeline and the Docs API publish path (K3). Nothing leaves
    /// this table without an explicit Approve, and nothing publishes
    /// without an explicit publish click on an approved draft.
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

        // --- Request a draft from an open gap ---
        if !self.gaps.is_empty() {
            design::card_titled(ui, "Draft an article from a gap", None, |ui| {
                egui::ComboBox::from_id_salt("k2-gap-pick")
                    .selected_text(
                        self.gap_pick
                            .and_then(|id| {
                                self.gaps
                                    .iter()
                                    .find(|(gid, _)| *gid == id)
                                    .map(|(_, q)| q.chars().take(60).collect::<String>())
                            })
                            .unwrap_or_else(|| "Select a gap…".into()),
                    )
                    .width(ui.available_width() - 140.0)
                    .show_ui(ui, |ui| {
                        for (id, question) in &self.gaps {
                            let label: String = question.chars().take(80).collect();
                            ui.selectable_value(&mut self.gap_pick, Some(*id), label);
                        }
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

        // --- The queue list ---
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
                    let _ = ui.selectable_label(
                        selected,
                        egui::RichText::new(format!("{} — {}", draft.id, draft.title)).strong(),
                    );
                    ui.label(egui::RichText::new(status_chip).small().color(
                        if draft.status == "approved" {
                            ui.visuals().hyperlink_color
                        } else {
                            ui.visuals().weak_text_color()
                        },
                    ));
                    if !publish_chip.is_empty() {
                        ui.label(egui::RichText::new(publish_chip).small().color(
                            if draft.publish_state == "publish_failed" {
                                ui.visuals().error_fg_color
                            } else {
                                ui.visuals().weak_text_color()
                            },
                        ));
                    }
                    if ui.small_button("Open").clicked() {
                        self.draft_pick = Some(draft.id);
                    }
                });
            });
        }
        ui.add_space(8.0);

        // --- The reader + decisions ---
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
                                ui.add_space(3.0);
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
                        if ui.button("Reject").clicked() {
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
                        egui::ComboBox::from_id_salt("k2-publish-collection")
                            .selected_text(
                                self.publish_collection_pick
                                    .and_then(|id| {
                                        self.collections
                                            .iter()
                                            .find(|(cid, _)| *cid == id)
                                            .map(|(_, name)| name.clone())
                                    })
                                    .unwrap_or_else(|| "Select collection…".into()),
                            )
                            .show_ui(ui, |ui| {
                                for (id, name) in &self.collections {
                                    ui.selectable_value(
                                        &mut self.publish_collection_pick,
                                        Some(*id),
                                        name.clone(),
                                    );
                                }
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
            design::field_label(ui, "collection:");
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
            design::field_label(ui, "status:");
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
            if design::button(ui, "apply", ButtonVariant::Outline).clicked() {
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
            if design::button(ui, "search", ButtonVariant::Primary).clicked() {
                let trimmed = self.search_edit.trim();
                self.filter.query = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                self.filter.collection = self.collection_pick;
                self.filter.status = self.status_pick.clone();
                self.reload_docs(db);
                // Item 150: a non-blank search that returns nothing is a
                // knowledge gap — record it (deduped by the core writer)
                // so the tracked gaps list can drive new articles.
                if self.filter.query.is_some()
                    && self.articles.is_empty()
                    && let Some(q) = self.filter.query.clone()
                {
                    // FIND-007: the write used to be fire-and-forget
                    // (`let _ = …`) — a read-looking control silently
                    // recorded rows. Surface it via the page's standard
                    // status message.
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

        // Item 150: the tracked knowledge gaps — every zero-hit search
        // gains a row, and the stale count comes from the freshness rows
        // the ingest stamps and the intelligence sweep maintains.
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
                            design::hint_text(ui, "(no zero-hit searches recorded)");
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
                "(no articles match)",
                "Adjust the collection, status, or search terms.",
            );
            return;
        }
        let mut open: Option<i64> = None;
        egui::ScrollArea::vertical().show_rows(ui, 26.0, self.articles.len(), |ui, range| {
            design::table(
                ui,
                "docs-articles",
                &["article", "collection", "status", "views", ""],
                26.0,
                |ui| {
                    let t = design::tokens(ui);
                    for article in self.articles[range].iter().cloned() {
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
                        if design::button(ui, "read", ButtonVariant::Outline).clicked() {
                            open = Some(article.id);
                        }
                        ui.end_row();
                    }
                },
            );
        });
        if let Some(id) = open {
            self.open_article(db, id);
        }
    }

    fn reader_ui(&mut self, ui: &mut egui::Ui, _db: &DbHandle, id: i64) {
        if design::button(ui, "← back to list", ButtonVariant::Ghost).clicked() {
            self.reader = None;
            self.reader_text = None;
            return;
        }
        let Some((article, text)) = self.reader_text.clone() else {
            design::hint_text(ui, &format!("Article {id} not found."));
            return;
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
                ui.label(
                    egui::RichText::new(text)
                        .monospace()
                        .size(14.0)
                        .color(design::tokens(ui).foreground),
                );
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

        let _ = page.delete_document(&db, doc_id);
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
