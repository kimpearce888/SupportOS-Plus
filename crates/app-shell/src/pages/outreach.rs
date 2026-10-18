//! Outreach page (port of the web `pages/outreach.rs`).
//!
//! Campaigns tab — `list_outreach_campaigns` (`/api/outreach/campaigns`)
//! with the queue/pause/resume/cancel/retry actions
//! (`outreach::campaign_*`, the `/api/outreach/campaigns/{id}/{action}`
//! routes), the new-campaign form (the create route's validation +
//! `create_outreach_campaign` with a segment snapshot), and the campaign
//! report (`campaign_report`). Segments tab — `list_segments_v2` /
//! `create_segment` / `delete_segment`. DNC tab — `list_dnc` /
//! `add_to_dnc` / `remove_from_dnc`.

use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::segment::{SegmentDefinition, SegmentEngine};

use super::{i64_field, str_field};

/// Page tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Campaigns,
    Segments,
    Dnc,
}

impl Tab {
    const ALL: [Self; 3] = [Self::Campaigns, Self::Segments, Self::Dnc];

    fn label(self) -> &'static str {
        match self {
            Self::Campaigns => "Campaigns",
            Self::Segments => "Segments",
            Self::Dnc => "Do not contact",
        }
    }
}

/// The interactive outreach page state.
pub struct OutreachPage {
    tab: Tab,
    campaigns: Vec<serde_json::Value>,
    segments: Vec<serde_json::Value>,
    dnc: Vec<(i64, String, String)>,
    selected: Option<i64>,
    report: Option<serde_json::Value>,
    show_new: bool,
    new_name: String,
    new_subject: String,
    new_body: String,
    new_mailbox: String,
    new_segment_json: String,
    show_new_segment: bool,
    new_segment_name: String,
    new_segment_criteria: String,
    new_dnc_customer: String,
    new_dnc_reason: String,
    message: Option<String>,
    error: Option<String>,
}

