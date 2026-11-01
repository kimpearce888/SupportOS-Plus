use super::*;

#[must_use]
pub fn invalidated_pages(event: &ServerEvent) -> Vec<PageKey> {
    match event {
        ServerEvent::RatingReceived(_) => {
            vec![PageKey::Dashboard, PageKey::Customers, PageKey::Reports]
        }
        ServerEvent::SyncCompleted(_) => vec![
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
            vec![PageKey::Inbox, PageKey::Diagnostics]
        }
        ServerEvent::NotificationReceived(_) => {
            vec![PageKey::Inbox, PageKey::Notifications, PageKey::Operations]
        }
        ServerEvent::CampaignUpdated(_) => vec![PageKey::Outreach],
        ServerEvent::ErrorOccurred(e) => {
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

pub(crate) fn str_field(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|f| f.as_str())
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn i64_field(v: &serde_json::Value, key: &str) -> i64 {
    v.get(key).and_then(|f| f.as_i64()).unwrap_or_default()
}

pub(crate) fn opt_str_field(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|f| f.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}
