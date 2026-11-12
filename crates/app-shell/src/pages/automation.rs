use eframe::egui;
use spp_core::automation::{Action, AutomationRule, Trigger};
use spp_core::db::DbHandle;

use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;
use crate::pages::{ConfirmRequest, NavRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriggerKind {
    NewConversation,
    CustomerReply,
}

impl TriggerKind {
    // AU2-1/D10: the creation form offers exactly the engine's fire vocabulary.
    // The legacy M007 triggers (status_changed/tag_added/sla_risk) have no
    // emitter in program code — offering them created rules that could never fire.
    const ALL: [Self; 2] = [Self::NewConversation, Self::CustomerReply];

    fn label(self) -> &'static str {
        match self {
            Self::NewConversation => "New conversation",
            Self::CustomerReply => "Customer reply",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionKind {
    Assign,
    AddTag,
    SendNote,
    ChangeStatus,
}

impl ActionKind {
    // AU2-1/D10: only actions the engine executes. SetPriority was removed —
    // the engine has no handler and silently recorded "completed" no-ops.
    const ALL: [Self; 4] = [
        Self::Assign,
        Self::AddTag,
        Self::SendNote,
        Self::ChangeStatus,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Assign => "Assign",
            Self::AddTag => "Add tag",
            Self::SendNote => "Send note",
            Self::ChangeStatus => "Change status",
        }
    }
}

#[must_use]
pub(crate) fn trigger_description(trigger: &Trigger) -> String {
    match trigger {
        Trigger::StatusChanged {
            from_status,
            to_status,
        } => {
            let from = from_status.clone().unwrap_or_else(|| "any".into());
            let to = to_status.clone().unwrap_or_else(|| "any".into());
            format!("status changed from {from} to {to}")
        }
        Trigger::TagAdded { tag } => format!("tag '{tag}' added"),
        Trigger::SlaRisk => "SLA at risk".into(),
        // AU2-1/D10: the engine's fire vocabulary.
        Trigger::NewConversation => "a new customer conversation starts".into(),
        Trigger::CustomerReply => "the customer replies on a conversation".into(),
    }
}

// AUD-P0-5 (B1): the three M007 triggers have no emitter in program code —
// the only production fire path emits new_conversation/customer_reply, and
// the creation form deliberately offers nothing else (AU2-1/D10). Stored
// legacy rules must not be presented as functional: they never fire and
// record nothing.
#[must_use]
pub(crate) fn trigger_is_legacy(trigger: &Trigger) -> bool {
    matches!(
        trigger,
        Trigger::StatusChanged { .. } | Trigger::TagAdded { .. } | Trigger::SlaRisk
    )
}

#[must_use]
pub(crate) fn action_description(action: &Action) -> String {
    match action {
        Action::Assign { assignee_remote_id } => {
            format!("assign to user #{assignee_remote_id}")
        }
        Action::AddTag { tag } => format!("add tag '{tag}'"),
        Action::SendNote { body } => format!("send note: {}", truncate(body, 40)),
        Action::ChangeStatus { new_status } => format!("change status to {new_status}"),
        Action::SetPriority { new_priority } => format!("set priority to {new_priority}"),
    }
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// AUD-P1-5 (C6): the Help Scout status vocabulary — the same list the
/// inbox status control uses and `ticket_ops::execute_change_status`
/// enforces. The automation form validates against it instead of accepting
/// free text.
pub(crate) const CONVERSATION_STATUSES: [&str; 4] = ["active", "pending", "closed", "spam"];

#[derive(Debug, Clone)]
pub(crate) struct ApprovalRow {
    pub id: i64,
    pub rule_id: i64,
    pub conversation_id: i64,
    pub proposed_action: String,
}

#[must_use]
pub(crate) fn load_pending_approvals(conn: &rusqlite::Connection) -> Vec<ApprovalRow> {
    conn.prepare(
        "SELECT id, rule_id, conversation_id, proposed_action_json
           FROM automation_approvals WHERE status = 'pending' ORDER BY id DESC LIMIT 100",
    )
    .and_then(|mut stmt| {
        stmt.query_map([], |r| {
            Ok(ApprovalRow {
                id: r.get(0)?,
                rule_id: r.get(1)?,
                conversation_id: r.get(2)?,
                proposed_action: r.get(3)?,
            })
        })
        .map(|rows| rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

pub struct AutomationPage {
    rules: Vec<AutomationRule>,
    loaded: bool,
    approvals: Vec<ApprovalRow>,
    show_new_rule: bool,
    /// AUD-P1-5 (C6): Some(id) puts the form into edit mode for that rule
    /// (prefilled); None is the plain create form.
    edit_id: Option<i64>,
    new_name: String,
    trigger_kind: TriggerKind,
    action_kind: ActionKind,
    /// AUD-P1-5 (C6): the assignee picker (the assignable-users list the
    /// Inbox already uses) replaces the raw numeric-id text field.
    action_assignee_pick: Option<i64>,
    action_tag: String,
    action_note: String,
    action_status: String,
    users: Vec<(i64, String)>,
    error: Option<String>,
}

impl AutomationPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            loaded: false,
            approvals: Vec::new(),
            show_new_rule: false,
            edit_id: None,
            new_name: String::new(),
            trigger_kind: TriggerKind::CustomerReply,
            action_kind: ActionKind::AddTag,
            action_assignee_pick: None,
            action_tag: String::new(),
            action_note: String::new(),
            action_status: String::new(),
            users: Vec::new(),
            error: None,
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.loaded = false;
        self.rules.clear();
        self.approvals.clear();
    }

    fn reload(&mut self, db: &DbHandle) {
        let rules = db.with_read(spp_core::automation::list_rules);
        match rules {
            Ok(rules) => {
                self.rules = rules;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.approvals = db.with_read(load_pending_approvals);
        // AUD-P1-5 (C6): the assignable-users list for the assignee picker —
        // the same source the Inbox uses.
        self.users = db.with_read(crate::pages::inbox::assignable_users);
        self.loaded = true;
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if !self.loaded {
            self.reload(db);
        }
    }

    fn toggle_rule(&mut self, db: &DbHandle, id: i64, enabled: bool) {
        let result =
            db.with_write(|conn| spp_core::automation::set_rule_enabled(conn, id, enabled));
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
        self.reload(db);
    }

    fn form_trigger(&self) -> Result<Trigger, String> {
        // AU2-1/D10: the creation form offers exactly the engine's vocabulary.
        match self.trigger_kind {
            TriggerKind::NewConversation => Ok(Trigger::NewConversation),
            TriggerKind::CustomerReply => Ok(Trigger::CustomerReply),
        }
    }

    fn form_action(&self) -> Result<Action, String> {
        match self.action_kind {
            ActionKind::Assign => {
                // AUD-P1-5 (C6): the picker replaced the free numeric-id
                // text field — an unpicked user is the only failure left.
                let id = self
                    .action_assignee_pick
                    .ok_or_else(|| "assignee required — pick a user".to_string())?;
                Ok(Action::Assign {
                    assignee_remote_id: id,
                })
            }
            ActionKind::AddTag => {
                if self.action_tag.trim().is_empty() {
                    return Err("tag required for the add-tag action".into());
                }
                Ok(Action::AddTag {
                    tag: self.action_tag.trim().to_string(),
                })
            }
            ActionKind::SendNote => {
                if self.action_note.trim().is_empty() {
                    return Err("body required for the note action".into());
                }
                Ok(Action::SendNote {
                    body: self.action_note.trim().to_string(),
                })
            }
            ActionKind::ChangeStatus => {
                // AUD-P1-5 (C6): the status vocabulary is validated, not
                // free text — a typo used to store silently and only
                // Rejected at execute time.
                let status = required(&self.action_status, "status")?;
                if !CONVERSATION_STATUSES.contains(&status.as_str()) {
                    return Err(format!(
                        "status must be one of: {}",
                        CONVERSATION_STATUSES.join(", ")
                    ));
                }
                Ok(Action::ChangeStatus { new_status: status })
            }
        }
    }

    fn start_edit(&mut self, db: &DbHandle, rule: &AutomationRule) {
        let id = rule.id;
        // Only the form's own vocabulary is editable — legacy triggers
        // (never fire) and action kinds the form doesn't offer are refused
        // honestly; delete + recreate is the right path for those.
        let trigger_kind = match &rule.trigger {
            Trigger::NewConversation => Some(TriggerKind::NewConversation),
            Trigger::CustomerReply => Some(TriggerKind::CustomerReply),
            _ => None,
        };
        let (action_kind, pick, tag, note, status) = match &rule.action {
            Action::Assign { assignee_remote_id } => {
                // the executor resolves local OR remote ids — map the stored
                // id onto the picker's local ids.
                let local: Option<i64> = db.with_read(|conn| {
                    conn.query_row(
                        "SELECT id FROM users WHERE id = ?1 OR remote_id = ?1 LIMIT 1",
                        rusqlite::params![assignee_remote_id],
                        |r| r.get(0),
                    )
                    .ok()
                });
                (Some(ActionKind::Assign), local, None, None, None)
            }
            Action::AddTag { tag } => (
                Some(ActionKind::AddTag),
                None,
                Some(tag.clone()),
                None,
                None,
            ),
            Action::SendNote { body } => (
                Some(ActionKind::SendNote),
                None,
                None,
                Some(body.clone()),
                None,
            ),
            Action::ChangeStatus { new_status } => (
                Some(ActionKind::ChangeStatus),
                None,
                None,
                None,
                Some(new_status.clone()),
            ),
            Action::SetPriority { .. } => (None, None, None, None, None),
        };
        let Some(trigger_kind) = trigger_kind else {
            self.error = Some(
                "this rule's trigger is a legacy kind that never fires — delete and \
                 recreate the rule with a live trigger"
                    .into(),
            );
            return;
        };
        let Some(action_kind) = action_kind else {
            self.error = Some(
                "this rule's action kind is not editable in the form — delete and \
                 recreate the rule"
                    .into(),
            );
            return;
        };
        self.edit_id = id;
        self.show_new_rule = true;
        self.error = None;
        self.new_name = rule.name.clone();
        self.trigger_kind = trigger_kind;
        self.action_kind = action_kind;
        self.action_assignee_pick = pick;
        self.action_tag = tag.unwrap_or_default();
        self.action_note = note.unwrap_or_default();
        self.action_status = status.unwrap_or_default();
    }

    fn close_form(&mut self) {
        self.show_new_rule = false;
        self.edit_id = None;
        self.error = None;
    }

    fn create_rule(&mut self, db: &DbHandle) {
        if self.new_name.trim().is_empty() {
            self.error = Some("rule name required".into());
            return;
        }
        let trigger = match self.form_trigger() {
            Ok(trigger) => trigger,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        let action = match self.form_action() {
            Ok(action) => action,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        // AUD-P1-5 (C6): edit mode reuses the same form — the write goes to
        // update_rule and keeps the enabled flag untouched.
        if let Some(edit_id) = self.edit_id {
            let rule = AutomationRule {
                id: Some(edit_id),
                name: self.new_name.trim().to_string(),
                trigger,
                action,
                enabled: true,
                created_at: None,
            };
            let result =
                db.with_write(|conn| spp_core::automation::update_rule(conn, edit_id, &rule));
            match result {
                Ok(true) => {
                    self.close_form();
                    self.reload(db);
                }
                Ok(false) => self.error = Some("rule not found — it may have been deleted".into()),
                Err(e) => self.error = Some(e.to_string()),
            }
            return;
        }
        let rule = AutomationRule {
            id: None,
            name: self.new_name.trim().to_string(),
            trigger,
            action,
            enabled: true,
            created_at: None,
        };
        let result = db.with_write(|conn| spp_core::automation::create_rule(conn, &rule));
        match result {
            Ok(_) => {
                self.show_new_rule = false;
                self.new_name.clear();
                self.reload(db);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn decide(&mut self, db: &DbHandle, approval_id: i64, approve: bool) {
        let actor = db
            .with_read(spp_core::notifications::me_user_local_id)
            .ok()
            .flatten()
            .unwrap_or(5);
        let result = db.with_write(|conn| {
            if approve {
                spp_core::automation::approve(conn, approval_id, actor)
            } else {
                spp_core::automation::reject(conn, approval_id, actor)
            }
        });
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
        self.reload(db);
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        self.ensure_loaded(db);

        design::page_header(
            ui,
            "Automation",
            Some(
                "Rules trigger actions when conversations match certain conditions. \
                 Actions that need review park in the approvals queue.",
            ),
            |ui| {
                if design::button_with_icon(ui, Icon::Plus, "New Rule", ButtonVariant::Primary)
                    .clicked()
                {
                    self.show_new_rule = !self.show_new_rule;
                }
            },
        );
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        design::divider(ui);

        if self.show_new_rule {
            self.new_rule_ui(ui, db);
            ui.add_space(10.0);
        }

        let rules = self.rules.clone();
        let mut toggled: Option<(i64, bool)> = None;
        let mut delete_requested: Option<(i64, String)> = None;
        // AUD-P1-5 (C6): the edit target, if any.
        let mut edit_requested: Option<AutomationRule> = None;
        design::card_titled_wide(
            ui,
            "Rules",
            Some(&format!(
                "{} rule{}",
                rules.len(),
                if rules.len() == 1 { "" } else { "s" }
            )),
            |ui| {
                // UX-2: a header-only table with no rows reads as a
                // broken component — give the empty case a real
                // empty state instead.
                if rules.is_empty() {
                    design::empty_state(
                        ui,
                        Icon::Zap,
                        "No automation rules yet",
                        "Create a rule with \"New Rule\" above — enabled \
                         rules act on incoming conversations \
                         automatically; actions that need review park \
                         in the approvals queue below.",
                    );
                    return;
                }
                // UXG-1-5: exact-column data_table (virtualized rows built
                // in) — the rules list spans the card at every width.
                design::data_table(
                    ui,
                    "automation-rule-table",
                    &["Name", "When", "Then", ""],
                    rules.len(),
                    26.0,
                    |ui, i| {
                        let t = design::tokens(ui);
                        let rule = &rules[i];
                        let id = rule.id.unwrap_or(0);
                        ui.label(
                            egui::RichText::new(rule.name.as_str())
                                .size(13.0)
                                .strong()
                                .color(t.foreground),
                        );
                        // AUD-P0-5 (B1): legacy-trigger rules are displayed
                        // with the truth — they never fire, so they must not
                        // read as working automations.
                        if trigger_is_legacy(&rule.trigger) {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(trigger_description(&rule.trigger))
                                        .size(13.0)
                                        .color(t.muted_foreground),
                                );
                                Icon::AlertTriangle.show(ui, 13.0, t.warning_foreground);
                                ui.label(
                                    egui::RichText::new("legacy trigger — never fires")
                                        .small()
                                        .color(t.warning_foreground),
                                );
                            })
                            .response
                            .on_hover_text(
                                "This rule uses a legacy trigger with no emitter in the \
                                 engine — it can never run. Rules can only start on a new \
                                 customer conversation or a customer reply.",
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(trigger_description(&rule.trigger))
                                    .size(13.0)
                                    .color(t.foreground),
                            );
                        }
                        ui.horizontal(|ui| {
                            let action = action_description(&rule.action);
                            ui.label(egui::RichText::new(action).size(13.0).color(t.foreground));
                            if rule.action.requires_approval() {
                                Icon::AlertTriangle.show(ui, 13.0, t.warning_foreground);
                                ui.label(
                                    egui::RichText::new("(needs approval)")
                                        .small()
                                        .color(t.warning_foreground),
                                );
                            }
                        });
                        ui.horizontal(|ui| {
                            if design::chip(
                                ui,
                                rule.enabled,
                                if rule.enabled { "enabled" } else { "disabled" },
                            )
                            .clicked()
                            {
                                toggled = Some((id, !rule.enabled));
                            }
                            // AUD-P1-5 (C6): edit the rule in the form above —
                            // delete + recreate is no longer the only path.
                            if design::button(ui, "Edit", ButtonVariant::Outline)
                                .on_hover_text("open this rule in the form above")
                                .clicked()
                            {
                                edit_requested = Some(rule.clone());
                            }
                            if design::button(ui, "Delete", ButtonVariant::Destructive)
                                .on_hover_text("asks for confirmation before deleting")
                                .clicked()
                            {
                                delete_requested = Some((id, rule.name.clone()));
                            }
                        });
                    },
                );
            },
        );
        if let Some((id, enabled)) = toggled {
            self.toggle_rule(db, id, enabled);
        }
        if let Some(rule) = edit_requested {
            self.start_edit(db, &rule);
        }
        let mut nav = None;
        if let Some((id, name)) = delete_requested {
            nav = Some(NavRequest::Confirm(ConfirmRequest::delete_automation_rule(
                id, &name,
            )));
        }
        ui.add_space(10.0);

        let approvals = self.approvals.clone();
        let approvals_empty = approvals.is_empty();
        let mut decided: Option<(i64, bool)> = None;
        design::card_titled(
            ui,
            "Pending approvals",
            Some(&format!(
                "{} pending approval{}",
                approvals.len(),
                if approvals.len() == 1 { "" } else { "s" }
            )),
            |ui| {
                if approvals_empty {
                    design::empty_state(
                        ui,
                        Icon::Clock,
                        "No approvals waiting",
                        "Actions that need review will park here.",
                    );
                    return;
                }
                let t = design::tokens(ui);
                egui::ScrollArea::vertical()
                    .id_salt("automation-approvals")
                    .show(ui, |ui| {
                        for approval in &approvals {
                            design::card(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "rule #{} on conversation #{}",
                                        approval.rule_id, approval.conversation_id
                                    ))
                                    .size(13.0)
                                    .strong()
                                    .color(t.foreground),
                                );
                                // AU2-1c: rows hold canonical engine actions —
                                // describe both storage shapes.
                                let human = spp_core::automation::describe_action_json(
                                    &approval.proposed_action,
                                );
                                design::badge(ui, &human, BadgeTone::Neutral);
                                ui.collapsing("raw proposed action", |ui| {
                                    ui.monospace(&approval.proposed_action);
                                });
                                ui.horizontal(|ui| {
                                    if design::button(ui, "Approve", ButtonVariant::Outline)
                                        .clicked()
                                    {
                                        decided = Some((approval.id, true));
                                    }
                                    if design::button(ui, "Reject", ButtonVariant::Destructive)
                                        .clicked()
                                    {
                                        decided = Some((approval.id, false));
                                    }
                                });
                            });
                        }
                    });
            },
        );
        // VF-3 (C5): the page's short-content state — with a handful of
        // rules the window's bottom half was an unmanaged void. The
        // end-note bounds the page with the list's true count.
        if !rules.is_empty() {
            design::list_end_note(
                ui,
                &format!(
                    "{} rule{} · automations fire on new customer conversations and replies",
                    rules.len(),
                    if rules.len() == 1 { "" } else { "s" }
                ),
            );
        }
        if let Some((id, approve)) = decided {
            self.decide(db, id, approve);
        }
        nav
    }

    fn new_rule_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let t = design::tokens(ui);
        // AUD-P1-5 (C6): one form, two modes — create and edit.
        let editing = self.edit_id.is_some();
        design::card(ui, |ui| {
            design::section_title(ui, if editing { "Edit Rule" } else { "New Rule" });
            ui.horizontal(|ui| {
                Icon::AlertTriangle.show(ui, 14.0, t.warning_foreground);
                ui.label(
                    egui::RichText::new(
                        "enabled rules act on conversations automatically — actions that \
                         need review park in the approvals queue",
                    )
                    .small()
                    .color(t.warning_foreground),
                );
            });
            design::form_grid(ui, "automation-new-rule", |ui| {
                design::field_label(ui, "Name");
                ui.add_sized(
                    [220.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_name).hint_text("rule name"),
                );
                ui.end_row();

                design::field_label(ui, "Trigger");
                ui.horizontal(|ui| {
                    for kind in TriggerKind::ALL {
                        if design::chip(ui, self.trigger_kind == kind, kind.label()).clicked() {
                            self.trigger_kind = kind;
                        }
                    }
                });
                ui.end_row();
                // AU2-1/D10: the engine-vocabulary triggers carry no parameters.

                design::field_label(ui, "Action");
                ui.horizontal(|ui| {
                    for kind in ActionKind::ALL {
                        if design::chip(ui, self.action_kind == kind, kind.label()).clicked() {
                            self.action_kind = kind;
                        }
                    }
                });
                ui.end_row();

                match self.action_kind {
                    ActionKind::Assign => {
                        // AUD-P1-5 (C6): the assignable-users list — the
                        // same picker the Inbox uses — replaces the raw
                        // numeric-id text field.
                        design::field_label(ui, "Assignee");
                        let assign_label = self
                            .action_assignee_pick
                            .and_then(|id| {
                                self.users
                                    .iter()
                                    .find(|(uid, _)| *uid == id)
                                    .map(|(_, name)| name.clone())
                            })
                            .unwrap_or_else(|| "pick a user…".into());
                        design::select(ui, "automation-assign", &assign_label, 200.0, |ui| {
                            for (id, name) in self.users.clone() {
                                ui.selectable_value(
                                    &mut self.action_assignee_pick,
                                    Some(id),
                                    &name,
                                );
                            }
                        });
                        ui.end_row();
                    }
                    ActionKind::AddTag => {
                        design::field_label(ui, "Tag");
                        ui.add_sized(
                            [180.0, 28.0],
                            egui::TextEdit::singleline(&mut self.action_tag).hint_text("tag"),
                        );
                        ui.end_row();
                    }
                    ActionKind::SendNote => {
                        design::field_label(ui, "Note body");
                        ui.add_sized(
                            [260.0, 28.0],
                            egui::TextEdit::singleline(&mut self.action_note)
                                .hint_text("note body"),
                        );
                        ui.end_row();
                    }
                    ActionKind::ChangeStatus => {
                        // AUD-P1-5 (C6): the status vocabulary is a segmented
                        // control — no unvalidated free text.
                        design::field_label(ui, "Status");
                        let active = CONVERSATION_STATUSES
                            .iter()
                            .position(|s| *s == self.action_status)
                            .unwrap_or(usize::MAX);
                        if let Some(i) = design::segmented(
                            ui,
                            "automation-status",
                            &CONVERSATION_STATUSES,
                            active,
                        ) {
                            self.action_status = CONVERSATION_STATUSES[i].to_string();
                        }
                        ui.end_row();
                    }
                }
            });
            ui.horizontal(|ui| {
                if design::button(
                    ui,
                    if editing {
                        "Save Changes"
                    } else {
                        "Create Rule"
                    },
                    ButtonVariant::Primary,
                )
                .clicked()
                {
                    self.create_rule(db);
                }
                if editing && design::button(ui, "Cancel", ButtonVariant::Outline).clicked() {
                    self.close_form();
                }
            });
        });
    }
}

impl Default for AutomationPage {
    fn default() -> Self {
        Self::new()
    }
}

fn required(s: &str, what: &str) -> Result<String, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        Err(format!("{what} required"))
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn create_toggle_and_delete_rules_round_trip() {
        let db = fixture(Vec::new());
        let mut page = AutomationPage::new();
        page.new_name = "Escalate urgent".into();
        // AU2-1/D10: the form now offers the engine's fire vocabulary.
        page.trigger_kind = TriggerKind::CustomerReply;
        page.action_kind = ActionKind::ChangeStatus;
        page.action_status = "pending".into();
        page.create_rule(&db);

        assert!(page.error.is_none());
        assert_eq!(page.rules.len(), 1);
        let rule = &page.rules[0];
        assert_eq!(rule.name, "Escalate urgent");
        assert!(rule.enabled);
        assert_eq!(
            trigger_description(&rule.trigger),
            "the customer replies on a conversation"
        );
        assert_eq!(action_description(&rule.action), "change status to pending");

        let id = rule.id.unwrap_or(0);
        page.toggle_rule(&db, id, false);
        assert!(!page.rules[0].enabled);
        let _ = db
            .with_write(|conn| spp_core::automation::delete_rule(conn, id))
            .expect("delete");
        page.reload(&db);
        assert!(page.rules.is_empty());
    }

    #[test]
    fn audit70_delete_needs_confirmation_and_cancel_keeps_the_rule() {
        let db = fixture(Vec::new());
        let mut page = AutomationPage::new();
        page.new_name = "Escalate urgent".into();
        page.trigger_kind = TriggerKind::NewConversation;
        page.action_kind = ActionKind::ChangeStatus;
        page.action_status = "pending".into();
        page.create_rule(&db);
        let id = page.rules[0].id.unwrap_or(0);
        assert_eq!(page.rules.len(), 1);

        let request = ConfirmRequest::delete_automation_rule(id, "Escalate urgent");
        assert_eq!(
            request.action,
            crate::pages::DestructiveAction::DeleteAutomationRule { id }
        );
        let rules: usize = db
            .with_read(spp_core::automation::list_rules)
            .map(|rules| rules.len())
            .unwrap_or_default();
        assert_eq!(rules, 1, "cancel keeps the rule");
        let _ = db
            .with_write(|conn| spp_core::automation::delete_rule(conn, id))
            .expect("delete");
        page.reload(&db);
        assert!(page.rules.is_empty(), "confirm deletes the rule");
    }

    #[test]
    fn form_validation_rejects_empty_fields() {
        let mut page = AutomationPage::new();
        page.new_name = "Rule".into();
        // AU2-1/D10: trigger forms carry no parameters; validation lives on the
        // action side now. The trigger form always succeeds.
        assert!(page.form_trigger().is_ok());
        page.action_kind = ActionKind::Assign;
        // AUD-P1-5 (C6): the numeric-id text field is gone — an unpicked
        // user is the rejection case now.
        assert!(page.form_action().is_err());
        page.action_kind = ActionKind::SendNote;
        assert!(page.form_action().is_err());
        page.action_note = "  ".into();
        assert!(page.form_action().is_err());
        // AUD-P1-5 (C6): the status vocabulary is validated, not free text.
        page.action_kind = ActionKind::ChangeStatus;
        page.action_status = "Extenguished".into();
        let err = page.form_action().expect_err("junk status rejected");
        assert!(err.contains("status must be one of"), "got: {err}");
        page.action_status = "closed".into();
        assert!(page.form_action().is_ok());
        page.new_name.clear();
        page.action_kind = ActionKind::ChangeStatus;
        let db = fixture(Vec::new());
        page.create_rule(&db);
        assert!(page.error.is_some(), "empty name is rejected");
    }

    #[test]
    fn approvals_queue_lists_and_decides() {
        let db = fixture(vec![(
            "conv",
            2,
            "Approval case",
            "active",
            "email",
            1,
            5,
            9,
        )]);
        db.with_write(|conn| {
            let rule_id = spp_core::automation::create_rule(
                conn,
                &AutomationRule {
                    id: None,
                    name: "Escalate".into(),
                    trigger: Trigger::SlaRisk,
                    action: Action::ChangeStatus {
                        new_status: "pending".into(),
                    },
                    enabled: true,
                    created_at: None,
                },
            )
            .expect("rule");
            conn.execute(
                "INSERT INTO automation_approvals (rule_id, conversation_id, proposed_action_json, status, created_at)
                 VALUES (?1, 2, '{\"kind\":\"change_status\"}', 'pending', '2026-01-01T00:00:00Z')",
                rusqlite::params![rule_id],
            )
            .expect("approval");
        });
        let pending = db.with_read(load_pending_approvals);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].conversation_id, 2);

        let mut page = AutomationPage::new();
        page.reload(&db);
        assert_eq!(page.approvals.len(), 1);
        page.decide(&db, pending[0].id, true);
        assert!(page.approvals.is_empty(), "decision clears the queue");
        let status: String = db.with_read(|conn| {
            conn.query_row(
                "SELECT status FROM automation_approvals WHERE id = ?1",
                rusqlite::params![pending[0].id],
                |r| r.get(0),
            )
            .expect("status row")
        });
        assert_eq!(status, "approved");
    }

    #[test]
    fn descriptions_are_human_readable() {
        let trigger = Trigger::StatusChanged {
            from_status: None,
            to_status: Some("closed".into()),
        };
        assert_eq!(
            trigger_description(&trigger),
            "status changed from any to closed"
        );
        let action = Action::SetPriority {
            new_priority: "urgent".into(),
        };
        assert!(action.requires_approval());
        assert_eq!(action_description(&action), "set priority to urgent");
        let note = Action::SendNote {
            body: "a very long note body that should be truncated somewhere around here".into(),
        };
        assert!(!note.requires_approval());
        assert!(action_description(&note).starts_with("send note: "));
    }

    // AUD-P0-5 (B1): exactly the three M007 triggers are legacy — the two
    // fire-vocabulary triggers are live.
    #[test]
    fn legacy_triggers_are_identified_for_honest_display() {
        assert!(trigger_is_legacy(&Trigger::StatusChanged {
            from_status: None,
            to_status: None
        }));
        assert!(trigger_is_legacy(&Trigger::TagAdded { tag: "vip".into() }));
        assert!(trigger_is_legacy(&Trigger::SlaRisk));
        assert!(!trigger_is_legacy(&Trigger::NewConversation));
        assert!(!trigger_is_legacy(&Trigger::CustomerReply));
    }
}

#[cfg(test)]
mod batch27_audit80_tests {
    use super::*;
    use crate::testsupport::fixture;

    fn seed_approval(conn: &rusqlite::Connection) -> i64 {
        conn.execute(
            "INSERT INTO automation_rules (name, trigger_json, action_json)
             VALUES ('t', '{\"kind\":\"sla_risk\"}', '{\"kind\":\"add_tag\",\"tag\":\"x\"}')",
            [],
        )
        .expect("seed rule");
        let rule_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO automation_approvals (rule_id, conversation_id, proposed_action_json)
             VALUES (?1, 1, '{}')",
            rusqlite::params![rule_id],
        )
        .expect("seed approval");
        conn.last_insert_rowid()
    }

    fn decided_by(conn: &rusqlite::Connection, id: i64) -> Option<i64> {
        conn.query_row(
            "SELECT decided_by_user_id FROM automation_approvals WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .expect("read decision")
    }

    #[test]
    fn approvals_attribute_to_the_real_acting_user() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            spp_core::settings::set_string(conn, "me_remote_id", "506").expect("me");
        });

        let approval = db.with_write(|conn| seed_approval(conn));
        let mut page = AutomationPage::new();
        page.decide(&db, approval, true);

        assert!(
            page.error.is_none(),
            "the decision must succeed: {:?}",
            page.error
        );
        assert_eq!(
            db.with_read(|conn| decided_by(conn, approval)),
            Some(6),
            "the approval carries the ACTING user (Grace), not Ada"
        );
    }

    #[test]
    fn no_identity_falls_back_to_the_demo_stand_in() {
        let db = fixture(Vec::new());
        let approval = db.with_write(|conn| seed_approval(conn));

        let mut page = AutomationPage::new();
        page.decide(&db, approval, false);

        assert!(page.error.is_none(), "decision failed: {:?}", page.error);
        assert_eq!(
            db.with_read(|conn| decided_by(conn, approval)),
            Some(5),
            "without a captured identity the Ada stand-in decides (pre-fix behavior)"
        );
    }
    // ---- AUD-P1-5 (C6): rule editing + validated inputs ----

    #[test]
    fn audp15_edit_round_trips_through_the_form() {
        let db = fixture(Vec::new());
        let mut page = AutomationPage::new();
        page.reload(&db);
        assert!(!page.users.is_empty(), "the assignee picker has users");

        // create a rule to edit
        page.new_name = "Triage".into();
        page.trigger_kind = TriggerKind::CustomerReply;
        page.action_kind = ActionKind::AddTag;
        page.action_tag = "triage".into();
        page.create_rule(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        let rule = page.rules[0].clone();
        let id = rule.id.expect("id");

        // open the edit form — prefilled
        page.start_edit(&db, &rule);
        assert_eq!(page.edit_id, Some(id));
        assert_eq!(page.new_name, "Triage");
        assert_eq!(page.trigger_kind, TriggerKind::CustomerReply);
        assert_eq!(page.action_kind, ActionKind::AddTag);
        assert_eq!(page.action_tag, "triage");

        // change name + action, save
        page.new_name = "Triage v2".into();
        page.action_kind = ActionKind::ChangeStatus;
        page.action_status = "pending".into();
        page.create_rule(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert!(page.edit_id.is_none(), "edit mode closes on save");

        let updated = page
            .rules
            .iter()
            .find(|r| r.id == Some(id))
            .expect("rule")
            .clone();
        assert_eq!(updated.name, "Triage v2");
        assert!(
            matches!(updated.action, Action::ChangeStatus { ref new_status } if new_status == "pending")
        );
    }

    #[test]
    fn audp15_edit_preserves_the_disabled_flag() {
        let db = fixture(Vec::new());
        let mut page = AutomationPage::new();
        page.reload(&db);
        page.new_name = "Keep".into();
        page.action_kind = ActionKind::AddTag;
        page.action_tag = "x".into();
        page.create_rule(&db);
        let id = page.rules[0].id.expect("id");
        page.toggle_rule(&db, id, false);
        assert!(
            !page
                .rules
                .iter()
                .find(|r| r.id == Some(id))
                .expect("r")
                .enabled,
            "precondition: disabled"
        );

        let rule = page
            .rules
            .iter()
            .find(|r| r.id == Some(id))
            .expect("r")
            .clone();
        page.start_edit(&db, &rule);
        page.action_tag = "y".into();
        page.create_rule(&db);
        assert!(page.error.is_none(), "{:?}", page.error);
        assert!(
            !page
                .rules
                .iter()
                .find(|r| r.id == Some(id))
                .expect("r")
                .enabled,
            "the edit must not silently re-enable a disabled rule"
        );
    }

    #[test]
    fn audp15_legacy_shapes_are_refused_honestly() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO automation_rules (name, trigger_json, action_json, enabled)
                 VALUES ('Legacy', '{\"kind\":\"sla_risk\"}', '{\"kind\":\"assign\",\"assignee_remote_id\":5}', 1)",
                [],
            )
            .expect("legacy rule");
        });
        let mut page = AutomationPage::new();
        page.reload(&db);
        let legacy = page
            .rules
            .iter()
            .find(|r| r.name == "Legacy")
            .cloned()
            .expect("legacy rule");
        page.start_edit(&db, &legacy);
        assert!(
            page.edit_id.is_none(),
            "legacy triggers never open the form"
        );
        assert!(
            page.error.as_deref().unwrap_or("").contains("legacy"),
            "the refusal names the reason: {:?}",
            page.error
        );
    }
}