impl OutreachPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::Campaigns,
            campaigns: Vec::new(),
            segments: Vec::new(),
            dnc: Vec::new(),
            selected: None,
            report: None,
            show_new: false,
            new_name: String::new(),
            new_subject: String::new(),
            new_body: String::new(),
            new_mailbox: String::new(),
            new_segment_json: String::new(),
            show_new_segment: false,
            new_segment_name: String::new(),
            new_segment_criteria: String::new(),
            new_dnc_customer: String::new(),
            new_dnc_reason: String::new(),
            message: None,
            error: None,
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.campaigns.clear();
        self.segments.clear();
        self.dnc.clear();
        self.report = None;
    }

    fn reload(&mut self, db: &DbHandle) {
        let (campaigns, segments, dnc) = db.with_read(|conn| {
            (
                spp_core::outreach::list_outreach_campaigns(conn).unwrap_or_default(),
                spp_core::outreach::list_segments_v2(conn).unwrap_or_default(),
                spp_core::outreach::list_dnc(conn).unwrap_or_default(),
            )
        });
        self.campaigns = campaigns;
        self.segments = segments;
        self.dnc = dnc;
        if let Some(id) = self.selected {
            self.reload_report(db, id);
        }
    }

    fn reload_report(&mut self, db: &DbHandle, id: i64) {
        self.report = Some(db.with_read(|conn| spp_core::outreach::campaign_report(conn, id)));
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.campaigns.is_empty() {
            self.reload(db);
        }
    }

    fn create_campaign(&mut self, db: &DbHandle) {
        let name = self.new_name.trim().to_string();
        if name.is_empty() || name.len() > 200 {
            self.error = Some("name must be 1-200 characters.".into());
            return;
        }
        let subject = self.new_subject.trim().to_string();
        if subject.is_empty() || subject.len() > 500 {
            self.error = Some("subject must be 1-500 characters.".into());
            return;
        }
        let body = self.new_body.clone();
        if body.is_empty() || body.len() > 200_000 {
            self.error = Some("body must be 1-200000 characters.".into());
            return;
        }
        let Ok(mailbox_local_id) = self.new_mailbox.trim().parse::<i64>() else {
            self.error = Some("A valid sending mailbox is required.".into());
            return;
        };
        let mailbox_ok: bool = db.with_read(|conn| {
            conn.query_row(
                "SELECT 1 FROM mailboxes WHERE id = ?1 AND deleted_at IS NULL",
                rusqlite::params![mailbox_local_id],
                |_| Ok(true),
            )
            .unwrap_or(false)
        });
        if !mailbox_ok {
            self.error = Some("A valid sending mailbox is required.".into());
            return;
        }
        // A saved segment id or a raw definition must be present (the
        // route's guard against accidental whole-address-book sends).
        let segment_id: Option<i64> = self
            .new_segment_json
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|v| *v > 0);
        let tree: Option<SegmentDefinition> = db.with_read(|conn| {
            if let Some(sid) = segment_id {
                spp_core::outreach::get_segment_v2(conn, sid)
                    .ok()
                    .flatten()
                    .and_then(|saved| {
                        saved
                            .get("definition")
                            .cloned()
                            .and_then(|def| spp_core::segment::parse_segment_tree(&def).ok())
                    })
            } else {
                None
            }
        });
        let Some(tree) = tree else {
            self.error = Some(
                "A saved segment id is required (campaigns never target the whole address \
                 book by accident)."
                    .into(),
            );
            return;
        };
        const SNAPSHOT_LIMIT: i64 = 5000;
        let engine_result = db.with_read(|conn| {
            let engine = SegmentEngine::new(conn);
            let preview = engine.preview(&tree, 1, SNAPSHOT_LIMIT);
            let rows = preview
                .get("rows")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            (
                preview.get("matched").and_then(|v| v.as_i64()).unwrap_or(0),
                rows,
            )
        });
        let (_matched, rows) = engine_result;
        if rows.is_empty() {
            self.error = Some("No recipients selected.".into());
            return;
        }
        let recipients: Vec<serde_json::Value> = rows;
        let snapshot = serde_json::to_string(&tree.to_json()).unwrap_or_default();
        let result = db.with_write(|conn| {
            spp_core::outreach::create_outreach_campaign(
                conn,
                &name,
                &subject,
                &body,
                mailbox_local_id,
                &[],
                segment_id,
                Some(&snapshot),
                &recipients,
            )
        });
        match result {
            Ok(id) => {
                self.show_new = false;
                self.new_name.clear();
                self.new_subject.clear();
                self.new_body.clear();
                self.new_segment_json.clear();
                self.selected = Some(id);
                self.message = Some(format!(
                    "campaign created with {} recipient(s)",
                    recipients.len()
                ));
                self.reload(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn campaign_action(&mut self, db: &DbHandle, id: i64, action: CampaignAction) {
        let result = db.with_write(|conn| action.run(conn, id));
        self.message = Some(
            result
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("(no message)")
                .to_string(),
        );
        self.reload(db);
    }

    fn create_segment(&mut self, db: &DbHandle) {
        let name = self.new_segment_name.trim().to_string();
        if name.is_empty() {
            self.error = Some("segment name required".into());
            return;
        }
        let criteria = self.new_segment_criteria.trim().to_string();
        if spp_core::segment::parse_segment_tree(
            &serde_json::from_str::<serde_json::Value>(&criteria)
                .unwrap_or(serde_json::Value::Null),
        )
        .is_err()
        {
            self.error =
                Some("criteria must be { combinator, conditions[], exclude[] } JSON".into());
            return;
        }
        let result = db.with_write(|conn| {
            spp_core::outreach::save_segment_v2(conn, None, &name, None, &criteria)
        });
        match result {
            Ok(_) => {
                self.show_new_segment = false;
                self.new_segment_name.clear();
                self.new_segment_criteria.clear();
                self.reload(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn delete_segment(&mut self, db: &DbHandle, id: i64) {
        let result = db.with_write(|conn| spp_core::outreach::delete_segment(conn, id));
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
        self.reload(db);
    }

    fn add_dnc(&mut self, db: &DbHandle) {
        let Some(customer_id) = self
            .new_dnc_customer
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|v| *v > 0)
        else {
            self.error = Some("customer id must be a positive integer".into());
            return;
        };
        let reason = if self.new_dnc_reason.trim().is_empty() {
            "manual"
        } else {
            self.new_dnc_reason.trim()
        };
        let result =
            db.with_write(|conn| spp_core::outreach::add_to_dnc(conn, customer_id, reason));
        match result {
            Ok(()) => {
                self.new_dnc_customer.clear();
                self.new_dnc_reason.clear();
                self.reload(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn remove_dnc(&mut self, db: &DbHandle, customer_id: i64) {
        let result = db.with_write(|conn| spp_core::outreach::remove_from_dnc(conn, customer_id));
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
        self.reload(db);
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        ui.heading("Outreach");
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
        });
        if let Some(message) = &self.message {
            ui.label(format!("status: {message}"));
        }
        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("error: {err}"));
        }
        ui.separator();

        match self.tab {
            Tab::Campaigns => self.campaigns_ui(ui, db),
            Tab::Segments => self.segments_ui(ui, db),
            Tab::Dnc => self.dnc_ui(ui, db),
        }
    }

    fn campaigns_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.horizontal(|ui| {
            if ui.button("New Campaign").clicked() {
                self.show_new = !self.show_new;
            }
        });
        if self.show_new {
            self.new_campaign_ui(ui, db);
            ui.separator();
        }

        let campaigns = self.campaigns.clone();
        let mut acted: Option<(i64, CampaignAction)> = None;
        let mut picked: Option<i64> = None;
        egui::ScrollArea::vertical()
            .id_salt("outreach-campaigns")
            .show(ui, |ui| {
                for campaign in &campaigns {
                    let id = i64_field(campaign, "id");
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    self.selected == Some(id),
                                    format!(
                                        "{} — {}",
                                        str_field(campaign, "name"),
                                        str_field(campaign, "subject")
                                    ),
                                )
                                .clicked()
                            {
                                picked = Some(id);
                            }
                            ui.colored_label(
                                match str_field(campaign, "status").as_str() {
                                    "draft" => egui::Color32::GRAY,
                                    "queued" | "running" => egui::Color32::LIGHT_BLUE,
                                    "paused" => egui::Color32::YELLOW,
                                    "completed" => egui::Color32::GREEN,
                                    "cancelled" => egui::Color32::RED,
                                    _ => egui::Color32::GRAY,
                                },
                                str_field(campaign, "status"),
                            );
                        });
                        ui.weak(format!(
                            "{} recipients · {} sent · {} failed · {} replied · mailbox {}",
                            i64_field(campaign, "recipients"),
                            i64_field(campaign, "sent"),
                            i64_field(campaign, "failed"),
                            i64_field(campaign, "replied"),
                            str_field(campaign, "mailbox_name")
                        ));
                        ui.horizontal(|ui| {
                            for action in [
                                CampaignAction::Queue,
                                CampaignAction::Pause,
                                CampaignAction::Resume,
                                CampaignAction::CancelRemaining,
                                CampaignAction::RetryFailed,
                            ] {
                                if ui.button(action.label()).clicked() {
                                    acted = Some((id, action));
                                }
                            }
                        });
                    });
                }
                if campaigns.is_empty() {
                    ui.weak("(no campaigns)");
                }
            });
        if let Some(id) = picked {
            self.selected = Some(id);
            self.reload_report(db, id);
        }
        if let Some((id, action)) = acted {
            self.campaign_action(db, id, action);
        }

        if self.selected.is_some() {
            ui.separator();
            self.report_ui(ui);
        }
    }

    fn report_ui(&mut self, ui: &mut egui::Ui) {
        let Some(report) = self.report.clone() else {
            return;
        };
        ui.strong("Campaign report");
        egui::Grid::new("outreach-report")
            .num_columns(2)
            .show(ui, |ui| {
                for key in [
                    "total",
                    "sent",
                    "failed",
                    "skipped",
                    "replied",
                    "unknown",
                    "remaining",
                ] {
                    ui.strong(key);
                    ui.monospace(i64_field(&report, key).to_string());
                    ui.end_row();
                }
            });
    }

    fn new_campaign_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.strong("New Campaign");
        egui::Grid::new("outreach-new")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("name:");
                ui.text_edit_singleline(&mut self.new_name);
                ui.end_row();
                ui.label("subject:");
                ui.text_edit_singleline(&mut self.new_subject);
                ui.end_row();
                ui.label("body:");
                ui.text_edit_multiline(&mut self.new_body);
                ui.end_row();
                ui.label("mailbox id:");
                ui.text_edit_singleline(&mut self.new_mailbox)
                    .on_hover_text("local mailbox id (see Settings)");
                ui.end_row();
                ui.label("segment id:");
                ui.text_edit_singleline(&mut self.new_segment_json)
                    .on_hover_text("saved segment id — required guard");
                ui.end_row();
            });
        let segments = self.segments.clone();
        if !segments.is_empty() {
            ui.weak(format!(
                "saved segments: {}",
                segments
                    .iter()
                    .map(|s| format!("#{} {}", i64_field(s, "id"), str_field(s, "name")))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if ui.button("create campaign").clicked() {
            self.create_campaign(db);
        }
    }

    fn segments_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.horizontal(|ui| {
            if ui.button("New Segment").clicked() {
                self.show_new_segment = !self.show_new_segment;
            }
        });
        if self.show_new_segment {
            egui::Grid::new("outreach-new-segment")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("name:");
                    ui.text_edit_singleline(&mut self.new_segment_name);
                    ui.end_row();
                    ui.label("criteria (JSON):");
                    ui.text_edit_multiline(&mut self.new_segment_criteria)
                        .on_hover_text("segment definition JSON");
                    ui.end_row();
                });
            if ui.button("create segment").clicked() {
                self.create_segment(db);
            }
            ui.separator();
        }

        let segments = self.segments.clone();
        let mut deleted: Option<i64> = None;
        egui::ScrollArea::vertical()
            .id_salt("outreach-segments")
            .show(ui, |ui| {
                for segment in &segments {
                    ui.horizontal(|ui| {
                        ui.strong(format!(
                            "#{} {}",
                            i64_field(segment, "id"),
                            str_field(segment, "name")
                        ));
                        if ui.button("delete").clicked() {
                            deleted = Some(i64_field(segment, "id"));
                        }
                    });
                }
                if segments.is_empty() {
                    ui.weak("(no saved segments)");
                }
            });
        if let Some(id) = deleted {
            self.delete_segment(db, id);
        }
    }

    fn dnc_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.strong("Do not contact");
        ui.weak("Customers on this list are excluded from every campaign.");
        egui::Grid::new("outreach-dnc-new")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("customer id:");
                ui.text_edit_singleline(&mut self.new_dnc_customer);
                ui.end_row();
                ui.label("reason:");
                ui.text_edit_singleline(&mut self.new_dnc_reason);
                ui.end_row();
            });
        if ui.button("add").clicked() {
            self.add_dnc(db);
        }
        ui.separator();

        let dnc = self.dnc.clone();
        let mut removed: Option<i64> = None;
        egui::ScrollArea::vertical()
            .id_salt("outreach-dnc")
            .show(ui, |ui| {
                for (customer_id, reason, created_at) in &dnc {
                    ui.horizontal(|ui| {
                        ui.monospace(format!("customer #{customer_id}"));
                        ui.weak(reason);
                        ui.weak(created_at);
                        if ui.button("remove").clicked() {
                            removed = Some(*customer_id);
                        }
                    });
                }
                if dnc.is_empty() {
                    ui.weak("(no customers on the list)");
                }
            });
        if let Some(customer_id) = removed {
            self.remove_dnc(db, customer_id);
        }
    }
}

