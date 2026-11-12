use super::*;

#[must_use]
pub(crate) fn invalidated_pages(event: &ServerEvent) -> Vec<PageKey> {
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
            PageKey::Outreach,
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
        ServerEvent::ConnectorRefreshed(_) => vec![PageKey::Connectors],
        ServerEvent::NotificationReceived(_) => {
            vec![PageKey::Inbox, PageKey::Notifications, PageKey::Operations]
        }
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
