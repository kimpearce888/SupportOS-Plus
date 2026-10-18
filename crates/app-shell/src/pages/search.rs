//! Search page (port of the web `pages/search.rs`).
//!
//! One input, scope selector (`all` / `tickets` / `knowledge` /
//! `customers`), and the hit list from `spp_core::search::search` —
//! the FTS5 engine behind `/api/search` (the route's semantic re-ranking
//! for embeddings is skipped in-process the same way the web page degrades
//! when LM Studio is offline; `used_semantic` reflects it).

use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::search::{SearchFilters, SearchResponse};

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
        }
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
            spp_core::search::search(conn, &query, self.scope.as_str(), &filters)
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
        if self.response.is_none() && self.error.is_none() && !self.query.is_empty() {
            self.reload(db);
        }
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        ui.heading("Search");
        ui.horizontal(|ui| {
            let mut query = self.query_edit.clone();
            let response = ui.text_edit_singleline(&mut query);
            if response.changed() {
                self.query_edit = query;
            }
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (response.lost_focus() && ui.button("search").clicked())
                || (response.lost_focus() && enter)
            {
                self.query = self.query_edit.clone();
                self.response = None;
                self.error = None;
                self.reload(db);
            }
            if ui.button("clear").clicked() {
                self.query_edit.clear();
                self.query.clear();
                self.response = None;
                self.error = None;
            }
        });
        ui.horizontal(|ui| {
            ui.label("scope:");
            for scope in Scope::ALL {
                if ui
                    .selectable_label(self.scope == scope, scope.label())
                    .clicked()
                {
                    self.scope = scope;
                    if !self.query.is_empty() {
                        self.reload(db);
                    }
                }
            }
        });
        ui.separator();

        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("error: {err}"));
            return;
        }
        let Some(response) = &self.response else {
            if self.query.is_empty() {
                ui.weak("Type a query and press Enter to search conversations, knowledge, and customers.");
            } else {
                ui.spinner();
            }
            return;
        };

        ui.weak(format!(
            "{} hit{} for {:?}{}",
            response.total,
            if response.total == 1 { "" } else { "s" },
            response.query,
            if response.used_semantic {
                " (semantic)"
            } else {
                ""
            }
        ));
        if response.hits.is_empty() {
            ui.weak("(no matches)");
            return;
        }
        let hits = response.hits.clone();
        egui::ScrollArea::vertical().show_rows(ui, 40.0, hits.len(), |ui, range| {
            for hit in &hits[range] {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(hit.scope.as_str());
                        ui.monospace(hit.title.as_str());
                        if !hit.subtitle.is_empty() {
                            ui.weak(hit.subtitle.as_str());
                        }
                    });
                    if !hit.snippet.is_empty() {
                        ui.weak(hit.snippet.as_str());
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
