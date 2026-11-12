use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::segment::{SegmentDefinition, SegmentEngine};

use super::{ConfirmRequest, NavRequest, i64_field, str_field};
use crate::design::{self, BadgeTone, ButtonVariant, CalloutRole};
use crate::icons::Icon;

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

pub struct OutreachPage {
    tab: Tab,
    campaigns: Vec<serde_json::Value>,
    segments: Vec<serde_json::Value>,
    dnc: Vec<(i64, String, String)>,
    mailboxes: Vec<(i64, String)>,
    selected: Option<i64>,
    report: Option<serde_json::Value>,
    show_new: bool,
    edit_campaign_id: Option<i64>,
    new_name: String,
    new_subject: String,
    new_body: String,
    new_mailbox_pick: Option<i64>,
    new_mailbox: String,
    new_segment_pick: Option<i64>,
    new_segment_json: String,
    show_new_segment: bool,
    edit_segment_id: Option<i64>,
    new_segment_name: String,
    new_segment_description: String,
    new_segment_criteria: String,
    segment_preview: Option<SegmentPreview>,
    row_preview: Option<(i64, SegmentPreview)>,
    dnc_query: String,
    dnc_results_for: Option<String>,
    dnc_results: Vec<serde_json::Value>,
    new_dnc_pick: Option<i64>,
    new_dnc_reason: String,
    message: Option<String>,
    error: Option<String>,
    message_at: Option<std::time::Instant>,
    error_at: Option<std::time::Instant>,
    pending_confirm: Option<ConfirmRequest>,
    loaded: bool,
}

