//! Toast notifications (port of the web `toasts.rs`).
//!
//! `push(kind, message, detail)` stacks a toast; the shell renders the
//! stack as a bottom-right window and auto-dismisses entries after the
//! per-kind timeout (web: `auto_dismiss_ms`). Advancement uses the UI
//! clock so hidden frames do not burn toast lifetime — eframe 0.37 runs
//! no ui pass at all while the window is hidden (only `logic`), so a
//! wall-clock deadline would expire toasts the user never saw: an
//! error raised while the window was hidden vanished silently the
//! moment it was shown again (clean-audit item 69). Lifetime therefore
//! starts at the first RENDERED pass and burns only across rendered
//! passes, with each pass's burn capped at one repaint heartbeat.

use std::time::{Duration, Instant};

use eframe::egui;

use crate::design;
use crate::icons::Icon;

/// The largest lifetime burn a single rendered pass may apply. Normal
/// visible gaps sit at or below the shell's 1.5 s repaint heartbeat, so
/// steady-state expiry keeps its wall-clock pace; a hidden window
/// (arbitrarily long gap between rendered passes) burns at most this
/// much on return and the toast still shows for (nearly) its lifetime.
const MAX_PASS_GAP: Duration = Duration::from_millis(1600);

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

    /// Icon + tint for the redesign (shadcn toast look: leading icon,
    /// strong message, muted detail).
    fn visuals(self, t: &design::Tokens) -> (Icon, egui::Color32) {
        match self {
            Self::Success => (Icon::Check, t.success_foreground),
            Self::Error => (Icon::AlertCircle, t.destructive_text),
            Self::Warning => (Icon::AlertTriangle, t.warning_foreground),
            Self::Info => (Icon::Info, t.info_foreground),
        }
    }
}

/// One toast entry (web: `Toast`).
#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub message: String,
    pub detail: Option<String>,
    /// Lifetime left. Burns only on rendered passes (audit item 69), so
    /// a toast pushed while the window is hidden waits — visible for
    /// its full duration once the window returns.
    pub remaining: Duration,
    /// Wall-clock stamp of the last rendered pass that showed this
    /// toast (`None` until the first render — lifetime starts there).
    pub last_paint: Option<Instant>,
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
            remaining: kind.auto_dismiss(),
            last_paint: None,
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
        // Clean-audit item 69: burn lifetime only across RENDERED passes.
        // A toast that has never rendered keeps its full lifetime (it
        // starts when the user can actually see it); a previously
        // rendered toast burns the gap since its last painted pass,
        // capped at one heartbeat so a hidden window cannot consume the
        // remaining lifetime in one go.
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
                            color: t.shadow, // DC e2 (RS-12 token port)
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
        assert!(toasts.is_empty());
        toasts.success("saved");
        toasts.error("boom");
        toasts.push(ToastKind::Warning, "careful", Some("detail".into()));
        assert_eq!(toasts.len(), 3);
        assert_eq!(toasts.stack[0].message, "saved");
        assert_eq!(toasts.stack[1].kind, ToastKind::Error);
        assert_eq!(toasts.stack[2].detail.as_deref(), Some("detail"));
        // Full lifetime on push; the countdown starts at the first
        // rendered pass, not at push time (audit item 69).
        assert_eq!(toasts.stack[0].remaining, ToastKind::Success.auto_dismiss());
        assert!(toasts.stack[0].last_paint.is_none());

        // Force expiry: every toast's remaining lifetime is exhausted.
        for toast in &mut toasts.stack {
            toast.remaining = Duration::ZERO;
        }
        toasts.stack.retain(|t| t.remaining > Duration::ZERO);
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

    /// Audit item 69: a toast pushed while the window is hidden (no ui
    /// pass runs — eframe only calls `logic`) never burned any lifetime,
    /// so it still has its FULL duration left when the window returns
    /// and the first rendered pass happens arbitrarily late.
    #[test]
    fn a_hidden_window_never_burns_unrendered_toast_lifetime() {
        let mut toasts = Toasts::new();
        toasts.error("sync failed while hidden");
        // The window was hidden for an hour before this first render.
        let ctx = egui::Context::default();
        ctx.run_pass(egui::RawInput::default(), |ctx| toasts.ui(ctx))
            .drop_without_applying_deltas();
        assert_eq!(toasts.len(), 1, "the error toast survives the hidden hour");
        let remaining = toasts.stack[0].remaining;
        assert_eq!(
            remaining,
            ToastKind::Error.auto_dismiss(),
            "lifetime starts at the first rendered pass — the hidden gap burned nothing"
        );
    }

    /// Audit item 69: a toast RENDERED before the window was hidden keeps
    /// (nearly) its remaining lifetime — the hidden gap burns at most one
    /// repaint heartbeat (MAX_PASS_GAP), never the whole gap.
    #[test]
    fn a_hidden_gap_burns_at_most_one_heartbeat() {
        let mut toasts = Toasts::new();
        toasts.error("boom");
        let ctx = egui::Context::default();
        // First render: lifetime starts.
        ctx.run_pass(egui::RawInput::default(), |ctx| toasts.ui(ctx))
            .drop_without_applying_deltas();
        // The window is hidden for an hour: no ui pass runs at all.
        for toast in &mut toasts.stack {
            toast.last_paint = Some(Instant::now() - Duration::from_secs(3600));
        }
        // The window returns: the next rendered pass must not expire the
        // toast — the burn is capped.
        ctx.run_pass(egui::RawInput::default(), |ctx| toasts.ui(ctx))
            .drop_without_applying_deltas();
        assert_eq!(toasts.len(), 1, "the toast survives the return from hidden");
        assert!(
            toasts.stack[0].remaining + MAX_PASS_GAP >= ToastKind::Error.auto_dismiss(),
            "at most one heartbeat burned: {} left of {}",
            toasts.stack[0].remaining.as_secs(),
            ToastKind::Error.auto_dismiss().as_secs()
        );
    }

    /// Visible steady state: consecutive rendered passes at normal frame
    /// gaps burn real time, so a toast still expires on schedule.
    #[test]
    fn consecutive_rendered_passes_burn_real_lifetime() {
        let mut toasts = Toasts::new();
        toasts.success("saved");
        let ctx = egui::Context::default();
        ctx.run_pass(egui::RawInput::default(), |ctx| toasts.ui(ctx))
            .drop_without_applying_deltas();
        // Simulate elapsed visible time by back-dating each pass's paint
        // stamp to 2 s ago: three passes burn 3 × 1.6 s (the cap) = 4.8 s,
        // past the 4 s success lifetime.
        for _ in 0..3 {
            for toast in &mut toasts.stack {
                toast.last_paint = Some(Instant::now() - Duration::from_secs(2));
            }
            ctx.run_pass(egui::RawInput::default(), |ctx| toasts.ui(ctx))
                .drop_without_applying_deltas();
        }
        assert!(
            toasts.is_empty(),
            "6 s of visible passes expire a 4 s toast"
        );
    }
}
