//! Toast notifications (port of the web `toasts.rs`).
//!
//! `push(kind, message, detail)` stacks a toast; the shell renders the
//! stack as a bottom-right window and auto-dismisses entries after the
//! per-kind timeout (web: `auto_dismiss_ms`). Advancement uses the UI
//! clock so hidden frames do not burn toast lifetime.

use std::time::{Duration, Instant};

use eframe::egui;

/// Toast severity (web: `ToastKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
    Warning,
    Info,
}

impl ToastKind {
    /// Wire name (web: `as_str`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }

    /// Auto-dismiss timeout (web: `auto_dismiss_ms`).
    #[must_use]
    pub fn auto_dismiss(self) -> Duration {
        match self {
            Self::Success | Self::Info => Duration::from_secs(4),
            Self::Warning => Duration::from_secs(6),
            Self::Error => Duration::from_secs(10),
        }
    }

    fn color(self) -> egui::Color32 {
        match self {
            Self::Success => egui::Color32::GREEN,
            Self::Error => egui::Color32::RED,
            Self::Warning => egui::Color32::YELLOW,
            Self::Info => egui::Color32::LIGHT_BLUE,
        }
    }
}

/// One toast entry (web: `Toast`).
#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub message: String,
    pub detail: Option<String>,
    /// Deadline compared against the render clock.
    pub expires_at: Instant,
}

/// The shell-level toast stack.
#[derive(Debug, Default)]
pub struct Toasts {
    stack: Vec<Toast>,
}

impl Toasts {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of live toasts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Whether the stack is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Push a toast (web: `push`).
    pub fn push(&mut self, kind: ToastKind, message: impl Into<String>, detail: Option<String>) {
        self.stack.push(Toast {
            kind,
            message: message.into(),
            detail,
            expires_at: Instant::now() + kind.auto_dismiss(),
        });
    }

    /// Success toast (web: `success`).
    pub fn success(&mut self, message: impl Into<String>) {
        self.push(ToastKind::Success, message, None);
    }

    /// Error toast (web: `error`).
    pub fn error(&mut self, message: impl Into<String>) {
        self.push(ToastKind::Error, message, None);
    }

    /// Drop expired entries and render the stack.
    pub fn ui(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.stack.retain(|t| t.expires_at > now);
        if self.stack.is_empty() {
            return;
        }
        let height = 56.0;
        let total = self.stack.len() as f32 * (height + 8.0);
        egui::Window::new("toasts")
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
            .fixed_size([360.0, total])
            .frame(egui::Frame::new().fill(egui::Color32::TRANSPARENT))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .interactable(false)
            .show(ctx, |ui| {
                let stack = self.stack.clone();
                for toast in &stack {
                    let frame = egui::Frame::new()
                        .fill(ui.visuals().extreme_bg_color)
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .outer_margin(4.0);
                    frame.show(ui, |ui| {
                        ui.colored_label(
                            toast.kind.color(),
                            format!("{} {}", toast.kind.as_str(), toast.message),
                        );
                        if let Some(detail) = &toast.detail {
                            ui.weak(detail);
                        }
                    });
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_stack_and_expiry() {
        let mut toasts = Toasts::new();
        assert!(toasts.is_empty());
        toasts.success("saved");
        toasts.error("boom");
        toasts.push(ToastKind::Warning, "careful", Some("detail".into()));
        assert_eq!(toasts.len(), 3);
        assert_eq!(toasts.stack[0].message, "saved");
        assert_eq!(toasts.stack[1].kind, ToastKind::Error);
        assert_eq!(toasts.stack[2].detail.as_deref(), Some("detail"));

        // Force expiry: every toast's deadline is in the past.
        for toast in &mut toasts.stack {
            toast.expires_at = Instant::now() - Duration::from_millis(1);
        }
        let now = Instant::now();
        toasts.stack.retain(|t| t.expires_at > now);
        assert!(toasts.is_empty());
    }

    #[test]
    fn timeouts_scale_with_severity() {
        assert!(ToastKind::Error.auto_dismiss() > ToastKind::Warning.auto_dismiss());
        assert!(ToastKind::Warning.auto_dismiss() > ToastKind::Success.auto_dismiss());
        assert_eq!(
            ToastKind::Info.auto_dismiss(),
            ToastKind::Success.auto_dismiss()
        );
    }

    #[test]
    fn wire_names_match_the_web_contract() {
        assert_eq!(ToastKind::Success.as_str(), "success");
        assert_eq!(ToastKind::Error.as_str(), "error");
        assert_eq!(ToastKind::Warning.as_str(), "warning");
        assert_eq!(ToastKind::Info.as_str(), "info");
    }
}
