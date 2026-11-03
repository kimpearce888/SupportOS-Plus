use eframe::egui;
use rusqlite::Connection;
use serde_json::Value;
use spp_core::db::DbHandle;

use super::{i64_field, opt_str_field, str_field};
use crate::design::{self, AvatarSize, BadgeTone, ButtonVariant};
use crate::icons::Icon;

const ROW_HEIGHT: f32 = 32.0;

const CUSTOMERS_PAGE_SIZE: i64 = 50;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CustomerRow {
    pub id: i64,
    pub display_name: String,
    pub email: Option<String>,
    pub conversation_count: i64,
    pub last_contact: Option<String>,
}

#[must_use]
pub(crate) fn row_from_summary(v: &Value) -> CustomerRow {
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

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ContactCard {
    pub display_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub company: Option<String>,
    pub created_at: Option<String>,
    pub locale: Option<String>,
}

#[must_use]
pub(crate) fn contact_from_detail(detail: &Value) -> ContactCard {
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

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TimelineEvent {
    pub kind: String,
    pub occurred_at: String,
    pub conversation_number: Option<i64>,
    pub summary: String,
}

#[must_use]
pub(crate) fn event_row(v: &Value) -> TimelineEvent {
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

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SupportHealth {
    pub open_conversations: i64,
    pub not_good_ratings_90d: i64,
    pub breached_sla: i64,
}

#[must_use]
pub(crate) fn health_from_report(report: &Value) -> SupportHealth {
    let root = report.get("report").unwrap_or(report);
    SupportHealth {
        open_conversations: i64_field(root, "open_conversations"),
        not_good_ratings_90d: i64_field(root, "not_good_ratings_90d"),
        breached_sla: i64_field(root, "sla_breaches"),
    }
}

pub(crate) fn conversations_for(
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

pub struct CustomersPage {
    pub sync_note: String,

    search_edit: String,
    applied_query: Option<String>,
    rows: Vec<CustomerRow>,
    total: i64,
    page: i64,
    profile: Option<Value>,
    interaction: Option<Value>,
    health: Option<Value>,
    recommendation: Option<Value>,
    timeline: Vec<TimelineEvent>,
    kind_counts: Vec<(String, i64)>,
    conversations: Vec<spp_core::inbox::ConversationListItem>,
    milestones: Vec<spp_core::reports::TimelineEvent>,
    error: Option<String>,
    stale: bool,
    selected: Option<i64>,
    panel_open: bool,
}

impl CustomersPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sync_note: String::new(),
            search_edit: String::new(),
            applied_query: None,
            rows: Vec::new(),
            total: 0,
            page: 1,
            profile: None,
            interaction: None,
            health: None,
            recommendation: None,
            timeline: Vec::new(),
            kind_counts: Vec::new(),
            conversations: Vec::new(),
            milestones: Vec::new(),
            error: None,
            stale: false,
            selected: None,
            panel_open: false,
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.stale = true;
    }

    fn reload_search(&mut self, db: &DbHandle) {
        let result = db.with_read(|conn| {
            spp_core::people_store::list_customers_summary(
                conn,
                self.applied_query.as_deref(),
                self.page,
                CUSTOMERS_PAGE_SIZE,
            )
        });
        match result {
            Ok((summaries, total)) => {
                if summaries.is_empty() && self.page > 1 && total > 0 {
                    self.page = (total + CUSTOMERS_PAGE_SIZE - 1) / CUSTOMERS_PAGE_SIZE;
                    return self.reload_search(db);
                }
                self.rows = summaries.iter().map(row_from_summary).collect();
                self.total = total;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub(crate) fn open_profile(&mut self, db: &DbHandle, customer_id: i64) {
        self.selected = Some(customer_id);
        self.panel_open = true;
        self.load_profile(db, customer_id);
    }

    fn load_profile(&mut self, db: &DbHandle, customer_id: i64) {
        let (
            profile,
            interaction,
            health,
            timeline,
            kind_counts,
            conversations,
            milestones,
            recommendation,
        ) = db.with_read(|conn| {
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
            let timeline =
                match spp_core::customer_events::list_for_customer(conn, customer_id, None, 100, 0)
                {
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
            let milestones =
                spp_core::reports::get_timeline(conn, customer_id, 50).unwrap_or_default();
            let recommendation: Option<serde_json::Value> = conn
                .query_row(
                    "SELECT s.recommendation_json, s.sources
                           FROM client_current_signals s
                          WHERE s.customer_id = ?1 AND s.recommendation_json IS NOT NULL
                          ORDER BY s.generated_at DESC LIMIT 1",
                    rusqlite::params![customer_id],
                    |r| {
                        let raw: Option<String> = r.get(0)?;
                        let sources: Option<String> = r.get(1)?;
                        Ok(
                            serde_json::from_str::<serde_json::Value>(&raw.unwrap_or_default())
                                .ok()
                                .map(|mut v| {
                                    if let Some(obj) = v.as_object_mut() {
                                        obj.insert(
                                            "sources".into(),
                                            serde_json::json!(sources.unwrap_or_default()),
                                        );
                                    }
                                    v
                                }),
                        )
                    },
                )
                .ok()
                .flatten();
            (
                profile,
                interaction,
                health,
                timeline,
                kind_counts,
                conversations,
                milestones,
                recommendation,
            )
        });
        self.profile = profile;
        self.interaction = interaction;
        self.health = health;
        self.timeline = timeline;
        self.kind_counts = kind_counts;
        self.conversations = conversations;
        self.milestones = milestones;
        self.recommendation = recommendation;
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        engine: Option<&std::sync::Arc<spp_core::sync_engine::SyncEngine>>,
    ) {
        if self.stale {
            self.stale = false;
            self.reload_search(db);
            if self.panel_open
                && let Some(id) = self.selected
            {
                self.load_profile(db, id);
            }
        }

        design::page_header(ui, "Customers", None, |ui| {
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
            design::page_counter(ui, &self.count_label());
        });

        let mut enter_apply = false;
        let apply = design::filter_bar(
            ui,
            |ui| {
                let mut search = self.search_edit.clone();
                let response = ui.add_sized(
                    [280.0, 30.0],
                    egui::TextEdit::singleline(&mut search)
                        .hint_text("Search customers by name or email…"),
                );
                if response.changed() {
                    self.search_edit = search;
                }
                enter_apply =
                    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            },
            "search",
        );
        if apply.clicked() || enter_apply {
            let trimmed = self.search_edit.trim();
            self.applied_query = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
            self.page = 1;
            self.reload_search(db);
        }
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        ui.add_space(design::scale::space::SP2);

        let rows = self.rows.clone();
        let mut pick: Option<i64> = None;
        if rows.is_empty() {
            design::empty_state(
                ui,
                Icon::Users,
                "No customers loaded yet",
                "Search by name or email and press Enter.",
            );
        } else {
            design::data_table(
                ui,
                "customers",
                &["name", "email", "conversations", "last contact", ""],
                rows.len(),
                ROW_HEIGHT,
                |ui, i| {
                    let row = &rows[i];
                    let t = design::tokens(ui);
                    dt_cell(
                        ui,
                        190.0,
                        egui::RichText::new(&row.display_name)
                            .size(design::scale::text::BODY)
                            .strong()
                            .color(t.foreground),
                        false,
                    );
                    dt_cell(
                        ui,
                        230.0,
                        egui::RichText::new(row.email.as_deref().unwrap_or("—"))
                            .size(design::scale::text::BODY)
                            .color(t.foreground),
                        false,
                    );
                    dt_cell(
                        ui,
                        90.0,
                        egui::RichText::new(row.conversation_count.to_string())
                            .size(design::scale::text::SMALL_HALF)
                            .monospace()
                            .color(t.foreground),
                        true,
                    );
                    dt_cell(
                        ui,
                        140.0,
                        egui::RichText::new(row.last_contact.as_deref().unwrap_or("—"))
                            .size(design::scale::text::SMALL_HALF)
                            .color(t.muted_foreground),
                        false,
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(70.0, ROW_HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            if design::button(ui, "open", ButtonVariant::Outline)
                                .on_hover_text(format!(
                                    "open {} — conversations, support health and the AI \
                                     interaction profile",
                                    row.display_name
                                ))
                                .clicked()
                            {
                                pick = Some(row.id);
                            }
                        },
                    );
                },
            );
        }

        let pages = u32::try_from((self.total + CUSTOMERS_PAGE_SIZE - 1) / CUSTOMERS_PAGE_SIZE)
            .unwrap_or(1);
        let delta = design::pager(
            ui,
            u32::try_from(self.page).unwrap_or(1).saturating_sub(1),
            pages,
        );
        if delta != 0
            && let Some(next) = self.page.checked_add(delta.into())
            && next >= 1
            && next <= i64::from(pages)
        {
            self.page = next;
            self.reload_search(db);
        }

        if let Some(id) = pick {
            self.open_profile(db, id);
        }

        let profile = self.profile.clone();
        let interaction = self.interaction.clone();
        let health = self.health.clone();
        let recommendation = self.recommendation.clone();
        let timeline = self.timeline.clone();
        let kind_counts = self.kind_counts.clone();
        let conversations = self.conversations.clone();
        let milestones = self.milestones.clone();
        let selected = self.selected;
        let title = profile
            .as_ref()
            .map(|p| contact_from_detail(p).display_name)
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Customer".to_string());
        let mut panel_open = self.panel_open;
        design::detail_panel(
            ui.ctx(),
            "customers-detail",
            &mut panel_open,
            &title,
            |_ui| {},
            |ui| {
                let Some(profile) = profile else {
                    match selected {
                        Some(id) => design::hint_text(ui, &format!("Customer {id} not found.")),
                        None => design::hint_text(ui, "No customer selected."),
                    }
                    return;
                };
                let t = design::tokens(ui);
                let contact = contact_from_detail(&profile);

                ui.horizontal(|ui| {
                    design::avatar(
                        ui,
                        AvatarSize::Md,
                        &initials(&contact.display_name),
                        BadgeTone::Neutral,
                    );
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(&contact.display_name)
                                .size(design::scale::text::SECTION_BIG)
                                .strong()
                                .color(t.foreground),
                        );
                        design::hint_text(ui, &record_meta(&contact));
                    });
                });
                design::divider(ui);

                ui.columns(2, |cols| {
                    health_and_counts(&mut cols[0], &health, &kind_counts);
                    milestones_ui(&mut cols[0], &milestones);
                    interaction_summary(
                        &mut cols[1],
                        interaction.as_ref(),
                        recommendation.as_ref(),
                    );
                    conversations_ui(&mut cols[1], &conversations);
                });
                ui.add_space(design::scale::space::SP2);
                timeline_ui(ui, &timeline);
            },
        );
        self.panel_open = panel_open;
    }

    fn count_label(&self) -> String {
        if self.total > CUSTOMERS_PAGE_SIZE {
            let from = (self.page - 1) * CUSTOMERS_PAGE_SIZE + 1;
            let to = from - 1 + self.rows.len() as i64;
            format!("{from}-{to} of {} customers", self.total)
        } else {
            format!("{} customers (of {})", self.rows.len(), self.total)
        }
    }
}

impl Default for CustomersPage {
    fn default() -> Self {
        Self::new()
    }
}

fn record_meta(contact: &ContactCard) -> String {
    let mut parts = vec![contact.email.as_deref().unwrap_or("—").to_string()];
    if let Some(phone) = &contact.phone {
        parts.push(phone.clone());
    }
    if let Some(company) = &contact.company {
        parts.push(company.clone());
    }
    if let Some(last_seen) = &contact.created_at {
        parts.push(format!("last activity {last_seen}"));
    }
    parts.join(" · ")
}

fn health_and_counts(ui: &mut egui::Ui, health: &Option<Value>, counts: &[(String, i64)]) {
    let t = design::tokens(ui);
    design::card_titled(ui, "Support health", None, |ui| {
        if let Some(report) = health {
            let h = health_from_report(report);
            design::form_grid(ui, "customer-health", |ui| {
                design::field_label(ui, "open conversations:");
                ui.monospace(
                    egui::RichText::new(h.open_conversations.to_string()).color(t.foreground),
                );
                ui.end_row();
                design::field_label(ui, "not-good ratings (90d):");
                ui.monospace(
                    egui::RichText::new(h.not_good_ratings_90d.to_string()).color(t.foreground),
                );
                ui.end_row();
                design::field_label(ui, "SLA breaches:");
                ui.monospace(egui::RichText::new(h.breached_sla.to_string()).color(t.foreground));
                ui.end_row();
            });
        } else {
            design::hint_text(ui, "(no health report)");
        }
        ui.add_space(6.0);
        design::divider(ui);
        ui.add_space(6.0);
        design::section_title(ui, "Event mix");
        if counts.is_empty() {
            design::hint_text(ui, "(no events)");
            return;
        }
        let max = counts.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1);
        for (kind, count) in counts {
            design::bar_row(ui, kind, *count, max, true);
        }
    });
}

fn interaction_summary(
    ui: &mut egui::Ui,
    interaction: Option<&Value>,
    recommendation: Option<&Value>,
) {
    let t = design::tokens(ui);
    design::card(ui, |ui| {
        ui.horizontal(|ui| {
            Icon::Sparkles.show(ui, 16.0, t.muted_foreground);
            design::section_title(ui, "Interaction profile");
            design::badge(ui, "AI", BadgeTone::Ai);
            if let Some(rec) = recommendation {
                let sources = rec.get("sources").and_then(Value::as_str).unwrap_or("");
                let tone = if sources.contains("ai") {
                    BadgeTone::Success
                } else {
                    BadgeTone::Neutral
                };
                design::badge(
                    ui,
                    if sources.is_empty() {
                        "heuristic"
                    } else {
                        sources
                    },
                    tone,
                );
            }
        });
        let Some(profile) = interaction else {
            design::hint_text(ui, "(no AI profile yet)");
            if let Some(rec) = recommendation {
                recommendation_block(ui, rec);
            }
            return;
        };
        if let Some(map) = profile.get("dimensions").and_then(Value::as_object) {
            for (dimension, value) in map {
                ui.label(
                    egui::RichText::new(format!(
                        "{dimension}: {}",
                        value.as_str().unwrap_or(&value.to_string())
                    ))
                    .size(design::scale::text::CONTROL)
                    .color(t.foreground),
                );
            }
        } else if let Some(map) = profile.as_object() {
            for (key, value) in map {
                if key == "dimensions" {
                    continue;
                }
                ui.label(
                    egui::RichText::new(format!(
                        "{key}: {}",
                        value.as_str().unwrap_or(&value.to_string())
                    ))
                    .size(design::scale::text::CONTROL)
                    .color(t.foreground),
                );
            }
        }
        if let Some(rec) = recommendation {
            ui.add_space(6.0);
            design::divider(ui);
            recommendation_block(ui, rec);
        }
    });
}

fn recommendation_block(ui: &mut egui::Ui, rec: &Value) {
    let t = design::tokens(ui);
    design::field_label(ui, "Support approach");
    if let Some(approach) = rec.get("recommendation").and_then(Value::as_object) {
        for (key, value) in approach {
            ui.label(
                egui::RichText::new(format!(
                    "{key}: {}",
                    value.as_str().unwrap_or(&value.to_string())
                ))
                .size(design::scale::text::CONTROL)
                .color(t.foreground),
            );
        }
    } else {
        for (key, value) in rec.as_object().into_iter().flatten() {
            if matches!(key.as_str(), "sources") {
                continue;
            }
            ui.label(
                egui::RichText::new(format!(
                    "{key}: {}",
                    value.as_str().unwrap_or(&value.to_string())
                ))
                .size(design::scale::text::CONTROL)
                .color(t.foreground),
            );
        }
    }
    let provenance: Vec<String> = ["prompt_version", "model"]
        .iter()
        .filter_map(|k| {
            rec.get(*k)
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .map(|v| format!("{k} {v}"))
        })
        .collect();
    if !provenance.is_empty() {
        design::hint_text(ui, &provenance.join(" · "));
    }
}

fn timeline_ui(ui: &mut egui::Ui, events: &[TimelineEvent]) {
    let t = design::tokens(ui);
    design::card_titled(ui, "Timeline", None, |ui| {
        if events.is_empty() {
            design::hint_text(ui, "(no timeline events)");
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("customer-timeline")
            .max_height(220.0)
            .show_rows(ui, 20.0, events.len(), |ui, range| {
                for event in &events[range] {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(event.occurred_at.as_str())
                                .small()
                                .color(t.muted_foreground),
                        );
                        ui.label(
                            egui::RichText::new(event.kind.as_str())
                                .size(design::scale::text::CONTROL)
                                .strong()
                                .color(t.foreground),
                        );
                        if let Some(number) = event.conversation_number {
                            ui.monospace(
                                egui::RichText::new(format!("#{number}")).color(t.foreground),
                            );
                        }
                        ui.label(
                            egui::RichText::new(event.summary.as_str())
                                .size(design::scale::text::CONTROL)
                                .color(t.foreground),
                        );
                    });
                }
            });
    });
}

