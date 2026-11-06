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
    new_name: String,
    trigger_kind: TriggerKind,
    action_kind: ActionKind,
    action_assignee: String,
    action_tag: String,
    action_note: String,
    action_status: String,
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
            new_name: String::new(),
            trigger_kind: TriggerKind::CustomerReply,
            action_kind: ActionKind::AddTag,
            action_assignee: String::new(),
            action_tag: String::new(),
            action_note: String::new(),
            action_status: String::new(),
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
                let id: i64 = self
                    .action_assignee
                    .trim()
                    .parse()
                    .map_err(|_| "assignee must be a numeric user id")?;
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
                let status = required(&self.action_status, "status")?;
                Ok(Action::ChangeStatus { new_status: status })
            }
        }
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
        design::card_titled(
            ui,
            "Rules",
            Some(&format!("{} rule(s)", rules.len())),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("automation-rules")
                    .show(ui, |ui| {
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
                        design::table_stretched(
                            ui,
                            "automation-rule-table",
                            &["name", "when", "then", ""],
                            26.0,
                            |ui| {
                                let t = design::tokens(ui);
                                for rule in &rules {
                                    let id = rule.id.unwrap_or(0);
                                    ui.label(
                                        egui::RichText::new(rule.name.as_str())
                                            .size(13.0)
                                            .strong()
                                            .color(t.foreground),
                                    );
                                    ui.label(
                                        egui::RichText::new(trigger_description(&rule.trigger))
                                            .size(13.0)
                                            .color(t.foreground),
                                    );
                                    ui.horizontal(|ui| {
                                        let action = action_description(&rule.action);
                                        ui.label(
                                            egui::RichText::new(action)
                                                .size(13.0)
                                                .color(t.foreground),
                                        );
                                        if rule.action.requires_approval() {
                                            Icon::AlertTriangle.show(
                                                ui,
                                                13.0,
                                                t.warning_foreground,
                                            );
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
                                        if design::button(ui, "delete", ButtonVariant::Destructive)
                                            .on_hover_text("asks for confirmation before deleting")
                                            .clicked()
                                        {
                                            delete_requested = Some((id, rule.name.clone()));
                                        }
                                    });
                                    ui.end_row();
                                }
                            },
                        );
                    });
            },
        );
        if let Some((id, enabled)) = toggled {
            self.toggle_rule(db, id, enabled);
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
            Some(&format!("{} pending approval(s)", approvals.len())),
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
                                    if design::button(ui, "approve", ButtonVariant::Outline)
                                        .clicked()
                                    {
                                        decided = Some((approval.id, true));
                                    }
                                    if design::button(ui, "reject", ButtonVariant::Destructive)
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
        if let Some((id, approve)) = decided {
            self.decide(db, id, approve);
        }
        nav
    }

    fn new_rule_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let t = design::tokens(ui);
        design::card(ui, |ui| {
            design::section_title(ui, "New Rule");
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
                design::field_label(ui, "name:");
                ui.add_sized(
                    [220.0, 28.0],
                    egui::TextEdit::singleline(&mut self.new_name).hint_text("rule name"),
                );
                ui.end_row();

                design::field_label(ui, "trigger:");
                ui.horizontal(|ui| {
                    for kind in TriggerKind::ALL {
                        if design::chip(ui, self.trigger_kind == kind, kind.label()).clicked() {
                            self.trigger_kind = kind;
                        }
                    }
                });
                ui.end_row();
                // AU2-1/D10: the engine-vocabulary triggers carry no parameters.

                design::field_label(ui, "action:");
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
                        design::field_label(ui, "assignee id:");
                        ui.add_sized(
                            [180.0, 28.0],
                            egui::TextEdit::singleline(&mut self.action_assignee)
                                .hint_text("numeric user id"),
                        );
                        ui.end_row();
                    }
                    ActionKind::AddTag => {
                        design::field_label(ui, "tag:");
                        ui.add_sized(
                            [180.0, 28.0],
                            egui::TextEdit::singleline(&mut self.action_tag).hint_text("tag"),
                        );
                        ui.end_row();
                    }
                    ActionKind::SendNote => {
                        design::field_label(ui, "note body:");
                        ui.add_sized(
                            [260.0, 28.0],
                            egui::TextEdit::singleline(&mut self.action_note)
                                .hint_text("note body"),
                        );
                        ui.end_row();
                    }
                    ActionKind::ChangeStatus => {
                        design::field_label(ui, "status:");
                        ui.add_sized(
                            [180.0, 28.0],
                            egui::TextEdit::singleline(&mut self.action_status)
                                .hint_text("new status"),
                        );
                        ui.end_row();
                    }
                }
            });
            if design::button(ui, "Create Rule", ButtonVariant::Primary).clicked() {
                self.create_rule(db);
            }
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

    fn page() -> AutomationPage {
        let mut page = AutomationPage::new();
        page.show_new_rule = true;
        page
    }

    #[test]
    fn create_toggle_and_delete_rules_round_trip() {
        let db = fixture(Vec::new());
        let mut page = page();
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
        let mut page = page();
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
        let mut page = page();
        page.new_name = "Rule".into();
        // AU2-1/D10: trigger forms carry no parameters; validation lives on the
        // action side now. The trigger form always succeeds.
        assert!(page.form_trigger().is_ok());
        page.action_kind = ActionKind::Assign;
        page.action_assignee = "ada".into();
        assert!(page.form_action().is_err());
        page.action_kind = ActionKind::SendNote;
        assert!(page.form_action().is_err());
        page.action_note = "  ".into();
        assert!(page.form_action().is_err());
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
}
