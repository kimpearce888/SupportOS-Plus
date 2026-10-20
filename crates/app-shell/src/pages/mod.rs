//! Ported pages: dashboard, inbox, customers, knowledge/docs, reports,
//! operations, and the remaining Phase 3 surface (ai center, automation,
//! connectors, custom objects, incidents, issue radar, notifications,
//! onboarding, organizations, outreach, search, settings, support graph,
//! sync health) plus the shell-level surfaces (command palette, overlays,
//! toasts).
//!
//! Each page owns a typed model (queries run against the local database
//! through [`spp_core`] functions and small typed SQL ports of the axum
//! route handlers) plus an egui view. Live updates follow the same
//! invalidation semantics as the web UI: a [`ServerEvent`] marks affected
//! pages dirty, and the next frame re-queries them
//! (port of `queries::keys_for_event` from the web crate).

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

/// A cross-page navigation request raised by a page view (port of the
/// web router links that jumped between pages, e.g. an operations tile
/// linking `/inbox?view={tile}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavRequest {
    /// Open the inbox filtered by an operations tile key (`unassigned`, …).
    InboxTile(String),
    /// Open a specific page.
    Page(PageKey),
}

/// A navigable page of the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKey {
    /// Shell diagnostics (theme, database status) — the Phase 2 surface.
    Shell,
    Dashboard,
    Diagnostics,
    Inbox,
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
    /// Sidebar label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Shell => "Shell",
            Self::Dashboard => "Dashboard",
            Self::Diagnostics => "Diagnostics",
            Self::Inbox => "Inbox",
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

    /// Sidebar order (port of the web nav: work queues first, reference
    /// pages last).
    #[must_use]
    pub fn all() -> [Self; 22] {
        [
            Self::Dashboard,
            Self::Inbox,
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

/// Pages that must reload when a background event arrives.
///
/// Port of the web `queries::keys_for_event` invalidation map:
/// * `ratings` → dashboard + customer views,
/// * `sync` → conversations, conversation, dashboard, nav-counts,
///   operations-center (+ docs on incremental rows),
/// * `conversation` → inbox, dashboard, operations (center + workload),
/// * `notification` → notifications, mention queue, inbox, operations,
/// * `campaign` → outreach campaigns.
#[must_use]
pub fn invalidated_pages(event: &ServerEvent) -> Vec<PageKey> {
    match event {
        ServerEvent::RatingReceived(_) | ServerEvent::RatingsRefreshed(_) => {
            vec![PageKey::Dashboard, PageKey::Customers]
        }
        ServerEvent::SyncCompleted(e) => {
            let mut pages = vec![
                PageKey::Inbox,
                PageKey::Dashboard,
                PageKey::Operations,
                PageKey::SyncHealth,
                PageKey::Notifications,
            ];
            if e.kind == "incremental" && e.processed > 0 {
                pages.push(PageKey::Knowledge);
            }
            pages
        }
        ServerEvent::ConversationUpdated(_) => vec![
            PageKey::Inbox,
            PageKey::Dashboard,
            PageKey::Operations,
            PageKey::Notifications,
        ],
        ServerEvent::NotificationReceived(_) => {
            vec![PageKey::Inbox, PageKey::Notifications, PageKey::Operations]
        }
        ServerEvent::CampaignUpdated(_) => vec![PageKey::Outreach],
        ServerEvent::ErrorOccurred(_) => Vec::new(),
    }
}

/// Read a string field out of a [`serde_json::Value`] object.
pub(crate) fn str_field(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|f| f.as_str())
        .unwrap_or_default()
        .to_string()
}

/// Read an integer field out of a [`serde_json::Value`] object.
pub(crate) fn i64_field(v: &serde_json::Value, key: &str) -> i64 {
    v.get(key).and_then(|f| f.as_i64()).unwrap_or_default()
}

/// Read an optional string field, `None` when null/absent/empty.
pub(crate) fn opt_str_field(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|f| f.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_without_rows_keeps_knowledge_clean() {
        let pages = invalidated_pages(&ServerEvent::sync_completed("incremental", 0, 0));
        assert!(pages.contains(&PageKey::Dashboard));
        assert!(pages.contains(&PageKey::Inbox));
        assert!(!pages.contains(&PageKey::Knowledge));
    }

    #[test]
    fn incremental_sync_with_rows_invalidates_knowledge() {
        let event = ServerEvent::SyncCompleted(spp_core::events::SyncCompletedEvent {
            kind: "incremental".into(),
            processed: 3,
            errors: 0,
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
}