impl OutreachPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::Campaigns,
            loaded: false,
            campaigns: Vec::new(),
            segments: Vec::new(),
            dnc: Vec::new(),
            mailboxes: Vec::new(),
            selected: None,
            report: None,
            show_new: false,
            edit_campaign_id: None,
            new_name: String::new(),
            new_subject: String::new(),
            new_body: String::new(),
            new_mailbox_pick: None,
            new_mailbox: String::new(),
            new_segment_pick: None,
            new_segment_json: String::new(),
            show_new_segment: false,
            edit_segment_id: None,
            new_segment_name: String::new(),
            new_segment_description: String::new(),
            new_segment_criteria: String::new(),
            segment_preview: None,
            row_preview: None,
            dnc_query: String::new(),
            dnc_results_for: None,
            dnc_results: Vec::new(),
            new_dnc_pick: None,
            new_dnc_reason: String::new(),
            message: None,
            error: None,
            message_at: None,
            error_at: None,
            pending_confirm: None,
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.loaded = false;
        self.campaigns.clear();
        self.segments.clear();
        self.dnc.clear();
        self.mailboxes.clear();
        self.report = None;
    }

    fn reload(&mut self, db: &DbHandle) {
        let (campaigns, segments, dnc, mailboxes) = db.with_read(|conn| {
            (
                spp_core::outreach::list_outreach_campaigns(conn).unwrap_or_default(),
                spp_core::outreach::list_segments_v2(conn).unwrap_or_default(),
                spp_core::outreach::list_dnc(conn).unwrap_or_default(),
                crate::pages::dashboard::mailboxes(conn),
            )
        });
        self.campaigns = campaigns;
        self.segments = segments;
        self.dnc = dnc;
        self.mailboxes = mailboxes;
        self.loaded = true;
        if let Some(id) = self.selected {
            self.reload_report(db, id);
        }
    }

    fn reload_report(&mut self, db: &DbHandle, id: i64) {
        self.report = Some(db.with_read(|conn| spp_core::outreach::campaign_report(conn, id)));
    }

    fn set_message(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
        self.message_at = Some(std::time::Instant::now());
        self.error = None;
        self.error_at = None;
    }

    fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
        self.error_at = Some(std::time::Instant::now());
        self.message = None;
        self.message_at = None;
    }

    fn expire_status_lines(&mut self) {
        const STATUS_TTL: std::time::Duration = std::time::Duration::from_secs(45);
        if let Some(set_at) = self.message_at
            && set_at.elapsed() > STATUS_TTL
        {
            self.message = None;
            self.message_at = None;
        }
        if let Some(set_at) = self.error_at
            && set_at.elapsed() > STATUS_TTL
        {
            self.error = None;
            self.error_at = None;
        }
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if !self.loaded {
            self.reload(db);
        }
    }

    fn create_campaign(&mut self, db: &DbHandle) {
        let Some((name, subject, body, mailbox_local_id)) = self.validate_campaign_fields(db)
        else {
            return;
        };
        let segment_id: Option<i64> = self.new_segment_pick.filter(|v| *v > 0).or_else(|| {
            self.new_segment_json
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|v| *v > 0)
        });
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
            self.set_error(
                "A saved segment id is required (campaigns never target the whole address \
                 book by accident).",
            );
            return;
        };
        let engine_result = db.with_read(|conn| {
            let engine = SegmentEngine::new(conn);
            let preview = engine.preview(&tree, 1, i64::MAX);
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
        let (matched, rows) = engine_result;
        if rows.is_empty() {
            self.set_error("No recipients selected.");
            return;
        }
        debug_assert_eq!(
            matched as usize,
            rows.len(),
            "the unbounded snapshot must carry every matched recipient"
        );
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
                self.edit_campaign_id = None;
                self.reset_campaign_form();
                self.selected = Some(id);
                self.set_message(format!(
                    "campaign created with {} recipient(s)",
                    recipients.len()
                ));
                self.reload(db);
            }
            Err(e) => self.set_error(e.to_string()),
        }
    }

    fn validate_campaign_fields(&mut self, db: &DbHandle) -> Option<(String, String, String, i64)> {
        let name = self.new_name.trim().to_string();
        if name.is_empty() || name.len() > 200 {
            self.set_error("name must be 1-200 characters.");
            return None;
        }
        let subject = self.new_subject.trim().to_string();
        if subject.is_empty() || subject.len() > 500 {
            self.set_error("subject must be 1-500 characters.");
            return None;
        }
        let body = self.new_body.clone();
        if body.is_empty() || body.len() > 200_000 {
            self.set_error("body must be 1-200000 characters.");
            return None;
        }
        let mailbox_local_id = self
            .new_mailbox_pick
            .or_else(|| self.new_mailbox.trim().parse::<i64>().ok());
        let Some(mailbox_local_id) = mailbox_local_id else {
            self.set_error("A valid sending mailbox is required.");
            return None;
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
            self.set_error("A valid sending mailbox is required.");
            return None;
        }
        Some((name, subject, body, mailbox_local_id))
    }

    fn start_campaign_edit(&mut self, db: &DbHandle, campaign: &serde_json::Value) {
        let id = i64_field(campaign, "id");
        let body = db.with_read(|conn| {
            conn.query_row(
                "SELECT body FROM outreach_campaigns WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_default()
        });
        self.edit_campaign_id = Some(id);
        self.show_new = true;
        self.error = None;
        self.message = None;
        self.new_name = str_field(campaign, "name");
        self.new_subject = str_field(campaign, "subject");
        self.new_body = body;
        self.new_mailbox_pick = campaign.get("mailbox_local_id").and_then(|v| v.as_i64());
        self.new_mailbox.clear();
        self.new_segment_pick = campaign.get("segment_id").and_then(|v| v.as_i64());
        self.new_segment_json.clear();
    }

    fn save_campaign_edit(&mut self, db: &DbHandle) {
        let Some(id) = self.edit_campaign_id else {
            return;
        };
        let Some((name, subject, body, mailbox_local_id)) = self.validate_campaign_fields(db)
        else {
            return;
        };
        let result = db.with_write(|conn| {
            spp_core::outreach::update_outreach_campaign(
                conn,
                id,
                &name,
                &subject,
                &body,
                mailbox_local_id,
                &[],
            )
        });
        match result {
            Ok(()) => {
                self.edit_campaign_id = None;
                self.show_new = false;
                self.reset_campaign_form();
                self.selected = Some(id);
                self.set_message("campaign updated");
                self.reload(db);
            }
            Err(e) => self.set_error(e.to_string()),
        }
    }

    fn cancel_campaign_edit(&mut self) {
        self.edit_campaign_id = None;
        self.show_new = false;
        self.reset_campaign_form();
        self.error = None;
    }

    fn reset_campaign_form(&mut self) {
        self.new_name.clear();
        self.new_subject.clear();
        self.new_body.clear();
        self.new_mailbox_pick = None;
        self.new_mailbox.clear();
        self.new_segment_pick = None;
        self.new_segment_json.clear();
    }

    fn campaign_action(&mut self, db: &DbHandle, id: i64, action: CampaignAction) {
        let result = db.with_write(|conn| action.run(conn, id));
        self.set_message(
            result
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("(no message)"),
        );
        self.reload(db);
    }

    fn create_segment(&mut self, db: &DbHandle) {
        let name = self.new_segment_name.trim().to_string();
        if name.is_empty() {
            self.set_error("segment name required");
            return;
        }
        let criteria = self.new_segment_criteria.trim().to_string();
        if spp_core::segment::parse_segment_tree(
            &serde_json::from_str::<serde_json::Value>(&criteria)
                .unwrap_or(serde_json::Value::Null),
        )
        .is_err()
        {
            self.set_error("criteria must be { combinator, conditions[], exclude[] } JSON");
            return;
        }
        let description = self.new_segment_description.trim().to_string();
        let description = (!description.is_empty()).then_some(description);
        let editing = self.edit_segment_id;
        let result = db.with_write(|conn| {
            spp_core::outreach::save_segment_v2(
                conn,
                editing,
                &name,
                description.as_deref(),
                &criteria,
            )
        });
        match result {
            Ok(_) => {
                self.show_new_segment = false;
                self.edit_segment_id = None;
                self.reset_segment_form();
                if editing.is_some() {
                    self.set_message("segment updated");
                }
                self.reload(db);
            }
            Err(e) => self.set_error(e.to_string()),
        }
    }

    fn start_segment_edit(&mut self, segment: &serde_json::Value) {
        self.edit_segment_id = Some(i64_field(segment, "id"));
        self.show_new_segment = true;
        self.error = None;
        self.message = None;
        self.reset_segment_form();
        self.new_segment_name = str_field(segment, "name");
        self.new_segment_description = segment
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        self.new_segment_criteria = segment
            .get("definition")
            .map(|v| v.to_string())
            .unwrap_or_default();
    }

    fn cancel_segment_edit(&mut self) {
        self.edit_segment_id = None;
        self.show_new_segment = false;
        self.reset_segment_form();
        self.error = None;
    }

    fn reset_segment_form(&mut self) {
        self.new_segment_name.clear();
        self.new_segment_description.clear();
        self.new_segment_criteria.clear();
        self.segment_preview = None;
    }

    fn preview_new_segment(&mut self, db: &DbHandle) {
        match compute_segment_preview(db, &self.new_segment_criteria) {
            Ok(preview) => self.segment_preview = Some(preview),
            Err(e) => {
                self.segment_preview = None;
                self.set_error(e);
            }
        }
    }

    fn preview_segment_row(&mut self, db: &DbHandle, id: i64, definition: &str) {
        match compute_segment_preview(db, definition) {
            Ok(preview) => self.row_preview = Some((id, preview)),
            Err(e) => self.set_error(format!("segment #{id}: {e}")),
        }
    }

    pub(crate) fn delete_segment(&mut self, db: &DbHandle, id: i64) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::outreach::delete_segment(conn, id));
        if let Err(e) = &result {
            self.set_error(e.to_string());
        }
        self.reload(db);
        result.map(|_| ())
    }

    fn add_dnc(&mut self, db: &DbHandle) {
        let Some(customer_id) = self.new_dnc_pick.filter(|v| *v > 0) else {
            self.set_error("pick a customer first — search by name or email");
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
                self.new_dnc_pick = None;
                self.dnc_query.clear();
                self.dnc_results.clear();
                self.dnc_results_for = None;
                self.new_dnc_reason.clear();
                self.reload(db);
            }
            Err(e) => self.set_error(e.to_string()),
        }
    }

    fn refresh_dnc_results(&mut self, db: &DbHandle) {
        let query = self.dnc_query.trim().to_string();
        self.dnc_results = db
            .with_read(|conn| {
                spp_core::people_store::list_customers_summary(
                    conn,
                    (!query.is_empty()).then_some(query.as_str()),
                    1,
                    25,
                )
            })
            .map(|(rows, _)| rows)
            .unwrap_or_default();
        self.dnc_results_for = Some(self.dnc_query.clone());
    }

    pub(crate) fn remove_dnc(&mut self, db: &DbHandle, customer_id: i64) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::outreach::remove_from_dnc(conn, customer_id));
        if let Err(e) = &result {
            self.set_error(e.to_string());
        }
        self.reload(db);
        result.map(|_| ())
    }

    pub(crate) fn cancel_campaign_remaining_confirmed(
        &mut self,
        db: &DbHandle,
        id: i64,
    ) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::outreach::campaign_cancel_remaining(conn, id));
        self.set_message(
            result
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("(no message)"),
        );
        self.reload(db);
        Ok(())
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        self.ensure_loaded(db);

        design::page_header(ui, "Outreach", None, |_| {});
        if let Some(i) = design::tabs(
            ui,
            &[
                Tab::Campaigns.label(),
                Tab::Segments.label(),
                Tab::Dnc.label(),
            ],
            Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0),
        ) && let Some(tab) = Tab::ALL.get(i)
        {
            self.tab = *tab;
        }
        self.expire_status_lines();
        if let Some(message) = &self.message {
            design::success_text(ui, &format!("status: {message}"));
        }
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        design::divider(ui);

        let nav = match self.tab {
            Tab::Campaigns => {
                self.campaigns_ui(ui, db);
                None
            }
            Tab::Segments => self.segments_ui(ui, db),
            Tab::Dnc => {
                self.dnc_ui(ui, db);
                None
            }
        };
        if let Some(request) = self.pending_confirm.take() {
            return Some(NavRequest::Confirm(request));
        }
        nav
    }

    fn campaigns_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        ui.horizontal(|ui| {
            if design::button_with_icon(ui, Icon::Plus, "New Campaign", ButtonVariant::Primary)
                .clicked()
            {
                self.show_new = !self.show_new;
            }
        });
        if self.show_new {
            self.new_campaign_ui(ui, db);
            ui.add_space(10.0);
        }

        let campaigns = self.campaigns.clone();
        let mut acted: Option<(i64, CampaignAction)> = None;
        let mut picked: Option<i64> = None;
        let mut edit_requested: Option<serde_json::Value> = None;
        egui::ScrollArea::vertical()
            .id_salt("outreach-campaigns")
            .show(ui, |ui| {
                for campaign in &campaigns {
                    let id = i64_field(campaign, "id");
                    design::card(ui, |ui| {
                        ui.horizontal(|ui| {
                            if design::chip(
                                ui,
                                self.selected == Some(id),
                                &format!(
                                    "{} — {}",
                                    str_field(campaign, "name"),
                                    str_field(campaign, "subject")
                                ),
                            )
                            .clicked()
                            {
                                picked = Some(id);
                            }
                            let status = str_field(campaign, "status");
                            design::badge(ui, &status, campaign_status_tone(&status));
                        });
                        design::hint_text(
                            ui,
                            &format!(
                                "{} recipients · {} sent · {} failed · {} replied · mailbox {}",
                                i64_field(campaign, "recipients"),
                                i64_field(campaign, "sent"),
                                i64_field(campaign, "failed"),
                                i64_field(campaign, "replied"),
                                str_field(campaign, "mailbox_name")
                            ),
                        );
                        ui.horizontal(|ui| {
                            if str_field(campaign, "status") == "draft"
                                && design::button(ui, "Edit", ButtonVariant::Outline)
                                    .on_hover_text("open this draft in the form above")
                                    .clicked()
                            {
                                edit_requested = Some(campaign.clone());
                            }
                            for action in [
                                CampaignAction::Queue,
                                CampaignAction::Pause,
                                CampaignAction::Resume,
                            ] {
                                if design::button(
                                    ui,
                                    action.label(),
                                    match action {
                                        CampaignAction::Queue => ButtonVariant::Primary,
                                        _ => ButtonVariant::Outline,
                                    },
                                )
                                .clicked()
                                {
                                    acted = Some((id, action));
                                }
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if design::button(
                                        ui,
                                        CampaignAction::RetryFailed.label(),
                                        ButtonVariant::Outline,
                                    )
                                    .clicked()
                                    {
                                        acted = Some((id, CampaignAction::RetryFailed));
                                    }
                                    if design::button(
                                        ui,
                                        CampaignAction::CancelRemaining.label(),
                                        ButtonVariant::Destructive,
                                    )
                                    .clicked()
                                    {
                                        acted = Some((id, CampaignAction::CancelRemaining));
                                    }
                                },
                            );
                        });
                    });
                }
                if campaigns.is_empty()
                    && design::empty_state_cta(
                        ui,
                        Icon::Megaphone,
                        "No campaigns yet",
                        "Campaigns send a message to a saved segment. Create your \
                         first one to get started.",
                        "New Campaign",
                    )
                {
                    self.show_new = true;
                }
            });
        if let Some(campaign) = edit_requested {
            self.start_campaign_edit(db, &campaign);
        }
        if let Some(id) = picked {
            self.selected = Some(id);
            self.reload_report(db, id);
        }
        if let Some((id, action)) = acted {
            if action == CampaignAction::CancelRemaining {
                let name = campaigns
                    .iter()
                    .find(|c| i64_field(c, "id") == id)
                    .map(|c| str_field(c, "name").to_string())
                    .unwrap_or_else(|| format!("campaign {id}"));
                self.pending_confirm = Some(ConfirmRequest::cancel_campaign_remaining(id, &name));
            } else {
                self.campaign_action(db, id, action);
            }
        }

        if self.selected.is_some() {
            ui.add_space(10.0);
            self.report_ui(ui);
        }
    }

    fn report_ui(&mut self, ui: &mut egui::Ui) {
        let Some(report) = self.report.clone() else {
            return;
        };
        let t = design::tokens(ui);
        design::card_titled(ui, "Campaign report", None, |ui| {
            let totals = report.get("totals");
            let f64_or_dash = |v: Option<&serde_json::Value>| -> String {
                match v.and_then(|f| f.as_f64()) {
                    Some(n) => format!("{n}"),
                    None => "—".to_string(),
                }
            };
            design::form_grid(ui, "outreach-report", |ui| {
                for key in [
                    "recipients",
                    "sent",
                    "failed",
                    "skipped",
                    "cancelled",
                    "replied",
                    "unknown",
                ] {
                    design::field_label(ui, key);
                    let value = totals
                        .and_then(|t| t.get(key))
                        .and_then(|f| f.as_i64())
                        .unwrap_or(0);
                    ui.monospace(egui::RichText::new(value.to_string()).color(t.foreground));
                    ui.end_row();
                }
                design::field_label(ui, "reply_rate");
                ui.monospace(
                    egui::RichText::new(f64_or_dash(totals.and_then(|t| t.get("reply_rate"))))
                        .color(t.foreground),
                );
                ui.end_row();
            });
            ui.add_space(4.0);
            if let Some(note) = report.get("note").and_then(|n| n.as_str()) {
                design::callout(ui, CalloutRole::Neutral, note);
            }
        });
    }

    fn new_campaign_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let editing = self.edit_campaign_id.is_some();
        let t = design::tokens(ui);
        let mailboxes = self.mailboxes.clone();
        let segments = self.segments.clone();
        design::card(ui, |ui| {
            design::section_title(
                ui,
                if editing {
                    "Edit Campaign"
                } else {
                    "New Campaign"
                },
            );
            design::form_grid(ui, "outreach-new", |ui| {
                let name_label = design::field_label(ui, "Name");
                ui.add_sized(
                    [240.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_name).hint_text("campaign name"),
                )
                .labelled_by(name_label.id);
                ui.end_row();
                let subject_label = design::field_label(ui, "Subject");
                ui.add_sized(
                    [280.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_subject).hint_text("email subject"),
                )
                .labelled_by(subject_label.id);
                ui.end_row();
                let body_label = design::field_label(ui, "Body");
                ui.add_sized(
                    [320.0, 90.0],
                    egui::TextEdit::multiline(&mut self.new_body)
                        .hint_text("email body — {{first_name}} is substituted"),
                )
                .labelled_by(body_label.id);
                ui.end_row();
                let mailbox_label = design::field_label(ui, "Mailbox");
                if mailboxes.is_empty() {
                    ui.add_sized(
                        [140.0, 28.0],
                        egui::TextEdit::singleline(&mut self.new_mailbox),
                    )
                    .on_hover_text("local mailbox id (see Settings)")
                    .labelled_by(mailbox_label.id);
                } else {
                    let pick_label = self
                        .new_mailbox_pick
                        .and_then(|id| {
                            mailboxes
                                .iter()
                                .find(|(mid, _)| *mid == id)
                                .map(|(_, name)| name.clone())
                        })
                        .unwrap_or_else(|| "pick a mailbox…".to_string());
                    design::select(ui, "outreach-mailbox", &pick_label, 220.0, |ui| {
                        for (id, name) in mailboxes.clone() {
                            ui.selectable_value(&mut self.new_mailbox_pick, Some(id), &name);
                        }
                    })
                    .labelled_by(mailbox_label.id);
                }
                ui.end_row();
                let segment_label = design::field_label(ui, "Segment");
                if editing {
                    let fixed = self
                        .new_segment_pick
                        .map(|sid| {
                            segments
                                .iter()
                                .find(|s| i64_field(s, "id") == sid)
                                .map(|s| {
                                    format!("#{} {}", i64_field(s, "id"), str_field(s, "name"))
                                })
                                .unwrap_or_else(|| format!("segment #{sid}"))
                        })
                        .unwrap_or_else(|| "no segment".to_string());
                    ui.label(egui::RichText::new(fixed).color(t.foreground))
                        .on_hover_text(
                            "targeting is fixed when a campaign is created — create a new \
                             campaign to re-target",
                        )
                        .labelled_by(segment_label.id);
                } else if segments.is_empty() {
                    ui.add_sized(
                        [140.0, 28.0],
                        egui::TextEdit::singleline(&mut self.new_segment_json),
                    )
                    .on_hover_text("saved segment id — required guard")
                    .labelled_by(segment_label.id);
                } else {
                    let pick_label = self
                        .new_segment_pick
                        .and_then(|sid| {
                            segments
                                .iter()
                                .find(|s| i64_field(s, "id") == sid)
                                .map(|s| {
                                    format!("#{} {}", i64_field(s, "id"), str_field(s, "name"))
                                })
                        })
                        .unwrap_or_else(|| "pick a segment…".to_string());
                    design::select(ui, "outreach-segment", &pick_label, 240.0, |ui| {
                        for segment in segments.clone() {
                            let sid = i64_field(&segment, "id");
                            let text = format!("#{} {}", sid, str_field(&segment, "name"));
                            ui.selectable_value(&mut self.new_segment_pick, Some(sid), &text);
                        }
                    })
                    .labelled_by(segment_label.id);
                }
                ui.end_row();
            });
            if mailboxes.is_empty() {
                design::hint_text(
                    ui,
                    "no mailboxes to pick — enter a local mailbox id, or connect one first",
                );
            }
            if !editing && segments.is_empty() {
                design::hint_text(
                    ui,
                    "no saved segments to pick — enter a segment id, or create one in the \
                     Segments tab",
                );
            }
            ui.horizontal(|ui| {
                if design::button(
                    ui,
                    if editing {
                        "Save Changes"
                    } else {
                        "Create Campaign"
                    },
                    ButtonVariant::Primary,
                )
                .clicked()
                {
                    if editing {
                        self.save_campaign_edit(db);
                    } else {
                        self.create_campaign(db);
                    }
                }
                if editing && design::button(ui, "Cancel", ButtonVariant::Outline).clicked() {
                    self.cancel_campaign_edit();
                }
            });
        });
    }

    fn segments_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        ui.horizontal(|ui| {
            if design::button_with_icon(ui, Icon::Plus, "New Segment", ButtonVariant::Primary)
                .clicked()
            {
                self.show_new_segment = !self.show_new_segment;
            }
        });
        if self.show_new_segment {
            self.new_segment_ui(ui, db);
            ui.add_space(10.0);
        }

        let segments = self.segments.clone();
        let mut deleted: Option<(i64, String)> = None;
        let mut edit_requested: Option<serde_json::Value> = None;
        let mut preview_requested: Option<(i64, String)> = None;
        egui::ScrollArea::vertical()
            .id_salt("outreach-segments")
            .show(ui, |ui| {
                let t = design::tokens(ui);
                for segment in &segments {
                    let id = i64_field(segment, "id");
                    design::card(ui, |ui| {
                        let criteria = segment
                            .get("definition")
                            .map(|v| v.to_string())
                            .unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "#{} {}",
                                    i64_field(segment, "id"),
                                    str_field(segment, "name")
                                ))
                                .size(13.5)
                                .strong()
                                .color(t.foreground),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if design::button(ui, "Delete", ButtonVariant::Destructive)
                                        .clicked()
                                    {
                                        deleted = Some((
                                            i64_field(segment, "id"),
                                            str_field(segment, "name"),
                                        ));
                                    }
                                    if design::button(ui, "Preview", ButtonVariant::Outline)
                                        .on_hover_text("count the recipients this segment hits")
                                        .clicked()
                                    {
                                        preview_requested = Some((id, criteria.clone()));
                                    }
                                    if design::button(ui, "Edit", ButtonVariant::Outline)
                                        .on_hover_text("open this segment in the form above")
                                        .clicked()
                                    {
                                        edit_requested = Some(segment.clone());
                                    }
                                },
                            );
                        });
                        ui.monospace(
                            egui::RichText::new(criteria)
                                .small()
                                .color(t.muted_foreground),
                        );
                        if let Some((previewed_id, preview)) = &self.row_preview
                            && *previewed_id == id
                        {
                            design::hint_text(ui, &preview.hint());
                            for note in &preview.notes {
                                design::hint_text(ui, note);
                            }
                        }
                    });
                }
                if segments.is_empty() {
                    design::empty_state(
                        ui,
                        Icon::Tag,
                        "No saved segments",
                        "Create a segment to target specific customers.",
                    );
                }
            });
        if let Some((id, definition)) = preview_requested {
            self.preview_segment_row(db, id, &definition);
        }
        if let Some(segment) = edit_requested {
            self.start_segment_edit(&segment);
        }
        if let Some((id, name)) = deleted {
            return Some(NavRequest::Confirm(ConfirmRequest::delete_segment(
                id, &name,
            )));
        }
        None
    }

    fn new_segment_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let editing = self.edit_segment_id;
        design::card(ui, |ui| {
            design::section_title(
                ui,
                if editing.is_some() {
                    "Edit Segment"
                } else {
                    "New Segment"
                },
            );
            design::form_grid(ui, "outreach-new-segment", |ui| {
                let name_label = design::field_label(ui, "Name");
                ui.add_sized(
                    [220.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_segment_name)
                        .hint_text("segment name"),
                )
                .labelled_by(name_label.id);
                ui.end_row();
                let description_label = design::field_label(ui, "Description");
                ui.add_sized(
                    [240.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_segment_description)
                        .hint_text("what this segment is for (optional)"),
                )
                .labelled_by(description_label.id);
                ui.end_row();
                let criteria_label = design::field_label(ui, "Criteria (JSON)");
                ui.add_sized(
                    [320.0, 80.0],
                    egui::TextEdit::multiline(&mut self.new_segment_criteria),
                )
                .on_hover_text("segment definition JSON")
                .labelled_by(criteria_label.id);
                ui.end_row();
            });
            ui.horizontal(|ui| {
                if design::button(
                    ui,
                    if editing.is_some() {
                        "Save Changes"
                    } else {
                        "Create Segment"
                    },
                    ButtonVariant::Primary,
                )
                .clicked()
                {
                    self.create_segment(db);
                }
                if design::button(ui, "Preview", ButtonVariant::Outline)
                    .on_hover_text("count the recipients before saving")
                    .clicked()
                {
                    self.preview_new_segment(db);
                }
                if editing.is_some()
                    && design::button(ui, "Cancel", ButtonVariant::Outline).clicked()
                {
                    self.cancel_segment_edit();
                }
            });
            if let Some(preview) = &self.segment_preview {
                design::hint_text(ui, &preview.hint());
                for note in &preview.notes {
                    design::hint_text(ui, note);
                }
            }
        });
    }

    fn dnc_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.dnc_results_for.as_deref() != Some(self.dnc_query.as_str()) {
            self.refresh_dnc_results(db);
        }
        let t = design::tokens(ui);
        let dnc = self.dnc.clone();
        let results = self.dnc_results.clone();
        design::card_titled(
            ui,
            "Do not contact",
            Some("Customers on this list are excluded from every campaign."),
            |ui| {
                design::form_grid(ui, "outreach-dnc-new", |ui| {
                    let search_label = design::field_label(ui, "Search");
                    ui.add_sized(
                        [180.0, 28.0],
                        egui::TextEdit::singleline(&mut self.dnc_query)
                            .hint_text("customer name or email"),
                    )
                    .labelled_by(search_label.id);
                    ui.end_row();
                    let customer_label = design::field_label(ui, "Customer");
                    let pick_label = self
                        .new_dnc_pick
                        .map(|id| {
                            results
                                .iter()
                                .find(|row| row.get("id").and_then(|v| v.as_i64()) == Some(id))
                                .map(dnc_customer_label)
                                .unwrap_or_else(|| format!("customer #{id}"))
                        })
                        .unwrap_or_else(|| "pick a customer…".to_string());
                    design::select(ui, "outreach-dnc-customer", &pick_label, 280.0, |ui| {
                        if results.is_empty() {
                            ui.label("no customers match");
                        }
                        for row in &results {
                            let id = row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                            let text = dnc_customer_label(row);
                            ui.selectable_value(&mut self.new_dnc_pick, Some(id), &text);
                        }
                    })
                    .labelled_by(customer_label.id);
                    ui.end_row();
                    let reason_label = design::field_label(ui, "Reason");
                    ui.add_sized(
                        [220.0, 28.0],
                        egui::TextEdit::singleline(&mut self.new_dnc_reason)
                            .hint_text("why (optional)"),
                    )
                    .labelled_by(reason_label.id);
                    ui.end_row();
                });
                if results.is_empty() {
                    design::hint_text(
                        ui,
                        &format!("no customers match '{}'", self.dnc_query.trim()),
                    );
                }
                if design::button(ui, "Add", ButtonVariant::Primary).clicked() {
                    self.add_dnc(db);
                }
                ui.add_space(6.0);
                design::divider(ui);
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .id_salt("outreach-dnc")
                    .show(ui, |ui| {
                        for (customer_id, reason, created_at) in &dnc {
                            ui.horizontal(|ui| {
                                ui.monospace(
                                    egui::RichText::new(format!("customer #{customer_id}"))
                                        .color(t.foreground),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if design::button(ui, "Remove", ButtonVariant::Destructive)
                                            .clicked()
                                        {
                                            self.pending_confirm =
                                                Some(ConfirmRequest::remove_from_dnc(*customer_id));
                                        }
                                        ui.label(
                                            egui::RichText::new(created_at.as_str())
                                                .small()
                                                .color(t.muted_foreground),
                                        );
                                        ui.label(
                                            egui::RichText::new(reason.as_str())
                                                .small()
                                                .color(t.muted_foreground),
                                        );
                                    },
                                );
                            });
                        }
                        if dnc.is_empty() {
                            design::empty_state(
                                ui,
                                Icon::X,
                                "No customers on the list",
                                "Search and pick a customer above to exclude them from \
                                 campaigns.",
                            );
                        }
                    });
            },
        );
    }
}

