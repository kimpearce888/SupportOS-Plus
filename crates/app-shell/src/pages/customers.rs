//! Customers pages (port of the web `pages/customers.rs`): a search page
//! and a customer profile page.
//!
//! Data comes straight from the core functions the `/api/customers`
//! routes call — `people_store::list_customers_summary` /
//! `get_customer_detail`, `customer_events::list_for_customer` +
//! `kind_counts`, `people_store::customer_support_health`,
//! `interaction_engine::build_profile` — plus the conversation list via
//! `inbox::list_conversations` with a customer filter. No HTTP hop.

use eframe::egui;
use rusqlite::Connection;
use serde_json::Value;
use spp_core::db::DbHandle;

use super::{i64_field, opt_str_field, str_field};

/// Which surface is showing.
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Search,
    Profile { customer_id: i64 },
}

/// One row of the search results (typed view over the summary JSON).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CustomerRow {
    pub id: i64,
    pub display_name: String,
    pub email: Option<String>,
    pub conversation_count: i64,
    pub last_contact: Option<String>,
}

/// Read a summary row out of the people-store JSON (field names follow
/// `people_store::customer_summary_row`).
#[must_use]
pub fn row_from_summary(v: &Value) -> CustomerRow {
    let first = str_field(v, "first_name");
    let last = str_field(v, "last_name");
    let display_name = format!("{first} {last}").trim().to_string();
    CustomerRow {
        id: i64_field(v, "id"),
        display_name,
        email: v
            .get("emails")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .map(str::to_string),
        conversation_count: i64_field(v, "conversation_count"),
        last_contact: opt_str_field(v, "last_activity_at"),
    }
}

/// Contact card facts extracted from `get_customer_detail`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContactCard {
    pub display_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub company: Option<String>,
    pub created_at: Option<String>,
    pub locale: Option<String>,
}

/// Read the contact card out of the detail JSON. The detail nests the
/// summary row under `"customer"` (see `people_store::get_customer_detail`).
#[must_use]
pub fn contact_from_detail(detail: &Value) -> ContactCard {
    let customer = detail.get("customer").unwrap_or(detail);
    let first = str_field(customer, "first_name");
    let last = str_field(customer, "last_name");
    ContactCard {
        display_name: format!("{first} {last}").trim().to_string(),
        email: customer
            .get("emails")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .map(str::to_string),
        phone: customer
            .get("phones")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .map(str::to_string),
        company: opt_str_field(customer, "organization_name")
            .or_else(|| opt_str_field(customer, "organization")),
        created_at: opt_str_field(customer, "last_activity_at"),
        locale: opt_str_field(customer, "locale"),
    }
}

/// Timeline event row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimelineEvent {
    pub kind: String,
    pub occurred_at: String,
    pub conversation_number: Option<i64>,
    pub summary: String,
}

/// Read a timeline event out of the customer-events JSON.
#[must_use]
pub fn event_row(v: &Value) -> TimelineEvent {
    let occurred_at = str_field(v, "occurred_at");
    let occurred_at = if occurred_at.is_empty() {
        str_field(v, "created_at")
    } else {
        occurred_at
    };
    let summary = str_field(v, "summary");
    let summary = if summary.is_empty() {
        str_field(v, "title")
    } else {
        summary
    };
    TimelineEvent {
        kind: str_field(v, "kind"),
        occurred_at,
        conversation_number: v.get("conversation_number").and_then(|f| f.as_i64()),
        summary,
    }
}

/// Support-health summary (subset the page shows in the health card).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SupportHealth {
    pub open_conversations: i64,
    pub not_good_ratings_90d: i64,
    pub breached_sla: i64,
}

/// Read the health summary out of the support-health report JSON.
#[must_use]
pub fn health_from_report(report: &Value) -> SupportHealth {
    let root = report.get("report").unwrap_or(report);
    SupportHealth {
        open_conversations: i64_field(root, "open_conversations"),
        not_good_ratings_90d: i64_field(root, "not_good_ratings_90d"),
        breached_sla: i64_field(root, "sla_breaches"),
    }
}

/// Conversations for a customer (typed reuse of the inbox query).
pub fn conversations_for(
    conn: &Connection,
    customer_id: i64,
) -> spp_core::Result<Vec<spp_core::inbox::ConversationListItem>> {
    let filters = spp_core::inbox::InboxFilters {
        customer_id: Some(customer_id),
        limit: Some(20),
        ..Default::default()
    };
    Ok(spp_core::inbox::list_conversations(conn, &filters)?.0)
}

