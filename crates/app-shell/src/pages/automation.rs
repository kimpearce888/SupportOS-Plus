//! Automation page (port of the web `pages/automation.rs`).
//!
//! Redesigned with the shadcn-style design system (`crate::design`):
//! striped rule table with chips and destructive deletes, card-wrapped
//! approval queue, and a warning hint on the high-impact rule creator —
//! presentation only, all rule/approval behavior intact.
//!
//! The rules table (name, trigger, action, enabled toggle, delete —
//! `spp_core::automation::{list_rules,set_rule_enabled,delete_rule}`), the
//! new-rule form (`create_rule`) with trigger/action kinds and their
//! parameters, and the pending-approvals list (`approve`/`reject` on the
//! `automation_approvals` table; the web page showed the same queue behind
//! `/api/automation/rules`).

use eframe::egui;
use spp_core::automation::{Action, AutomationRule, Trigger};
use spp_core::db::DbHandle;

use crate::design::{self, ButtonVariant};
use crate::icons::Icon;

/// Trigger kinds offered by the new-rule form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriggerKind {
    StatusChanged,
    TagAdded,
    SlaRisk,
}

impl TriggerKind {
    const ALL: [Self; 3] = [Self::StatusChanged, Self::TagAdded, Self::SlaRisk];

    fn label(self) -> &'static str {
        match self {
            Self::StatusChanged => "Status changed",
            Self::TagAdded => "Tag added",
            Self::SlaRisk => "SLA at risk",
        }
    }
}

/// Action kinds offered by the new-rule form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionKind {
    Assign,
    AddTag,
    SendNote,
    ChangeStatus,
    SetPriority,
}

impl ActionKind {
    const ALL: [Self; 5] = [
        Self::Assign,
        Self::AddTag,
        Self::SendNote,
        Self::ChangeStatus,
        Self::SetPriority,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Assign => "Assign",
            Self::AddTag => "Add tag",
            Self::SendNote => "Send note",
            Self::ChangeStatus => "Change status",
            Self::SetPriority => "Set priority",
        }
    }
}

/// Human-readable trigger description (web: the rule card's trigger line).
#[must_use]
pub fn trigger_description(trigger: &Trigger) -> String {
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
    }
}

