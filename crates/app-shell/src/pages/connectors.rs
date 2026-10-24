//! Connectors page (port of the web `pages/connectors.rs`).
//!
//! Redesigned with the shadcn-style design system (`crate::design`):
//! health badges on a striped connector table, card-wrapped rows viewer
//! and a form-grid creator — presentation only, refresh behavior intact.
//!
//! Connector table (`connectors::list` — `/api/connectors`), a rows viewer
//! for the selected connector (`connectors::list_rows` —
//! `/api/connectors/{id}/rows`), the create form (`connectors::create`),
//! delete, and the refresh action (`connectors::refresh`, the route logic
//! without the HTTP hop, run on a background thread).

use std::path::PathBuf;

use eframe::egui;
use spp_core::db::DbHandle;

use super::{ConfirmRequest, NavRequest, i64_field, opt_str_field, str_field};
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

/// Connector kinds offered by the create form (web: kind select).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Http,
    Csv,
}

impl Kind {
    const ALL: [Self; 2] = [Self::Http, Self::Csv];

    fn label(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Csv => "csv",
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Csv => "csv",
        }
    }
}

/// Badge color for the health column (web: `health_badge_class`).
#[must_use]
pub fn health_color(health: &str) -> egui::Color32 {
    match health {
        "ok" => egui::Color32::GREEN,
        "stale" => egui::Color32::YELLOW,
        "error" => egui::Color32::RED,
        _ => egui::Color32::GRAY,
    }
}

/// Health → badge tone for the redesigned table (the badge equivalent
/// of [`health_color`]; display text stays the raw health value).
fn health_tone(health: &str) -> BadgeTone {
    match health {
        "ok" => BadgeTone::Success,
        "stale" => BadgeTone::Warning,
        "error" => BadgeTone::Destructive,
        _ => BadgeTone::Neutral,
    }
}

/// The interactive connectors page state.
pub struct ConnectorsPage {
    connectors: Vec<serde_json::Value>,
    selected: Option<i64>,
    rows: Vec<serde_json::Value>,
    rows_total: i64,
    rows_query: String,
    show_new: bool,
    new_name: String,
    new_kind: Kind,
    new_file: String,
    new_url: String,
    new_refresh_seconds: String,
    new_allowed_ai: bool,
    message: Option<String>,
    error: Option<String>,
    data_dir: Option<PathBuf>,
    /// Clean-audit item 147: reload latch — an empty mirror is a legal
    /// steady state, so "list empty" can no longer mean "re-query every
    /// frame".
    loaded: bool,
}