/// The interactive customers page state.
pub struct CustomersPage {
    mode: Mode,
    search_edit: String,
    applied_query: Option<String>,
    rows: Vec<CustomerRow>,
    total: i64,
    profile: Option<Value>,
    interaction: Option<Value>,
    health: Option<Value>,
    timeline: Vec<TimelineEvent>,
    kind_counts: Vec<(String, i64)>,
    conversations: Vec<spp_core::inbox::ConversationListItem>,
    error: Option<String>,
    /// Set by `invalidate`; the next frame reloads the visible surface.
    stale: bool,
}

impl CustomersPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: Mode::Search,
            search_edit: String::new(),
            applied_query: None,
            rows: Vec::new(),
            total: 0,
            profile: None,
            interaction: None,
            health: None,
            timeline: Vec::new(),
            kind_counts: Vec::new(),
            conversations: Vec::new(),
            error: None,
            stale: false,
        }
    }

    /// Live-update invalidation (rating/sync events refresh customer
    /// data): mark stale; the next frame reloads.
    pub fn invalidate(&mut self) {
        self.stale = true;
    }

    fn reload_search(&mut self, db: &DbHandle) {
        let result = db.with_read(|conn| {
            spp_core::people_store::list_customers_summary(
                conn,
                self.applied_query.as_deref(),
                1,
                50,
            )
        });
        match result {
            Ok((summaries, total)) => {
                self.rows = summaries.iter().map(row_from_summary).collect();
                self.total = total;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Open the profile surface for a customer.
    pub fn open_profile(&mut self, db: &DbHandle, customer_id: i64) {
        self.mode = Mode::Profile { customer_id };
        self.load_profile(db, customer_id);
    }

    fn load_profile(&mut self, db: &DbHandle, customer_id: i64) {
        let (profile, interaction, health, timeline, kind_counts, conversations) =
            db.with_read(|conn| {
                let profile = spp_core::people_store::get_customer_detail(conn, customer_id)
                    .ok()
                    .flatten();
                let interaction = spp_core::interaction_engine::build_profile(conn, customer_id)
                    .ok()
                    .flatten()
                    .and_then(|p| serde_json::to_value(&p).ok());
                let health = spp_core::people_store::customer_support_health(conn, customer_id)
                    .ok()
                    .flatten();
                let timeline = match spp_core::customer_events::list_for_customer(
                    conn,
                    customer_id,
                    None,
                    100,
                    0,
                ) {
                    Ok((events, _)) => events.iter().map(event_row).collect(),
                    Err(_) => Vec::new(),
                };
                let kind_counts = match spp_core::customer_events::kind_counts(conn, customer_id) {
                    Ok(counts) => counts
                        .iter()
                        .map(|c| (str_field(c, "kind"), i64_field(c, "count")))
                        .collect(),
                    Err(_) => Vec::new(),
                };
                let conversations = conversations_for(conn, customer_id).unwrap_or_default();
                (
                    profile,
                    interaction,
                    health,
                    timeline,
                    kind_counts,
                    conversations,
                )
            });
        self.profile = profile;
        self.interaction = interaction;
        self.health = health;
        self.timeline = timeline;
        self.kind_counts = kind_counts;
        self.conversations = conversations;
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.stale {
            self.stale = false;
            match self.mode {
                Mode::Search => self.reload_search(db),
                Mode::Profile { customer_id } => self.load_profile(db, customer_id),
            }
        }
        ui.heading("Customers");
        ui.separator();

        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("error: {err}"));
            ui.separator();
        }

        match self.mode {
            Mode::Search => self.search_ui(ui, db),
            Mode::Profile { customer_id } => self.profile_ui(ui, db, customer_id),
        }
    }

    fn search_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.horizontal(|ui| {
            let mut search = self.search_edit.clone();
            let response = ui.add(
                egui::TextEdit::singleline(&mut search)
                    .hint_text("Search customers by name or email…"),
            );
            if response.changed() {
                self.search_edit = search;
            }
            if ui.button("search").clicked()
                || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                let trimmed = self.search_edit.trim();
                self.applied_query = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                self.reload_search(db);
            }
        });
        ui.separator();

        if self.rows.is_empty() {
            ui.weak("No customers loaded yet — search or press Enter.");
            return;
        }
        ui.strong(format!("{} customers (of {})", self.rows.len(), self.total));
        let mut clicked: Option<i64> = None;
        egui::ScrollArea::vertical().show_rows(ui, 24.0, self.rows.len(), |ui, range| {
            egui::Grid::new("customer-results")
                .num_columns(5)
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("name");
                    ui.strong("email");
                    ui.strong("conversations");
                    ui.strong("last contact");
                    ui.strong("");
                    ui.end_row();
                    for row in self.rows[range].iter().cloned() {
                        ui.strong(row.display_name.as_str());
                        ui.label(row.email.as_deref().unwrap_or("—"));
                        ui.monospace(row.conversation_count.to_string());
                        ui.label(row.last_contact.as_deref().unwrap_or("—"));
                        if ui.button("open").clicked() {
                            clicked = Some(row.id);
                        }
                        ui.end_row();
                    }
                });
        });
        if let Some(id) = clicked {
            self.open_profile(db, id);
        }
    }

    fn profile_ui(&mut self, ui: &mut egui::Ui, _db: &DbHandle, customer_id: i64) {
        if ui.button("← back to search").clicked() {
            self.mode = Mode::Search;
            self.rows.clear();
            return;
        }
        let Some(profile) = self.profile.clone() else {
            ui.weak(format!("Customer {customer_id} not found."));
            return;
        };
        let contact = contact_from_detail(&profile);

        ui.strong(contact.display_name.as_str());
        ui.horizontal(|ui| {
            ui.label(contact.email.as_deref().unwrap_or("—"));
            if let Some(phone) = &contact.phone {
                ui.label(phone.as_str());
            }
            if let Some(company) = &contact.company {
                ui.weak(company.as_str());
            }
            if let Some(last_seen) = &contact.created_at {
                ui.weak(format!("last activity {last_seen}"));
            }
        });
        ui.separator();

        ui.columns(2, |cols| {
            health_and_counts(&mut cols[0], &self.health, &self.kind_counts);
            interaction_summary(&mut cols[1], self.interaction.as_ref());
        });
        ui.separator();

        timeline_ui(ui, &self.timeline);
        ui.separator();

        conversations_ui(ui, &self.conversations);
    }
}

