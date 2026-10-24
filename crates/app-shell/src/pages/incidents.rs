//! Incidents page (port of the web `pages/incidents.rs`).
//!
//! Redesigned with the shadcn-style design system (`crate::design`):
//! severity/status badges, card-wrapped workspace sections and a form-grid
//! incident creator — presentation only, all actions and filters intact.
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
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

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
    /// Post-action feedback (clean-audit item 131): which typed
    /// conversation ids were skipped and why, after a create.
    notice: Option<String>,
}

/// Incident severity → badge tone (redesign of the raw RED/YELLOW/GRAY).
fn severity_tone(severity: &str) -> BadgeTone {
    match severity {
        "sev1" => BadgeTone::Destructive,
        "sev2" => BadgeTone::Warning,
        _ => BadgeTone::Neutral,
    }
}

/// Incident status → badge tone.
fn incident_status_tone(status: &str) -> BadgeTone {
    match status {
        "resolved" => BadgeTone::Success,
        "investigating" | "monitoring" => BadgeTone::Warning,
        "identified" | "fix_in_progress" => BadgeTone::Info,
        _ => BadgeTone::Neutral,
    }
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
            notice: None,
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
        // Clean-audit item 131: keep what the user typed so the skipped
        // ids can be named in the notice — the old chain dropped a
        // non-numeric or non-positive token before anyone saw it.
        let parsed: Vec<(Option<i64>, String)> = self
            .new_conversation_ids
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                (
                    s.trim().parse::<i64>().ok().filter(|v| *v > 0),
                    s.trim().to_string(),
                )
            })
            .collect();
        let unparseable: Vec<String> = parsed
            .iter()
            .filter(|(v, _)| v.is_none())
            .map(|(_, s)| s.clone())
            .collect();
        let conversation_ids: Vec<i64> = parsed.iter().filter_map(|(v, _)| *v).collect();
        // The route validated each id exists; skip missing ones the same
        // way — but say so (clean-audit item 131: the drop used to be
        // silent, so a typo'd id just vanished from the link list).
        let mut valid_ids = Vec::new();
        let mut missing = unparseable;
        for id in conversation_ids {
            let exists = db.with_read(|conn| {
                conn.query_row(
                    "SELECT id FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
                    rusqlite::params![id],
                    |r| r.get::<_, i64>(0),
                )
                .is_ok()
            });
            if exists {
                valid_ids.push(id);
            } else {
                missing.push(id.to_string());
            }
        }
        // The count is read before `valid_ids` moves into the incident.
        let linked_count = valid_ids.len();
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
                // Clean-audit item 131: name the dropped ids instead of
                // silently losing them.
                self.notice = if missing.is_empty() {
                    None
                } else {
                    Some(format!(
                        "incident created; linked {} conversation(s) — unknown or \
                         nonexistent id(s) skipped: {}",
                        linked_count,
                        missing.join(", ")
                    ))
                };
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

        design::page_header(
            ui,
            "Incidents",
            Some(&format!("{} incident(s)", self.total)),
            |ui| {
                if design::button_with_icon(ui, Icon::Plus, "New Incident", ButtonVariant::Primary)
                    .clicked()
                {
                    self.show_new = !self.show_new;
                }
            },
        );
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        if let Some(note) = &self.notice {
            design::hint_text(ui, note);
        }
        design::divider(ui);

        design::card(ui, |ui| {
            ui.horizontal(|ui| {
                design::field_label(ui, "status:");
                // Clean-audit item 131: the selects and the checkbox only
                // mutated state — the list refetched only when the user
                // happened to hit "apply". Each control refetches on
                // change now (the text query keeps its explicit button —
                // per-keystroke queries would thrash).
                let prev_status = self.filter_status.clone();
                egui::ComboBox::from_id_salt("incident-filter-status")
                    .width(150.0)
                    .selected_text(self.filter_status.clone().unwrap_or_else(|| "Any".into()))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.filter_status, None, "Any");
                        for status in INCIDENT_STATUSES {
                            ui.selectable_value(
                                &mut self.filter_status,
                                Some(status.into()),
                                status,
                            );
                        }
                    });
                if self.filter_status != prev_status {
                    self.reload_list(db);
                }
                design::field_label(ui, "severity:");
                let prev_severity = self.filter_severity.clone();
                egui::ComboBox::from_id_salt("incident-filter-severity")
                    .width(130.0)
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
                if self.filter_severity != prev_severity {
                    self.reload_list(db);
                }
                let prev_open_only = self.filter_open_only;
                ui.checkbox(&mut self.filter_open_only, "open only");
                if self.filter_open_only != prev_open_only {
                    self.reload_list(db);
                }
                let mut q = self.filter_query.clone();
                ui.add_sized(
                    [180.0, 28.0],
                    egui::TextEdit::singleline(&mut q).hint_text("search incidents…"),
                );
                self.filter_query = q;
                if design::button(ui, "apply", ButtonVariant::Outline).clicked() {
                    self.applied_query = self.filter_query.trim().to_string();
                    self.reload_list(db);
                }
            });
        });
        ui.add_space(10.0);

        if self.show_new {
            self.new_incident_ui(ui, db);
            ui.add_space(10.0);
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
                        if design::chip(ui, selected, &format!("{code} {title}")).clicked() {
                            picked = Some(id);
                        }
                        ui.horizontal(|ui| {
                            design::badge(ui, &severity, severity_tone(&severity));
                            design::badge(ui, &status, incident_status_tone(&status));
                            ui.label(
                                egui::RichText::new(format!("{convs} conversations"))
                                    .small()
                                    .color(design::tokens(ui).muted_foreground),
                            );
                        });
                        ui.add_space(4.0);
                    }
                    if rows.is_empty() {
                        // Compact list-side note: the detail pane carries the
                        // large empty state, so two side-by-side heroes don't
                        // compete.
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let (id, rect) = ui.allocate_space(egui::vec2(16.0, 16.0));
                            Icon::AlertTriangle.paint_at(
                                ui.painter(),
                                rect,
                                design::tokens(ui).muted_foreground,
                            );
                            ui.advance_cursor_after_rect(rect);
                            let _ = id;
                            design::hint_text(
                                ui,
                                "No incidents match — adjust the filters or create a new incident.",
                            );
                        });
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
            design::empty_state(
                ui,
                Icon::AlertTriangle,
                "Select an incident",
                "Pick an incident from the list to open its workspace.",
            );
            return;
        };
        let _incident_id = i64_field(&detail, "id");
        let t = design::tokens(ui);
        design::section_title(
            ui,
            &format!(
                "{} · {}",
                str_field(&detail, "code"),
                str_field(&detail, "title")
            ),
        );
        ui.horizontal(|ui| {
            let status = str_field(&detail, "status");
            let severity = str_field(&detail, "severity");
            design::badge(ui, &status, incident_status_tone(&status));
            design::badge(ui, &severity, severity_tone(&severity));
            if let Some(started) = opt_str_field(&detail, "started_at") {
                design::field_label(ui, "started:");
                ui.monospace(egui::RichText::new(started).color(t.foreground));
            }
        });
        if let Some(owner) = opt_str_field(&detail, "owner_name") {
            design::hint_text(ui, &format!("owner: {owner}"));
        }
        for (label, key) in [
            ("description", "description"),
            ("customer-safe", "customer_safe_explanation"),
            ("workaround", "workaround"),
            ("resolution", "resolution"),
        ] {
            if let Some(text) = opt_str_field(&detail, key) {
                design::section_title(ui, label);
                ui.label(egui::RichText::new(text).size(13.0).color(t.foreground));
            }
        }
        ui.add_space(6.0);

        let conversations = self.conversations.clone();
        let link_conversation = self.link_conversation.clone();
        design::card_titled(
            ui,
            &format!("Linked conversations ({})", conversations.len()),
            None,
            |ui| {
                for conv in &conversations {
                    ui.label(
                        egui::RichText::new(format!(
                            "#{} · {}",
                            i64_field(conv, "number"),
                            str_field(conv, "subject")
                        ))
                        .size(13.0)
                        .color(t.foreground),
                    );
                }
                ui.horizontal(|ui| {
                    let mut link = link_conversation;
                    design::field_label(ui, "link conversation #:");
                    ui.add_sized(
                        [140.0, 28.0],
                        egui::TextEdit::singleline(&mut link).hint_text("id"),
                    );
                    self.link_conversation = link;
                    if design::button(ui, "link", ButtonVariant::Primary).clicked() {
                        self.link(db);
                    }
                });
            },
        );
        ui.add_space(10.0);

        let customers = self.customers.clone();
        design::card_titled(
            ui,
            &format!("Affected customers ({})", customers.len()),
            None,
            |ui| {
                for customer in &customers {
                    ui.label(
                        egui::RichText::new(str_field(customer, "name"))
                            .size(13.0)
                            .color(t.foreground),
                    );
                }
            },
        );
        ui.add_space(10.0);

        let notes = self.notes.clone();
        let releases = self.releases.clone();
        design::card_titled(ui, &format!("Notes ({})", notes.len()), None, |ui| {
            for note in &notes {
                design::card(ui, |ui| {
                    ui.label(
                        egui::RichText::new(str_field(note, "created_at"))
                            .small()
                            .color(t.muted_foreground),
                    );
                    ui.label(
                        egui::RichText::new(str_field(note, "body"))
                            .size(13.0)
                            .color(t.foreground),
                    );
                });
            }
            ui.add_sized(
                [ui.available_width(), 70.0],
                egui::TextEdit::multiline(&mut self.new_note).hint_text("write an incident note…"),
            );
            if design::button(ui, "add note", ButtonVariant::Primary).clicked() {
                self.add_note(db);
            }
        });
        if !releases.is_empty() {
            ui.add_space(10.0);
            design::card_titled(ui, &format!("Releases ({})", releases.len()), None, |ui| {
                for release in &releases {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {}",
                            str_field(release, "version"),
                            str_field(release, "status")
                        ))
                        .size(13.0)
                        .color(t.foreground),
                    );
                }
            });
        }
    }

    fn new_incident_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card(ui, |ui| {
            design::section_title(ui, "New Incident");
            design::form_grid(ui, "incident-new", |ui| {
                design::field_label(ui, "title:");
                ui.add_sized(
                    [260.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_title).hint_text("incident title"),
                );
                ui.end_row();
                design::field_label(ui, "status:");
                egui::ComboBox::from_id_salt("incident-new-status")
                    .width(170.0)
                    .selected_text(self.new_status.clone())
                    .show_ui(ui, |ui| {
                        for status in INCIDENT_STATUSES {
                            ui.selectable_value(&mut self.new_status, status.into(), status);
                        }
                    });
                ui.end_row();
                design::field_label(ui, "severity:");
                egui::ComboBox::from_id_salt("incident-new-severity")
                    .width(170.0)
                    .selected_text(self.new_severity.clone())
                    .show_ui(ui, |ui| {
                        for severity in INCIDENT_SEVERITIES {
                            ui.selectable_value(&mut self.new_severity, severity.into(), severity);
                        }
                    });
                ui.end_row();
                design::field_label(ui, "description:");
                ui.add_sized(
                    [320.0, 70.0],
                    egui::TextEdit::multiline(&mut self.new_description)
                        .hint_text("what is happening"),
                );
                ui.end_row();
                design::field_label(ui, "conversation ids:");
                ui.add_sized(
                    [260.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_conversation_ids)
                        .hint_text("comma-separated ids"),
                )
                .on_hover_text("comma-separated local conversation ids");
                ui.end_row();
            });
            if design::button(ui, "create incident", ButtonVariant::Primary).clicked() {
                self.create(db);
            }
        });
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

    /// Clean-audit item 131: the create path used to drop invalid and
    /// nonexistent conversation ids silently — a typo'd id just vanished
    /// from the link list. The notice names every skipped token.
    #[test]
    fn create_names_the_skipped_conversation_ids() {
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
        page.new_title = "Checkout outage".into();
        page.new_status = "investigating".into();
        page.new_severity = "sev2".into();
        page.new_conversation_ids = "1, abc, 999".into();
        page.create(&db);

        assert!(page.error.is_none(), "{:?}", page.error);
        let note = page.notice.as_deref().expect("the skips are named");
        assert!(note.contains("999"), "the nonexistent id is named: {note}");
        assert!(note.contains("abc"), "the unparseable id is named: {note}");
        assert!(
            note.contains("linked 1 conversation(s)"),
            "the kept count is stated: {note}"
        );
        assert_eq!(page.conversations.len(), 1, "the valid id still links");
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
