use leptos::*;

pub const THEME_STORAGE_KEY: &str = "supportos-theme";

pub const SIDEBAR_STORAGE_KEY: &str = "supportos-sidebar";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    #[must_use]
    pub fn from_stored(stored: Option<&str>) -> Self {
        match stored {
            Some("dark") => Self::Dark,
            _ => Self::Light,
        }
    }

    #[must_use]
    pub fn toggle(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }

    pub fn apply(self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            if let Some(html) = doc.document_element() {
                let _ = html.set_attribute("data-theme", self.as_str());
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self;
    }
}

fn local_storage_get(key: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(key).ok().flatten())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = key;
        None
    }
}

fn local_storage_set(key: &str, value: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(key, value);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (key, value);
}

pub fn apply_theme_at_boot() {
    Theme::from_stored(local_storage_get(THEME_STORAGE_KEY).as_deref()).apply();
}

#[derive(Clone, Copy)]
pub struct UiState {
    pub theme: RwSignal<Theme>,
    pub sidebar_collapsed: RwSignal<bool>,
    pub palette_open: RwSignal<bool>,
    pub onboarding_completed: RwSignal<Option<bool>>,
    pub inbox_count: RwSignal<Option<u32>>,
    pub unread_count: RwSignal<Option<u32>>,
}

impl UiState {
    #[must_use]
    pub fn create() -> Self {
        let theme = Theme::from_stored(local_storage_get(THEME_STORAGE_KEY).as_deref());
        let collapsed = local_storage_get(SIDEBAR_STORAGE_KEY).as_deref() == Some("collapsed");
        Self {
            theme: create_rw_signal(theme),
            sidebar_collapsed: create_rw_signal(collapsed),
            palette_open: create_rw_signal(false),
            onboarding_completed: create_rw_signal(None),
            inbox_count: create_rw_signal(None),
            unread_count: create_rw_signal(None),
        }
    }

    pub fn toggle_theme(&self) {
        let next = self.theme.get().toggle();
        self.theme.set(next);
        local_storage_set(THEME_STORAGE_KEY, next.as_str());
        next.apply();
    }

    pub fn toggle_sidebar(&self) {
        let next = !self.sidebar_collapsed.get();
        self.sidebar_collapsed.set(next);
        local_storage_set(SIDEBAR_STORAGE_KEY, if next { "collapsed" } else { "open" });
    }

    pub fn refresh_nav_counts(&self) {
        let inbox = self.inbox_count;
        let unread = self.unread_count;
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(v) = crate::api::get_json::<serde_json::Value>(
                "/api/conversations?view=active&pageSize=1",
            )
            .await
            {
                if let Some(total) = v.get("total").and_then(serde_json::Value::as_u64) {
                    inbox.set(Some(u32::try_from(total).unwrap_or(u32::MAX)));
                }
            }
            if let Ok(v) =
                crate::api::get_json::<serde_json::Value>("/api/notifications/unread-count").await
            {
                if let Some(count) = v.get("unread").and_then(serde_json::Value::as_u64) {
                    unread.set(Some(u32::try_from(count).unwrap_or(u32::MAX)));
                }
            }
        });
    }
}

#[must_use]
pub fn badge_text(count: u32) -> Option<String> {
    if count == 0 {
        None
    } else if count > 99 {
        Some("99+".to_string())
    } else {
        Some(count.to_string())
    }
}
