//! Theme persistence in the core `settings` table (plan: app-shell).
//!
//! The old webview UI kept its theme in browser localStorage, which does
//! not exist in a native shell. Per decision D5 there is no migration:
//! the setting starts at the default and the user re-picks it; the value
//! lives in the `settings` table so it survives restarts.

use eframe::egui;
use spp_core::db::DbHandle;

/// UI theme choice persisted under [`Theme::SETTING_KEY`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Theme {
    /// Light visuals (default; matches the old API's `default: "light"`).
    #[default]
    Light,
    /// Dark visuals.
    Dark,
}

impl Theme {
    /// Settings-table key holding `"light"` or `"dark"`.
    pub const SETTING_KEY: &'static str = "theme";

    /// Map a stored setting value to a theme; anything unknown (including
    /// a missing value) falls back to the default.
    #[must_use]
    pub fn from_setting_value(value: Option<&str>) -> Self {
        match value {
            Some("dark") => Self::Dark,
            _ => Self::Light,
        }
    }

    /// The value stored for this theme.
    #[must_use]
    pub fn setting_value(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// Load the theme from the `settings` table (default on any error).
    #[must_use]
    pub fn load(db: &DbHandle) -> Self {
        let stored: Option<String> = db.with_read(|conn| {
            spp_core::settings::get_string(conn, Self::SETTING_KEY)
                .ok()
                .flatten()
        });
        Self::from_setting_value(stored.as_deref())
    }

    /// Persist the theme to the `settings` table.
    ///
    /// # Errors
    /// Propagates SQLite errors from the settings write.
    pub fn store(self, db: &DbHandle) -> spp_core::error::Result<()> {
        db.with_write(|conn| {
            spp_core::settings::set_string(conn, Self::SETTING_KEY, self.setting_value())
        })
    }

    #[must_use]
    pub fn toggle(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }

    /// Apply the theme to an egui context.
    pub fn apply(self, ctx: &egui::Context) {
        ctx.set_theme(match self {
            Self::Light => egui::Theme::Light,
            Self::Dark => egui::Theme::Dark,
        });
    }

    /// Short label for UI display.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> DbHandle {
        let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap migrations");
        DbHandle::new(conn)
    }

    #[test]
    fn default_is_light_when_unset() {
        let db = test_db();
        assert_eq!(Theme::load(&db), Theme::Light);
    }

    #[test]
    fn unknown_value_falls_back_to_light() {
        assert_eq!(Theme::from_setting_value(Some("blue")), Theme::Light);
        assert_eq!(Theme::from_setting_value(Some("")), Theme::Light);
    }

    #[test]
    fn store_then_load_roundtrips() {
        let db = test_db();
        Theme::Dark.store(&db).expect("store dark");
        assert_eq!(Theme::load(&db), Theme::Dark);
        Theme::Light.store(&db).expect("store light");
        assert_eq!(Theme::load(&db), Theme::Light);
    }

    #[test]
    fn apply_sets_egui_theme() {
        let ctx = egui::Context::default();
        Theme::Dark.apply(&ctx);
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        Theme::Light.apply(&ctx);
        assert_eq!(ctx.theme(), egui::Theme::Light);
    }

    #[test]
    fn toggle_flips() {
        assert_eq!(Theme::Light.toggle(), Theme::Dark);
        assert_eq!(Theme::Dark.toggle(), Theme::Light);
    }
}
