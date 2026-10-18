//! Ported pages (Phase 3): dashboard, inbox, customers, knowledge/docs,
//! reports.
//!
//! Each page owns a typed model (queries run against the local database
//! through [`spp_core`] functions and small typed SQL ports of the axum
//! route handlers) plus an egui view. Live updates follow the same
//! invalidation semantics as the web UI: a [`ServerEvent`] marks affected
//! pages dirty, and the next frame re-queries them
//! (port of `queries::keys_for_event` from the web crate).

pub mod customers;
pub mod dashboard;
pub mod inbox;
pub mod knowledge;
pub mod reports;

use spp_core::events::ServerEvent;

/// A navigable page of the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKey {
    /// Shell diagnostics (theme, database status) — the Phase 2 surface.
    Shell,
    Dashboard,
    Inbox,
    Customers,
    Knowledge,
    Reports,
}

impl PageKey {
    /// Sidebar label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Shell => "Shell",
            Self::Dashboard => "Dashboard",
            Self::Inbox => "Inbox",
            Self::Customers => "Customers",
            Self::Knowledge => "Knowledge",
            Self::Reports => "Reports",
        }
    }

    /// Sidebar order.
    #[must_use]
    pub fn all() -> [Self; 6] {
        [
            Self::Dashboard,
            Self::Inbox,
            Self::Customers,
            Self::Knowledge,
            Self::Reports,
            Self::Shell,
        ]
    }
}

/// Pages that must reload when a background event arrives.
///
/// Port of the web `queries::keys_for_event` invalidation map:
/// * `ratings` → dashboard + customer views,
/// * `sync` → conversations, conversation, dashboard, nav-counts
///   (→ inbox + dashboard here), plus docs when an incremental sync
///   actually processed rows,
/// * `conversation` → inbox + dashboard,
/// * `notification` → mention queue + inbox,
/// * `campaign` → outreach page (not ported yet).
#[must_use]
pub fn invalidated_pages(event: &ServerEvent) -> Vec<PageKey> {
    match event {
        ServerEvent::RatingReceived(_) | ServerEvent::RatingsRefreshed(_) => {
            vec![PageKey::Dashboard, PageKey::Customers]
        }
        ServerEvent::SyncCompleted(e) => {
            let mut pages = vec![PageKey::Inbox, PageKey::Dashboard];
            if e.kind == "incremental" && e.processed > 0 {
                pages.push(PageKey::Knowledge);
            }
            pages
        }
        ServerEvent::ConversationUpdated(_) => vec![PageKey::Inbox, PageKey::Dashboard],
        ServerEvent::NotificationReceived(_) => vec![PageKey::Inbox],
        ServerEvent::CampaignUpdated(_) | ServerEvent::ErrorOccurred(_) => Vec::new(),
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
