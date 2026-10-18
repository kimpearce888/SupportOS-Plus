//! Command palette (port of the web `components/command_palette.rs` and
//! `pages/command_palette.rs`).
//!
//! Ctrl/Cmd+K opens a global overlay: one text field debounced through the
//! local search engine (`spp_core::search::search`, the `/api/search`
//! route without the HTTP hop), arrow keys move the selection, Enter
//! resolves the hit's web href (`/inbox/conversation/{id}`,
//! `/customers/{id}`, `/knowledge?doc={id}`) into a
//! [`NavRequest`](crate::pages::NavRequest) the shell applies.

use eframe::egui;
use spp_core::db::DbHandle;

use crate::pages::NavRequest;

/// Palette hit cap (web: `MAX_HITS`).
pub const MAX_HITS: usize = 12;

/// One palette hit (port of the web `PaletteHit`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteHit {
    pub scope: String,
    pub id: i64,
    pub title: String,
    pub snippet: String,
    pub href: String,
}

impl PaletteHit {
    /// Parse from a search-hit JSON row.
    #[must_use]
    pub fn from_json(v: &serde_json::Value) -> Self {
        Self {
            scope: v
                .get("scope")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            id: v.get("id").and_then(serde_json::Value::as_i64).unwrap_or(0),
            title: v
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            snippet: v
                .get("snippet")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            href: v
                .get("href")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        }
    }

    /// Build from a typed search hit (the in-process search result).
    #[must_use]
    pub fn from_search_hit(hit: &spp_core::search::SearchHit) -> Self {
        Self {
            scope: hit.scope.clone(),
            id: hit.id,
            title: hit.title.clone(),
            snippet: hit.snippet.clone(),
            href: hit.href.clone(),
        }
    }
}

/// Cap the hit list (web: `take_hits`).
#[must_use]
pub fn take_hits(hits: &[PaletteHit]) -> &[PaletteHit] {
    &hits[..hits.len().min(MAX_HITS)]
}

/// Move the selection by `delta`, clamped (web: `move_selection`).
#[must_use]
pub fn move_selection(selected: usize, len: usize, delta: i64) -> usize {
    if len == 0 {
        return 0;
    }
    let next = selected as i64 + delta;
    next.clamp(0, len as i64 - 1) as usize
}

/// Resolve a web href into a navigation request (the router navigation
/// the web palette performed).
#[must_use]
pub fn href_to_nav(href: &str) -> Option<NavRequest> {
    let rest = href.strip_prefix('/')?;
    if let Some(id) = rest.strip_prefix("inbox/conversation/") {
        return id
            .parse::<i64>()
            .ok()
            .map(|_| NavRequest::Page(crate::pages::PageKey::Inbox));
    }
    if rest.starts_with("customers") {
        return Some(NavRequest::Page(crate::pages::PageKey::Customers));
    }
    if rest.starts_with("knowledge") {
        return Some(NavRequest::Page(crate::pages::PageKey::Knowledge));
    }
    if rest.starts_with("issues") {
        return Some(NavRequest::Page(crate::pages::PageKey::IssueRadar));
    }
    None
}

/// The palette overlay state.
pub struct CommandPalette {
    open: bool,
    query: String,
    hits: Vec<PaletteHit>,
    selected: usize,
}