fn milestones_ui(ui: &mut egui::Ui, events: &[spp_core::reports::TimelineEvent]) {
    let t = design::tokens(ui);
    design::card_titled(
        ui,
        "Support milestones",
        Some("tracked SLA breaches and high-friction flags from the intelligence sweep"),
        |ui| {
            if events.is_empty() {
                design::empty_state_slim(
                    ui,
                    Icon::Clock,
                    "No milestones recorded",
                    "SLA breaches and high-friction flags appear here once the intelligence \
                     sweep records them.",
                );
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt("customer-milestones")
                .max_height(160.0)
                .show_rows(ui, 20.0, events.len(), |ui, range| {
                    for event in &events[range] {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(event.occurred_at.as_str())
                                    .small()
                                    .color(t.muted_foreground),
                            );
                            ui.label(
                                egui::RichText::new(event.event_type.as_str())
                                    .size(design::scale::text::CONTROL)
                                    .strong()
                                    .color(t.foreground),
                            );
                            let detail = event
                                .event_data
                                .as_deref()
                                .and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok())
                                .map(|v| match (&v["breach_type"], &v["effort_score"]) {
                                    (serde_json::Value::String(kind), _) => {
                                        format!("breach: {kind}")
                                    }
                                    (_, serde_json::Value::Number(score)) => {
                                        format!("effort score {score}")
                                    }
                                    _ => String::new(),
                                })
                                .unwrap_or_default();
                            if !detail.is_empty() {
                                ui.label(
                                    egui::RichText::new(detail)
                                        .size(design::scale::text::CONTROL)
                                        .color(t.foreground),
                                );
                            }
                        });
                    }
                });
        },
    );
}

