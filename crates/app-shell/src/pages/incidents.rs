//! Incidents page (port of the web `pages/incidents.rs`).
//!
//! The incident list with status/severity/open/query filters
//! (`incident_workspace::list_incident_rows` — the `/api/incidents` SQL),
//! the detail workspace (conversations, affected customers/organizations,
//! notes, releases — `/api/incidents/{id}` family), the new-incident form
//! (the create route's validation + `intelligence_features::
//! create_manual_incident`), add-note, and link-conversation actions.

use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::incident_workspace::IncidentListFilters;
use spp_core::intelligence_features::ManualIncident;

use super::{i64_field, opt_str_field, str_field};

/// Incident statuses (web: the status select).
pub const INCIDENT_STATUSES: [&str; 5] = [
    "investigating",
    "identified",
    "fix_in_progress",
    "monitoring",
    "resolved",
];

/// Incident severities (web: the severity select).
pub const INCIDENT_SEVERITIES: [&str; 4] = ["sev1", "sev2", "sev3", "sev4"];

/// The interactive incidents page state.
pub struct IncidentsPage {
    filter_status: Option<String>,
    filter_severity: Option<String>,
    filter_open_only: bool,
    filter_query: String,
    applied_query: String,
    rows: Vec<serde_json::Value>,
    total: i64,
    selected: Option<i64>,
    detail: Option<serde_json::Value>,
    conversations: Vec<serde_json::Value>,
    customers: Vec<serde_json::Value>,
    notes: Vec<serde_json::Value>,
    releases: Vec<serde_json::Value>,
    show_new: bool,
    new_title: String,
    new_status: String,
    new_severity: String,
    new_description: String,
    new_conversation_ids: String,
    new_note: String,
    link_conversation: String,
    error: Option<String>,
}