impl Default for OutreachPage {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CampaignAction {
    Queue,
    Pause,
    Resume,
    CancelRemaining,
    RetryFailed,
}

fn campaign_status_tone(status: &str) -> BadgeTone {
    match status {
        "completed" => BadgeTone::Success,
        _ => BadgeTone::Neutral,
    }
}

#[derive(Debug, Clone)]
struct SegmentPreview {
    matched: i64,
    without_email: i64,
    on_dnc: i64,
    notes: Vec<String>,
}

impl SegmentPreview {
    fn hint(&self) -> String {
        format!(
            "≈{} recipients ({} without email, {} on DNC)",
            self.matched, self.without_email, self.on_dnc
        )
    }
}

fn compute_segment_preview(db: &DbHandle, criteria: &str) -> Result<SegmentPreview, String> {
    let body = serde_json::from_str::<serde_json::Value>(criteria)
        .map_err(|e| format!("criteria is not valid JSON: {e}"))?;
    let tree = spp_core::segment::parse_segment_tree(&body)?;
    let preview = db.with_read(|conn| {
        let engine = SegmentEngine::new(conn);
        engine.preview(&tree, 1, i64::MAX)
    });
    let notes = preview
        .get("notes")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|note| note.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Ok(SegmentPreview {
        matched: preview.get("matched").and_then(|v| v.as_i64()).unwrap_or(0),
        without_email: preview
            .get("without_email")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        on_dnc: preview.get("on_dnc").and_then(|v| v.as_i64()).unwrap_or(0),
        notes,
    })
}

fn dnc_customer_label(row: &serde_json::Value) -> String {
    let id = row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let first = row
        .get("first_name")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let last = row
        .get("last_name")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let email = row
        .get("emails")
        .and_then(|v| v.as_array())
        .and_then(|emails| emails.first())
        .and_then(|e| e.as_str())
        .unwrap_or("no email");
    let name = format!("{first} {last}");
    let name = name.trim();
    if name.is_empty() {
        format!("#{id} <{email}>")
    } else {
        format!("#{id} {name} <{email}>")
    }
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

    fn seed_segment(conn: &rusqlite::Connection, name: &str, definition: &str) -> i64 {
        spp_core::outreach::save_segment_v2(conn, None, name, None, definition).expect("segment")
    }

    const TICKET_CUSTOMERS_DEFINITION: &str = r#"{"combinator":"all","conditions":[{"kind":"history","metric":"ticket_count","op":"gte","value":1}],"exclude":[]}"#;

    #[test]
    fn ensure_loaded_latches_and_invalidate_refetches() {
        let db = fixture(vec![("conv", 1, "Hello", "active", "email", 1, 5, 9)]);

        let mut page = OutreachPage::new();
        assert!(!page.loaded);
        page.ensure_loaded(&db);
        assert!(page.loaded);
        assert!(page.campaigns.is_empty(), "no campaigns seeded yet");

        let segment_id = db.with_write(|conn| seed_segment(conn, "S", TICKET_CUSTOMERS_DEFINITION));
        let mailbox = db
            .with_read(|conn| {
                conn.query_row(
                    "INSERT INTO mailboxes (remote_id, name, email)
                 VALUES (777, 'Latched', 'latched@example.test') RETURNING id",
                    [],
                    |r| r.get::<_, i64>(0),
                )
            })
            .expect("mailbox");
        page.new_name = "Latched".into();
        page.new_subject = "Sub".into();
        page.new_body = "Body".into();
        page.new_mailbox_pick = Some(mailbox);
        page.new_segment_pick = Some(segment_id);
        page.create_campaign(&db);
        assert!(page.error.is_none(), "{:?}", page.error);

        page.ensure_loaded(&db);
        assert_eq!(page.campaigns.len(), 1);

        page.invalidate();
        assert!(!page.loaded);
        page.ensure_loaded(&db);
        assert!(page.loaded);
        assert_eq!(page.campaigns.len(), 1);
    }

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
        page.new_mailbox_pick = Some(1);
        page.new_segment_pick = Some(segment_id);
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
        page.new_mailbox_pick = Some(1);
        page.new_segment_pick = None;
        page.create_campaign(&db);
        let error = page.error.clone().expect("segment guard");
        assert!(error.contains("segment"));
    }

