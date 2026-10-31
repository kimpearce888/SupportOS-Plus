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
