use eframe::egui;
use spp_core::db::DbHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

impl Theme {
    pub(crate) const SETTING_KEY: &'static str = "theme";

    #[must_use]
    pub(crate) fn from_setting_value(value: Option<&str>) -> Self {
        match value {
            Some("dark") => Self::Dark,
            _ => Self::Light,
        }
    }

    #[must_use]
    pub(crate) fn setting_value(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    #[must_use]
    pub(crate) fn load(db: &DbHandle) -> Self {
        let stored: Option<String> = db.with_read(|conn| {
            spp_core::settings::get_string(conn, Self::SETTING_KEY)
                .ok()
                .flatten()
        });
        Self::from_setting_value(stored.as_deref())
    }

    pub fn store(self, db: &DbHandle) -> spp_core::error::Result<()> {
        db.with_write(|conn| {
            spp_core::settings::set_string(conn, Self::SETTING_KEY, self.setting_value())
        })
    }

    #[must_use]
    pub(crate) fn toggle(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }

    pub fn apply(self, ctx: &egui::Context) {
        ctx.set_theme(match self {
            Self::Light => egui::Theme::Light,
            Self::Dark => egui::Theme::Dark,
        });
    }

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
