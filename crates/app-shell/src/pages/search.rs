//! Search page (port of the web `pages/search.rs`).
//!
//! Presentational pass (shadcn redesign): this page renders through the
//! shared design system in `crate::design`; the queries, state, and
//! behavior are unchanged.
//!
//! One input, scope selector (`all` / `tickets` / `knowledge` /
//! `customers`), and the hit list from `spp_core::search::search` —
//! the FTS5 engine behind `/api/search` (the route's semantic re-ranking
//! for embeddings is skipped in-process the same way the web page degrades
//! when LM Studio is offline; `used_semantic` reflects it).

use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::search::{SearchFilters, SearchResponse};

use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

/// Search scopes (web: the scope radio group).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    All,
    Tickets,
    Knowledge,
    Customers,
}

impl Scope {
    const ALL: [Self; 4] = [Self::All, Self::Tickets, Self::Knowledge, Self::Customers];

    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Tickets => "Tickets",
            Self::Knowledge => "Knowledge",
            Self::Customers => "Customers",
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Tickets => "tickets",
            Self::Knowledge => "knowledge",
            Self::Customers => "customers",
        }
    }
}

/// The interactive search page state.
pub struct SearchPage {
    query_edit: String,
    query: String,
    scope: Scope,
    status_filter: Option<String>,
    response: Option<SearchResponse>,
    error: Option<String>,
    /// Shared vector index (A4): the fused search's dense lane.
    vector_index: Option<std::sync::Arc<spp_core::vectorstore_sqlitevec::VectorIndex>>,
}

impl SearchPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            query_edit: String::new(),
            query: String::new(),
            scope: Scope::All,
            status_filter: None,
            response: None,
            error: None,
            vector_index: None,
        }
    }

    /// A4: receive the shared vector index so search can fuse BM25 +
    /// sqlite-vec hits instead of the hardcoded FTS-only mode.
    pub fn set_vector_index(
        &mut self,
        index: Option<std::sync::Arc<spp_core::vectorstore_sqlitevec::VectorIndex>>,
    ) {
        self.vector_index = index;
    }

    /// Live-update invalidation (web: the search version key).
    pub fn invalidate(&mut self) {
        if self.query.is_empty() {
            return;
        }
        self.response = None;
    }

    fn reload(&mut self, db: &DbHandle) {
        let query = self.query.trim().to_string();
        if query.is_empty() {
            self.response = None;
            self.error = None;
            return;
        }
        let filters = SearchFilters {
            status: self.status_filter.clone(),
            ..SearchFilters::default()
        };
        let result = db.with_read(|conn| {
            spp_core::search::search_with_semantics(
                conn,
                self.vector_index.as_deref(),
                &query,
                self.scope.as_str(),
                &filters,
            )
        });
        match result {
            Ok(response) => {
                self.response = Some(response);
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        // Clean-audit item 147: the trim guard — a whitespace-only query
        // can never produce a response (reload early-returns), so without
        // the trim this lane re-ran the (no-op) reload every frame.
        if self.response.is_none() && self.error.is_none() && !self.query.trim().is_empty() {
            self.reload(db);
        }
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        design::page_header(
            ui,
            "Search",
            Some("conversations, knowledge, and customers in one query"),
            |_| {},
        );
        ui.horizontal(|ui| {
            let mut query = self.query_edit.clone();
            let response = ui.add_sized(
                [320.0, 28.0],
                egui::TextEdit::singleline(&mut query).hint_text("Search…"),
            );
            if response.changed() {
                self.query_edit = query;
            }
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (response.lost_focus()
                && design::button(ui, "search", ButtonVariant::Primary).clicked())
                || (response.lost_focus() && enter)
            {
                self.query = self.query_edit.clone();
                self.response = None;
                self.error = None;
                self.reload(db);
            }
            if design::button(ui, "clear", ButtonVariant::Ghost).clicked() {
                self.query_edit.clear();
                self.query.clear();
                self.response = None;
                self.error = None;
            }
        });
        ui.horizontal(|ui| {
            design::hint_text(ui, "scope:");
            for scope in Scope::ALL {
                if design::chip(ui, self.scope == scope, scope.label()).clicked() {
                    self.scope = scope;
                    if !self.query.is_empty() {
                        self.reload(db);
                    }
                }
            }
        });
        design::divider(ui);

        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
            return;
        }
        let Some(response) = &self.response else {
            if self.query.is_empty() {
                design::empty_state(
                    ui,
                    Icon::Search,
                    "Search across your workspace",
                    "Type a query and press Enter to search conversations, knowledge, and customers.",
                );
            } else {
                ui.spinner();
            }
            return;
        };

        design::hint_text(
            ui,
            &format!(
                "{} hit{} for {:?}{}",
                response.total,
                if response.total == 1 { "" } else { "s" },
                response.query,
                if response.used_semantic {
                    " (semantic)"
                } else {
                    ""
                }
            ),
        );
        // A4: the honest mode line — which lanes actually ran, and why the
        // semantic lane was skipped when it was.
        if let Some(note) = &response.mode_note {
            design::hint_text(ui, note);
        }
        if response.hits.is_empty() {
            design::empty_state(
                ui,
                Icon::Search,
                "(no matches)",
                "Nothing matched this query in the selected scope.",
            );
            return;
        }
        let hits = response.hits.clone();
        egui::ScrollArea::vertical().show_rows(ui, 40.0, hits.len(), |ui, range| {
            for hit in &hits[range] {
                design::card(ui, |ui| {
                    ui.horizontal(|ui| {
                        design::badge(ui, hit.scope.as_str(), BadgeTone::Neutral);
                        ui.monospace(
                            egui::RichText::new(hit.title.as_str())
                                .color(design::tokens(ui).foreground),
                        );
                        if !hit.subtitle.is_empty() {
                            design::hint_text(ui, hit.subtitle.as_str());
                        }
                        // A4: real lane provenance — "keyword match",
                        // "semantic match", "recent".
                        for reason in &hit.why {
                            design::badge(
                                ui,
                                reason,
                                if reason == "semantic match" {
                                    BadgeTone::Info
                                } else {
                                    BadgeTone::Neutral
                                },
                            );
                        }
                    });
                    if !hit.snippet.is_empty() {
                        design::hint_text(ui, hit.snippet.as_str());
                    }
                });
            }
        });
    }
}

