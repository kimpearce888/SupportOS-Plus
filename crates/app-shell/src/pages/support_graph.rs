//! Support Graph page (port of the web `pages/support_graph.rs`).
//!
//! Presentational pass (shadcn redesign): this page renders through the
//! shared design system in `crate::design`; the queries, state, and
//! behavior are unchanged.
//!
//! Search box → `spp_core::support_graph::graph_search` (the `/api/graph/search`
//! SQL) listing node references; picking one loads its neighborhood via
//! `neighbors_json` (the `/api/graph/neighbors/{kind}/{id}` SQL, direction
//! `both`, limit 5 — same defaults as the web page) and renders the edge
//! list. The header stats row ports `graph_stats`.

use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::support_graph::GraphNodeRef;

use crate::design::{self, ButtonVariant};
use crate::icons::Icon;

/// Parsed neighborhood view of one graph node.
#[derive(Debug, Clone, Default)]
pub struct Neighborhood {
    pub center: Option<GraphNodeRef>,
    pub edges: Vec<(String, String, serde_json::Value)>,
}

impl Neighborhood {
    fn from_json(value: serde_json::Value) -> Self {
        let center: Option<GraphNodeRef> = value
            .get("node")
            .and_then(|c| serde_json::from_value(c.clone()).ok());
        let edges = value
            .get("edges")
            .and_then(|e| e.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        let relation = super::str_field(e, "relation");
                        if relation.is_empty() {
                            return None;
                        }
                        // Each edge carries (source, target); the far side
                        // is whichever end is not the center node.
                        let source = e.get("source");
                        let target = e.get("target");
                        let far = match &center {
                            Some(c) => {
                                let source_is_center = source
                                    .map(|s| {
                                        super::str_field(s, "kind") == c.kind
                                            && super::i64_field(s, "local_id") == c.local_id
                                    })
                                    .unwrap_or(false);
                                if source_is_center {
                                    target
                                } else {
                                    source.or(target)
                                }
                            }
                            None => target.or(source),
                        };
                        Some((
                            relation,
                            super::str_field(e, "origin"),
                            far.cloned().unwrap_or(serde_json::Value::Null),
                        ))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Self { center, edges }
    }

    fn target_label(target: &serde_json::Value) -> String {
        let label = super::str_field(target, "label");
        if label.is_empty() {
            return "(removed)".into();
        }
        label
    }
}

/// The interactive support-graph page state.
pub struct SupportGraphPage {
    query_edit: String,
    query: String,
    results: Vec<serde_json::Value>,
    stats: Option<serde_json::Value>,
    selected: Option<(String, i64)>,
    neighborhood: Option<Neighborhood>,
    error: Option<String>,
    /// Clean-audit item 144: invalidation clears the visible results of
    /// an active search — this flag schedules the refetch on the next
    /// frame so the search survives sync events instead of blanking
    /// until the user re-submits it.
    results_stale: bool,
    /// Audit fix (M21): when the stats query errors, `stats` stays None —
    /// without a backoff stamp the aggregate query re-ran every frame.
    stats_at: Option<std::time::Instant>,
}

impl SupportGraphPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            query_edit: String::new(),
            query: String::new(),
            results: Vec::new(),
            stats: None,
            selected: None,
            neighborhood: None,
            error: None,
            results_stale: false,
            // Audit fix (M21): last stats-reload attempt — a failing
            // stats query used to re-run on the UI thread every frame.
            stats_at: None,
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.stats = None;
        // Audit fix (M21): an explicit invalidation reloads next frame.
        self.stats_at = None;
        if self.query.is_empty() {
            return;
        }
        self.results.clear();
        // Clean-audit item 144: the cleared search refetches next frame.
        self.results_stale = true;
    }

    /// Clean-audit item 144: the invalidation-cleared search refetches
    /// against the refreshed mirror.
    fn refetch_if_stale(&mut self, db: &DbHandle) {
        if self.results_stale {
            self.results_stale = false;
            self.reload_results(db);
        }
    }

    fn reload_stats(&mut self, db: &DbHandle) {
        self.stats = db.with_read(spp_core::support_graph::graph_stats).ok();
    }

    fn reload_results(&mut self, db: &DbHandle) {
        let query = self.query.trim().to_string();
        if query.is_empty() {
            self.results.clear();
            return;
        }
        let results =
            db.with_read(|conn| spp_core::support_graph::graph_search(conn, &query, None));
        match results {
            Ok(results) => {
                self.results = results;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn load_neighborhood(&mut self, db: &DbHandle, kind: &str, id: i64) {
        let Some(parsed_kind) = kind_from_str(kind) else {
            self.error = Some(format!("unknown node kind {kind}"));
            return;
        };
        let result = db.with_read(|conn| {
            spp_core::support_graph::neighbors_json(conn, parsed_kind, id, "both", 5)
        });
        match result {
            Ok(Some(value)) => {
                self.neighborhood = Some(Neighborhood::from_json(value));
                self.error = None;
            }
            Ok(None) => {
                self.neighborhood = None;
                self.error = Some(format!("node {kind}#{id} not found"));
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        // Audit fix (M21): when the stats query fails, `stats` stays None
        // and this re-ran the aggregate query every rendered frame —
        // throttle reload attempts to at most one per second.
        if self.stats.is_none() && self.stats_at.is_none_or(|t| t.elapsed().as_secs() >= 1) {
            self.stats_at = Some(std::time::Instant::now());
            self.reload_stats(db);
        }
        self.refetch_if_stale(db);

        let subtitle = if let Some(stats) = &self.stats {
            format!(
                "{} conversation links · {} customer links · {} human edges",
                super::i64_field(stats, "conversation_links"),
                super::i64_field(stats, "customer_links"),
                super::i64_field(stats, "human_edges")
            )
        } else {
            "linked customers, conversations, and users".to_string()
        };
        design::page_header(ui, "Support Graph", Some(subtitle.as_str()), |_| {});
        ui.horizontal(|ui| {
            let mut query = self.query_edit.clone();
            let response = ui.add_sized(
                [320.0, 28.0],
                egui::TextEdit::singleline(&mut query).hint_text("Search the graph…"),
            );
            if response.changed() {
                self.query_edit = query;
            }
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if design::button(ui, "search", ButtonVariant::Primary).clicked()
                || (response.lost_focus() && enter)
            {
                self.query = self.query_edit.clone();
                self.selected = None;
                self.neighborhood = None;
                self.reload_results(db);
            }
        });

        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        design::divider(ui);

        if self.query.is_empty() {
            design::empty_state(
                ui,
                Icon::ShareGraph,
                "Search the graph",
                "Search customers, conversations, organizations, users, or incidents by name, subject, or number.",
            );
            return;
        }

        let results = self.results.clone();
        design::section_title(ui, &format!("{} node(s)", results.len()));
        let mut clicked: Option<(String, i64)> = None;
        egui::ScrollArea::vertical()
            .id_salt("graph-results")
            .show(ui, |ui| {
                for node in &results {
                    let kind = super::str_field(node, "kind");
                    let id = super::i64_field(node, "local_id");
                    let label = super::str_field(node, "label");
                    let sub = super::opt_str_field(node, "sublabel");
                    let selected = self.selected == Some((kind.clone(), id));
                    if design::chip(ui, selected, &format!("{kind}: {label}")).clicked() {
                        clicked = Some((kind.clone(), id));
                    }
                    if let Some(sub) = sub {
                        design::hint_text(ui, &format!("   {sub}"));
                    }
                }
            });
        if let Some((kind, id)) = clicked {
            self.selected = Some((kind.clone(), id));
            self.load_neighborhood(db, &kind, id);
        }
        design::divider(ui);

        let neighborhood = self.neighborhood.clone();
        if let Some(neighborhood) = neighborhood {
            design::card_titled(ui, "Neighborhood", None, |ui| {
                if let Some(center) = &neighborhood.center {
                    ui.label(
                        egui::RichText::new(format!("{} #{}", center.kind, center.local_id))
                            .size(13.0)
                            .strong()
                            .color(design::tokens(ui).foreground),
                    );
                    if !center.label.is_empty() {
                        design::hint_text(ui, &center.label);
                    }
                }
                if neighborhood.edges.is_empty() {
                    design::empty_state(
                        ui,
                        Icon::ShareGraph,
                        "(no edges)",
                        "This node has no links yet.",
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("graph-edges")
                        .show_rows(ui, 20.0, neighborhood.edges.len(), |ui, range| {
                            let t = design::tokens(ui);
                            for (relation, origin, target) in &neighborhood.edges[range] {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{relation}{} {}",
                                        if origin.is_empty() {
                                            String::new()
                                        } else {
                                            format!(" ({origin})")
                                        },
                                        Neighborhood::target_label(target)
                                    ))
                                    .size(13.0)
                                    .color(t.foreground),
                                );
                            }
                        });
                }
            });
        }
    }
}

impl Default for SupportGraphPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a wire node kind (graceful for unknown values).
fn kind_from_str(kind: &str) -> Option<spp_core::catalog::GraphNodeKind> {
    spp_core::catalog::GraphNodeKind::ALL
        .into_iter()
        .find(|k| k.as_str() == kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn graph_search_finds_seeded_customer_and_conversation() {
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
        let mut page = SupportGraphPage::new();
        page.query = "Kim".into();
        page.reload_results(&db);
        assert!(
            page.results
                .iter()
                .any(|n| super::super::str_field(n, "kind") == "customer")
        );

        page.query = "Export".into();
        page.reload_results(&db);
        assert!(
            page.results
                .iter()
                .any(|n| super::super::str_field(n, "kind") == "conversation")
        );
    }

    /// Clean-audit item 144: invalidation used to clear the visible
    /// results of an active search with nothing refetching them — the
    /// search stayed blank until the user re-submitted it. The cleared
    /// search now refetches on the next frame.
    #[test]
    fn invalidation_refetches_the_cleared_search() {
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
        let mut page = SupportGraphPage::new();
        page.query = "Export".into();
        page.query_edit = page.query.clone();
        page.reload_results(&db);
        assert!(!page.results.is_empty());

        // A sync event invalidates the page...
        page.invalidate();
        assert!(page.results.is_empty(), "the cache is cleared");
        assert!(page.results_stale, "a refetch is scheduled");

        // ...and the next frame's refetch restores the active search.
        page.refetch_if_stale(&db);
        assert!(
            !page.results.is_empty(),
            "the active search survives the sync event"
        );
        assert!(!page.results_stale, "the refetch consumed the flag");
    }

    #[test]
    fn neighborhood_loads_edges_for_customer() {
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
        let mut page = SupportGraphPage::new();
        page.load_neighborhood(&db, "customer", 9);
        assert!(page.error.is_none());
        let neighborhood = page.neighborhood.clone().expect("neighborhood");
        let center = neighborhood.center.expect("center");
        assert_eq!(center.kind, "customer");
        assert_eq!(center.local_id, 9);
        assert!(center.label.contains("Kim"));
    }

    #[test]
    fn unknown_node_reports_error_without_panicking() {
        let db = fixture(Vec::new());
        let mut page = SupportGraphPage::new();
        page.load_neighborhood(&db, "customer", 999);
        assert!(page.error.is_some());
        assert!(page.neighborhood.is_none());

        page.load_neighborhood(&db, "spaceship", 1);
        assert!(page.error.is_some());
    }

    #[test]
    fn kind_from_str_maps_all_known_kinds() {
        assert!(kind_from_str("customer").is_some());
        assert!(kind_from_str("conversation").is_some());
        assert!(kind_from_str("nonsense").is_none());
    }

    #[test]
    fn neighborhood_json_parses_center_and_edges() {
        let value = serde_json::json!({
            "node": {"kind": "customer", "local_id": 9, "label": "Kim", "sublabel": null, "deleted": false},
            "edges": [
                {"relation": "conversation", "origin": "derived", "source": {"kind": "customer", "local_id": 9, "label": "Kim"}, "target": {"kind": "conversation", "local_id": 1, "label": "#501 Export"}},
                {"relation": "owner", "origin": "human_local", "source": {"kind": "user", "local_id": 5, "label": "Ada"}, "target": {"kind": "customer", "local_id": 9, "label": "Kim"}}
            ]
        });
        let n = Neighborhood::from_json(value);
        assert_eq!(n.center.expect("center").local_id, 9);
        assert_eq!(n.edges.len(), 2);
        // Out-edge: far side is the target.
        assert_eq!(Neighborhood::target_label(&n.edges[0].2), "#501 Export");
        // In-edge: far side is the source.
        assert_eq!(Neighborhood::target_label(&n.edges[1].2), "Ada");
    }
}
