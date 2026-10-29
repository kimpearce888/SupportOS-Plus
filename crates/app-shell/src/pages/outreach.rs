//! Outreach page (port of the web `pages/outreach.rs`).
//!
//! RS-19e reskin: rebuilt onto the DC widget set (`crate::design`) per the
//! conformed lab's `outreach` sheet (DS "stack" archetype): the tab row is
//! the DS-C-09 underline rail (same three labels, same state machine);
//! campaign cards keep the lab's chip + raw-status badge head and split
//! action row (lifecycle acts left, destructive cancel + retry right);
//! the report card grows the lab's warn-callout receipt for the
//! totals-read gap; segment rows adopt the lab's card anatomy (strong id —
//! name with the delete pushed right, the criteria JSON as the mono line
//! beneath, DC C-1); DNC rows push the reason/ts/remove cluster right.
//! Presentation only — every query and write is byte-identical.
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
use crate::design::{self, BadgeTone, ButtonVariant, CalloutRole};
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
    /// Audit fix (M22): wall-clock stamps for the transient status
    /// lines — a finished note ("status: …") or a stale validation
    /// error expires on its own instead of sitting on screen until the
    /// next action replaces it.
    message_at: Option<std::time::Instant>,
    error_at: Option<std::time::Instant>,
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
            message_at: None,
            error_at: None,
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

    /// Audit fix (M22): transient status lines carry their set-time so
    /// rendering can expire them (see [`Self::expire_status_lines`]); a
    /// success also supersedes a stale error (and vice versa) so an old
    /// line never survives a later, opposite outcome.
    fn set_message(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
        self.message_at = Some(std::time::Instant::now());
        self.error = None;
        self.error_at = None;
    }

    /// Audit fix (M22): see [`Self::set_message`].
    fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
        self.error_at = Some(std::time::Instant::now());
        self.message = None;
        self.message_at = None;
    }

    /// Audit fix (M22): drop status lines past their TTL before
    /// rendering.
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
        let name = self.new_name.trim().to_string();
        if name.is_empty() || name.len() > 200 {
            self.set_error("name must be 1-200 characters.");
            return;
        }
        let subject = self.new_subject.trim().to_string();
        if subject.is_empty() || subject.len() > 500 {
            self.set_error("subject must be 1-500 characters.");
            return;
        }
        let body = self.new_body.clone();
        if body.is_empty() || body.len() > 200_000 {
            self.set_error("body must be 1-200000 characters.");
            return;
        }
        let Ok(mailbox_local_id) = self.new_mailbox.trim().parse::<i64>() else {
            self.set_error("A valid sending mailbox is required.");
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
            self.set_error("A valid sending mailbox is required.");
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
            self.set_error(
                "A saved segment id is required (campaigns never target the whole address \
                 book by accident).",
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
                self.new_name.clear();
                self.new_subject.clear();
                self.new_body.clear();
                self.new_segment_json.clear();
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
            Err(e) => self.set_error(e.to_string()),
        }
    }

    /// Delete an outreach segment (runs on shell confirmation only —
    /// clean-audit item 140). Returns the core outcome for the toast.
    pub fn delete_segment(&mut self, db: &DbHandle, id: i64) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::outreach::delete_segment(conn, id));
        if let Err(e) = &result {
            self.set_error(e.to_string());
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
            self.set_error("customer id must be a positive integer");
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
            Err(e) => self.set_error(e.to_string()),
        }
    }

    /// FIND-005: runs only after the shell dialog confirms.
    pub fn remove_dnc(&mut self, db: &DbHandle, customer_id: i64) -> spp_core::Result<()> {
        let result = db.with_write(|conn| spp_core::outreach::remove_from_dnc(conn, customer_id));
        if let Err(e) = &result {
            self.set_error(e.to_string());
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
        self.set_message(
            result
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("(no message)"),
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
        // DS-C-09: the chip row is the underline tab rail — same three
        // labels, same state machine.
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
        // Audit fix (M22): expire stale status lines before rendering.
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
                        // The lab's split ds-tool-row: the lifecycle acts
                        // left, the destructive cancel + retry right.
                        ui.horizontal(|ui| {
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
                                    // right_to_left paints the first added
                                    // widget rightmost — retry first so the
                                    // visual order matches the lab's
                                    // [cancel remaining] [retry failed].
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
            // The lab's reportNote — the DS-C-15 warn receipt for the
            // totals-read gap (data-layer, out of reskin scope; the read
            // stays byte-identical per the lab's own note).
            ui.add_space(4.0);
            design::callout(
                ui,
                CalloutRole::Warn,
                "The report panel reads top-level keys while the core nests \
                 them under totals (outreach.rs:1336) — every row renders \
                 0. Data-layer bug, out of reskin scope; documented, not \
                 fixed here.",
            );
        });
    }

    fn new_campaign_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card(ui, |ui| {
            design::section_title(ui, "New Campaign");
            // Every input binds its visible label (RS-18 wave-wide a11y
            // rule — egui never reads a preceding label on its own).
            design::form_grid(ui, "outreach-new", |ui| {
                let name_label = design::field_label(ui, "name:");
                ui.add_sized(
                    [240.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_name).hint_text("campaign name"),
                )
                .labelled_by(name_label.id);
                ui.end_row();
                let subject_label = design::field_label(ui, "subject:");
                ui.add_sized(
                    [280.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_subject).hint_text("email subject"),
                )
                .labelled_by(subject_label.id);
                ui.end_row();
                let body_label = design::field_label(ui, "body:");
                ui.add_sized(
                    [320.0, 90.0],
                    egui::TextEdit::multiline(&mut self.new_body)
                        .hint_text("email body — {{first_name}} is substituted"),
                )
                .labelled_by(body_label.id);
                ui.end_row();
                let mailbox_label = design::field_label(ui, "mailbox id:");
                ui.add_sized(
                    [140.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_mailbox),
                )
                .on_hover_text("local mailbox id (see Settings)")
                .labelled_by(mailbox_label.id);
                ui.end_row();
                let segment_label = design::field_label(ui, "segment id:");
                ui.add_sized(
                    [140.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_segment_json),
                )
                .on_hover_text("saved segment id — required guard")
                .labelled_by(segment_label.id);
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
                // Every input binds its visible label (RS-18 wave-wide
                // a11y rule — egui never reads a preceding label alone).
                design::form_grid(ui, "outreach-new-segment", |ui| {
                    let name_label = design::field_label(ui, "name:");
                    ui.add_sized(
                        [220.0, 28.0],
                        egui::TextEdit::singleline(&mut self.new_segment_name)
                            .hint_text("segment name"),
                    )
                    .labelled_by(name_label.id);
                    ui.end_row();
                    let criteria_label = design::field_label(ui, "criteria (JSON):");
                    ui.add_sized(
                        [320.0, 80.0],
                        egui::TextEdit::multiline(&mut self.new_segment_criteria),
                    )
                    .on_hover_text("segment definition JSON")
                    .labelled_by(criteria_label.id);
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
                    // The lab's segment card: strong id — name with the
                    // danger delete pushed right (DS.row's side cluster),
                    // the criteria JSON as the mono line beneath (raw
                    // tokens visible per DC C-1 — the pre-rebuild build
                    // kept the definition invisible).
                    design::card(ui, |ui| {
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
                                    if design::button(ui, "delete", ButtonVariant::Destructive)
                                        .clicked()
                                    {
                                        deleted = Some((
                                            i64_field(segment, "id"),
                                            str_field(segment, "name"),
                                        ));
                                    }
                                },
                            );
                        });
                        let criteria = segment
                            .get("definition")
                            .map(|v| v.to_string())
                            .unwrap_or_default();
                        ui.monospace(
                            egui::RichText::new(criteria)
                                .small()
                                .color(t.muted_foreground),
                        );
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
                    // Every input binds its visible label (RS-18 wave-wide
                    // a11y rule — egui never reads a preceding label alone).
                    let customer_label = design::field_label(ui, "customer id:");
                    ui.add_sized(
                        [140.0, 28.0],
                        egui::TextEdit::singleline(&mut self.new_dnc_customer)
                            .hint_text("customer id"),
                    )
                    .labelled_by(customer_label.id);
                    ui.end_row();
                    let reason_label = design::field_label(ui, "reason:");
                    ui.add_sized(
                        [220.0, 28.0],
                        egui::TextEdit::singleline(&mut self.new_dnc_reason)
                            .hint_text("why (optional)"),
                    )
                    .labelled_by(reason_label.id);
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
                            // The lab's DS.row: the mono id left, the
                            // reason/ts/remove cluster pushed right.
                            ui.horizontal(|ui| {
                                ui.monospace(
                                    egui::RichText::new(format!("customer #{customer_id}"))
                                        .color(t.foreground),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        // right_to_left paints the first
                                        // added widget rightmost — remove
                                        // first so the visual order matches
                                        // the lab's [reason][ts][remove].
                                        if design::button(ui, "remove", ButtonVariant::Destructive)
                                            .clicked()
                                        {
                                            // FIND-005: the removal used to run on the
                                            // click — arm the shell confirm instead.
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

/// Campaign status → badge tone. The lab's CAMP map rides the raw token
/// through `statusBadge`: `completed` maps onto the resolved success
/// badge, every other lifecycle token (queued/running/paused/cancelled)
/// keeps the neutral badge with the raw token visible (DC C-1).
fn campaign_status_tone(status: &str) -> BadgeTone {
    match status {
        "completed" => BadgeTone::Success,
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