impl Default for CustomersPage {
    fn default() -> Self {
        Self::new()
    }
}

fn health_and_counts(ui: &mut egui::Ui, health: &Option<Value>, counts: &[(String, i64)]) {
    ui.strong("Support health");
    if let Some(report) = health {
        let h = health_from_report(report);
        egui::Grid::new("customer-health")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("open conversations");
                ui.monospace(h.open_conversations.to_string());
                ui.end_row();
                ui.label("not-good ratings (90d)");
                ui.monospace(h.not_good_ratings_90d.to_string());
                ui.end_row();
                ui.label("SLA breaches");
                ui.monospace(h.breached_sla.to_string());
                ui.end_row();
            });
    } else {
        ui.weak("(no health report)");
    }
    ui.add_space(6.0);
    ui.strong("Event mix");
    if counts.is_empty() {
        ui.weak("(no events)");
        return;
    }
    let max = counts.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1);
    for (kind, count) in counts {
        let ratio = *count as f32 / max as f32;
        ui.add(
            egui::ProgressBar::new(ratio.clamp(0.0, 1.0))
                .text(format!("{kind}: {count}"))
                .desired_width(220.0),
        );
    }
}

fn interaction_summary(ui: &mut egui::Ui, interaction: Option<&Value>) {
    ui.strong("Interaction profile");
    let Some(profile) = interaction else {
        ui.weak("(no AI profile yet)");
        return;
    };
    // The profile JSON carries dimension/value pairs; render whatever is
    // present as key: value lines.
    if let Some(map) = profile.get("dimensions").and_then(Value::as_object) {
        for (dimension, value) in map {
            ui.label(format!(
                "{dimension}: {}",
                value.as_str().unwrap_or(&value.to_string())
            ));
        }
    } else if let Some(map) = profile.as_object() {
        for (key, value) in map {
            if key == "dimensions" {
                continue;
            }
            ui.label(format!(
                "{key}: {}",
                value.as_str().unwrap_or(&value.to_string())
            ));
        }
    }
}

