use std::collections::HashMap;
use std::sync::OnceLock;

use leptos::prelude::*;

pub const KEYS: [&str; 13] = [
    "dashboard",
    "customer",
    "interaction-profile",
    "conversations",
    "conversation",
    "nav-counts",
    "operations-center",
    "operations-workload",
    "docs",
    "outreach-campaigns",
    "notifications",
    "notification-unread",
    "mention-queue",
];

static COUNTERS: OnceLock<HashMap<&'static str, RwSignal<u32>>> = OnceLock::new();

pub fn init() {
    let mut map = HashMap::new();
    for key in KEYS {
        map.insert(key, RwSignal::new(0u32));
    }
    let _ = COUNTERS.set(map);
}

#[must_use]
pub fn version(key: &str) -> RwSignal<u32> {
    let counters = COUNTERS.get().unwrap_or_else(|| {
        panic!("queries::init() must run in app_view before any page reads a version")
    });
    *counters
        .get(key)
        .unwrap_or_else(|| panic!("unknown query key: {key} (not in KEYS)"))
}

pub fn invalidate(key: &str) {
    let Some(counters) = COUNTERS.get() else {
        return;
    };
    if let Some(signal) = counters.get(key) {
        signal.update(|v| *v = v.wrapping_add(1));
    }
}

#[must_use]
pub fn keys_for_event(
    event: &str,
    sync_kind: Option<&str>,
    sync_processed: u32,
) -> Vec<&'static str> {
    match event {
        "ratings" => vec!["dashboard", "customer", "interaction-profile"],
        "sync" => {
            let mut keys = vec![
                "conversations",
                "conversation",
                "dashboard",
                "nav-counts",
                "operations-center",
            ];
            if sync_kind == Some("incremental") && sync_processed > 0 {
                keys.push("docs");
            }
            keys
        }
        "conversation" => vec![
            "conversations",
            "conversation",
            "nav-counts",
            "dashboard",
            "operations-center",
            "operations-workload",
        ],
        "campaign" => vec!["outreach-campaigns"],
        "notification" => vec![
            "notifications",
            "notification-unread",
            "mention-queue",
            "operations-center",
            "operations-workload",
        ],
        _ => Vec::new(),
    }
}