impl IncidentsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            filter_status: None,
            filter_severity: None,
            filter_open_only: false,
            filter_query: String::new(),
            applied_query: String::new(),
            rows: Vec::new(),
            total: 0,
            selected: None,
            detail: None,
            conversations: Vec::new(),
            customers: Vec::new(),
            notes: Vec::new(),
            releases: Vec::new(),
            show_new: false,
            new_title: String::new(),
            new_status: "investigating".into(),
            new_severity: "sev3".into(),
            new_description: String::new(),
            new_conversation_ids: String::new(),
            new_note: String::new(),
            link_conversation: String::new(),
            error: None,
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.rows.clear();
        self.detail = None;
    }

    fn reload_list(&mut self, db: &DbHandle) {
        let filters = IncidentListFilters {
            status: self.filter_status.clone(),
            severity: self.filter_severity.clone(),
            open: self.filter_open_only,
            query: Some(self.applied_query.clone()).filter(|q| !q.is_empty()),
            limit: 100,
            offset: 0,
        };
        let result =
            db.with_read(|conn| spp_core::incident_workspace::list_incident_rows(conn, &filters));
        match result {
            Ok((rows, total)) => {
                self.rows = rows;
                self.total = total;
                self.error = None;
                if let Some(id) = self.selected
                    && !self.rows.iter().any(|r| i64_field(r, "id") == id)
                {
                    // Detail may still exist beyond the filtered page;
                    // keep the selection, just refresh it.
                    self.reload_detail(db, id);
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn reload_detail(&mut self, db: &DbHandle, id: i64) {
        let (detail, conversations, customers, notes, releases) = db.with_read(|conn| {
            (
                spp_core::incident_workspace::incident_detail(conn, id),
                spp_core::incident_workspace::list_conversations(conn, id, 100),
                spp_core::incident_workspace::affected_customers(conn, id, 100),
                spp_core::incident_workspace::list_notes(conn, id, 100),
                spp_core::incident_workspace::list_releases(conn, id),
            )
        });
        self.detail = detail;
        self.conversations = conversations;
        self.customers = customers;
        self.notes = notes;
        self.releases = releases;
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.rows.is_empty() {
            self.reload_list(db);
        }
    }

    fn select(&mut self, db: &DbHandle, id: i64) {
        self.selected = Some(id);
        self.reload_detail(db, id);
    }

    fn create(&mut self, db: &DbHandle) {
        let title = self.new_title.trim().to_string();
        if title.is_empty() || title.chars().count() > 200 {
            self.error = Some("Title is required (max 200 characters).".into());
            return;
        }
        if !INCIDENT_STATUSES.contains(&self.new_status.as_str()) {
            self.error = Some("Invalid status.".into());
            return;
        }
        if !INCIDENT_SEVERITIES.contains(&self.new_severity.as_str()) {
            self.error = Some("Invalid severity.".into());
            return;
        }
        let conversation_ids: Vec<i64> = self
            .new_conversation_ids
            .split(',')
            .filter_map(|id| id.trim().parse::<i64>().ok().filter(|v| *v > 0))
            .collect();
        // The route validated each id exists; skip missing ones the same way.
        let valid_ids = db.with_read(|conn| {
            conversation_ids
                .into_iter()
                .filter(|id| {
                    conn.query_row(
                        "SELECT id FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
                        rusqlite::params![id],
                        |r| r.get::<_, i64>(0),
                    )
                    .is_ok()
                })
                .collect::<Vec<_>>()
        });
        let incident = ManualIncident {
            title,
            status: self.new_status.clone(),
            severity: self.new_severity.clone(),
            owner_user_local_id: None,
            product: None,
            feature: None,
            description: if self.new_description.trim().is_empty() {
                None
            } else {
                Some(self.new_description.trim().to_string())
            },
            internal_explanation: None,
            customer_safe_explanation: None,
            known_cause: None,
            workaround: None,
            resolution: None,
            started_at: None,
            conversation_ids: valid_ids,
        };
        let result = db.with_write(|conn| {
            spp_core::intelligence_features::create_manual_incident(conn, &incident)
        });
        match result {
            Ok(id) => {
                self.show_new = false;
                self.new_title.clear();
                self.new_description.clear();
                self.new_conversation_ids.clear();
                self.reload_list(db);
                self.select(db, id);
            }
            Err(e) => self.error = Some(format!("Could not create incident: {e}")),
        }
    }

    fn add_note(&mut self, db: &DbHandle) {
        let Some(incident_id) = self.selected else {
            return;
        };
        let note = self.new_note.trim();
        if note.is_empty() || note.chars().count() > 4000 {
            self.error = Some("A non-empty note body is required.".into());
            return;
        }
        let result = db.with_write(|conn| {
            spp_core::intelligence_features::add_incident_note(conn, incident_id, None, note)
        });
        match result {
            Ok(_) => {
                self.new_note.clear();
                self.reload_detail(db, incident_id);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn link(&mut self, db: &DbHandle) {
        let Some(incident_id) = self.selected else {
            return;
        };
        let Some(conversation_id) = self
            .link_conversation
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|v| *v > 0)
        else {
            self.error = Some("Conversation id must be a positive integer.".into());
            return;
        };
        let result = db.with_write(|conn| {
            spp_core::intelligence_features::link_incident_conversation(
                conn,
                incident_id,
                conversation_id,
                "manual",
            )
        });
        match result {
            Ok(_) => {
                self.link_conversation.clear();
                self.reload_detail(db, incident_id);
                self.reload_list(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        ui.heading("Incidents");
        ui.weak(format!("{} incident(s)", self.total));
        ui.horizontal(|ui| {
            if ui.button("New Incident").clicked() {
                self.show_new = !self.show_new;
            }
        });
        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("error: {err}"));
        }
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("status:");
            egui::ComboBox::from_id_salt("incident-filter-status")
                .selected_text(self.filter_status.clone().unwrap_or_else(|| "Any".into()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filter_status, None, "Any");
                    for status in INCIDENT_STATUSES {
                        ui.selectable_value(&mut self.filter_status, Some(status.into()), status);
                    }
                });
            ui.label("severity:");
            egui::ComboBox::from_id_salt("incident-filter-severity")
                .selected_text(self.filter_severity.clone().unwrap_or_else(|| "Any".into()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filter_severity, None, "Any");
                    for severity in INCIDENT_SEVERITIES {
                        ui.selectable_value(
                            &mut self.filter_severity,
                            Some(severity.into()),
                            severity,
                        );
                    }
                });
            ui.checkbox(&mut self.filter_open_only, "open only");
            let mut q = self.filter_query.clone();
            ui.text_edit_singleline(&mut q);
            self.filter_query = q;
            if ui.button("apply").clicked() {
                self.applied_query = self.filter_query.trim().to_string();
                self.reload_list(db);
            }
        });
        ui.separator();

        if self.show_new {
            self.new_incident_ui(ui, db);
            ui.separator();
        }

        ui.columns(2, |columns| {
            let rows = self.rows.clone();
            let mut picked: Option<i64> = None;
            egui::ScrollArea::vertical()
                .id_salt("incident-list")
                .show(&mut columns[0], |ui| {
                    for row in &rows {
                        let id = i64_field(row, "id");
                        let code = str_field(row, "code");
                        let title = str_field(row, "title");
                        let status = str_field(row, "status");
                        let severity = str_field(row, "severity");
                        let convs = i64_field(row, "conversation_count");
                        let selected = self.selected == Some(id);
                        let color = match severity.as_str() {
                            "sev1" => egui::Color32::RED,
                            "sev2" => egui::Color32::YELLOW,
                            _ => egui::Color32::GRAY,
                        };
                        if ui
                            .selectable_label(selected, format!("{code} {title}"))
                            .clicked()
                        {
                            picked = Some(id);
                        }
                        ui.horizontal(|ui| {
                            ui.colored_label(color, severity);
                            ui.weak(status);
                            ui.weak(format!("{convs} conversations"));
                        });
                        ui.add_space(4.0);
                    }
                    if rows.is_empty() {
                        ui.weak("(no incidents match)");
                    }
                });
            if let Some(id) = picked {
                self.select(db, id);
            }
            self.detail_ui(&mut columns[1], db);
        });
    }

    fn detail_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let Some(detail) = self.detail.clone() else {
            ui.weak("Select an incident to open its workspace.");
            return;
        };
        let _incident_id = i64_field(&detail, "id");
        ui.heading(format!(
            "{} · {}",
            str_field(&detail, "code"),
            str_field(&detail, "title")
        ));
        ui.horizontal(|ui| {
            ui.strong("status:");
            ui.monospace(str_field(&detail, "status"));
            ui.strong("severity:");
            ui.monospace(str_field(&detail, "severity"));
            if let Some(started) = opt_str_field(&detail, "started_at") {
                ui.strong("started:");
                ui.monospace(started);
            }
        });
        if let Some(owner) = opt_str_field(&detail, "owner_name") {
            ui.weak(format!("owner: {owner}"));
        }
        for (label, key) in [
            ("description", "description"),
            ("customer-safe", "customer_safe_explanation"),
            ("workaround", "workaround"),
            ("resolution", "resolution"),
        ] {
            if let Some(text) = opt_str_field(&detail, key) {
                ui.strong(label);
                ui.label(text);
            }
        }
        ui.separator();

        ui.strong(format!(
            "Linked conversations ({})",
            self.conversations.len()
        ));
        for conv in &self.conversations {
            ui.label(format!(
                "#{} · {}",
                i64_field(conv, "number"),
                str_field(conv, "subject")
            ));
        }
        ui.horizontal(|ui| {
            let mut link = self.link_conversation.clone();
            ui.label("link conversation #:");
            ui.text_edit_singleline(&mut link);
            self.link_conversation = link;
            if ui.button("link").clicked() {
                self.link(db);
            }
        });
        ui.separator();

        ui.strong(format!("Affected customers ({})", self.customers.len()));
        for customer in &self.customers {
            ui.label(str_field(customer, "name"));
        }
        ui.separator();

        ui.strong(format!("Notes ({})", self.notes.len()));
        for note in &self.notes {
            ui.group(|ui| {
                ui.monospace(str_field(note, "created_at"));
                ui.label(str_field(note, "body"));
            });
        }
        ui.text_edit_multiline(&mut self.new_note);
        if ui.button("add note").clicked() {
            self.add_note(db);
        }
        if !self.releases.is_empty() {
            ui.separator();
            ui.strong(format!("Releases ({})", self.releases.len()));
            for release in &self.releases {
                ui.label(format!(
                    "{} · {}",
                    str_field(release, "version"),
                    str_field(release, "status")
                ));
            }
        }
    }

    fn new_incident_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.strong("New Incident");
        egui::Grid::new("incident-new")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("title:");
                ui.text_edit_singleline(&mut self.new_title);
                ui.end_row();
                ui.label("status:");
                egui::ComboBox::from_id_salt("incident-new-status")
                    .selected_text(self.new_status.clone())
                    .show_ui(ui, |ui| {
                        for status in INCIDENT_STATUSES {
                            ui.selectable_value(&mut self.new_status, status.into(), status);
                        }
                    });
                ui.end_row();
                ui.label("severity:");
                egui::ComboBox::from_id_salt("incident-new-severity")
                    .selected_text(self.new_severity.clone())
                    .show_ui(ui, |ui| {
                        for severity in INCIDENT_SEVERITIES {
                            ui.selectable_value(&mut self.new_severity, severity.into(), severity);
                        }
                    });
                ui.end_row();
                ui.label("description:");
                ui.text_edit_multiline(&mut self.new_description);
                ui.end_row();
                ui.label("conversation ids:");
                ui.text_edit_singleline(&mut self.new_conversation_ids)
                    .on_hover_text("comma-separated local conversation ids");
                ui.end_row();
            });
        if ui.button("create incident").clicked() {
            self.create(db);
        }
    }
}