    #[test]
    fn campaign_create_requires_valid_fields() {
        let db = fixture(Vec::new());
        let mut page = OutreachPage::new();
        page.create_campaign(&db);
        assert!(page.error.is_some());
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
        let _ = page.delete_segment(&db, segment_id);
        assert!(page.segments.is_empty());

        page.new_dnc_pick = Some(9);
        page.new_dnc_reason = "asked politely".into();
        page.add_dnc(&db);
        assert!(page.error.is_none());
        assert_eq!(page.dnc.len(), 1);
        assert_eq!(page.dnc[0].0, 9);
        assert_eq!(page.dnc[0].1, "asked politely");

        let on_dnc = db.with_read(spp_core::outreach::list_dnc);
        assert!(on_dnc.expect("dnc list").iter().any(|(id, _, _)| *id == 9));

        page.remove_dnc(&db, 9).ok();
        assert!(page.dnc.is_empty());
        let after = db.with_read(spp_core::outreach::list_dnc);
        assert!(after.expect("dnc list 2").iter().all(|(id, _, _)| *id != 9));
    }

    #[test]
    fn dnc_requires_a_picked_customer() {
        let db = fixture(Vec::new());
        let mut page = OutreachPage::new();
        page.new_dnc_pick = None;
        page.add_dnc(&db);
        assert!(page.error.is_some());
    }
    #[test]
    fn campaign_snapshot_carries_every_matched_recipient_past_the_cap() {
        let db = fixture(vec![("conv", 1, "Hello", "active", "email", 1, 5, 9)]);
        db.with_write(|conn| {
            for i in 0..5001 {
                conn.execute(
                    "INSERT INTO customers (id, remote_id, first_name, last_name, email)
                     VALUES (?1, ?1, 'Bulk', 'Segment', ?2)",
                    rusqlite::params![10_000 + i, format!("bulk{i}@bulkmail.example")],
                )
                .expect("bulk customer");
                conn.execute(
                    "INSERT INTO customer_emails (customer_id, value, type)
                     VALUES (?1, ?2, 'work')",
                    rusqlite::params![10_000 + i, format!("bulk{i}@bulkmail.example")],
                )
                .expect("bulk email");
            }
        });
        const BULK_DOMAIN_DEFINITION: &str = r#"{"combinator":"all","conditions":[{"kind":"contact","field":"email_domain","op":"equals","value":"bulkmail.example"}],"exclude":[]}"#;
        let segment_id = db.with_write(|conn| seed_segment(conn, "Bulk", BULK_DOMAIN_DEFINITION));

        let mut page = OutreachPage::new();
        page.reload(&db);
        page.new_name = "Everyone".into();
        page.new_subject = "Hello all".into();
        page.new_body = "Hi {{first_name}}.".into();
        page.new_mailbox_pick = Some(1);
        page.new_segment_pick = Some(segment_id);
        page.create_campaign(&db);

        assert!(page.error.is_none(), "{:?}", page.error);
        let recipients: i64 = db
            .with_read(|conn| {
                conn.query_row("SELECT COUNT(*) FROM outreach_recipients", [], |r| r.get(0))
            })
            .expect("count");
        assert_eq!(
            recipients, 5001,
            "every matched customer ships, not just the first 5000"
        );
        assert!(
            page.message
                .as_deref()
                .is_some_and(|m| m.contains("5001 recipient(s)")),
            "the report names the true count: {:?}",
            page.message
        );
    }