impl ConnectorsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            connectors: Vec::new(),
            loaded: false,
            selected: None,
            rows: Vec::new(),
            rows_total: 0,
            rows_query: String::new(),
            show_new: false,
            new_name: String::new(),
            new_kind: Kind::Csv,
            new_file: String::new(),
            new_url: String::new(),
            new_refresh_seconds: "3600".into(),
            new_allowed_ai: false,
            message: None,
            error: None,
            data_dir: None,
        }
    }

    /// Data directory for file-kind connectors (set by the shell at boot).
    pub fn set_data_dir(&mut self, data_dir: PathBuf) {
        self.data_dir = Some(data_dir);
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.loaded = false;
        self.connectors.clear();
        self.rows.clear();
    }

    fn reload(&mut self, db: &DbHandle) {
        let connectors = db.with_read(spp_core::connectors::list);
        match connectors {
            Ok(connectors) => {
                self.connectors = connectors;
                self.error = None;
                if let Some(id) = self.selected
                    && !self.connectors.iter().any(|c| i64_field(c, "id") == id)
                {
                    self.selected = None;
                    self.rows.clear();
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.loaded = true;
        if let Some(id) = self.selected {
            self.reload_rows(db, id);
        }
    }

    fn reload_rows(&mut self, db: &DbHandle, id: i64) {
        let q = self.rows_query.trim().to_string();
        let result = db.with_read(|conn| {
            spp_core::connectors::list_rows(
                conn,
                id,
                if q.is_empty() { None } else { Some(q.as_str()) },
                50,
                0,
            )
        });
        match result {
            Ok((rows, total)) => {
                self.rows = rows;
                self.rows_total = total;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if !self.loaded {
            self.reload(db);
        }
    }

    fn create(&mut self, db: &DbHandle) {
        let name = self.new_name.trim().to_string();
        if name.is_empty() {
            self.error = Some("name required".into());
            return;
        }
        let refresh_seconds: i64 = self
            .new_refresh_seconds
            .trim()
            .parse()
            .unwrap_or(3600)
            .clamp(60, 86400);
        let (file, url) = match self.new_kind {
            Kind::Csv => (self.new_file.trim().to_string(), String::new()),
            Kind::Http => (String::new(), self.new_url.trim().to_string()),
        };
        let config = serde_json::json!({
            "kind": self.new_kind.as_str(),
            "file": file,
            "url": url,
        });
        let auth = serde_json::json!({});
        let result = db.with_write(|conn| {
            spp_core::connectors::create(
                conn,
                &name,
                self.new_kind.as_str(),
                &config,
                &auth,
                "manual",
                refresh_seconds,
                self.new_allowed_ai,
            )
        });
        match result {
            Ok(record) => {
                self.show_new = false;
                self.new_name.clear();
                self.selected = Some(i64_field(&record, "id"));
                self.message = Some(format!("connector {name} created"));
                self.reload(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Delete a connector (runs on shell confirmation only — clean-audit
    /// item 140). Returns the core outcome for the shell's toast.
    pub fn delete(&mut self, db: &DbHandle, id: i64) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::connectors::delete(conn, id));
        match &result {
            Ok(_) => {
                if self.selected == Some(id) {
                    self.selected = None;
                    self.rows.clear();
                }
                self.message = Some("connector deleted".into());
                self.reload(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        result.map(|_| ())
    }

    /// Refresh one connector on a background thread (the async route
    /// handler, minus HTTP).
    pub fn refresh(&mut self, db: &DbHandle, id: i64) {
        let db = db.clone();
        let data_dir = self.data_dir.clone().unwrap_or_else(std::env::temp_dir);
        std::thread::Builder::new()
            .name("spp-connector-refresh".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                if let Ok(rt) = runtime {
                    rt.block_on(async move {
                        let _ = spp_core::connectors::refresh(&db, id, &data_dir).await;
                    });
                }
            })
            .ok();
        self.message = Some("refresh started".into());
    }

    /// Render the page. Returns a confirm request when the user asked to
    /// delete a connector (clean-audit item 140 — the delete itself runs
    /// only after the shell dialog confirms).
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        self.ensure_loaded(db);

        design::page_header(
            ui,
            "Connectors",
            Some(
                "Attach external data sources (HTTP endpoints, CSV files inside the data \
                 directory) as read-only tables; the AI tools can query them when allowed.",
            ),
            |ui| {
                if design::button_with_icon(ui, Icon::Plus, "New Connector", ButtonVariant::Primary)
                    .clicked()
                {
                    self.show_new = !self.show_new;
                }
            },
        );
        if let Some(message) = &self.message {
            design::success_text(ui, &format!("status: {message}"));
        }
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        design::divider(ui);

        if self.show_new {
            self.new_connector_ui(ui, db);
            ui.add_space(10.0);
        }

        let connectors = self.connectors.clone();
        let mut picked: Option<i64> = None;
        let mut deleted: Option<(i64, String)> = None;
        let mut refreshed: Option<i64> = None;
        egui::ScrollArea::vertical()
            .id_salt("connector-list")
            .show(ui, |ui| {
                design::table(
                    ui,
                    "connector-table",
                    &["name", "kind", "health", "refresh", "last sync", ""],
                    26.0,
                    |ui| {
                        let t = design::tokens(ui);
                        for record in &connectors {
                            let id = i64_field(record, "id");
                            let name = str_field(record, "name");
                            let kind = str_field(record, "kind");
                            let health = str_field(record, "health");
                            let refresh_method = str_field(record, "refresh_method");
                            let last_sync =
                                opt_str_field(record, "last_sync_at").unwrap_or_else(|| "—".into());
                            if design::chip(ui, self.selected == Some(id), &name).clicked() {
                                picked = Some(id);
                            }
                            ui.monospace(egui::RichText::new(kind).color(t.foreground));
                            design::badge(ui, &health, health_tone(&health));
                            ui.label(
                                egui::RichText::new(refresh_method)
                                    .size(13.0)
                                    .color(t.foreground),
                            );
                            ui.monospace(egui::RichText::new(last_sync).color(t.foreground));
                            ui.horizontal(|ui| {
                                if design::button(ui, "rows", ButtonVariant::Outline).clicked() {
                                    picked = Some(id);
                                }
                                if design::button_with_icon(
                                    ui,
                                    Icon::Refresh,
                                    "refresh",
                                    ButtonVariant::Outline,
                                )
                                .clicked()
                                {
                                    refreshed = Some(id);
                                }
                                if design::button(ui, "delete", ButtonVariant::Destructive)
                                    .clicked()
                                {
                                    deleted = Some((id, name.clone()));
                                }
                            });
                            ui.end_row();
                        }
                    },
                );
            });
        if let Some(id) = picked {
            self.selected = Some(id);
            self.reload_rows(db, id);
        }
        if let Some(id) = refreshed {
            self.refresh(db, id);
        }
        // Clean-audit item 140: the delete arms the shell dialog instead
        // of running unconfirmed.
        let nav = deleted
            .map(|(id, name)| NavRequest::Confirm(ConfirmRequest::delete_connector(id, &name)));

        if self.selected.is_some() {
            ui.add_space(10.0);
            self.rows_ui(ui, db);
        }
        nav
    }

    fn rows_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let t = design::tokens(ui);
        let rows = self.rows.clone();
        let rows_query = self.rows_query.clone();
        let rows_total = self.rows_total;
        design::card(ui, |ui| {
            design::section_title(ui, &format!("Rows ({rows_total})"));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let mut q = rows_query;
                let response = ui.add_sized(
                    [220.0, 28.0],
                    egui::TextEdit::singleline(&mut q).hint_text("search rows…"),
                );
                if response.changed() {
                    self.rows_query = q;
                }
                if design::button_with_icon(ui, Icon::Search, "search", ButtonVariant::Primary)
                    .clicked()
                    && let Some(id) = self.selected
                {
                    self.reload_rows(db, id);
                }
            });
            if rows.is_empty() {
                design::empty_state(
                    ui,
                    Icon::Plug,
                    "No rows",
                    "Create the connector with data or run a refresh.",
                );
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt("connector-rows")
                .show(ui, |ui| {
                    for row in &rows {
                        let data = row.get("data").and_then(|d| d.as_str()).unwrap_or("");
                        ui.monospace(
                            egui::RichText::new(format!("{} {data}", i64_field(row, "id")))
                                .color(t.foreground),
                        );
                    }
                });
        });
    }

    fn new_connector_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card(ui, |ui| {
            design::section_title(ui, "New Connector");
            design::form_grid(ui, "connector-new", |ui| {
                design::field_label(ui, "name:");
                ui.add_sized(
                    [220.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_name).hint_text("connector name"),
                );
                ui.end_row();
                design::field_label(ui, "kind:");
                ui.horizontal(|ui| {
                    for kind in Kind::ALL {
                        if design::chip(ui, self.new_kind == kind, kind.label()).clicked() {
                            self.new_kind = kind;
                        }
                    }
                });
                ui.end_row();
                match self.new_kind {
                    Kind::Csv => {
                        design::field_label(ui, "file (data dir):");
                        ui.add_sized(
                            [260.0, 28.0],
                            egui::TextEdit::singleline(&mut self.new_file).hint_text("billing.csv"),
                        )
                        .on_hover_text("relative to the app data directory");
                        ui.end_row();
                    }
                    Kind::Http => {
                        design::field_label(ui, "url:");
                        ui.add_sized(
                            [280.0, 28.0],
                            egui::TextEdit::singleline(&mut self.new_url).hint_text("https://…"),
                        );
                        ui.end_row();
                    }
                }
                design::field_label(ui, "refresh seconds:");
                ui.add_sized(
                    [140.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_refresh_seconds).hint_text("3600"),
                );
                ui.end_row();
                design::field_label(ui, "allow AI:");
                ui.checkbox(&mut self.new_allowed_ai, "AI tools may query");
                ui.end_row();
            });
            if design::button(ui, "create", ButtonVariant::Primary).clicked() {
                self.create(db);
            }
        });
    }
}

impl Default for ConnectorsPage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn create_lists_and_deletes_connectors() {
        let db = fixture(Vec::new());
        let mut page = ConnectorsPage::new();
        page.set_data_dir(std::env::temp_dir());
        page.reload(&db);
        assert!(page.connectors.is_empty());

        page.new_name = "Billing export".into();
        page.new_kind = Kind::Csv;
        page.new_file = "billing.csv".into();
        page.create(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.connectors.len(), 1);
        let record = page.connectors[0].clone();
        assert_eq!(str_field(&record, "name"), "Billing export");
        assert_eq!(str_field(&record, "kind"), "csv");
        assert_eq!(str_field(&record, "refresh_method"), "manual");
        let id = i64_field(&record, "id");

        let _ = page.delete(&db, id);
        assert!(page.connectors.is_empty());
        assert!(page.selected.is_none());
    }

    #[test]
    fn duplicate_names_are_rejected() {
        let db = fixture(Vec::new());
        let mut page = ConnectorsPage::new();
        page.set_data_dir(std::env::temp_dir());
        page.new_name = "Same".into();
        page.create(&db);
        assert!(page.error.is_none());

        // The successful create clears the form; retype the same name.
        page.new_name = "Same".into();
        page.create(&db);
        let error = page.error.clone().expect("duplicate rejected");
        assert!(error.contains("already exists"), "got: {error}");
    }

    #[test]
    fn rows_view_reads_seeded_connector_rows() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            let record = spp_core::connectors::create(
                conn,
                "Products",
                "csv",
                &serde_json::json!({"kind": "csv", "file": "products.csv"}),
                &serde_json::json!({}),
                "manual",
                3600,
                false,
            )
            .expect("create");
            let id = i64_field(&record, "id");
            conn.execute(
                "INSERT INTO connector_rows (connector_id, row_key, data, fetched_at)
                 VALUES (?1, 'a', '{\"sku\":\"A-1\"}', datetime('now')),
                        (?1, 'b', '{\"sku\":\"B-2\"}', datetime('now'))",
                rusqlite::params![id],
            )
            .expect("rows");
        });
        let mut page = ConnectorsPage::new();
        page.reload(&db);
        assert_eq!(page.connectors.len(), 1);
        let id = i64_field(&page.connectors[0], "id");
        page.selected = Some(id);
        page.reload_rows(&db, id);
        assert_eq!(page.rows_total, 2);
        assert_eq!(page.rows.len(), 2);
    }

    #[test]
    fn health_badge_colors_map() {
        assert_eq!(health_color("ok"), egui::Color32::GREEN);
        assert_eq!(health_color("stale"), egui::Color32::YELLOW);
        assert_eq!(health_color("error"), egui::Color32::RED);
        assert_eq!(health_color("unknown"), egui::Color32::GRAY);
    }
}
