use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::incident_workspace::IncidentListFilters;
use spp_core::intelligence_features::ManualIncident;

use super::{i64_field, opt_str_field, str_field};
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

const ROW_HEIGHT: f32 = 32.0;

pub(crate) const INCIDENT_STATUSES: [&str; 5] = [
    "investigating",
    "identified",
    "fix_in_progress",
    "monitoring",
    "resolved",
];

pub(crate) const INCIDENT_SEVERITIES: [&str; 4] = ["sev1", "sev2", "sev3", "sev4"];

pub struct IncidentsPage {
    filter_status: Option<String>,
    filter_severity: Option<String>,
    filter_open_only: bool,
    filter_query: String,
    applied_query: String,
    rows: Vec<serde_json::Value>,
    loaded: bool,
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
    notice: Option<String>,
    panel_open: bool,
}

fn severity_tone(severity: &str) -> BadgeTone {
    match severity {
        "sev1" => BadgeTone::Destructive,
        "sev2" => BadgeTone::Warning,
        _ => BadgeTone::Neutral,
    }
}

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
            loaded: false,
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
            panel_open: false,
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.loaded = false;
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
                    self.reload_detail(db, id);
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.loaded = true;
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
        if !self.loaded {
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
                self.notice = if missing.is_empty() {
                    None
                } else {
                    Some(format!(
                        "incident created; linked {} conversation{} — unknown or \
                         nonexistent id{} skipped: {}",
                        linked_count,
                        if linked_count == 1 { "" } else { "s" },
                        if missing.len() == 1 { "" } else { "s" },
                        missing.join(", ")
                    ))
                };
                self.reload_list(db);
                self.panel_open = true;
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

    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        let subtitle = if (self.rows.len() as i64) < self.total {
            format!(
                "Showing the first {} of {} incidents",
                self.rows.len(),
                self.total
            )
        } else if self.total == 1 {
            "1 incident".to_string()
        } else {
            format!("{} incidents", self.total)
        };

        design::page_header(ui, "Incidents", Some(&subtitle), |ui| {
            if design::button_with_icon(ui, Icon::Plus, "New Incident", ButtonVariant::Primary)
                .on_hover_text("open the new-incident form")
                .clicked()
            {
                self.show_new = !self.show_new;
            }
            design::page_counter(ui, &format!("{} incidents", self.rows.len()));
        });
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        if let Some(note) = &self.notice {
            design::hint_text(ui, note);
        }

        let apply = design::filter_bar(
            ui,
            |ui| {
                // UX-17: the two dropdowns carry visible labels — two bare
                // "Any" combos were indistinguishable.
                design::field_label(ui, "Status:");
                let prev_status = self.filter_status.clone();
                let status_label = self.filter_status.clone().unwrap_or_else(|| "Any".into());
                egui::ComboBox::from_id_salt("incident-filter-status")
                    .width(150.0)
                    .selected_text(status_label.clone())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.filter_status, None, "Any");
                        for status in INCIDENT_STATUSES {
                            ui.selectable_value(
                                &mut self.filter_status,
                                Some(status.into()),
                                status,
                            );
                        }
                    })
                    .response
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            ui.is_enabled(),
                            format!("status: {status_label}"),
                        )
                    });
                if self.filter_status != prev_status {
                    self.reload_list(db);
                }
                design::field_label(ui, "Severity:");
                let prev_severity = self.filter_severity.clone();
                let severity_label = self.filter_severity.clone().unwrap_or_else(|| "Any".into());
                egui::ComboBox::from_id_salt("incident-filter-severity")
                    .width(130.0)
                    .selected_text(severity_label.clone())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.filter_severity, None, "Any");
                        for severity in INCIDENT_SEVERITIES {
                            ui.selectable_value(
                                &mut self.filter_severity,
                                Some(severity.into()),
                                severity,
                            );
                        }
                    })
                    .response
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            ui.is_enabled(),
                            format!("severity: {severity_label}"),
                        )
                    });
                if self.filter_severity != prev_severity {
                    self.reload_list(db);
                }
                let prev_open_only = self.filter_open_only;
                ui.checkbox(&mut self.filter_open_only, "Open only");
                if self.filter_open_only != prev_open_only {
                    self.reload_list(db);
                }
                let mut q = self.filter_query.clone();
                ui.add_sized(
                    [180.0, 28.0],
                    egui::TextEdit::singleline(&mut q).hint_text("search incidents…"),
                );
                self.filter_query = q;
            },
            "Apply",
        );
        if apply.clicked() {
            self.applied_query = self.filter_query.trim().to_string();
            self.reload_list(db);
        }
        ui.add_space(design::scale::space::SP2);

        if self.show_new {
            self.new_incident_ui(ui, db);
            ui.add_space(design::scale::space::SP2);
        }

        let rows = self.rows.clone();
        let mut pick: Option<i64> = None;
        if rows.is_empty() {
            // UX-17: the empty state must match reality — "no filters set"
            // is a first-run state, not a filter mismatch.
            let filters_active = self.filter_status.is_some()
                || self.filter_severity.is_some()
                || self.filter_open_only
                || !self.applied_query.trim().is_empty();
            if filters_active {
                design::empty_state(
                    ui,
                    Icon::AlertTriangle,
                    "No incidents match these filters",
                    "Adjust or clear the filters, or create a new incident.",
                );
                ui.vertical_centered(|ui| {
                    if design::button(ui, "Clear Filters", ButtonVariant::Outline).clicked() {
                        self.filter_status = None;
                        self.filter_severity = None;
                        self.filter_open_only = false;
                        self.filter_query.clear();
                        self.applied_query.clear();
                        self.reload_list(db);
                    }
                    ui.add_space(4.0);
                });
            } else if design::empty_state_cta(
                ui,
                Icon::AlertTriangle,
                "No incidents yet",
                "Track outages and recurring problems — create your first incident.",
                "New Incident",
            ) {
                self.show_new = true;
            }
        } else {
            design::data_table(
                ui,
                "incidents",
                &["Code", "Title", "Severity", "Status", "Conversations", ""],
                rows.len(),
                ROW_HEIGHT,
                |ui, i| {
                    let row = &rows[i];
                    let id = i64_field(row, "id");
                    let code = str_field(row, "code");
                    let title = str_field(row, "title");
                    let status = str_field(row, "status");
                    let severity = str_field(row, "severity");
                    let convs = i64_field(row, "conversation_count");
                    let t = design::tokens(ui);
                    dt_cell(
                        ui,
                        90.0,
                        egui::RichText::new(&code)
                            .size(design::scale::text::BODY)
                            .strong()
                            .color(t.foreground),
                        false,
                    );
                    dt_cell(
                        ui,
                        240.0,
                        egui::RichText::new(&title)
                            .size(design::scale::text::BODY)
                            .color(t.foreground),
                        false,
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(110.0, ROW_HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            design::badge(ui, &severity, severity_tone(&severity));
                        },
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(120.0, ROW_HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            design::badge(ui, &status, incident_status_tone(&status));
                        },
                    );
                    dt_cell(
                        ui,
                        110.0,
                        egui::RichText::new(convs.to_string())
                            .size(design::scale::text::SMALL_HALF)
                            .monospace()
                            .color(t.foreground),
                        true,
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(70.0, ROW_HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            if design::button(ui, "Open", ButtonVariant::Outline)
                                .on_hover_text(format!(
                                    "open {code} — linked conversations, notes, releases"
                                ))
                                .clicked()
                            {
                                pick = Some(id);
                            }
                        },
                    );
                },
            );
        }

        if let Some(id) = pick {
            self.panel_open = true;
            self.select(db, id);
        }

        let detail = self.detail.clone();
        let title = detail
            .as_ref()
            .map(|d| format!("{} · {}", str_field(d, "code"), str_field(d, "title")))
            .filter(|t| !t.is_empty() && t != " · ")
            .unwrap_or_else(|| "Incident".to_string());
        let mut panel_open = self.panel_open;
        design::detail_panel(
            ui.ctx(),
            "incidents-detail",
            &mut panel_open,
            &title,
            |_ui| {},
            |ui| {
                let Some(detail) = detail else {
                    design::empty_state_slim(
                        ui,
                        Icon::AlertTriangle,
                        "Select an incident",
                        "Pick an incident from the list to open its workspace.",
                    );
                    return;
                };
                let incident = detail
                    .get("incident")
                    .cloned()
                    .unwrap_or_else(|| detail.clone());
                let t = design::tokens(ui);
                let status = str_field(&incident, "status");
                let severity = str_field(&incident, "severity");

                let code = str_field(&incident, "code");
                let incident_title = str_field(&incident, "title");
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{code} · {incident_title}"))
                            .size(design::scale::text::LEAD)
                            .strong()
                            .color(t.foreground),
                    );
                    let started = opt_str_field(&incident, "started_at")
                        .map(|s| format!("started: {s}"))
                        .unwrap_or_else(|| "not started".into());
                    let owner = opt_str_field(&incident, "owner_name")
                        .map(|o| format!(" · owner: {o}"))
                        .unwrap_or_default();
                    design::hint_text(ui, &format!("{started}{owner}"));
                    ui.horizontal(|ui| {
                        design::badge(ui, &status, incident_status_tone(&status));
                        design::badge(ui, &severity, severity_tone(&severity));
                    });
                });
                design::divider(ui);

                for (label, key) in [
                    ("description", "description"),
                    ("customer-safe", "customer_safe_explanation"),
                    ("workaround", "workaround"),
                    ("resolution", "resolution"),
                ] {
                    if let Some(text) = opt_str_field(&incident, key) {
                        design::card_titled(ui, label, None, |ui| {
                            ui.label(
                                egui::RichText::new(text)
                                    .size(design::scale::text::BODY)
                                    .color(t.foreground),
                            );
                        });
                        ui.add_space(design::scale::space::SP2);
                    }
                }

                ui.columns(2, |columns| {
                    let conversations = self.conversations.clone();
                    let releases = self.releases.clone();
                    let link_conversation = self.link_conversation.clone();
                    let customers = self.customers.clone();
                    let notes = self.notes.clone();

                    design::card_titled(
                        &mut columns[0],
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
                                    .size(design::scale::text::SMALL)
                                    .color(t.foreground),
                                );
                            }
                            ui.add_space(design::scale::space::SP1);
                            design::form_row(ui, "link #", |ui| {
                                let mut link = link_conversation;
                                let response = ui.add_sized(
                                    [90.0, 28.0],
                                    egui::TextEdit::singleline(&mut link).hint_text("id"),
                                );
                                if response.changed() {
                                    self.link_conversation = link;
                                }
                            });
                            if design::button(ui, "Link", ButtonVariant::Primary).clicked() {
                                self.link(db);
                            }
                        },
                    );
                    columns[0].add_space(design::scale::space::SP2);
                    if !releases.is_empty() {
                        design::card_titled(
                            &mut columns[0],
                            &format!("Releases ({})", releases.len()),
                            None,
                            |ui| {
                                for release in &releases {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} · {}",
                                            str_field(release, "version"),
                                            str_field(release, "status")
                                        ))
                                        .size(design::scale::text::SMALL)
                                        .color(t.foreground),
                                    );
                                }
                            },
                        );
                        columns[0].add_space(design::scale::space::SP2);
                    }

                    design::card_titled(
                        &mut columns[1],
                        &format!("Affected customers ({})", customers.len()),
                        None,
                        |ui| {
                            for customer in &customers {
                                ui.label(
                                    egui::RichText::new(str_field(customer, "name"))
                                        .size(design::scale::text::SMALL)
                                        .color(t.foreground),
                                );
                            }
                        },
                    );
                    columns[1].add_space(design::scale::space::SP2);
                    design::card_titled(
                        &mut columns[1],
                        &format!("Notes ({})", notes.len()),
                        None,
                        |ui| {
                            for note in &notes {
                                design::card(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(str_field(note, "created_at"))
                                            .size(design::scale::text::META)
                                            .color(t.muted_foreground),
                                    );
                                    ui.label(
                                        egui::RichText::new(str_field(note, "body"))
                                            .size(design::scale::text::SMALL)
                                            .color(t.foreground),
                                    );
                                });
                            }
                            ui.add_space(design::scale::space::SP1);
                            ui.add_sized(
                                [ui.available_width(), 70.0],
                                egui::TextEdit::multiline(&mut self.new_note)
                                    .hint_text("write an incident note…"),
                            );
                            if design::button(ui, "Add Note", ButtonVariant::Primary).clicked() {
                                self.add_note(db);
                            }
                        },
                    );
                });
            },
        );
        self.panel_open = panel_open;
    }

    fn new_incident_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card_titled(ui, "New Incident", None, |ui| {
            design::form_grid(ui, "incident-new", |ui| {
                design::field_label(ui, "Title");
                ui.add_sized(
                    [260.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_title).hint_text("incident title"),
                );
                ui.end_row();
                design::field_label(ui, "Status");
                egui::ComboBox::from_id_salt("incident-new-status")
                    .width(170.0)
                    .selected_text(self.new_status.clone())
                    .show_ui(ui, |ui| {
                        for status in INCIDENT_STATUSES {
                            ui.selectable_value(&mut self.new_status, status.into(), status);
                        }
                    });
                ui.end_row();
                design::field_label(ui, "Severity");
                egui::ComboBox::from_id_salt("incident-new-severity")
                    .width(170.0)
                    .selected_text(self.new_severity.clone())
                    .show_ui(ui, |ui| {
                        for severity in INCIDENT_SEVERITIES {
                            ui.selectable_value(&mut self.new_severity, severity.into(), severity);
                        }
                    });
                ui.end_row();
                design::field_label(ui, "Description");
                ui.add_sized(
                    [320.0, 70.0],
                    egui::TextEdit::multiline(&mut self.new_description)
                        .hint_text("what is happening"),
                );
                ui.end_row();
                design::field_label(ui, "Conversation ids");
                ui.add_sized(
                    [260.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_conversation_ids)
                        .hint_text("comma-separated ids"),
                )
                .on_hover_text("comma-separated local conversation ids");
                ui.end_row();
            });
            ui.add_space(design::scale::space::SP1);
            if design::button(ui, "Create Incident", ButtonVariant::Primary)
                .on_hover_text("create with the fields above")
                .clicked()
            {
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

fn dt_cell(ui: &mut egui::Ui, width: f32, text: egui::RichText, right: bool) {
    let layout = if right {
        egui::Layout::right_to_left(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    ui.allocate_ui_with_layout(egui::vec2(width, ROW_HEIGHT), layout, |ui| {
        ui.add(egui::Label::new(text).truncate());
    });
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
        assert!(page.panel_open, "creation opens the detail panel");
        assert_eq!(page.conversations.len(), 1);
    }

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
            note.contains("linked 1 conversation —"),
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