fn timeline_ui(ui: &mut egui::Ui, events: &[TimelineEvent]) {
    ui.strong("Timeline");
    if events.is_empty() {
        ui.weak("(no timeline events)");
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("customer-timeline")
        .max_height(220.0)
        .show_rows(ui, 20.0, events.len(), |ui, range| {
            for event in &events[range] {
                ui.horizontal(|ui| {
                    ui.weak(event.occurred_at.as_str());
                    ui.strong(event.kind.as_str());
                    if let Some(number) = event.conversation_number {
                        ui.monospace(format!("#{number}"));
                    }
                    ui.label(event.summary.as_str());
                });
            }
        });
}

fn conversations_ui(ui: &mut egui::Ui, items: &[spp_core::inbox::ConversationListItem]) {
    ui.strong("Conversations");
    if items.is_empty() {
        ui.weak("(no conversations)");
        return;
    }
    egui::Grid::new("customer-conversations")
        .num_columns(4)
        .striped(true)
        .show(ui, |ui| {
            ui.strong("#");
            ui.strong("subject");
            ui.strong("status");
            ui.strong("updated");
            ui.end_row();
            for item in items {
                ui.monospace(item.number.to_string());
                ui.label(item.subject.as_deref().unwrap_or("(no subject)"));
                ui.label(item.status.as_str());
                ui.label(item.updated_at.as_deref().unwrap_or("—"));
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::{empty_db, fixture};

    #[test]
    fn row_from_summary_maps_fields() {
        // Field names follow people_store::customer_summary_row.
        let v = serde_json::json!({
            "id": 9,
            "first_name": "Kim",
            "last_name": "Pearce",
            "emails": ["kim@example.com"],
            "phones": [],
            "conversation_count": 4,
            "last_activity_at": "2026-10-18T10:00:00Z",
        });
        let row = row_from_summary(&v);
        assert_eq!(row.id, 9);
        assert_eq!(row.display_name, "Kim Pearce");
        assert_eq!(row.email.as_deref(), Some("kim@example.com"));
        assert_eq!(row.conversation_count, 4);
        assert!(row.last_contact.is_some());
    }

    #[test]
    fn contact_and_health_extraction() {
        // The detail JSON nests the summary under "customer".
        let detail = serde_json::json!({
            "customer": {
                "first_name": "Kim",
                "last_name": "Pearce",
                "emails": ["kim@example.com"],
                "phones": ["+1 555"],
                "organization_name": "Acme",
                "last_activity_at": "2026-10-18T10:00:00Z",
            }
        });
        let contact = contact_from_detail(&detail);
        assert_eq!(contact.display_name, "Kim Pearce");
        assert_eq!(contact.email.as_deref(), Some("kim@example.com"));
        assert_eq!(contact.phone.as_deref(), Some("+1 555"));
        assert_eq!(contact.company.as_deref(), Some("Acme"));

        let report = serde_json::json!({
            "report": {
                "open_conversations": 3,
                "not_good_ratings_90d": 1,
                "sla_breaches": 2,
            }
        });
        let health = health_from_report(&report);
        assert_eq!(health.open_conversations, 3);
        assert_eq!(health.not_good_ratings_90d, 1);
        assert_eq!(health.breached_sla, 2);
    }

    #[test]
    fn event_row_maps_fallback_fields() {
        let v = serde_json::json!({
            "kind": "conversation_created",
            "created_at": "2026-01-02T00:00:00Z",
            "conversation_number": 12,
            "title": "Opened a ticket",
        });
        let event = event_row(&v);
        assert_eq!(event.kind, "conversation_created");
        assert_eq!(event.occurred_at, "2026-01-02T00:00:00Z");
        assert_eq!(event.conversation_number, Some(12));
        assert_eq!(event.summary, "Opened a ticket");
    }

    #[test]
    fn conversations_for_filters_by_customer() {
        let db = fixture(vec![
            ("conv", 1, "Kim's ticket", "active", "email", 1, 5, 9),
            ("conv", 2, "Other ticket", "active", "email", 1, 5, 10),
        ]);
        let items = db
            .with_read(|conn| conversations_for(conn, 9))
            .expect("conversations");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].subject.as_deref(), Some("Kim's ticket"));
    }

    #[test]
    fn search_and_profile_flow_through_core() {
        let db = fixture(vec![(
            "conv",
            1,
            "Kim's ticket",
            "active",
            "email",
            1,
            5,
            9,
        )]);
        let mut page = CustomersPage::new();
        page.reload_search(&db);
        assert!(page.rows.iter().any(|r| r.display_name.contains("Kim")));
        assert!(page.total >= 1);

        let kim_id = page
            .rows
            .iter()
            .find(|r| r.display_name.contains("Kim"))
            .map(|r| r.id)
            .expect("kim row");
        page.load_profile(&db, kim_id);
        let profile = page.profile.clone().expect("profile");
        let customer = profile.get("customer").cloned().unwrap_or_default();
        let contact = contact_from_detail(&profile);
        assert_eq!(contact.email.as_deref(), Some("kim@example.com"));
        assert_eq!(contact.display_name, "Kim Pearce");
        assert!(customer.is_object() || contact.display_name == "Kim Pearce");
        assert_eq!(page.conversations.len(), 1);
    }

    #[test]
    fn empty_db_search_is_empty_not_error() {
        let db = empty_db();
        let mut page = CustomersPage::new();
        page.reload_search(&db);
        assert!(page.rows.is_empty());
        assert!(page.error.is_none());
    }
}