impl Default for SearchPage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    /// Seed one knowledge document + chunk (the store behind the search
    /// page's knowledge scope) and rebuild the FTS indexes.
    fn seed_knowledge_doc(conn: &rusqlite::Connection) {
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
        let _ = spp_core::search::rebuild_indexes(conn);
    }

    #[test]
    fn search_finds_seeded_conversation_by_word() {
        let db = fixture(vec![(
            "conv",
            1,
            "Export broken",
            "active",
            "email",
            1,
            5,
            9,
        )]);
        // Direct table writes bypass the sync path that maintains the FTS
        // indexes, so rebuild them before searching (same as the sync
        // pipeline does after an upsert).
        db.with_write(|conn| spp_core::search::rebuild_indexes(conn))
            .expect("rebuild");
        let mut page = SearchPage::new();
        page.query = "Export".into();
        page.reload(&db);
        let response = page.response.clone().expect("response");
        assert!(!response.hits.is_empty());
        assert!(response.hits.iter().any(|h| h.title.contains("Export")));
        assert!(!response.used_semantic);
    }

    #[test]
    fn empty_query_keeps_previous_results_unsearched() {
        let db = fixture(Vec::new());
        let mut page = SearchPage::new();
        page.query = String::new();
        page.reload(&db);
        assert!(page.response.is_none());
        assert!(page.error.is_none());
    }

    #[test]
    fn knowledge_scope_hits_seeded_article() {
        let db = fixture(Vec::new());
        db.with_write(|conn| seed_knowledge_doc(conn));
        let mut page = SearchPage::new();
        page.scope = Scope::Knowledge;
        page.query = "password".into();
        page.reload(&db);
        let response = page.response.clone().expect("response 2");
        assert!(response.hits.iter().any(|h| h.scope == "knowledge"));
    }

    #[test]
    fn scope_labels_and_values_map() {
        assert_eq!(Scope::All.as_str(), "all");
        assert_eq!(Scope::Tickets.as_str(), "tickets");
        assert_eq!(Scope::Knowledge.as_str(), "knowledge");
        assert_eq!(Scope::Customers.as_str(), "customers");
        assert_eq!(Scope::Tickets.label(), "Tickets");
    }
}
