use std::path::PathBuf;

use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::event_bus::EventBus;

use super::{ConfirmRequest, NavRequest, i64_field, opt_str_field, str_field};
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

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

    fn parse(s: &str) -> Self {
        if s == "http" { Self::Http } else { Self::Csv }
    }
}

/// AUD-P1-2 (C2): HTTP connector credentials — the shapes the fetch branch
/// already consumes (`connectors.rs` auth modes). CSV connectors read local
/// files and need none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthMode {
    None,
    Header,
    Bearer,
}

impl AuthMode {
    const ALL: [Self; 3] = [Self::None, Self::Header, Self::Bearer];

    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Header => "header",
            Self::Bearer => "bearer",
        }
    }
}

/// AUD-P1-2 (C2): build the auth JSON from the form fields — the exact
/// payload shapes the HTTP fetch branch understands.
fn auth_json(
    mode: AuthMode,
    header_name: &str,
    header_value: &str,
    token: &str,
) -> serde_json::Value {
    match mode {
        AuthMode::None => serde_json::json!({}),
        AuthMode::Header => serde_json::json!({
            "mode": "header",
            "headerName": header_name.trim(),
            "headerValue": header_value.trim(),
        }),
        AuthMode::Bearer => serde_json::json!({
            "mode": "bearer",
            "token": token.trim(),
        }),
    }
}