impl Default for IncidentsPage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn create_incident_lists_and_details() {
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
        let mut page = IncidentsPage::new();
        page.reload_list(&db);
        assert_eq!(page.total, 0);

        page.new_title = "Checkout outage".into();
        page.new_status = "investigating".into();
        page.new_severity = "sev2".into();
        page.new_description = "Payments failing since deploy".into();
        page.new_conversation_ids = "1, 999".into();
        page.create(&db);

        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.total, 1);
        let row = page.rows[0].clone();
        assert_eq!(str_field(&row, "title"), "Checkout outage");
        assert_eq!(str_field(&row, "status"), "investigating");
        assert_eq!(str_field(&row, "severity"), "sev2");
        assert_eq!(i64_field(&row, "conversation_count"), 1, "id 999 skipped");
        assert!(page.detail.is_some(), "creation selects the incident");
        assert_eq!(page.conversations.len(), 1);
    }

    #[test]
    fn create_requires_a_title() {
        let db = fixture(Vec::new());
        let mut page = IncidentsPage::new();
        page.create(&db);
        let error = page.error.clone().expect("title required");
        assert!(error.contains("Title"));
    }

    #[test]
    fn notes_append_through_the_workspace() {
        let db = fixture(Vec::new());
        let mut page = IncidentsPage::new();
        page.new_title = "Radar down".into();
        page.create(&db);
        let id = page.selected.expect("selected");

        page.new_note = "Mitigated by rolling back".into();
        page.add_note(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.notes.len(), 1);
        assert_eq!(
            str_field(&page.notes[0], "body"),
            "Mitigated by rolling back"
        );

        page.new_note = "   ".into();
        page.add_note(&db);
        assert!(page.error.is_some(), "empty note rejected");
        assert_eq!(page.notes.len(), 1);
        let _ = id;
    }

    #[test]
    fn filters_narrow_the_list() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            for (title, status) in [("Alpha", "resolved"), ("Beta", "investigating")] {
                spp_core::intelligence_features::create_manual_incident(
                    conn,
                    &ManualIncident {
                        title: title.into(),
                        status: status.into(),
                        severity: "sev3".into(),
                        owner_user_local_id: None,
                        product: None,
                        feature: None,
                        description: None,
                        internal_explanation: None,
                        customer_safe_explanation: None,
                        known_cause: None,
                        workaround: None,
                        resolution: None,
                        started_at: None,
                        conversation_ids: Vec::new(),
                    },
                )
                .expect("incident");
            }
        });
        let mut page = IncidentsPage::new();
        page.reload_list(&db);
        assert_eq!(page.total, 2);

        page.filter_open_only = true;
        page.reload_list(&db);
        assert_eq!(page.total, 1);
        assert_eq!(str_field(&page.rows[0], "title"), "Beta");

        page.filter_open_only = false;
        page.applied_query = "Alpha".into();
        page.reload_list(&db);
        assert_eq!(page.total, 1);
        assert_eq!(str_field(&page.rows[0], "title"), "Alpha");
    }
}
