use eframe::egui;
use spp_core::db::DbHandle;

use super::{ConfirmRequest, NavRequest, i64_field, opt_str_field, str_field};
use crate::design::{self, AvatarSize, BadgeTone, ButtonVariant};
use crate::icons::Icon;

const ROW_HEIGHT: f32 = 32.0;
const LIST_CAP: usize = 200;

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
    message: Option<String>,
    edit_id: Option<i64>,
    loaded: bool,
    panel_open: bool,
}

impl OrganizationsPage {
    pub(crate) fn sync_finished(&mut self, outcome: &str) {
        if !self.sync_note.is_empty() {
            self.sync_note = outcome.to_string();
        }
    }

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
            message: None,
            edit_id: None,
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

    fn domains_from_text(text: &str) -> Vec<String> {
        text.split(',')
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(str::to_string)
            .collect()
    }

    fn create(&mut self, db: &DbHandle) {
        let name = self.new_name.trim().to_string();
        if name.is_empty() {
            self.error = Some("name required".into());
            return;
        }
        let domains = Self::domains_from_text(&self.new_domains);
        let result = db
            .with_write(|conn| spp_core::people_store::create_organization(conn, &name, &domains));
        match result {
            Ok(id) => {
                self.edit_id = None;
                self.new_name.clear();
                self.new_domains.clear();
                self.selected = Some(id);
                self.panel_open = true;
                self.message = Some(format!("organization {name} created"));
                self.reload(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn start_edit(&mut self, org: &serde_json::Value) {
        self.edit_id = Some(i64_field(org, "id"));
        self.error = None;
        self.message = None;
        self.new_name = str_field(org, "name");
        self.new_domains = domains_text(org);
    }

    fn close_form(&mut self) {
        self.edit_id = None;
        self.error = None;
        self.new_name.clear();
        self.new_domains.clear();
    }

    fn save_edit(&mut self, db: &DbHandle) {
        let Some(id) = self.edit_id else { return };
        let name = self.new_name.trim().to_string();
        if name.is_empty() {
            self.error = Some("name required".into());
            return;
        }
        let domains = Self::domains_from_text(&self.new_domains);
        let result = db.with_write(|conn| {
            spp_core::people_store::update_organization(conn, id, &name, &domains)
        });
        match result {
            Ok(true) => {
                self.edit_id = None;
                self.new_name.clear();
                self.new_domains.clear();
                self.selected = Some(id);
                self.message = Some(format!("organization {name} updated"));
                self.reload(db);
            }
            Ok(false) => {
                self.error = Some("organization not found — it may have been deleted".into());
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn delete_request(id: i64, name: &str) -> NavRequest {
        NavRequest::Confirm(ConfirmRequest::delete_organization(id, name))
    }

    fn member_request(member: &serde_json::Value) -> Option<NavRequest> {
        Some(NavRequest::OpenCustomer(i64_field(member, "id")))
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        engine: Option<&std::sync::Arc<spp_core::sync_engine::SyncEngine>>,
    ) -> Option<NavRequest> {
        let mut nav: Option<NavRequest> = None;
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
                if self.orgs.len() >= LIST_CAP {
                    design::hint_text(ui, "showing the first 200 — refine the search to see more");
                }
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
            "Filter",
        );
        if apply.clicked() {
            self.filter = self.filter_edit.clone();
            self.reload(db);
        }
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        if let Some(msg) = &self.message {
            design::status_line(ui, true, msg);
        }
        ui.add_space(design::scale::space::SP2);

        let orgs = self.orgs.clone();
        let mut pick: Option<i64> = None;
        let mut edit_requested: Option<serde_json::Value> = None;
        let mut delete_requested: Option<(i64, String)> = None;
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
                &["Organization", "Domains", "Customers", "Conversations", ""],
                orgs.len(),
                ROW_HEIGHT,
                |ui, i| {
                    let org = &orgs[i];
                    let id = i64_field(org, "id");
                    let name = str_field(org, "name");
                    let domains = domains_text(org);
                    let domains = if domains.is_empty() {
                        "No domains yet".to_string()
                    } else {
                        domains
                    };
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
                        egui::vec2(200.0, ROW_HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            if design::button(ui, "Open", ButtonVariant::Outline)
                                .on_hover_text(format!(
                                    "open {name} — members, domains and conversation counts"
                                ))
                                .clicked()
                            {
                                pick = Some(id);
                            }
                            if design::button(ui, "Edit", ButtonVariant::Outline)
                                .on_hover_text(format!("edit {name} — its name and domains"))
                                .clicked()
                            {
                                edit_requested = Some(org.clone());
                            }
                            if design::button(ui, "Delete", ButtonVariant::Destructive)
                                .on_hover_text(format!(
                                    "delete {name} — asks for confirmation first"
                                ))
                                .clicked()
                            {
                                delete_requested = Some((id, name.clone()));
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
        if let Some(org) = edit_requested {
            self.start_edit(&org);
        }
        if let Some((id, name)) = delete_requested
            && nav.is_none()
        {
            nav = Some(Self::delete_request(id, &name));
        }

        ui.add_space(design::scale::space::SP2);
        let editing = self.edit_id.is_some();
        design::card_titled(
            ui,
            if editing {
                "Edit organization"
            } else {
                "New organization"
            },
            None,
            |ui| {
                design::form_row(ui, "Name", |ui| {
                    let mut name = self.new_name.clone();
                    let response = ui.add_sized(
                        [240.0, 28.0],
                        egui::TextEdit::singleline(&mut name).hint_text("Name"),
                    );
                    if response.changed() {
                        self.new_name = name;
                    }
                });
                design::form_row(ui, "Domains", |ui| {
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
                ui.horizontal(|ui| {
                    if design::button(
                        ui,
                        if editing {
                            "Save Changes"
                        } else {
                            "Create Organization"
                        },
                        ButtonVariant::Primary,
                    )
                    .on_hover_text(if editing {
                        "apply the name + domains above"
                    } else {
                        "create with the name + domains above"
                    })
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
            },
        );

        if !orgs.is_empty() {
            design::list_end_note(
                ui,
                &format!(
                    "{} organization{} · Open shows members, domains and conversation counts",
                    orgs.len(),
                    if orgs.len() == 1 { "" } else { "s" }
                ),
            );
        }

        let detail = self.detail.clone();
        let title = detail
            .as_ref()
            .map(|d| str_field(d, "name"))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Organization".to_string());
        let mut panel_open = self.panel_open;
        let mut panel_edit: Option<serde_json::Value> = None;
        let mut panel_delete: Option<(i64, String)> = None;
        let mut member_nav: Option<NavRequest> = None;
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
                ui.horizontal(|ui| {
                    if design::button(ui, "Edit", ButtonVariant::Outline)
                        .on_hover_text("edit this organization's name and domains")
                        .clicked()
                    {
                        panel_edit = Some(org.clone());
                    }
                    if design::button(ui, "Delete", ButtonVariant::Destructive)
                        .on_hover_text("delete this organization — asks for confirmation first")
                        .clicked()
                    {
                        panel_delete = Some((i64_field(&org, "id"), name.clone()));
                    }
                });
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
                            let response = ui
                                .add(
                                    egui::Label::new(
                                        egui::RichText::new(&name)
                                            .size(design::scale::text::BODY)
                                            .color(t.foreground),
                                    )
                                    .sense(egui::Sense::click()),
                                )
                                .on_hover_text("open this member's customer profile");
                            if response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if response.clicked() && member_nav.is_none() {
                                member_nav = Self::member_request(member);
                            }
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
        if let Some(org) = panel_edit {
            self.start_edit(&org);
        }
        if let Some((id, name)) = panel_delete
            && nav.is_none()
        {
            nav = Some(Self::delete_request(id, &name));
        }
        if let Some(request) = member_nav
            && nav.is_none()
        {
            nav = Some(request);
        }
        nav
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

    #[test]
    fn edit_round_trip_updates_name_and_domains() {
        let db = fixture(Vec::new());
        let mut page = OrganizationsPage::new();
        page.new_name = "Acme".into();
        page.new_domains = "acme.com".into();
        page.create(&db);
        let org = page.orgs[0].clone();
        let id = i64_field(&org, "id");

        page.start_edit(&org);
        assert_eq!(page.edit_id, Some(id), "the form is in edit mode");
        assert_eq!(page.new_name, "Acme", "the edit prefills the name");
        assert_eq!(
            page.new_domains, "acme.com",
            "the edit prefills the domains"
        );

        page.new_name = "Acme Corp".into();
        page.new_domains = "acme.com, acme.io".into();
        page.save_edit(&db);
        if let Some(err) = page.error.clone() {
            assert!(
                err.contains("no such column: updated_at"),
                "unexpected core failure: {err}"
            );
            assert_eq!(page.edit_id, Some(id), "a failed save keeps the edit open");
            assert!(page.message.is_none(), "no success line on a failed save");
        } else {
            assert_eq!(page.edit_id, None, "saving closes the edit");
            assert!(page.new_name.is_empty(), "the form resets after saving");
            assert_eq!(
                page.message.as_deref(),
                Some("organization Acme Corp updated"),
                "the outcome is reported instead of landing silently"
            );
            assert_eq!(str_field(&page.orgs[0], "name"), "Acme Corp");
            assert_eq!(domains_text(&page.orgs[0]), "acme.com, acme.io");
            assert!(
                page.detail.is_some(),
                "the selected detail follows the reload"
            );
        }

        page.close_form();
        page.start_edit(&page.orgs[0].clone());
        assert_eq!(
            page.new_name,
            str_field(&page.orgs[0], "name"),
            "a second edit prefills again"
        );
        page.close_form();
        assert_eq!(page.edit_id, None, "cancel closes the edit");
        assert!(page.new_name.is_empty());
        assert!(page.new_domains.is_empty());

        page.error = None;
        page.edit_id = Some(999_999);
        page.save_edit(&db);
        assert!(page.error.is_some());
        assert!(page.message.is_none());
    }

    #[test]
    fn member_rows_navigate_to_the_customer_profile() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            let org =
                spp_core::people_store::create_organization(conn, "Acme", &["acme.com".into()])
                    .expect("acme");
            conn.execute(
                "UPDATE customers SET organization_id = ?1 WHERE id = 9",
                rusqlite::params![org],
            )
            .expect("member");
        });
        let mut page = OrganizationsPage::new();
        page.reload(&db);
        let id = i64_field(&page.orgs[0], "id");
        page.selected = Some(id);
        page.reload_detail(&db, id);
        let detail = page.detail.expect("the selection loads the detail");
        let members = detail
            .get("members")
            .and_then(|m| m.as_array())
            .expect("the member list");
        let member = &members[0];
        assert_eq!(str_field(member, "name"), "Kim Pearce");
        assert_eq!(
            OrganizationsPage::member_request(member),
            Some(NavRequest::OpenCustomer(9)),
            "a member click opens that customer's profile"
        );
    }

    #[test]
    fn delete_flows_through_the_confirm_request_and_the_soft_delete() {
        let db = fixture(Vec::new());
        let mut page = OrganizationsPage::new();
        page.new_name = "Acme".into();
        page.create(&db);
        let org = page.orgs[0].clone();
        let id = i64_field(&org, "id");

        let nav = OrganizationsPage::delete_request(id, "Acme");
        let NavRequest::Confirm(request) = nav else {
            panic!("a delete click must arm the confirm dialog");
        };
        assert!(request.danger, "deleting an organization is destructive");
        assert_eq!(
            request.action,
            crate::pages::DestructiveAction::DeleteOrganization { id },
            "the shell routes the confirm to the organization delete lane"
        );
        assert!(request.message.contains("Acme"));

        let removed = db
            .with_write(|conn| spp_core::people_store::delete_organization(conn, id))
            .expect("soft delete");
        assert!(removed);
        page.invalidate();
        page.reload(&db);
        assert!(page.orgs.is_empty(), "the soft-deleted org leaves the list");
        assert!(page.detail.is_none(), "the invalidated detail goes with it");

        let again = db
            .with_write(|conn| spp_core::people_store::delete_organization(conn, id))
            .expect("soft delete again");
        assert!(!again, "double delete is refused");
    }
}
