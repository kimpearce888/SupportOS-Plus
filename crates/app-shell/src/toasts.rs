use std::time::{Duration, Instant};

use eframe::egui;

use crate::design;
use crate::icons::Icon;

const MAX_PASS_GAP: Duration = Duration::from_millis(1600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToastKind {
    Success,
    Error,
    Warning,
    Info,
}

impl ToastKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }

    #[must_use]
    pub(crate) fn auto_dismiss(self) -> Duration {
        match self {
            Self::Success | Self::Info => Duration::from_secs(4),
            Self::Warning => Duration::from_secs(6),
            Self::Error => Duration::from_secs(10),
        }
    }

    fn visuals(self, t: &design::Tokens) -> (Icon, egui::Color32) {
        match self {
            Self::Success => (Icon::Check, t.success_foreground),
            Self::Error => (Icon::AlertCircle, t.destructive_text),
            Self::Warning => (Icon::AlertTriangle, t.warning_foreground),
            Self::Info => (Icon::Info, t.info_foreground),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Toast {
    pub kind: ToastKind,
    pub message: String,
    pub detail: Option<String>,
    pub remaining: Duration,
    pub last_paint: Option<Instant>,
}

#[derive(Debug, Default)]
pub(crate) struct Toasts {
    stack: Vec<Toast>,
}

impl Toasts {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, kind: ToastKind, message: impl Into<String>, detail: Option<String>) {
        self.stack.push(Toast {
            kind,
            message: message.into(),
            detail,
            remaining: kind.auto_dismiss(),
            last_paint: None,
        });
    }

    pub fn success(&mut self, message: impl Into<String>) {
        self.push(ToastKind::Success, message, None);
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.push(ToastKind::Error, message, None);
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        for toast in &mut self.stack {
            if let Some(last) = toast.last_paint {
                let gap = now.saturating_duration_since(last).min(MAX_PASS_GAP);
                toast.remaining = toast.remaining.saturating_sub(gap);
            }
            toast.last_paint = Some(now);
        }
        self.stack.retain(|t| t.remaining > Duration::ZERO);
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
                let t = design::tokens_for(ctx);
                let stack = self.stack.clone();
                for toast in &stack {
                    let (icon, tint) = toast.kind.visuals(t);
                    let frame = egui::Frame::new()
                        .fill(t.card)
                        .stroke(egui::Stroke::new(1.0, t.border))
                        .corner_radius(10)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .shadow(egui::Shadow {
                            offset: [0, 4],
                            blur: 16,
                            spread: 0,
                            color: t.shadow,
                        });
                    frame.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (id, rect) = ui.allocate_space(egui::vec2(17.0, 17.0));
                            icon.paint_at(ui.painter(), rect, tint);
                            ui.advance_cursor_after_rect(rect);
                            let _ = id;
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} {}",
                                        toast.kind.as_str(),
                                        toast.message
                                    ))
                                    .size(13.0)
                                    .strong()
                                    .color(t.foreground),
                                );
                                if let Some(detail) = &toast.detail {
                                    ui.label(
                                        egui::RichText::new(detail.as_str())
                                            .small()
                                            .color(t.muted_foreground),
                                    );
                                }
                            });
                        });
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
        assert!(toasts.stack.is_empty());
        toasts.success("saved");
        toasts.error("boom");
        toasts.push(ToastKind::Warning, "careful", Some("detail".into()));
        assert_eq!(toasts.stack.len(), 3);
        assert_eq!(toasts.stack[0].message, "saved");
        assert_eq!(toasts.stack[1].kind, ToastKind::Error);
        assert_eq!(toasts.stack[2].detail.as_deref(), Some("detail"));
        assert_eq!(toasts.stack[0].remaining, ToastKind::Success.auto_dismiss());
        assert!(toasts.stack[0].last_paint.is_none());

        for toast in &mut toasts.stack {
            toast.remaining = Duration::ZERO;
        }
        toasts.stack.retain(|t| t.remaining > Duration::ZERO);
        assert!(toasts.stack.is_empty());
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

    #[test]
    fn a_hidden_window_never_burns_unrendered_toast_lifetime() {
        let mut toasts = Toasts::new();
        toasts.error("sync failed while hidden");
        let ctx = egui::Context::default();
        ctx.run_pass(egui::RawInput::default(), |ctx| toasts.ui(ctx))
            .drop_without_applying_deltas();
        assert_eq!(
            toasts.stack.len(),
            1,
            "the error toast survives the hidden hour"
        );
        let remaining = toasts.stack[0].remaining;
        assert_eq!(
            remaining,
            ToastKind::Error.auto_dismiss(),
            "lifetime starts at the first rendered pass — the hidden gap burned nothing"
        );
    }

    #[test]
    fn a_hidden_gap_burns_at_most_one_heartbeat() {
        let mut toasts = Toasts::new();
        toasts.error("boom");
        let ctx = egui::Context::default();
        ctx.run_pass(egui::RawInput::default(), |ctx| toasts.ui(ctx))
            .drop_without_applying_deltas();
        for toast in &mut toasts.stack {
            toast.last_paint = Some(Instant::now() - Duration::from_secs(3600));
        }
        ctx.run_pass(egui::RawInput::default(), |ctx| toasts.ui(ctx))
            .drop_without_applying_deltas();
        assert_eq!(
            toasts.stack.len(),
            1,
            "the toast survives the return from hidden"
        );
        assert!(
            toasts.stack[0].remaining + MAX_PASS_GAP >= ToastKind::Error.auto_dismiss(),
            "at most one heartbeat burned: {} left of {}",
            toasts.stack[0].remaining.as_secs(),
            ToastKind::Error.auto_dismiss().as_secs()
        );
    }

    #[test]
    fn consecutive_rendered_passes_burn_real_lifetime() {
        let mut toasts = Toasts::new();
        toasts.success("saved");
        let ctx = egui::Context::default();
        ctx.run_pass(egui::RawInput::default(), |ctx| toasts.ui(ctx))
            .drop_without_applying_deltas();
        for _ in 0..3 {
            for toast in &mut toasts.stack {
                toast.last_paint = Some(Instant::now() - Duration::from_secs(2));
            }
            ctx.run_pass(egui::RawInput::default(), |ctx| toasts.ui(ctx))
                .drop_without_applying_deltas();
        }
        assert!(
            toasts.stack.is_empty(),
            "6 s of visible passes expire a 4 s toast"
        );
    }
}
