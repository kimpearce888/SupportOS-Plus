//! Organizations page (port of the web `pages/organizations.rs`).
//!
//! Presentational pass (shadcn redesign): this page renders through the
//! shared design system in `crate::design`; the queries, state, and
//! behavior are unchanged.
//!
//! Left: the organization list from `spp_core::people_store::list_organizations`
//! (the `/api/organizations` SQL) with a name filter and create form
//! (`create_organization`). Right: the selected organization's detail from
//! `get_organization_detail` (the `/api/organizations/{id}` SQL) — customer
//! members, domain list, conversation counts.

use eframe::egui;
use spp_core::db::DbHandle;

use super::{i64_field, str_field};
use crate::design::{self, ButtonVariant};
use crate::icons::Icon;

/// The interactive organizations page state.
pub struct OrganizationsPage {
    /// R8 (D5): outcome note for the page's "Sync now" action — the
    /// completion itself is reported by the global sync toast.
    pub sync_note: String,

    filter_edit: String,
    filter: String,
    orgs: Vec<serde_json::Value>,
    detail: Option<serde_json::Value>,
    selected: Option<i64>,
    new_name: String,
    new_domains: String,
    error: Option<String>,
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
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
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
                // Keep the selection valid across reloads.
                if let Some(id) = self.selected
                    && !self.orgs.iter().any(|o| i64_field(o, "id") == id)
                {
                    self.selected = None;
                    self.detail = None;
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
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
        if self.orgs.is_empty() {
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
                self.reload(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Render the page.
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
                if design::button_with_icon(ui, Icon::Refresh, "Sync now", ButtonVariant::Outline)
                    .on_hover_text(
                        "pull fresh data from Help Scout now — the completion toast \
                     reports the outcome; the page re-reads on every sync anyway",
                    )
                    .clicked()
                {
                    // R8 (D5): this used to be a local-only "Reload" that silently
                    // re-read sqlite and looked like a refresh. It now performs a
                    // real scoped fetch and reports the outcome; the invalidation
                    // below re-reads whatever arrives.
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
                if !self.sync_note.is_empty() {
                    ui.label(
                        egui::RichText::new(&self.sync_note)
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                }
            },
        );
        ui.horizontal(|ui| {
            let mut filter = self.filter_edit.clone();
            let response = ui.add_sized(
                [260.0, 28.0],
                egui::TextEdit::singleline(&mut filter).hint_text("Filter by name…"),
            );
            if response.changed() {
                self.filter_edit = filter;
            }
            if design::button(ui, "filter", ButtonVariant::Outline).clicked() {
                self.filter = self.filter_edit.clone();
                self.reload(db);
            }
        });
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        design::divider(ui);

        ui.columns(2, |columns| {
            // Left: list + create form.
            let orgs = self.orgs.clone();
            let mut pick: Option<i64> = None;
            egui::ScrollArea::vertical()
                .id_salt("org-list")
                .show(&mut columns[0], |ui| {
                    design::section_title(ui, &format!("{} organization(s)", orgs.len()));
                    for org in &orgs {
                        let id = i64_field(org, "id");
                        let name = str_field(org, "name");
                        let customers = i64_field(org, "customer_count");
                        let convs = i64_field(org, "conversation_count");
                        let selected = self.selected == Some(id);
                        if design::chip(
                            ui,
                            selected,
                            &format!("{name} · {customers} customers · {convs} conversations"),
                        )
                        .clicked()
                        {
                            pick = Some(id);
                        }
                    }
                    ui.add_space(10.0);
                    design::card_titled(ui, "New organization", None, |ui| {
                        ui.add_sized(
                            [ui.available_width(), 28.0],
                            egui::TextEdit::singleline(&mut self.new_name).hint_text("Name"),
                        );
                        ui.add_sized(
                            [ui.available_width(), 28.0],
                            egui::TextEdit::singleline(&mut self.new_domains)
                                .hint_text("acme.com, acme.io"),
                        )
                        .on_hover_text("comma-separated domains");
                    });
                });
            if let Some(id) = pick {
                self.selected = Some(id);
                self.reload_detail(db, id);
            }
            if design::button(
                &mut columns[0],
                "create organization",
                ButtonVariant::Primary,
            )
            .on_hover_text("create with the name + domains above")
            .clicked()
            {
                self.create(db);
            }

            // Right: detail.
            let detail = self.detail.clone();
            egui::ScrollArea::vertical()
                .id_salt("org-detail")
                .show(&mut columns[1], |ui| {
                    let Some(org) = detail else {
                        design::empty_state(
                            ui,
                            Icon::Building,
                            "Select an organization",
                            "Its members, domains, and conversation counts appear here.",
                        );
                        return;
                    };
                    design::section_title(ui, &str_field(&org, "name"));
                    let domains = org
                        .get("domains")
                        .and_then(|d| d.as_str())
                        .map(|d| d.to_string())
                        .unwrap_or_default();
                    if !domains.is_empty() {
                        ui.monospace(
                            egui::RichText::new(domains).color(design::tokens(ui).foreground),
                        );
                    }
                    design::divider(ui);
                    let members = org
                        .get("customers")
                        .and_then(|c| c.as_array())
                        .cloned()
                        .unwrap_or_default();
                    design::hint_text(ui, &format!("{} member(s)", members.len()));
                    let t = design::tokens(ui);
                    for member in &members {
                        let first = str_field(member, "first_name");
                        let last = str_field(member, "last_name");
                        let email = str_field(member, "email");
                        let title = super::opt_str_field(member, "job_title");
                        let line = format!("{first} {last} · {email}");
                        let line = if let Some(job) = title {
                            format!("{line} ({job})")
                        } else {
                            line
                        };
                        ui.label(egui::RichText::new(line).size(13.0).color(t.foreground));
                    }
                });
        });
    }
}

impl Default for OrganizationsPage {
    fn default() -> Self {
        Self::new()
    }
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
