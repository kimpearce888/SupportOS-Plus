//! Outreach page (port of the web `pages/outreach.rs`).
//!
//! Redesigned with the shadcn-style design system (`crate::design`):
//! chip tabs, status-badge campaign cards with destructive lifecycle
//! actions, and card-wrapped forms — presentation only.
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

use super::{ConfirmRequest, NavRequest, i64_field, str_field};
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

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
    /// FIND-005: armed shell-confirm request for a destructive action —
    /// raised by [`Self::ui`] at the end of the frame.
    pending_confirm: Option<ConfirmRequest>,
    /// Clean-audit item 147: reload latch — an empty mirror is a legal
    /// steady state, so "lists empty" can no longer mean "re-query every
    /// frame".
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
            pending_confirm: None,
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.loaded = false;
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
        self.loaded = true;
        if let Some(id) = self.selected {
            self.reload_report(db, id);
        }
    }

    fn reload_report(&mut self, db: &DbHandle, id: i64) {
        self.report = Some(db.with_read(|conn| spp_core::outreach::campaign_report(conn, id)));
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if !self.loaded {
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
        // Clean-audit item 146: the snapshot previewed a fixed 5000-row
        // page and threw the matched total away — every recipient past
        // the cap silently never became one, while the segment said
        // otherwise. The snapshot now takes EVERY matched row (page 1,
        // unbounded page size); the success message reports the same
        // count the user matched against.
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
            self.error = Some("No recipients selected.".into());
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

    /// Delete an outreach segment (runs on shell confirmation only —
    /// clean-audit item 140). Returns the core outcome for the toast.
    pub fn delete_segment(&mut self, db: &DbHandle, id: i64) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::outreach::delete_segment(conn, id));
        if let Err(e) = &result {
            self.error = Some(e.to_string());
        }
        self.reload(db);
        result.map(|_| ())
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

    /// FIND-005: runs only after the shell dialog confirms.
    pub fn remove_dnc(&mut self, db: &DbHandle, customer_id: i64) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::outreach::remove_from_dnc(conn, customer_id));
        if let Err(e) = &result {
            self.error = Some(e.to_string());
        }
        self.reload(db);
        result.map(|_| ())
    }

    /// FIND-005: runs only after the shell dialog confirms — discards the
    /// campaign's remaining queued sends.
    pub fn cancel_campaign_remaining_confirmed(
        &mut self,
        db: &DbHandle,
        id: i64,
    ) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::outreach::campaign_cancel_remaining(conn, id));
        self.message = Some(
            result
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("(no message)")
                .to_string(),
        );
        self.reload(db);
        Ok(())
    }

    /// Render the page. Returns a confirm request when the user asked to
    /// delete a segment (clean-audit item 140 — the delete runs only
    /// after the shell dialog confirms).
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        self.ensure_loaded(db);

        design::page_header(ui, "Outreach", None, |_| {});
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                if design::chip(ui, self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
        });
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
        // FIND-005: raise the armed confirm request (see the field doc).
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
                            for action in [
                                CampaignAction::Queue,
                                CampaignAction::Pause,
                                CampaignAction::Resume,
                                CampaignAction::CancelRemaining,
                                CampaignAction::RetryFailed,
                            ] {
                                if design::button(
                                    ui,
                                    action.label(),
                                    match action {
                                        CampaignAction::Queue => ButtonVariant::Primary,
                                        CampaignAction::CancelRemaining => {
                                            ButtonVariant::Destructive
                                        }
                                        _ => ButtonVariant::Outline,
                                    },
                                )
                                .clicked()
                                {
                                    acted = Some((id, action));
                                }
                            }
                        });
                    });
                }
                if campaigns.is_empty() {
                    design::empty_state(
                        ui,
                        Icon::Megaphone,
                        "No campaigns",
                        "Create a campaign to reach a saved segment.",
                    );
                }
            });
        if let Some(id) = picked {
            self.selected = Some(id);
            self.reload_report(db, id);
        }
        if let Some((id, action)) = acted {
            if action == CampaignAction::CancelRemaining {
                // FIND-005: discarding every queued send used to run on the
                // click — arm the shell confirm instead.
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
            design::form_grid(ui, "outreach-report", |ui| {
                for key in [
                    "total",
                    "sent",
                    "failed",
                    "skipped",
                    "replied",
                    "unknown",
                    "remaining",
                ] {
                    design::field_label(ui, key);
                    ui.monospace(
                        egui::RichText::new(i64_field(&report, key).to_string())
                            .color(t.foreground),
                    );
                    ui.end_row();
                }
            });
        });
    }

    fn new_campaign_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card(ui, |ui| {
            design::section_title(ui, "New Campaign");
            design::form_grid(ui, "outreach-new", |ui| {
                design::field_label(ui, "name:");
                ui.add_sized(
                    [240.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_name).hint_text("campaign name"),
                );
                ui.end_row();
                design::field_label(ui, "subject:");
                ui.add_sized(
                    [280.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_subject).hint_text("email subject"),
                );
                ui.end_row();
                design::field_label(ui, "body:");
                ui.add_sized(
                    [320.0, 90.0],
                    egui::TextEdit::multiline(&mut self.new_body)
                        .hint_text("email body — {{first_name}} is substituted"),
                );
                ui.end_row();
                design::field_label(ui, "mailbox id:");
                ui.add_sized(
                    [140.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_mailbox),
                )
                .on_hover_text("local mailbox id (see Settings)");
                ui.end_row();
                design::field_label(ui, "segment id:");
                ui.add_sized(
                    [140.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_segment_json),
                )
                .on_hover_text("saved segment id — required guard");
                ui.end_row();
            });
            let segments = self.segments.clone();
            if !segments.is_empty() {
                design::hint_text(
                    ui,
                    &format!(
                        "saved segments: {}",
                        segments
                            .iter()
                            .map(|s| format!("#{} {}", i64_field(s, "id"), str_field(s, "name")))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                );
            }
            if design::button(ui, "create campaign", ButtonVariant::Primary).clicked() {
                self.create_campaign(db);
            }
        });
    }

    /// The Segments tab. Returns a confirm request when the user asked
    /// to delete a segment (clean-audit item 140).
    fn segments_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        ui.horizontal(|ui| {
            if design::button_with_icon(ui, Icon::Plus, "New Segment", ButtonVariant::Primary)
                .clicked()
            {
                self.show_new_segment = !self.show_new_segment;
            }
        });
        if self.show_new_segment {
            design::card(ui, |ui| {
                design::section_title(ui, "New Segment");
                design::form_grid(ui, "outreach-new-segment", |ui| {
                    design::field_label(ui, "name:");
                    ui.add_sized(
                        [220.0, 28.0],
                        egui::TextEdit::singleline(&mut self.new_segment_name)
                            .hint_text("segment name"),
                    );
                    ui.end_row();
                    design::field_label(ui, "criteria (JSON):");
                    ui.add_sized(
                        [320.0, 80.0],
                        egui::TextEdit::multiline(&mut self.new_segment_criteria),
                    )
                    .on_hover_text("segment definition JSON");
                    ui.end_row();
                });
                if design::button(ui, "create segment", ButtonVariant::Primary).clicked() {
                    self.create_segment(db);
                }
            });
            ui.add_space(10.0);
        }

        let segments = self.segments.clone();
        let mut deleted: Option<(i64, String)> = None;
        egui::ScrollArea::vertical()
            .id_salt("outreach-segments")
            .show(ui, |ui| {
                let t = design::tokens(ui);
                for segment in &segments {
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
                        if design::button(ui, "delete", ButtonVariant::Destructive).clicked() {
                            deleted = Some((i64_field(segment, "id"), str_field(segment, "name")));
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
        // Clean-audit item 140: arming the dialog replaces the
        // unconfirmed delete.
        if let Some((id, name)) = deleted {
            return Some(NavRequest::Confirm(ConfirmRequest::delete_segment(
                id, &name,
            )));
        }
        None
    }

    fn dnc_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let t = design::tokens(ui);
        let dnc = self.dnc.clone();
        design::card_titled(
            ui,
            "Do not contact",
            Some("Customers on this list are excluded from every campaign."),
            |ui| {
                design::form_grid(ui, "outreach-dnc-new", |ui| {
                    design::field_label(ui, "customer id:");
                    ui.add_sized(
                        [140.0, 28.0],
                        egui::TextEdit::singleline(&mut self.new_dnc_customer)
                            .hint_text("customer id"),
                    );
                    ui.end_row();
                    design::field_label(ui, "reason:");
                    ui.add_sized(
                        [220.0, 28.0],
                        egui::TextEdit::singleline(&mut self.new_dnc_reason)
                            .hint_text("why (optional)"),
                    );
                    ui.end_row();
                });
                if design::button(ui, "add", ButtonVariant::Primary).clicked() {
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
                                ui.label(
                                    egui::RichText::new(reason.as_str())
                                        .small()
                                        .color(t.muted_foreground),
                                );
                                ui.label(
                                    egui::RichText::new(created_at.as_str())
                                        .small()
                                        .color(t.muted_foreground),
                                );
                                if design::button(ui, "remove", ButtonVariant::Destructive)
                                    .clicked()
                                {
                                    // FIND-005: the removal used to run on the
                                    // click — arm the shell confirm instead.
                                    self.pending_confirm =
                                        Some(ConfirmRequest::remove_from_dnc(*customer_id));
                                }
                            });
                        }
                        if dnc.is_empty() {
                            design::empty_state(
                                ui,
                                Icon::X,
                                "No customers on the list",
                                "Add a customer id above to exclude them from campaigns.",
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

/// Campaign lifecycle actions (web: the `{action}` path segment).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CampaignAction {
    Queue,
    Pause,
    Resume,
    CancelRemaining,
    RetryFailed,
}

/// Campaign status → badge tone (redesign of the raw GRAY/LIGHT_BLUE/
/// YELLOW/GREEN/RED label).
fn campaign_status_tone(status: &str) -> BadgeTone {
    match status {
        "queued" | "running" => BadgeTone::Info,
        "paused" => BadgeTone::Warning,
        "completed" => BadgeTone::Success,
        "cancelled" => BadgeTone::Destructive,
        _ => BadgeTone::Neutral,
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

    /// Build a saved segment matching customers with tickets (v2 shape).
    fn seed_segment(conn: &rusqlite::Connection, name: &str, definition: &str) -> i64 {
        spp_core::outreach::save_segment_v2(conn, None, name, None, definition).expect("segment")
    }

    const TICKET_CUSTOMERS_DEFINITION: &str = r#"{"combinator":"all","conditions":[{"kind":"history","metric":"ticket_count","op":"gte","value":1}],"exclude":[]}"#;

    /// Clean-audit item 147: the reload latch — an empty outreach mirror
    /// is a legal steady state, so the second `ensure_loaded` on an empty
    /// database must not flip any state (the old `is_empty()` gate
    /// re-queried every frame); a real invalidation still reloads.
    #[test]
    fn ensure_loaded_latches_and_invalidate_refetches() {
        let db = fixture(vec![("conv", 1, "Hello", "active", "email", 1, 5, 9)]);

        let mut page = OutreachPage::new();
        assert!(!page.loaded);
        page.ensure_loaded(&db);
        assert!(page.loaded);
        assert!(page.campaigns.is_empty(), "no campaigns seeded yet");

        // Seed a campaign behind the page's back (as a sync would) and
        // prove the stale page does NOT hot-reload without invalidation.
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
        page.new_mailbox = mailbox.to_string();
        page.new_segment_json = segment_id.to_string();
        page.create_campaign(&db);
        assert!(page.error.is_none(), "{:?}", page.error);

        // The latch path: a second ensure_loaded keeps whatever reload
        // produced (no re-query churn, same mirror).
        page.ensure_loaded(&db);
        assert_eq!(page.campaigns.len(), 1);

        // Invalidation clears the latch; the next ensure_loaded refetches.
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
        let _ = page.delete_segment(&db, segment_id);
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

        page.remove_dnc(&db, 9).ok();
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
    /// Clean-audit item 146: the recipient snapshot previewed a fixed
    /// 5000-row page and threw the matched total away — recipients past
    /// the cap silently never became recipients. A segment matching
    /// 5001 customers now snapshots all 5001.
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
                // The contact/email_domain condition reads
                // customer_emails — give each bulk customer its row.
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
        page.new_mailbox = "1".into();
        page.new_segment_json = segment_id.to_string();
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
}
