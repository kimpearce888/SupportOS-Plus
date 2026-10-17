use rusqlite::Connection;

use crate::catalog::NotificationType;
use crate::error::Result;
use crate::settings;

const PREF_KEY_PREFIX: &str = "notifications.";
const PREF_KEY_SUFFIX: &str = ".enabled";

fn pref_key(notif_type: NotificationType) -> String {
    format!("{PREF_KEY_PREFIX}{}{PREF_KEY_SUFFIX}", notif_type.as_str())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPreference {
    Enabled,
    Disabled,
}

impl NotificationPreference {
    #[must_use]
    pub fn as_bool(self) -> bool {
        match self {
            Self::Enabled => true,
            Self::Disabled => false,
        }
    }

    #[must_use]
    pub fn from_bool(b: bool) -> Self {
        if b { Self::Enabled } else { Self::Disabled }
    }
}

pub fn get_user_preference(
    conn: &Connection,
    _user_id: i64,
    notif_type: NotificationType,
) -> Result<NotificationPreference> {
    let key = pref_key(notif_type);
    let default = notif_type.default_enabled();
    let stored = settings::get_bool(conn, &key, default)?;
    Ok(NotificationPreference::from_bool(stored))
}

pub fn set_user_preference(
    conn: &Connection,
    _user_id: i64,
    notif_type: NotificationType,
    preference: NotificationPreference,
) -> Result<()> {
    let key = pref_key(notif_type);
    settings::set_bool(conn, &key, preference.as_bool())
}

pub fn filter_by_preferences(
    conn: &Connection,
    _user_id: i64,
    types: &[NotificationType],
) -> Result<Vec<NotificationType>> {
    let mut filtered = Vec::with_capacity(types.len());
    for &t in types {
        if let NotificationPreference::Enabled = get_user_preference(conn, _user_id, t)? {
            filtered.push(t);
        }
    }
    Ok(filtered)
}