    #[test]
    fn audp2_segment_preview_counts_recipients_before_save() {
        let db = fixture(vec![("conv", 1, "Hello", "active", "email", 1, 5, 9)]);
        let mut page = OutreachPage::new();

        page.new_segment_criteria = TICKET_CUSTOMERS_DEFINITION.into();
        page.preview_new_segment(&db);
        let preview = page.segment_preview.clone().expect("preview computed");
        assert!(preview.matched >= 1, "the seeded ticket matches");
        assert_eq!(preview.without_email, 0, "the seeded customer has an email");
        assert!(preview.notes.is_empty());

        page.new_segment_criteria = "not json".into();
        page.preview_new_segment(&db);
        assert!(
            page.segment_preview.is_none(),
            "no preview on a parse error"
        );
        let error = page.error.clone().expect("parse error surfaces");
        assert!(
            error.contains("criteria"),
            "the error names the field: {error}"
        );

        let segment_id = db.with_write(|conn| seed_segment(conn, "S", TICKET_CUSTOMERS_DEFINITION));
        let definition = db.with_read(|conn| {
            spp_core::outreach::get_segment_v2(conn, segment_id)
                .expect("segment")
                .and_then(|s| s.get("definition").cloned())
                .map(|d| d.to_string())
                .unwrap_or_default()
        });
        page.preview_segment_row(&db, segment_id, &definition);
        let (_, row_preview) = page.row_preview.clone().expect("row preview computed");
        assert!(row_preview.matched >= 1);
        assert_eq!(row_preview.hint().matches("recipients").count(), 1);
    }

