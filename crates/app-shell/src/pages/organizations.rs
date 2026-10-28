//! Organizations page (port of the web `pages/organizations.rs`).
//!
//! RS-17 (wave A) rebuild on **Design Contract v1.0 archetype DS-A-01** —
//! the list-page recipe (docs/audit/design-contract-v1.md §4): PageHeader
//! with the counter pill (DS-C-01, register A-6 — counts live beside the
//! header) → FilterBar with exactly one primary apply (DS-C-10, A-2) →
//! DataTable (DS-C-05, virtualized rows) → the row `open` act opens the
//! DetailPanel (DS-C-17, A-1) whose content is the DS-A-03 record
//! composition (record-head avatar + name + meta, then the members card).
//! The conformed lab reference is `mockups/design-lab/pages-ds.js`
//! (`DS.definePage("organizations", …)`).
//!
//! Behavior, queries, and data flow are unchanged (decisions.md D6:
//! presentation only). The list still reads
//! `spp_core::people_store::list_organizations` (the `/api/organizations`
//! SQL, capped at 200) with the two-field filter (edit vs applied), the
//! detail still reads `get_organization_detail` (the
//! `/api/organizations/{id}` SQL — customer members, domain list,
//! conversation counts), create still runs `create_organization` with the
//! same name + comma-separated-domains validation, and the sync action /
//! invalidate latch (clean-audit item 147) / selection-validity semantics
//! are the exact same state machine as before the rebuild.

use eframe::egui;
use spp_core::db::DbHandle;

use super::{i64_field, opt_str_field, str_field};
use crate::design::{self, AvatarSize, BadgeTone, ButtonVariant};
use crate::icons::Icon;

/// DataTable row height — the 32px button row (DS-C-05 cells ride it).
const ROW_HEIGHT: f32 = 32.0;

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
    /// Clean-audit item 147: reload latch — an empty mirror is a legal
    /// steady state, so "list empty" can no longer mean "re-query every
    /// frame".
    loaded: bool,
    /// DS-C-17 DetailPanel open flag (A-1): the row `open` act opens the
    /// right sheet; Esc, the scrim, or the close button close it.
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

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
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
                // The created org's detail becomes visible — the panel takes
                // the role the old right column played.
                self.panel_open = true;
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

        // DS-C-01 PageHeader: title + subtitle, right cluster = counter pill
        // (A-6) + the sync composite (A-8). egui right_to_left puts the first
        // added widget rightmost, so the add order below paints
        // [counter] [Sync now] [status line] — the lab's DS-C-01 order.
        design::page_header(
            ui,
            "Organizations",
            Some("customer organizations, their domains, and their members"),
            |ui| {
                if !self.sync_note.is_empty() {
                    // The in-flight/already-running note: the lab's unclassed
                    // `.ds-status-line` state (text-2) — the completion tone
                    // is the global toast's job (R8, D5).
                    design::hint_text(ui, &self.sync_note);
                }
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
                design::page_counter(ui, &format!("{} organizations", self.orgs.len()));
            },
        );

        // DS-C-10 FilterBar (A-2): filters left, exactly one primary apply
        // right. The two-field design is preserved — `filter_edit` is the
        // edit state, `filter` the applied state; the primary action is the
        // same apply trigger as before the rebuild.
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

        // DS-C-05 DataTable — virtualized rows over the same 200-capped
        // list query; columns per the conformed lab list (organization,
        // domains, customers, conversations, open act). With no rows the
        // lab engine swaps the table for the empty state (A-7).
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

        // A-1: the row `open` act selects the org and opens the DetailPanel.
        if let Some(id) = pick {
            self.selected = Some(id);
            self.panel_open = true;
            self.reload_detail(db, id);
        }

        // The lab's create affordance: a page-flow card under the table
        // (`blocks` in the DS lab — `card_titled` + DS-C-13 form rows +
        // one primary create action). Same validation, same write.
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

        // DS-C-17 DetailPanel (A-1) over the scrim — content is the DS-A-03
        // record composition: record-head (avatar + name + meta + domains),
        // then the members card.
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
                // The store emits the member array under `members`
                // (people_store::get_organization_detail) — the pre-rebuild
                // page read `customers` and silently rendered none of them.
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

                // DS-A-03 record-head: avatar + name + meta line, domains as text.
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

                // Members block: per-member line (avatar + name + email, job
                // title as meta).
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

/// The organization's domains as display text (`"acme.com, acme.io"`).
/// The mirror stores a JSON array; a plain string passes through.
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

/// DS-C-19 initials: the first character of up to two words, uppercased.
fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

/// One DataTable cell: fixed width, vertically centered in the 32px row,
/// truncated (DS-C-05 cell anatomy). `right` right-aligns the text (the
/// mono numeral cells).
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