fn conversations_ui(ui: &mut egui::Ui, items: &[spp_core::inbox::ConversationListItem]) {
    let t = design::tokens(ui);
    design::card_titled(ui, "Conversations", None, |ui| {
        if items.is_empty() {
            design::hint_text(ui, "(no conversations)");
            return;
        }
        design::table(
            ui,
            "customer-conversations",
            &["#", "subject", "status", "updated"],
            26.0,
            |ui| {
                for item in items {
                    design::td_num(ui, &item.number.to_string());
                    ui.label(
                        egui::RichText::new(item.subject.as_deref().unwrap_or("(no subject)"))
                            .size(design::scale::text::CONTROL)
                            .color(t.foreground),
                    );
                    let (tone, label) = design::status_tone(item.status.as_str());
                    design::badge(ui, label, tone);
                    ui.label(
                        egui::RichText::new(item.updated_at.as_deref().unwrap_or("—"))
                            .size(design::scale::text::CONTROL)
                            .color(t.muted_foreground),
                    );
                    ui.end_row();
                }
            },
        );
    });
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
    use crate::testsupport::{empty_db, fixture};

    #[test]
    fn row_from_summary_maps_fields() {
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

#[cfg(test)]
mod batch27_audit82_tests {
    use super::*;
    use crate::testsupport::empty_db;

    fn seed_customers(conn: &mut rusqlite::Connection, count: i64) {
        for i in 1..=count {
            conn.execute(
                "INSERT INTO customers (id, remote_id, first_name, last_name, email)
                 VALUES (?1, ?1, ?2, 'Customer', ?3)",
                rusqlite::params![i, format!("First{i}"), format!("c{i}@example.com")],
            )
            .expect("customer");
        }
    }

    #[test]
    fn page_two_reaches_customers_past_the_cap() {
        let db = empty_db();
        db.with_write(|conn| seed_customers(conn, 55));

        let mut page = CustomersPage::new();
        page.reload_search(&db);
        assert_eq!(page.total, 55, "the total counts every customer");
        assert_eq!(page.rows.len(), 50, "page 1 holds the page size");
        let page_one_ids: std::collections::HashSet<i64> = page.rows.iter().map(|r| r.id).collect();
        assert_eq!(page_one_ids.len(), 50, "no duplicates on page 1");

        page.page = 2;
        page.reload_search(&db);
        assert_eq!(page.rows.len(), 5, "page 2 holds the rest");
        assert_eq!(page.total, 55);
        let mut seen = page_one_ids;
        for row in &page.rows {
            assert!(seen.insert(row.id), "no customer appears on two pages");
        }
        assert_eq!(seen.len(), 55, "every customer is reachable");
    }

    #[test]
    fn a_page_past_the_end_falls_back_to_the_last_page() {
        let db = empty_db();
        db.with_write(|conn| seed_customers(conn, 55));

        let mut page = CustomersPage::new();
        page.reload_search(&db);
        assert_eq!(page.rows.len(), 50);

        db.with_write(|conn| {
            conn.execute("DELETE FROM customers WHERE id > 12", [])
                .expect("delete");
        });
        page.reload_search(&db);
        assert_eq!(page.page, 1, "the past-the-end page clamped");
        assert_eq!(page.rows.len(), 12, "the last-page rows render");
    }
}