/// Human-readable action description (web: the rule card's action line).
#[must_use]
pub fn action_description(action: &Action) -> String {
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

/// One pending approval row (typed port of the approvals table).
#[derive(Debug, Clone)]
pub struct ApprovalRow {
    pub id: i64,
    pub rule_id: i64,
    pub conversation_id: i64,
    pub proposed_action: String,
    pub status: String,
}

/// Load pending approvals (the `/api/automation/approvals` equivalent).
#[must_use]
pub fn load_pending_approvals(conn: &rusqlite::Connection) -> Vec<ApprovalRow> {
    conn.prepare(
        "SELECT id, rule_id, conversation_id, proposed_action_json, status
           FROM automation_approvals WHERE status = 'pending' ORDER BY id DESC LIMIT 100",
    )
    .and_then(|mut stmt| {
        stmt.query_map([], |r| {
            Ok(ApprovalRow {
                id: r.get(0)?,
                rule_id: r.get(1)?,
                conversation_id: r.get(2)?,
                proposed_action: r.get(3)?,
                status: r.get(4)?,
            })
        })
        .map(|rows| rows.filter_map(Result::ok).collect())
    })
    .unwrap_or_default()
}

/// The interactive automation page state.
pub struct AutomationPage {
    rules: Vec<AutomationRule>,
    approvals: Vec<ApprovalRow>,
    show_new_rule: bool,
    new_name: String,
    trigger_kind: TriggerKind,
    trigger_from: String,
    trigger_to: String,
    trigger_tag: String,
    action_kind: ActionKind,
    action_assignee: String,
    action_tag: String,
    action_note: String,
    action_status: String,
    action_priority: String,
    error: Option<String>,
}

impl AutomationPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            approvals: Vec::new(),
            show_new_rule: false,
            new_name: String::new(),
            trigger_kind: TriggerKind::StatusChanged,
            trigger_from: String::new(),
            trigger_to: String::new(),
            trigger_tag: String::new(),
            action_kind: ActionKind::AddTag,
            action_assignee: String::new(),
            action_tag: String::new(),
            action_note: String::new(),
            action_status: String::new(),
            action_priority: String::new(),
            error: None,
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
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
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.rules.is_empty() {
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

    fn delete_rule(&mut self, db: &DbHandle, id: i64) {
        let result = db.with_write(|conn| spp_core::automation::delete_rule(conn, id));
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
        self.reload(db);
    }

    /// Build the trigger from the form state.
    fn form_trigger(&self) -> Result<Trigger, String> {
        match self.trigger_kind {
            TriggerKind::StatusChanged => Ok(Trigger::StatusChanged {
                from_status: optional(&self.trigger_from),
                to_status: optional(&self.trigger_to),
            }),
            TriggerKind::TagAdded => {
                if self.trigger_tag.trim().is_empty() {
                    return Err("tag required for the tag-added trigger".into());
                }
                Ok(Trigger::TagAdded {
                    tag: self.trigger_tag.trim().to_string(),
                })
            }
            TriggerKind::SlaRisk => Ok(Trigger::SlaRisk),
        }
    }

    /// Build the action from the form state.
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
            ActionKind::SetPriority => {
                let priority = required(&self.action_priority, "priority")?;
                Ok(Action::SetPriority {
                    new_priority: priority,
                })
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
        // The web route decided as the me user; Ada (5) stands in.
        let result = db.with_write(|conn| {
            if approve {
                spp_core::automation::approve(conn, approval_id, 5)
            } else {
                spp_core::automation::reject(conn, approval_id, 5)
            }
        });
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
        self.reload(db);
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
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
        let mut deleted: Option<i64> = None;
        design::card_titled(
            ui,
            "Rules",
            Some(&format!("{} rule(s)", rules.len())),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("automation-rules")
                    .show(ui, |ui| {
                        design::table(
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
                                            .clicked()
                                        {
                                            deleted = Some(id);
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
        if let Some(id) = deleted {
            self.delete_rule(db, id);
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
                                        "rule #{} on conversation #{} · {}",
                                        approval.rule_id,
                                        approval.conversation_id,
                                        approval.proposed_action
                                    ))
                                    .size(13.0)
                                    .color(t.foreground),
                                );
                                ui.horizontal(|ui| {
                                    if design::button(ui, "approve", ButtonVariant::Primary)
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

                match self.trigger_kind {
                    TriggerKind::StatusChanged => {
                        design::field_label(ui, "from → to:");
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [100.0, 28.0],
                                egui::TextEdit::singleline(&mut self.trigger_from)
                                    .hint_text("from status"),
                            );
                            ui.label(
                                egui::RichText::new("→")
                                    .size(13.0)
                                    .color(t.muted_foreground),
                            );
                            ui.add_sized(
                                [100.0, 28.0],
                                egui::TextEdit::singleline(&mut self.trigger_to)
                                    .hint_text("to status"),
                            );
                        });
                        ui.end_row();
                    }
                    TriggerKind::TagAdded => {
                        design::field_label(ui, "tag:");
                        ui.add_sized(
                            [180.0, 28.0],
                            egui::TextEdit::singleline(&mut self.trigger_tag).hint_text("tag"),
                        );
                        ui.end_row();
                    }
                    TriggerKind::SlaRisk => {}
                }

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
                    ActionKind::SetPriority => {
                        design::field_label(ui, "priority:");
                        ui.add_sized(
                            [180.0, 28.0],
                            egui::TextEdit::singleline(&mut self.action_priority)
                                .hint_text("new priority"),
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

fn optional(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
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
        page.trigger_kind = TriggerKind::TagAdded;
        page.trigger_tag = "urgent".into();
        page.action_kind = ActionKind::ChangeStatus;
        page.action_status = "pending".into();
        page.create_rule(&db);

        assert!(page.error.is_none());
        assert_eq!(page.rules.len(), 1);
        let rule = &page.rules[0];
        assert_eq!(rule.name, "Escalate urgent");
        assert!(rule.enabled);
        assert_eq!(trigger_description(&rule.trigger), "tag 'urgent' added");
        assert_eq!(action_description(&rule.action), "change status to pending");

        let id = rule.id.unwrap_or(0);
        page.toggle_rule(&db, id, false);
        assert!(!page.rules[0].enabled);
        page.delete_rule(&db, id);
        assert!(page.rules.is_empty());
    }

    #[test]
    fn form_validation_rejects_empty_fields() {
        let mut page = page();
        page.new_name = "Rule".into();
        // Tag trigger with empty tag.
        page.trigger_kind = TriggerKind::TagAdded;
        assert!(page.form_trigger().is_err());
        // Assign action with non-numeric id.
        page.trigger_kind = TriggerKind::SlaRisk;
        page.action_kind = ActionKind::Assign;
        page.action_assignee = "ada".into();
        assert!(page.form_action().is_err());
        // Note action with empty body.
        page.action_kind = ActionKind::SendNote;
        assert!(page.form_action().is_err());
        // Missing rule name.
        page.new_name.clear();
        page.action_kind = ActionKind::SetPriority;
        page.action_priority = "urgent".into();
        let db = fixture(Vec::new());
        page.create_rule(&db);
        assert!(page.error.is_some());
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
