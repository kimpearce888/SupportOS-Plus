use super::*;

/// Pages that must reload when a background event arrives.
///
/// Port of the web `queries::keys_for_event` invalidation map, made
/// **complete** in v1.4.0 (R5, D19): before, eleven pages (Incidents,
/// Issue Radar, AI Center, Automation, Connectors, Custom Objects,
/// Onboarding, Organizations, Search, Settings, Support Graph) were
/// subscribed to nothing at all, so their cached snapshots went stale
/// for the whole session. Now every page that renders mirror data is
/// invalidated by the events that change mirror data. Invalidation is
/// cheap (it clears a cached model; only the visible page re-queries),
/// so erring on the side of freshness costs nothing.
#[must_use]
pub fn invalidated_pages(event: &ServerEvent) -> Vec<PageKey> {
    match event {
        ServerEvent::RatingReceived(_) => {
            vec![PageKey::Dashboard, PageKey::Customers, PageKey::Reports]
        }
        ServerEvent::SyncCompleted(_) => vec![
            // A finished sync can touch any mirrored table.
            PageKey::Inbox,
            PageKey::LiveChats,
            PageKey::Dashboard,
            PageKey::Operations,
            PageKey::Reports,
            PageKey::SyncHealth,
            PageKey::Notifications,
            PageKey::Knowledge,
            PageKey::Customers,
            PageKey::Organizations,
            PageKey::Search,
            PageKey::IssueRadar,
            PageKey::Incidents,
            PageKey::CustomObjects,
            PageKey::SupportGraph,
            PageKey::AiCenter,
            PageKey::Onboarding,
            PageKey::Automation,
            PageKey::Settings,
            PageKey::Connectors,
        ],
        ServerEvent::ConversationUpdated(_) => vec![
            PageKey::Inbox,
            PageKey::LiveChats,
            PageKey::Dashboard,
            PageKey::Operations,
            PageKey::Reports,
            PageKey::Notifications,
            PageKey::Customers,
            PageKey::Search,
            PageKey::SupportGraph,
            PageKey::IssueRadar,
            PageKey::Incidents,
            PageKey::CustomObjects,
            PageKey::AiCenter,
        ],
        ServerEvent::SyncProgress(_) => vec![PageKey::SyncHealth],
        ServerEvent::OutboundJobFinished(_) => {
            // A reply/note/status change reached its terminal state: the
            // inbox re-reads the mirror (the dispatcher pulled the remote
            // truth back) and Diagnostics picks up the audit trail.
            vec![PageKey::Inbox, PageKey::Diagnostics]
        }
        ServerEvent::NotificationReceived(_) => {
            vec![PageKey::Inbox, PageKey::Notifications, PageKey::Operations]
        }
        ServerEvent::CampaignUpdated(_) => vec![PageKey::Outreach],
        ServerEvent::ErrorOccurred(e) => {
            // R5 (D4): background failures now invalidate the pages that
            // can explain them, plus service-specific surfaces.
            let mut pages = vec![PageKey::Diagnostics, PageKey::SyncHealth];
            match e.service.as_str() {
                "docs" => pages.push(PageKey::Knowledge),
                "connect" => pages.push(PageKey::Settings),
                "jobs" => pages.push(PageKey::AiCenter),
                _ => {}
            }
            pages
        }
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