    #[test]
    fn audp2_campaign_edit_round_trips_on_drafts_only() {
        let db = fixture(vec![("conv", 1, "Hello", "active", "email", 1, 5, 9)]);
        let segment_id =
            db.with_write(|conn| seed_segment(conn, "Ticket owners", TICKET_CUSTOMERS_DEFINITION));
        let mut page = OutreachPage::new();
        page.reload(&db);
        page.new_name = "Check-in".into();
        page.new_subject = "How is it going?".into();
        page.new_body = "Hi {{first_name}}.".into();
        page.new_mailbox_pick = Some(1);
        page.new_segment_pick = Some(segment_id);
        page.create_campaign(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        let draft = page.campaigns[0].clone();
        let id = i64_field(&draft, "id");
        assert_eq!(str_field(&draft, "status"), "draft");

        page.start_campaign_edit(&db, &draft);
        assert_eq!(page.edit_campaign_id, Some(id));
        assert_eq!(page.new_name, "Check-in");
        assert_eq!(page.new_subject, "How is it going?");
        assert_eq!(page.new_body, "Hi {{first_name}}.");
        assert_eq!(page.new_mailbox_pick, Some(1));

        page.new_subject = "Updated subject".into();
        page.save_campaign_edit(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert!(page.edit_campaign_id.is_none(), "edit mode closes on save");
        assert!(
            page.message
                .as_deref()
                .is_some_and(|m| m.contains("updated")),
            "toast-style message: {:?}",
            page.message
        );
        assert_eq!(str_field(&page.campaigns[0], "subject"), "Updated subject");

        page.campaign_action(&db, id, CampaignAction::Queue);
        assert_eq!(str_field(&page.campaigns[0], "status"), "queued");
        page.start_campaign_edit(&db, &page.campaigns[0].clone());
        page.new_subject = "Must not stick".into();
        page.save_campaign_edit(&db);
        let error = page.error.clone().expect("queued campaigns refuse edits");
        assert!(error.contains("draft"), "got: {error}");
        let after = db
            .with_read(|conn| {
                conn.query_row(
                    "SELECT subject FROM outreach_campaigns WHERE id = ?1",
                    rusqlite::params![id],
                    |r| r.get::<_, String>(0),
                )
            })
            .expect("subject");
        assert_eq!(after, "Updated subject", "the refused edit did not land");
    }

    #[test]
    fn audp2_segment_edit_bumps_version_via_save_segment_v2() {
        let db = fixture(Vec::new());
        let mut page = OutreachPage::new();
        page.new_segment_name = "Churn risk".into();
        page.new_segment_criteria = TICKET_CUSTOMERS_DEFINITION.into();
        page.create_segment(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.segments.len(), 1);
        assert_eq!(i64_field(&page.segments[0], "version"), 1);

        let saved = page.segments[0].clone();
        let segment_id = i64_field(&saved, "id");
        page.start_segment_edit(&saved);
        assert_eq!(page.edit_segment_id, Some(segment_id));
        assert_eq!(page.new_segment_name, "Churn risk");
        assert!(page.new_segment_criteria.contains("ticket_count"));

        page.new_segment_name = "Churn risk v2".into();
        page.new_segment_description = "customers with at least one ticket".into();
        page.create_segment(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert!(page.edit_segment_id.is_none(), "edit mode closes on save");
        assert!(
            page.message
                .as_deref()
                .is_some_and(|m| m.contains("segment updated")),
            "toast-style message: {:?}",
            page.message
        );
        assert_eq!(
            page.segments.len(),
            1,
            "the row was updated, not duplicated"
        );
        assert_eq!(str_field(&page.segments[0], "name"), "Churn risk v2");
        assert_eq!(
            str_field(&page.segments[0], "description"),
            "customers with at least one ticket"
        );
        assert_eq!(
            i64_field(&page.segments[0], "version"),
            2,
            "save_segment_v2 bumped the version"
        );
    }

    #[test]
    fn audp2_dnc_picker_resolves_a_customer() {
        let db = fixture(Vec::new());
        let mut page = OutreachPage::new();

        page.dnc_query = "Kim".into();
        page.refresh_dnc_results(&db);
        assert_eq!(page.dnc_results.len(), 1, "one customer matches 'Kim'");
        let row = page.dnc_results[0].clone();
        assert_eq!(i64_field(&row, "id"), 9);
        let label = dnc_customer_label(&row);
        assert!(label.contains("Kim Pearce"), "{label}");
        assert!(label.contains("kim@example.com"), "{label}");

        page.dnc_query = "zzz-no-match".into();
        page.refresh_dnc_results(&db);
        assert!(page.dnc_results.is_empty(), "no customers match");

        page.new_dnc_pick = Some(9);
        page.new_dnc_reason = "asked politely".into();
        page.add_dnc(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert_eq!(page.dnc.len(), 1);
        assert_eq!(page.dnc[0].0, 9);
        assert_eq!(page.dnc[0].1, "asked politely");
        assert!(page.new_dnc_pick.is_none(), "the pick clears after adding");
    }
}