/// AUD-P1-2 (C2): prefill the auth form from a stored connector record.
fn auth_prefill(record: &serde_json::Value) -> (AuthMode, String, String, String) {
    let auth = record.get("auth").cloned().unwrap_or(serde_json::json!({}));
    let mode = auth.get("mode").and_then(|m| m.as_str()).unwrap_or("");
    match mode {
        "header" => (
            AuthMode::Header,
            auth.get("headerName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            auth.get("headerValue")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            String::new(),
        ),
        "bearer" => (
            AuthMode::Bearer,
            String::new(),
            String::new(),
            auth.get("token")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        ),
        _ => (AuthMode::None, String::new(), String::new(), String::new()),
    }
}

fn health_tone(health: &str) -> BadgeTone {
    match health {
        "ok" => BadgeTone::Success,
        "stale" => BadgeTone::Warning,
        "error" => BadgeTone::Destructive,
        _ => BadgeTone::Neutral,
    }
}

pub struct ConnectorsPage {
    connectors: Vec<serde_json::Value>,
    selected: Option<i64>,
    rows: Vec<serde_json::Value>,
    rows_total: i64,
    rows_query: String,
    show_new: bool,
    /// AUD-P1-2 (C2): Some(id) puts the form into edit mode for that
    /// connector (prefilled); None is the plain create form.
    edit_id: Option<i64>,
    new_name: String,
    new_kind: Kind,
    new_file: String,
    new_url: String,
    new_auth_mode: AuthMode,
    new_header_name: String,
    new_header_value: String,
    new_token: String,
    new_refresh_seconds: String,
    new_allowed_ai: bool,
    message: Option<String>,
    error: Option<String>,
    data_dir: Option<PathBuf>,
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
            edit_id: None,
            new_name: String::new(),
            new_kind: Kind::Csv,
            new_file: String::new(),
            new_url: String::new(),
            new_auth_mode: AuthMode::None,
            new_header_name: String::new(),
            new_header_value: String::new(),
            new_token: String::new(),
            new_refresh_seconds: "3600".into(),
            new_allowed_ai: false,
            message: None,
            error: None,
            data_dir: None,
        }
    }

    pub(crate) fn set_data_dir(&mut self, data_dir: PathBuf) {
        self.data_dir = Some(data_dir);
    }

    pub(crate) fn invalidate(&mut self) {
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

    /// AUD-P1-2 (C2): shared form validation — name, refresh cadence, the
    /// per-kind source field, and (new) HTTP credential completeness.
    /// Returns (name, refresh_seconds, config, auth).
    fn validate_form(&mut self) -> Option<(String, i64, serde_json::Value, serde_json::Value)> {
        let name = self.new_name.trim().to_string();
        if name.is_empty() {
            self.error = Some("name required".into());
            return None;
        }
        let refresh_seconds: i64 = match self.new_refresh_seconds.trim().parse::<i64>() {
            Ok(v) if (60..=86_400).contains(&v) => v,
            _ => {
                self.error =
                    Some("refresh seconds must be a whole number between 60 and 86400".into());
                return None;
            }
        };
        let (file, url) = match self.new_kind {
            Kind::Csv => {
                if self.new_file.trim().is_empty() {
                    self.error = Some("file path required for a csv connector".into());
                    return None;
                }
                (self.new_file.trim().to_string(), String::new())
            }
            Kind::Http => {
                if self.new_url.trim().is_empty() {
                    self.error = Some("url required for an http connector".into());
                    return None;
                }
                (String::new(), self.new_url.trim().to_string())
            }
        };
        if self.new_kind == Kind::Http {
            match self.new_auth_mode {
                AuthMode::Header
                    if self.new_header_name.trim().is_empty()
                        || self.new_header_value.trim().is_empty() =>
                {
                    self.error = Some("header auth needs both a header name and a value".into());
                    return None;
                }
                AuthMode::Bearer if self.new_token.trim().is_empty() => {
                    self.error = Some("bearer auth needs a token".into());
                    return None;
                }
                _ => {}
            }
        }
        let config = serde_json::json!({
            "kind": self.new_kind.as_str(),
            "file": file,
            "url": url,
        });
        let auth = if self.new_kind == Kind::Http {
            auth_json(
                self.new_auth_mode,
                &self.new_header_name,
                &self.new_header_value,
                &self.new_token,
            )
        } else {
            serde_json::json!({})
        };
        Some((name, refresh_seconds, config, auth))
    }

    fn create(&mut self, db: &DbHandle) {
        let Some((name, refresh_seconds, config, auth)) = self.validate_form() else {
            return;
        };
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

    /// AUD-P1-2 (C2): save the edit form back to the connector.
    fn save_edit(&mut self, db: &DbHandle) {
        let Some(id) = self.edit_id else { return };
        let Some((name, refresh_seconds, config, auth)) = self.validate_form() else {
            return;
        };
        let result = db.with_write(|conn| {
            spp_core::connectors::update(
                conn,
                id,
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
                self.edit_id = None;
                self.show_new = false;
                self.selected = Some(i64_field(&record, "id"));
                self.message = Some(format!("connector {name} updated"));
                self.reload(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// AUD-P1-2 (C2): open the form prefilled with an existing connector.
    fn start_edit(&mut self, record: &serde_json::Value) {
        self.edit_id = Some(i64_field(record, "id"));
        self.show_new = true;
        self.error = None;
        self.message = None;
        self.new_name = str_field(record, "name");
        self.new_kind = Kind::parse(&str_field(record, "kind"));
        let config = record
            .get("config")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        self.new_file = config
            .get("file")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        self.new_url = config
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        self.new_refresh_seconds = i64_field(record, "refresh_seconds").to_string();
        self.new_allowed_ai = i64_field(record, "allowed_ai") != 0;
        let (mode, header_name, header_value, token) = auth_prefill(record);
        self.new_auth_mode = mode;
        self.new_header_name = header_name;
        self.new_header_value = header_value;
        self.new_token = token;
    }

    fn close_form(&mut self) {
        self.show_new = false;
        self.edit_id = None;
        self.error = None;
    }

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

    pub fn refresh(&mut self, db: &DbHandle, bus: &EventBus, id: i64) {
        let db = db.clone();
        let bus = bus.clone();
        let data_dir = self.data_dir.clone().unwrap_or_else(std::env::temp_dir);
        std::thread::Builder::new()
            .name("spp-connector-refresh".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                if let Ok(rt) = runtime {
                    rt.block_on(async move {
                        // AUD-P1-2 (C2): core emits `ConnectorRefreshed` on
                        // the bus — the shell toasts the outcome and
                        // invalidates this page, so health/last-sync/rows
                        // land without waiting for an unrelated sync.
                        let _ = spp_core::connectors::refresh(&db, Some(&bus), id, &data_dir).await;
                    });
                }
            })
            .ok();
        self.message = Some("refresh started".into());
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, bus: &EventBus) -> Option<NavRequest> {
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
                design::page_counter(ui, &format!("{} connectors", self.connectors.len()));
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
        let mut edit_requested: Option<serde_json::Value> = None;
        // UXG-1-5: exact-column data_table (virtualized rows built in) —
        // the connector list spans the content column at every width; the
        // empty case gets a real empty state instead of a header-only
        // strip (the automation register).
        if connectors.is_empty() {
            design::empty_state(
                ui,
                Icon::Plug,
                "No connectors yet",
                "Create a connector with \"New Connector\" above — HTTP endpoints \
                 and CSV files attach as read-only tables the AI tools can query.",
            );
        } else {
            let selected = self.selected;
            design::data_table(
                ui,
                "connector-table",
                &["name", "kind", "health", "refresh", "last sync", ""],
                connectors.len(),
                26.0,
                |ui, i| {
                    let t = design::tokens(ui);
                    let record = &connectors[i];
                    let id = i64_field(record, "id");
                    let name = str_field(record, "name");
                    let kind = str_field(record, "kind");
                    let health = str_field(record, "health");
                    let refresh_method = str_field(record, "refresh_method");
                    let last_sync =
                        opt_str_field(record, "last_sync_at").unwrap_or_else(|| "—".into());
                    // AUD-P1-2 (C2): the last-sync cell answers "and how many
                    // rows?" — the count core already records — and the
                    // failure detail rides the hover.
                    let last_sync_rows = record.get("last_sync_rows").and_then(|v| v.as_i64());
                    let sync_label = match last_sync_rows {
                        Some(n) => format!("{last_sync} · {n} rows"),
                        None => last_sync.clone(),
                    };
                    let sync_hover = opt_str_field(record, "last_sync_error")
                        .or_else(|| opt_str_field(record, "last_sync_status"));
                    if design::chip(ui, selected == Some(id), &name).clicked() {
                        picked = Some(id);
                    }
                    ui.monospace(egui::RichText::new(kind).color(t.muted_foreground));
                    ui.horizontal(|ui| {
                        design::status_dot(ui, health_tone(&health), false);
                        design::badge(ui, &health, health_tone(&health));
                    });
                    ui.label(
                        egui::RichText::new(refresh_method)
                            .size(13.0)
                            .color(t.foreground),
                    );
                    ui.monospace(egui::RichText::new(sync_label).color(t.muted_foreground))
                        .on_hover_text(sync_hover.unwrap_or_else(|| {
                            "the last refresh outcome for this connector".into()
                        }));
                    ui.horizontal(|ui| {
                        if design::button_with_icon(
                            ui,
                            Icon::Refresh,
                            "refresh",
                            ButtonVariant::Outline,
                        )
                        .on_hover_text("fetch once now — the outcome lands as a toast and here")
                        .clicked()
                        {
                            refreshed = Some(id);
                        }
                        // AUD-P1-2 (C2): edit an existing connector — name,
                        // source, credentials, cadence, AI permission.
                        if design::button(ui, "Edit", ButtonVariant::Outline).clicked() {
                            edit_requested = Some(record.clone());
                        }
                        if design::button(ui, "Delete", ButtonVariant::Destructive).clicked() {
                            deleted = Some((id, name.clone()));
                        }
                    });
                },
            );
        }
        if let Some(id) = picked {
            self.selected = Some(id);
            self.reload_rows(db, id);
        }
        if let Some(record) = edit_requested {
            self.start_edit(&record);
        }
        if let Some(id) = refreshed {
            self.refresh(db, bus, id);
        }
        let nav = deleted
            .map(|(id, name)| NavRequest::Confirm(ConfirmRequest::delete_connector(id, &name)));

        if self.selected.is_some() {
            ui.add_space(10.0);
            self.rows_ui(ui, db);
        }
        // VF-3 (C5): the short-content end-of-page marker — the connector
        // list is short by nature; the note bounds the page with the true
        // count instead of leaving the bottom half an unmanaged void.
        if !connectors.is_empty() {
            design::list_end_note(
                ui,
                &format!(
                    "{} connector{} · read-only tables the AI tools can query",
                    connectors.len(),
                    if connectors.len() == 1 { "" } else { "s" }
                ),
            );
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
                if design::button_with_icon(ui, Icon::Search, "Search", ButtonVariant::Primary)
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
        let editing = self.edit_id.is_some();
        design::card(ui, |ui| {
            design::section_title(
                ui,
                if editing {
                    "Edit Connector"
                } else {
                    "New Connector"
                },
            );
            design::form_grid(ui, "connector-new", |ui| {
                let name_label = design::field_label(ui, "Name");
                ui.add_sized(
                    [220.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_name).hint_text("connector name"),
                )
                .labelled_by(name_label.id);
                ui.end_row();
                design::field_label(ui, "Kind");
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
                        let file_label = design::field_label(ui, "File (data dir)");
                        ui.add_sized(
                            [260.0, 28.0],
                            egui::TextEdit::singleline(&mut self.new_file).hint_text("billing.csv"),
                        )
                        .on_hover_text("relative to the app data directory")
                        .labelled_by(file_label.id);
                        ui.end_row();
                    }
                    Kind::Http => {
                        let url_label = design::field_label(ui, "Url");
                        ui.add_sized(
                            [280.0, 28.0],
                            egui::TextEdit::singleline(&mut self.new_url).hint_text("https://…"),
                        )
                        .labelled_by(url_label.id);
                        ui.end_row();
                    }
                }
                let refresh_label = design::field_label(ui, "Refresh seconds");
                ui.add_sized(
                    [140.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_refresh_seconds).hint_text("3600"),
                )
                .labelled_by(refresh_label.id);
                ui.end_row();
                design::field_label(ui, "Allow AI");
                ui.checkbox(&mut self.new_allowed_ai, "AI tools may query");
                ui.end_row();
            });
            // AUD-P1-2 (C2): HTTP credentials — the shapes the fetch branch
            // already consumes. Before this, `auth` was hard-coded to an
            // empty object at creation and could never be configured or
            // edited from the UI at all.
            if self.new_kind == Kind::Http {
                ui.add_space(4.0);
                design::form_grid(ui, "connector-auth", |ui| {
                    design::field_label(ui, "Credentials");
                    ui.horizontal(|ui| {
                        for mode in AuthMode::ALL {
                            if design::chip(ui, self.new_auth_mode == mode, mode.label()).clicked()
                            {
                                self.new_auth_mode = mode;
                            }
                        }
                    })
                    .response
                    .on_hover_text(
                        "http connectors only — a custom header (e.g. an API key) \
                         or a bearer token; csv connectors read local files",
                    );
                    ui.end_row();
                    match self.new_auth_mode {
                        AuthMode::None => {}
                        AuthMode::Header => {
                            let hn_label = design::field_label(ui, "Header name");
                            ui.add_sized(
                                [180.0, 28.0],
                                egui::TextEdit::singleline(&mut self.new_header_name)
                                    .hint_text("X-Api-Key"),
                            )
                            .labelled_by(hn_label.id);
                            ui.end_row();
                            let hv_label = design::field_label(ui, "Header value");
                            ui.add_sized(
                                [260.0, 28.0],
                                egui::TextEdit::singleline(&mut self.new_header_value)
                                    .hint_text("the API key value"),
                            )
                            .labelled_by(hv_label.id);
                            ui.end_row();
                        }
                        AuthMode::Bearer => {
                            let token_label = design::field_label(ui, "Bearer token");
                            ui.add_sized(
                                [280.0, 28.0],
                                egui::TextEdit::singleline(&mut self.new_token)
                                    .hint_text("the bearer token"),
                            )
                            .labelled_by(token_label.id);
                            ui.end_row();
                        }
                    }
                });
            }
            ui.horizontal(|ui| {
                if design::button(
                    ui,
                    if editing { "Save Changes" } else { "Create" },
                    ButtonVariant::Primary,
                )
                .clicked()
                {
                    if editing {
                        self.save_edit(db);
                    } else {
                        self.create(db);
                    }
                }
                if editing && design::button(ui, "Cancel", ButtonVariant::Outline).clicked() {
                    self.close_form();
                }
            });
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
        page.new_file = "a.csv".into();
        page.create(&db);
        assert!(page.error.is_none());

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

    // ---- AUD-P1-2 (C2): credentials, edit, refresh-event feedback ----

    #[test]
    fn audp12_http_credentials_ride_the_create_path() {
        let db = fixture(Vec::new());
        let mut page = ConnectorsPage::new();
        page.set_data_dir(std::env::temp_dir());
        page.new_name = "Ledger API".into();
        page.new_kind = Kind::Http;
        page.new_url = "https://example.com/ledger".into();
        page.new_auth_mode = AuthMode::Header;
        page.new_header_name = "X-Api-Key".into();
        page.new_header_value = "sekrit".into();
        page.create(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        let record = page.connectors[0].clone();
        let auth = record.get("auth").cloned().unwrap_or_default();
        assert_eq!(auth.get("mode").and_then(|m| m.as_str()), Some("header"));
        assert_eq!(
            auth.get("headerName").and_then(|v| v.as_str()),
            Some("X-Api-Key")
        );
        assert_eq!(
            auth.get("headerValue").and_then(|v| v.as_str()),
            Some("sekrit")
        );
    }

    #[test]
    fn audp12_incomplete_credentials_are_rejected() {
        let db = fixture(Vec::new());
        let mut page = ConnectorsPage::new();
        page.set_data_dir(std::env::temp_dir());
        page.new_name = "Broken".into();
        page.new_kind = Kind::Http;
        page.new_url = "https://example.com".into();
        page.new_auth_mode = AuthMode::Header;
        page.new_header_name = "X-Api-Key".into();
        // header value missing
        page.create(&db);
        let error = page.error.clone().expect("rejected");
        assert!(error.contains("header auth"), "got: {error}");
        assert!(page.connectors.is_empty());

        page.error = None;
        page.new_auth_mode = AuthMode::Bearer;
        // token missing
        page.create(&db);
        let error = page.error.clone().expect("rejected");
        assert!(error.contains("bearer auth"), "got: {error}");
    }

    #[test]
    fn audp12_edit_round_trips_name_source_and_credentials() {
        let db = fixture(Vec::new());
        let mut page = ConnectorsPage::new();
        page.set_data_dir(std::env::temp_dir());
        page.new_name = "Ledger".into();
        page.new_kind = Kind::Http;
        page.new_url = "https://example.com/a".into();
        page.new_auth_mode = AuthMode::Bearer;
        page.new_token = "tok-1".into();
        page.create(&db);
        assert!(page.error.is_none(), "{:?}", page.error);

        // open the edit form from the stored record — prefilled
        let record = page.connectors[0].clone();
        page.start_edit(&record);
        assert_eq!(page.edit_id, Some(i64_field(&record, "id")));
        assert_eq!(page.new_name, "Ledger");
        assert_eq!(page.new_kind, Kind::Http);
        assert_eq!(page.new_url, "https://example.com/a");
        assert_eq!(page.new_auth_mode, AuthMode::Bearer);
        assert_eq!(page.new_token, "tok-1");

        // change the credentials and the source, save
        page.new_url = "https://example.com/b".into();
        page.new_auth_mode = AuthMode::Header;
        page.new_header_name = "X-Api-Key".into();
        page.new_header_value = "sekrit".into();
        page.new_token.clear();
        page.save_edit(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert!(page.edit_id.is_none(), "edit mode closes on save");

        let updated = page.connectors[0].clone();
        assert_eq!(
            updated
                .get("config")
                .and_then(|c| c.get("url"))
                .and_then(|v| v.as_str()),
            Some("https://example.com/b")
        );
        let auth = updated.get("auth").cloned().unwrap_or_default();
        assert_eq!(auth.get("mode").and_then(|m| m.as_str()), Some("header"));
        assert_eq!(
            auth.get("headerName").and_then(|v| v.as_str()),
            Some("X-Api-Key")
        );
    }

    #[test]
    fn audp12_edit_keeps_its_own_name_but_rejects_a_duplicate() {
        let db = fixture(Vec::new());
        let mut page = ConnectorsPage::new();
        page.set_data_dir(std::env::temp_dir());
        page.new_name = "One".into();
        page.new_file = "one.csv".into();
        page.create(&db);
        page.new_name = "Two".into();
        page.new_file = "two.csv".into();
        page.create(&db);
        assert!(page.error.is_none(), "{:?}", page.error);

        // editing "One" without touching its name is fine
        let one = page
            .connectors
            .iter()
            .find(|c| str_field(c, "name") == "One")
            .cloned()
            .expect("one");
        page.start_edit(&one);
        page.new_file = "one-v2.csv".into();
        page.save_edit(&db);
        assert!(page.error.is_none(), "{:?}", page.error);

        // renaming to the other connector's name is rejected
        let one = page
            .connectors
            .iter()
            .find(|c| str_field(c, "name") == "One")
            .cloned()
            .expect("one");
        page.start_edit(&one);
        page.new_name = "Two".into();
        page.save_edit(&db);
        let error = page.error.clone().expect("duplicate rejected");
        assert!(error.contains("already exists"), "got: {error}");
    }
}
