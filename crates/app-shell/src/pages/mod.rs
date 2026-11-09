pub mod ai_center;
pub mod automation;
pub mod connectors;
pub mod custom_objects;
pub mod customers;
pub mod dashboard;
pub mod diagnostics;
pub mod inbox;
pub mod incidents;
pub mod issue_radar;
pub mod knowledge;
pub mod live_chats;
pub mod notifications;
pub mod onboarding;
pub mod operations;
pub mod organizations;
pub mod outreach;
pub mod reports;
pub mod search;
pub mod settings;
pub mod support_graph;
pub mod sync_health;

use spp_core::events::ServerEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DestructiveAction {
    DeleteAutomationRule {
        id: i64,
    },
    DeleteKnowledgeDoc {
        id: i64,
    },
    DeleteCustomType {
        id: i64,
    },
    DeleteCustomObject {
        id: i64,
    },
    DeleteConnector {
        id: i64,
    },
    /// AUD-P1-3 (C4): delete an incident (with confirm).
    DeleteIncident {
        id: i64,
    },
    DeleteSegment {
        id: i64,
    },
    ClearDiagnosticsLog,
    DeleteSavedReport {
        id: i64,
    },
    RemoveFromDnc {
        customer_id: i64,
    },
    CancelCampaignRemaining {
        id: i64,
    },
    DisableAllAi,
    DeleteSavedReply {
        mailbox_remote_id: i64,
        remote_id: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmRequest {
    pub title: String,
    pub message: String,
    pub confirm_label: String,
    pub danger: bool,
    pub action: DestructiveAction,
}

impl ConfirmRequest {
    #[must_use]
    pub(crate) fn delete_automation_rule(id: i64, name: &str) -> Self {
        Self {
            title: "Delete automation rule?".into(),
            message: format!(
                "The rule \"{name}\" stops firing immediately. This cannot be undone."
            ),
            confirm_label: "Delete rule".into(),
            danger: true,
            action: DestructiveAction::DeleteAutomationRule { id },
        }
    }

    #[must_use]
    pub(crate) fn delete_knowledge_doc(id: i64, title: &str) -> Self {
        Self {
            title: "Delete knowledge document?".into(),
            message: format!(
                "The document \"{title}\" and its embedded chunks are removed. \
                 Search and AI answers lose it immediately. This cannot be undone."
            ),
            confirm_label: "Delete document".into(),
            danger: true,
            action: DestructiveAction::DeleteKnowledgeDoc { id },
        }
    }

    #[must_use]
    pub(crate) fn delete_custom_type(id: i64, name: &str) -> Self {
        Self {
            title: "Delete custom type?".into(),
            message: format!(
                "The type \"{name}\" and every record stored under it are removed. \
                 This cannot be undone."
            ),
            confirm_label: "Delete type".into(),
            danger: true,
            action: DestructiveAction::DeleteCustomType { id },
        }
    }

    #[must_use]
    pub(crate) fn delete_custom_object(id: i64, name: &str) -> Self {
        Self {
            title: "Delete object?".into(),
            message: format!(
                "The object \"{name}\" is removed from its type. This cannot be undone."
            ),
            confirm_label: "Delete object".into(),
            danger: true,
            action: DestructiveAction::DeleteCustomObject { id },
        }
    }

    #[must_use]
    pub(crate) fn delete_connector(id: i64, name: &str) -> Self {
        Self {
            title: "Delete connector?".into(),
            message: format!(
                "The connector \"{name}\" stops syncing immediately; its stored \
                 rows stay in the mirror. This cannot be undone."
            ),
            confirm_label: "Delete connector".into(),
            danger: true,
            action: DestructiveAction::DeleteConnector { id },
        }
    }

    /// AUD-P1-3 (C4): incident deletion confirm — the workspace (links,
    /// notes, events) is removed with the incident.
    #[must_use]
    pub(crate) fn delete_incident(id: i64, code: &str, title: &str) -> Self {
        Self {
            title: "Delete incident?".into(),
            message: format!(
                "The incident {code} (\"{title}\") and its workspace — links, notes \
                 and events — are removed. Linked conversations themselves are \
                 untouched. This cannot be undone."
            ),
            confirm_label: "Delete incident".into(),
            danger: true,
            action: DestructiveAction::DeleteIncident { id },
        }
    }

    #[must_use]
    pub(crate) fn delete_segment(id: i64, name: &str) -> Self {
        Self {
            title: "Delete segment?".into(),
            message: format!(
                "The segment \"{name}\" is removed; campaigns that targeted it \
                 keep their recipient snapshots. This cannot be undone."
            ),
            confirm_label: "Delete segment".into(),
            danger: true,
            action: DestructiveAction::DeleteSegment { id },
        }
    }

    #[must_use]
    pub(crate) fn clear_diagnostics_log(count: usize) -> Self {
        Self {
            title: "Clear the event log?".into(),
            message: format!(
                "All {count} stored diagnostics event(s) are deleted. This cannot be undone."
            ),
            confirm_label: "Clear log".into(),
            danger: true,
            action: DestructiveAction::ClearDiagnosticsLog,
        }
    }

    #[must_use]
    pub(crate) fn delete_saved_report(id: i64, name: &str) -> Self {
        Self {
            title: "Delete saved report?".into(),
            message: format!("The saved report \"{name}\" is removed. This cannot be undone."),
            confirm_label: "Delete report".into(),
            danger: true,
            action: DestructiveAction::DeleteSavedReport { id },
        }
    }

    #[must_use]
    pub(crate) fn remove_from_dnc(customer_id: i64) -> Self {
        Self {
            title: "Remove from the do-not-contact list?".into(),
            message: format!("Customer {customer_id} becomes contactable by every campaign again."),
            confirm_label: "Remove".into(),
            danger: true,
            action: DestructiveAction::RemoveFromDnc { customer_id },
        }
    }

    #[must_use]
    pub(crate) fn cancel_campaign_remaining(id: i64, name: &str) -> Self {
        Self {
            title: "Cancel the remaining sends?".into(),
            message: format!(
                "Every queued send of the campaign \"{name}\" is discarded. \
                 This cannot be undone."
            ),
            confirm_label: "Cancel sends".into(),
            danger: true,
            action: DestructiveAction::CancelCampaignRemaining { id },
        }
    }

    #[must_use]
    pub(crate) fn disable_all_ai() -> Self {
        Self {
            title: "Disable all AI?".into(),
            message: "Every AI feature and every endpoint call is disabled at once. \
                      Re-enable it here when needed."
                .into(),
            confirm_label: "Disable AI".into(),
            danger: true,
            action: DestructiveAction::DisableAllAi,
        }
    }

    #[must_use]
    pub(crate) fn delete_saved_reply(mailbox_remote_id: i64, remote_id: i64, name: &str) -> Self {
        Self {
            title: "Delete saved reply?".into(),
            message: format!(
                "The saved reply \"{name}\" is deleted from Help Scout. This cannot be undone."
            ),
            confirm_label: "Delete reply".into(),
            danger: true,
            action: DestructiveAction::DeleteSavedReply {
                mailbox_remote_id,
                remote_id,
            },
        }
    }

    #[must_use]
    pub(crate) fn dialog(&self) -> crate::overlays::ConfirmDialog {
        let mut dialog =
            crate::overlays::ConfirmDialog::new(self.title.clone(), self.message.clone())
                .confirm_label(self.confirm_label.clone());
        if self.danger {
            dialog = dialog.danger();
        }
        dialog
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavRequest {
    InboxTile(String),
    Page(PageKey),
    Confirm(ConfirmRequest),
    /// Deep link `/inbox/conversation/{id}` — open Inbox on that conversation.
    OpenConversation(i64),
    /// Deep link `/customers/{id}` — open the customer profile panel.
    OpenCustomer(i64),
    /// Deep link `/knowledge?doc={id}` — open Knowledge on the documents list.
    OpenKnowledgeDoc(i64),
    /// Deep link `/knowledge/help-center/{id}` — open the help-center article reader.
    OpenHelpArticle(i64),
    /// Deep link `/issues?tab=known` — open Issue Radar on the Known tab.
    OpenKnownIssues,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKey {
    Shell,
    Dashboard,
    Diagnostics,
    Inbox,
    LiveChats,
    Customers,
    Knowledge,
    Reports,
    Operations,
    AiCenter,
    Automation,
    Connectors,
    CustomObjects,
    Incidents,
    IssueRadar,
    Notifications,
    Onboarding,
    Organizations,
    Outreach,
    Search,
    Settings,
    SupportGraph,
    SyncHealth,
}

impl PageKey {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Shell => "Shell",
            Self::Dashboard => "Dashboard",
            Self::Diagnostics => "Diagnostics",
            Self::Inbox => "Inbox",
            Self::LiveChats => "Live chats",
            Self::Customers => "Customers",
            Self::Knowledge => "Knowledge",
            Self::Reports => "Reports",
            Self::Operations => "Operations",
            Self::AiCenter => "AI Center",
            Self::Automation => "Automation",
            Self::Connectors => "Connectors",
            Self::CustomObjects => "Custom Objects",
            Self::Incidents => "Incidents",
            Self::IssueRadar => "Issue Radar",
            Self::Notifications => "Notifications",
            Self::Onboarding => "Onboarding",
            Self::Organizations => "Organizations",
            Self::Outreach => "Outreach",
            Self::Search => "Search",
            Self::Settings => "Settings",
            Self::SupportGraph => "Support Graph",
            Self::SyncHealth => "Sync Health",
        }
    }

    #[must_use]
    pub fn all() -> [Self; 23] {
        [
            Self::Dashboard,
            Self::Inbox,
            Self::LiveChats,
            Self::Operations,
            Self::Customers,
            Self::Organizations,
            Self::Knowledge,
            Self::Reports,
            Self::IssueRadar,
            Self::Incidents,
            Self::CustomObjects,
            Self::Automation,
            Self::AiCenter,
            Self::Outreach,
            Self::Notifications,
            Self::Search,
            Self::SupportGraph,
            Self::Connectors,
            Self::Diagnostics,
            Self::SyncHealth,
            Self::Settings,
            Self::Onboarding,
            Self::Shell,
        ]
    }
}

mod json_fields;
mod queries;

pub(crate) use json_fields::{i64_field, opt_str_field, str_field};
pub(crate) use queries::invalidated_pages;

/// UX-14: shared relative-time formatter (moved from the notifications page
/// so every page can render ISO stamps in human terms — "2m ago", not
/// `2026-11-05T22:56:25.925Z`).
pub(crate) fn relative_time(created_at: &str) -> String {
    let now = spp_core::business_hours::now_iso_millis();
    let Some(minutes) = spp_core::business_hours::wall_minutes_between(created_at, &now) else {
        return created_at.to_string();
    };
    match calendar_day_diff(created_at, &now) {
        Some(0) => format_intraday_minutes(minutes),
        Some(-1) => "Yesterday".to_string(),
        Some(days) if days < -1 => format!("{}d ago", -days),
        _ => created_at.to_string(),
    }
}

pub(crate) fn format_intraday_minutes(minutes: f64) -> String {
    if minutes < 1.0 {
        "just now".to_string()
    } else if minutes < 60.0 {
        format!("{}m ago", minutes as i64)
    } else {
        format!("{}h ago", (minutes / 60.0) as i64)
    }
}

/// UX-22: human-readable byte sizes — "2.1 MB", not "2145280 bytes" —
/// for user-facing metadata like backup files.
pub(crate) fn human_bytes(bytes: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 0 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn calendar_day_diff(a_iso: &str, b_iso: &str) -> Option<i64> {
    let (ay, am, ad) = date_parts(a_iso)?;
    let (by, bm, bd) = date_parts(b_iso)?;
    Some(days_from_civil(ay, am, ad) - days_from_civil(by, bm, bd))
}

fn date_parts(iso: &str) -> Option<(i64, i64, i64)> {
    let mut it = iso.get(..10)?.split('-');
    Some((
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
    ))
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe
}

#[cfg(test)]
mod tests {
    use super::json_fields::{i64_field, opt_str_field, str_field};
    use super::queries::invalidated_pages;
    use super::*;

    #[test]
    fn sync_without_rows_keeps_knowledge_clean() {
        let pages = invalidated_pages(&ServerEvent::sync_completed(
            "incremental",
            0,
            0,
            spp_core::events::SyncTrigger::Auto,
        ));
        assert!(pages.contains(&PageKey::Dashboard));
        assert!(pages.contains(&PageKey::Inbox));
        assert!(pages.contains(&PageKey::Knowledge));
    }

    #[test]
    fn r5_every_cacheable_page_is_invalidated_by_something() {
        let mut covered: Vec<PageKey> = Vec::new();
        let probes = vec![
            ServerEvent::sync_completed("incremental", 0, 0, spp_core::events::SyncTrigger::Auto),
            ServerEvent::sync_completed("initial", 5, 0, spp_core::events::SyncTrigger::Manual),
            ServerEvent::conversation_updated(None, None, None, None, "probe"),
            ServerEvent::sync_progress("probe", 0, 1, 0, 0),
            ServerEvent::outbound_job_finished(1, "create_reply", None, true, None),
            ServerEvent::rating_received(None, None, None, None, None, None),
            ServerEvent::notification_received(
                1, "mention", "info", "probe", None, None, None, None, 0,
            ),
            ServerEvent::error_occurred("error", "probe", "sync", None),
            ServerEvent::error_occurred("warn", "probe", "docs", None),
        ];
        for probe in &probes {
            for page in invalidated_pages(probe) {
                if !covered.contains(&page) {
                    covered.push(page);
                }
            }
        }
        for key in PageKey::all() {
            if key == PageKey::Shell {
                continue;
            }
            assert!(
                covered.contains(&key),
                "page {} is subscribed to no event at all (D19)",
                key.label()
            );
        }
    }

    #[test]
    fn r5_error_occurred_invalidates_diagnostics_and_the_service_surface() {
        let pages = invalidated_pages(&ServerEvent::error_occurred("error", "probe", "sync", None));
        assert!(pages.contains(&PageKey::Diagnostics));
        assert!(pages.contains(&PageKey::SyncHealth));
        let docs = invalidated_pages(&ServerEvent::error_occurred("warn", "probe", "docs", None));
        assert!(docs.contains(&PageKey::Knowledge));
        let connect = invalidated_pages(&ServerEvent::error_occurred(
            "warn", "probe", "connect", None,
        ));
        assert!(connect.contains(&PageKey::Settings));
    }

    #[test]
    fn incremental_sync_with_rows_invalidates_knowledge() {
        let event = ServerEvent::SyncCompleted(spp_core::events::SyncCompletedEvent {
            kind: "incremental".into(),
            processed: 3,
            errors: 0,
            trigger: spp_core::events::SyncTrigger::Auto,
            at: "2026-01-01T00:00:00Z".into(),
        });
        let pages = invalidated_pages(&event);
        assert!(pages.contains(&PageKey::Knowledge));
    }

    #[test]
    fn value_field_helpers() {
        let v = serde_json::json!({"name": "Kim", "n": 5, "empty": "", "null": null});
        assert_eq!(str_field(&v, "name"), "Kim");
        assert_eq!(i64_field(&v, "n"), 5);
        assert_eq!(opt_str_field(&v, "empty"), None);
        assert_eq!(opt_str_field(&v, "null"), None);
        assert_eq!(opt_str_field(&v, "missing"), None);
        assert_eq!(opt_str_field(&v, "name").as_deref(), Some("Kim"));
    }

    /// UX-22: metadata reads in user units, with the byte count exact for
    /// small files and one decimal past KB.
    #[test]
    fn human_bytes_renders_user_facing_sizes() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2_145_280), "2.0 MB");
        assert_eq!(human_bytes(1536), "1.5 KB");
        assert_eq!(human_bytes(3 * 1024 * 1024 * 1024), "3.0 GB");
        assert_eq!(human_bytes(-4), "-4 B");
    }

    #[test]
    fn confirm_requests_carry_the_dialog_spec_and_action() {
        let request = ConfirmRequest::delete_automation_rule(7, "Escalate VIP");
        assert_eq!(request.title, "Delete automation rule?");
        assert_eq!(request.confirm_label, "Delete rule");
        assert!(request.danger, "deleting a rule is a destructive action");
        assert!(request.message.contains("Escalate VIP"));
        assert_eq!(
            request.action,
            DestructiveAction::DeleteAutomationRule { id: 7 }
        );
        let dialog = request.dialog();
        assert!(dialog.danger);
        assert_eq!(dialog.title, request.title);
        assert_eq!(dialog.confirm_label, "Delete rule");
        let nav = NavRequest::Confirm(request);
        let NavRequest::Confirm(inner) = &nav else {
            panic!("the Confirm variant must pattern-match");
        };
        assert_eq!(
            inner.action,
            DestructiveAction::DeleteAutomationRule { id: 7 }
        );
    }
}
