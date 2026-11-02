use eframe::egui;
use spp_core::db::DbHandle;

use super::{i64_field, opt_str_field, str_field};
use crate::design::{self, AvatarSize, BadgeTone, ButtonVariant};
use crate::icons::Icon;

const ROW_HEIGHT: f32 = 32.0;

pub struct OrganizationsPage {
    pub sync_note: String,

    filter_edit: String,
    filter: String,
    orgs: Vec<serde_json::Value>,
    detail: Option<serde_json::Value>,
    selected: Option<i64>,
    new_name: String,
    new_domains: String,
    error: Option<String>,
    loaded: bool,
    panel_open: bool,
}

impl OrganizationsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sync_note: String::new(),
            filter_edit: String::new(),
            filter: String::new(),
            orgs: Vec::new(),
            detail: None,
            selected: None,
            new_name: String::new(),
            new_domains: String::new(),
            error: None,
            loaded: false,
            panel_open: false,
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.loaded = false;
        self.orgs.clear();
        self.detail = None;
    }

    fn reload(&mut self, db: &DbHandle) {
        let filter = self.filter.trim().to_string();
        let orgs = db.with_read(|conn| {
            spp_core::people_store::list_organizations(
                conn,
                if filter.is_empty() {
                    None
                } else {
                    Some(filter.as_str())
                },
                200,
            )
        });
        match orgs {
            Ok(orgs) => {
                self.orgs = orgs;
                self.error = None;
                if let Some(id) = self.selected
                    && !self.orgs.iter().any(|o| i64_field(o, "id") == id)
                {
                    self.selected = None;
                    self.detail = None;
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.loaded = true;
        if let Some(id) = self.selected {
            self.reload_detail(db, id);
        }
    }

    fn reload_detail(&mut self, db: &DbHandle, id: i64) {
        self.detail = db
            .with_read(|conn| spp_core::people_store::get_organization_detail(conn, id))
            .ok()
            .flatten();
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
        let domains: Vec<String> = self
            .new_domains
            .split(',')
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(str::to_string)
            .collect();
        let result = db
            .with_write(|conn| spp_core::people_store::create_organization(conn, &name, &domains));
        match result {
            Ok(id) => {
                self.new_name.clear();
                self.new_domains.clear();
                self.selected = Some(id);
                self.panel_open = true;
                self.reload(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        engine: Option<&std::sync::Arc<spp_core::sync_engine::SyncEngine>>,
    ) {
        self.ensure_loaded(db);

        design::page_header(
            ui,
            "Organizations",
            Some("customer organizations, their domains, and their members"),
            |ui| {
                if !self.sync_note.is_empty() {
                    design::hint_text(ui, &self.sync_note);
                }
                if design::button_with_icon(ui, Icon::Refresh, "Sync now", ButtonVariant::Outline)
                    .on_hover_text(
                        "pull fresh data from Help Scout now — the completion toast \
                     reports the outcome; the page re-reads on every sync anyway",
                    )
                    .clicked()
                {
                    match engine {
                        Some(engine) => {
                            if crate::pages::sync_health::spawn_sync_action(
                                db,
                                engine,
                                crate::pages::sync_health::SyncAction::Incremental,
                            ) {
                                self.sync_note = "syncing\u{2026}".into();
                            } else {
                                self.sync_note = "a sync is already running".into();
                            }
                        }
                        None => {
                            self.invalidate();
                            self.sync_note = "re-read the local mirror (no engine)".into();
                        }
                    }
                }
                design::page_counter(ui, &format!("{} organizations", self.orgs.len()));
            },
        );

        let apply = design::filter_bar(
            ui,
            |ui| {
                let mut edit = self.filter_edit.clone();
                let response = ui.add_sized(
                    [260.0, 28.0],
                    egui::TextEdit::singleline(&mut edit).hint_text("Filter by name…"),
                );
                if response.changed() {
                    self.filter_edit = edit;
                }
            },
            "filter",
        );
        if apply.clicked() {
            self.filter = self.filter_edit.clone();
            self.reload(db);
        }
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        ui.add_space(design::scale::space::SP2);

        let orgs = self.orgs.clone();
        let mut pick: Option<i64> = None;
        if orgs.is_empty() {
            design::empty_state(
                ui,
                Icon::Building,
                "No organizations here",
                "Adjust the filter or create one below — organizations arrive \
                 from the Help Scout mirror.",
            );
        } else {
            design::data_table(
                ui,
                "organizations",
                &["organization", "domains", "customers", "conversations", ""],
                orgs.len(),
                ROW_HEIGHT,
                |ui, i| {
                    let org = &orgs[i];
                    let id = i64_field(org, "id");
                    let name = str_field(org, "name");
                    let domains = domains_text(org);
                    let customers = i64_field(org, "customer_count");
                    let conversations = i64_field(org, "conversation_count");
                    let t = design::tokens(ui);
                    dt_cell(
                        ui,
                        240.0,
                        egui::RichText::new(&name)
                            .size(design::scale::text::BODY)
                            .strong()
                            .color(t.foreground),
                        false,
                    );
                    dt_cell(
                        ui,
                        220.0,
                        egui::RichText::new(domains)
                            .size(design::scale::text::SMALL_HALF)
                            .monospace()
                            .color(t.muted_foreground),
                        false,
                    );
                    dt_cell(
                        ui,
                        90.0,
                        egui::RichText::new(customers.to_string())
                            .size(design::scale::text::SMALL_HALF)
                            .monospace()
                            .color(t.foreground),
                        true,
                    );
                    dt_cell(
                        ui,
                        110.0,
                        egui::RichText::new(conversations.to_string())
                            .size(design::scale::text::SMALL_HALF)
                            .monospace()
                            .color(t.foreground),
                        true,
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(70.0, ROW_HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            if design::button(ui, "open", ButtonVariant::Outline)
                                .on_hover_text(format!(
                                    "open {name} — members, domains and conversation counts"
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
            self.selected = Some(id);
            self.panel_open = true;
            self.reload_detail(db, id);
        }

        ui.add_space(design::scale::space::SP2);
        design::card_titled(ui, "New organization", None, |ui| {
            design::form_row(ui, "name", |ui| {
                let mut name = self.new_name.clone();
                let response = ui.add_sized(
                    [240.0, 28.0],
                    egui::TextEdit::singleline(&mut name).hint_text("Name"),
                );
                if response.changed() {
                    self.new_name = name;
                }
            });
            design::form_row(ui, "domains", |ui| {
                let mut domains = self.new_domains.clone();
                let response = ui
                    .add_sized(
                        [240.0, 28.0],
                        egui::TextEdit::singleline(&mut domains).hint_text("acme.com, acme.io"),
                    )
                    .on_hover_text("comma-separated domains");
                if response.changed() {
                    self.new_domains = domains;
                }
            });
            ui.add_space(design::scale::space::SP1);
            if design::button(ui, "create organization", ButtonVariant::Primary)
                .on_hover_text("create with the name + domains above")
                .clicked()
            {
                self.create(db);
            }
        });

        let detail = self.detail.clone();
        let title = detail
            .as_ref()
            .map(|d| str_field(d, "name"))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Organization".to_string());
        let mut panel_open = self.panel_open;
        design::detail_panel(
            ui.ctx(),
            "organizations-detail",
            &mut panel_open,
            &title,
            |_ui| {},
            |ui| {
                let Some(org) = detail else {
                    design::empty_state_slim(
                        ui,
                        Icon::Building,
                        "Select an organization",
                        "Its members, domains, and conversation counts appear here.",
                    );
                    return;
                };
                let t = design::tokens(ui);
                let name = str_field(&org, "name");
                let members = org
                    .get("members")
                    .and_then(|c| c.as_array())
                    .cloned()
                    .unwrap_or_default();
                let member_count = i64_field(&org, "member_count");
                let member_count = if member_count > 0 {
                    member_count
                } else {
                    members.len() as i64
                };
                let conversation_count = i64_field(&org, "conversation_count");
                let domains = domains_text(&org);

                ui.horizontal(|ui| {
                    design::avatar(ui, AvatarSize::Md, &initials(&name), BadgeTone::Neutral);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(&name)
                                .size(design::scale::text::SECTION_BIG)
                                .strong()
                                .color(t.foreground),
                        );
                        design::hint_text(
                            ui,
                            &format!("{member_count} members · {conversation_count} conversations"),
                        );
                    });
                });
                if !domains.is_empty() {
                    ui.monospace(
                        egui::RichText::new(&domains)
                            .size(design::scale::text::SMALL_HALF)
                            .color(t.muted_foreground),
                    );
                }
                design::divider(ui);

                design::card_titled(ui, "Members", None, |ui| {
                    if members.is_empty() {
                        design::empty_state_slim(
                            ui,
                            Icon::Building,
                            "No members recorded",
                            "Members come from the mirror's customer→organization edges.",
                        );
                        return;
                    }
                    for member in &members {
                        let name = str_field(member, "name");
                        let email = str_field(member, "email");
                        let job_title = opt_str_field(member, "job_title");
                        ui.horizontal(|ui| {
                            design::avatar(
                                ui,
                                AvatarSize::Sm,
                                &initials(&name),
                                BadgeTone::Neutral,
                            );
                            ui.label(
                                egui::RichText::new(&name)
                                    .size(design::scale::text::BODY)
                                    .color(t.foreground),
                            );
                            if !email.is_empty() {
                                ui.label(
                                    egui::RichText::new(email)
                                        .size(design::scale::text::SMALL)
                                        .color(t.muted_foreground),
                                );
                            }
                            if let Some(job) = job_title {
                                ui.label(
                                    egui::RichText::new(job)
                                        .size(design::scale::text::META_HALF)
                                        .color(t.muted_foreground),
                                );
                            }
                        });
                    }
                });
            },
        );
        self.panel_open = panel_open;
    }
}

impl Default for OrganizationsPage {
    fn default() -> Self {
        Self::new()
    }
}

fn domains_text(org: &serde_json::Value) -> String {
    match org.get("domains") {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()
            .join(", "),
        Some(serde_json::Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
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
    fn create_and_list_organizations_round_trip() {
        let db = fixture(Vec::new());
        let mut page = OrganizationsPage::new();
        page.reload(&db);
        assert!(page.orgs.is_empty());

        page.new_name = "Acme".into();
        page.new_domains = "acme.com, acme.io".into();
        page.create(&db);
        assert!(page.error.is_none());
        assert_eq!(page.orgs.len(), 1);
        let created = &page.orgs[0];
        assert_eq!(str_field(created, "name"), "Acme");
        assert_eq!(page.selected, Some(i64_field(created, "id")));
        assert!(page.detail.is_some(), "selection loads the detail");
    }

    #[test]
    fn filter_narrows_the_list() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            spp_core::people_store::create_organization(conn, "Acme", &["acme.com".into()])
                .expect("acme");
            spp_core::people_store::create_organization(conn, "Globex", &["globex.com".into()])
                .expect("globex");
        });
        let mut page = OrganizationsPage::new();
        page.reload(&db);
        assert_eq!(page.orgs.len(), 2);

        page.filter = "Glob".into();
        page.reload(&db);
        assert_eq!(page.orgs.len(), 1);
        assert_eq!(str_field(&page.orgs[0], "name"), "Globex");
    }

    #[test]
    fn create_requires_a_name() {
        let db = fixture(Vec::new());
        let mut page = OrganizationsPage::new();
        page.create(&db);
        assert!(page.error.is_some());
    }
}