impl Default for OutreachPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Campaign lifecycle actions (web: the `{action}` path segment).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CampaignAction {
    Queue,
    Pause,
    Resume,
    CancelRemaining,
    RetryFailed,
}

impl CampaignAction {
    fn label(self) -> &'static str {
        match self {
            Self::Queue => "queue",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::CancelRemaining => "cancel remaining",
            Self::RetryFailed => "retry failed",
        }
    }

    fn run(self, conn: &rusqlite::Connection, id: i64) -> serde_json::Value {
        match self {
            Self::Queue => spp_core::outreach::campaign_queue(conn, id),
            Self::Pause => spp_core::outreach::campaign_pause(conn, id),
            Self::Resume => spp_core::outreach::campaign_resume(conn, id),
            Self::CancelRemaining => spp_core::outreach::campaign_cancel_remaining(conn, id),
            Self::RetryFailed => spp_core::outreach::campaign_retry_failed(conn, id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    /// Build a saved segment matching customers with tickets (v2 shape).
    fn seed_segment(conn: &rusqlite::Connection, name: &str, definition: &str) -> i64 {
        spp_core::outreach::save_segment_v2(conn, None, name, None, definition).expect("segment")
    }

    const TICKET_CUSTOMERS_DEFINITION: &str = r#"{"combinator":"all","conditions":[{"kind":"history","metric":"ticket_count","op":"gte","value":1}],"exclude":[]}"#;

    #[test]
    fn campaign_create_queues_and_reports() {
        let db = fixture(vec![("conv", 1, "Hello", "active", "email", 1, 5, 9)]);
        let segment_id =
            db.with_write(|conn| seed_segment(conn, "Ticket owners", TICKET_CUSTOMERS_DEFINITION));

        let mut page = OutreachPage::new();
        page.reload(&db);
        assert!(page.campaigns.is_empty());

        page.new_name = "Check-in".into();
        page.new_subject = "How is it going?".into();
        page.new_body = "Hi {{first_name}}, just checking in.".into();
        page.new_mailbox = "1".into();
        page.new_segment_json = segment_id.to_string();
        page.create_campaign(&db);

        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.campaigns.len(), 1);
        let campaign = page.campaigns[0].clone();
        let id = i64_field(&campaign, "id");
        assert_eq!(str_field(&campaign, "status"), "draft");
        assert!(
            i64_field(&campaign, "recipients") >= 1,
            "segment snapshot ran"
        );
        assert!(page.report.is_some());

        page.campaign_action(&db, id, CampaignAction::Queue);
        assert!(page.message.is_some());
        let queued = page.campaigns[0].clone();
        assert_eq!(str_field(&queued, "status"), "queued");
    }

    #[test]
    fn campaign_create_requires_a_segment() {
        let db = fixture(Vec::new());
        let mut page = OutreachPage::new();
        page.new_name = "No segment".into();
        page.new_subject = "Hi".into();
        page.new_body = "Body".into();
        page.new_mailbox = "1".into();
        page.new_segment_json = String::new();
        page.create_campaign(&db);
        let error = page.error.clone().expect("segment guard");
        assert!(error.contains("segment"));
    }

    #[test]
    fn campaign_create_requires_valid_fields() {
        let db = fixture(Vec::new());
        let mut page = OutreachPage::new();
        // Empty name.
        page.create_campaign(&db);
        assert!(page.error.is_some());
        // Bad mailbox.
        page.new_name = "X".into();
        page.new_subject = "Y".into();
        page.new_body = "Z".into();
        page.new_mailbox = "not-a-number".into();
        page.create_campaign(&db);
        assert!(page.error.is_some());
    }

    #[test]
    fn segments_and_dnc_round_trip() {
        let db = fixture(Vec::new());
        let mut page = OutreachPage::new();
        page.show_new_segment = true;
        page.new_segment_name = "Churn risk".into();
        page.new_segment_criteria = TICKET_CUSTOMERS_DEFINITION.into();
        page.create_segment(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.segments.len(), 1);

        let segment_id = i64_field(&page.segments[0], "id");
        page.delete_segment(&db, segment_id);
        assert!(page.segments.is_empty());

        page.new_dnc_customer = "9".into();
        page.new_dnc_reason = "asked politely".into();
        page.add_dnc(&db);
        assert!(page.error.is_none());
        assert_eq!(page.dnc.len(), 1);
        assert_eq!(page.dnc[0].0, 9);
        assert_eq!(page.dnc[0].1, "asked politely");

        let on_dnc = db.with_read(|conn| spp_core::outreach::is_on_dnc(conn, 9));
        assert!(on_dnc.expect("dnc check"));

        page.remove_dnc(&db, 9);
        assert!(page.dnc.is_empty());
        let after = db.with_read(|conn| spp_core::outreach::is_on_dnc(conn, 9));
        assert!(!after.expect("dnc check 2"));
    }

    #[test]
    fn dnc_requires_numeric_customer_id() {
        let db = fixture(Vec::new());
        let mut page = OutreachPage::new();
        page.new_dnc_customer = "abc".into();
        page.add_dnc(&db);
        assert!(page.error.is_some());
    }
}