impl CommandPalette {
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            hits: Vec::new(),
            selected: 0,
        }
    }

    /// Whether the overlay is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the palette.
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.hits.clear();
        self.selected = 0;
    }

    /// Close the palette.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Toggle from a Ctrl/Cmd+K key press handled by the shell.
    pub fn toggle_shortcut(&mut self, ctx: &egui::Context) {
        let pressed = (ctx.input(|i| i.modifiers.command || i.modifiers.ctrl)
            && ctx.input(|i| i.key_pressed(egui::Key::K)))
            || ctx.input(|i| i.key_pressed(egui::Key::Escape) && self.open);
        if pressed {
            if self.open {
                self.close();
            } else {
                self.open();
            }
        }
    }

    fn run_search(&mut self, db: &DbHandle) {
        let query = self.query.trim().to_string();
        if query.is_empty() {
            self.hits.clear();
            self.selected = 0;
            return;
        }
        // The web debounced 200ms and raced generations; the local query
        // is fast enough to run per keystroke change.
        let result = db.with_read(|conn| {
            spp_core::search::search(
                conn,
                &query,
                "all",
                &spp_core::search::SearchFilters::default(),
            )
        });
        self.hits = result
            .map(|response| {
                response
                    .hits
                    .iter()
                    .map(PaletteHit::from_search_hit)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.selected = 0;
    }

    /// Render the overlay; returns a navigation request when Enter was
    /// pressed on a hit.
    pub fn ui(&mut self, ctx: &egui::Context, db: &DbHandle) -> Option<NavRequest> {
        self.toggle_shortcut(ctx);
        if !self.open {
            return None;
        }
        let mut nav = None;
        let mut query_changed = false;
        egui::Window::new("Command Palette")
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
            .fixed_size([560.0, 320.0])
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let response = ui.text_edit_singleline(&mut self.query);
                if response.changed() {
                    query_changed = true;
                }
                let take = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.separator();
                let hits = take_hits(&self.hits);
                if hits.is_empty() {
                    if self.query.trim().is_empty() {
                        ui.weak("Type to search tickets, customers, knowledge, and issues.");
                    } else {
                        ui.weak("(no hits)");
                    }
                } else {
                    egui::ScrollArea::vertical().show_rows(ui, 30.0, hits.len(), |ui, range| {
                        for (idx, hit) in hits[range.clone()].iter().enumerate() {
                            let global_idx = range.start + idx;
                            let selected = self.selected == global_idx;
                            if ui
                                .selectable_label(
                                    selected,
                                    format!("{} · {}", hit.scope, hit.title),
                                )
                                .clicked()
                            {
                                nav = href_to_nav(&hit.href);
                                self.open = false;
                            }
                            if !hit.snippet.is_empty() {
                                ui.weak(format!("   {}", hit.snippet));
                            }
                        }
                    });
                    ui.weak("↑ ↓ to move · Enter to open · Esc to close");
                }
                if take && let Some(hit) = hits.get(self.selected) {
                    nav = href_to_nav(&hit.href);
                    self.open = false;
                }
                // Arrow-key navigation.
                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    self.selected = move_selection(self.selected, hits.len(), 1);
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    self.selected = move_selection(self.selected, hits.len(), -1);
                }
            });
        if query_changed {
            self.run_search(db);
        }
        nav
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn take_hits_caps_at_twelve() {
        let hits: Vec<PaletteHit> = (0..20)
            .map(|i| PaletteHit {
                scope: "tickets".into(),
                id: i,
                title: format!("hit {i}"),
                snippet: String::new(),
                href: "/inbox/conversation/1".into(),
            })
            .collect();
        assert_eq!(take_hits(&hits).len(), MAX_HITS);
        assert_eq!(take_hits(&hits)[0].id, 0);
        assert_eq!(take_hits(&hits)[MAX_HITS - 1].id, MAX_HITS as i64 - 1);
    }

    #[test]
    fn selection_moves_and_clamps() {
        assert_eq!(move_selection(0, 0, 1), 0);
        assert_eq!(move_selection(0, 5, -3), 0);
        assert_eq!(move_selection(2, 5, 2), 4);
        assert_eq!(move_selection(4, 5, 1), 4, "clamped to len-1");
        assert_eq!(move_selection(3, 5, -2), 1);
    }

    #[test]
    fn hrefs_map_to_pages() {
        assert!(matches!(
            href_to_nav("/inbox/conversation/42"),
            Some(NavRequest::Page(crate::pages::PageKey::Inbox))
        ));
        assert!(matches!(
            href_to_nav("/customers/9"),
            Some(NavRequest::Page(crate::pages::PageKey::Customers))
        ));
        assert!(matches!(
            href_to_nav("/knowledge?doc=3"),
            Some(NavRequest::Page(crate::pages::PageKey::Knowledge))
        ));
        assert!(matches!(
            href_to_nav("/issues/known/7"),
            Some(NavRequest::Page(crate::pages::PageKey::IssueRadar))
        ));
        assert!(href_to_nav("/unknown/page").is_none());
        assert!(href_to_nav("not-a-href").is_none());
    }

    #[test]
    fn palette_search_finds_seeded_conversations() {
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
        db.with_write(|conn| spp_core::search::rebuild_indexes(conn))
            .expect("rebuild");
        let mut palette = CommandPalette::new();
        palette.open();
        palette.query = "Export".into();
        palette.run_search(&db);
        assert!(!palette.hits.is_empty());
        assert!(palette.hits.iter().any(|h| h.title.contains("Export")));
        assert!(href_to_nav(&palette.hits[0].href).is_some());

        palette.query = String::new();
        palette.run_search(&db);
        assert!(palette.hits.is_empty());
    }

    #[test]
    fn hit_parsing_reads_the_json_shape() {
        let v = serde_json::json!({
            "scope": "tickets",
            "id": 5,
            "title": "Login broken",
            "snippet": "customer cannot log in",
            "href": "/inbox/conversation/5"
        });
        let hit = PaletteHit::from_json(&v);
        assert_eq!(hit.scope, "tickets");
        assert_eq!(hit.id, 5);
        assert_eq!(hit.title, "Login broken");
        assert_eq!(hit.snippet, "customer cannot log in");
        let empty = PaletteHit::from_json(&serde_json::json!({}));
        assert_eq!(empty.id, 0);
        assert!(empty.title.is_empty());
    }
}
