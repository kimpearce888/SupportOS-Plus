#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    #[must_use]
    pub fn class_suffix(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }

    #[must_use]
    pub fn color_var(self) -> &'static str {
        match self {
            Self::Info => "var(--spp-info)",
            Self::Warning => "var(--spp-warning)",
            Self::Critical => "var(--spp-critical)",
        }
    }
}
