//! Modal overlays (port of the web `components/overlays.rs`).
//!
//! The web stack had two overlays: `Modal` (a titled dialog that closes on
//! Escape/backdrop click) and `ConfirmDialog` (a Modal with a message plus
//! Cancel/Confirm actions, optionally danger-styled). The egui port models
//! the dialog as a value the shell holds and renders with
//! [`ConfirmDialog::ui`]; Escape and Cancel both map to
//! [`DialogOutcome::Cancelled`].

use eframe::egui;

use crate::design::{self, ButtonVariant};
use crate::icons::Icon;

/// What the user did with a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogOutcome {
    /// Still open — render again next frame.
    Open,
    /// Confirm clicked.
    Confirmed,
    /// Cancel clicked, Escape pressed, or the window closed.
    Cancelled,
}

impl DialogOutcome {
    /// Whether the dialog should be dropped now.
    #[must_use]
    pub fn is_done(self) -> bool {
        !matches!(self, Self::Open)
    }
}

/// A confirm dialog request (port of `ConfirmDialog`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
    pub confirm_label: String,
    pub danger: bool,
}

impl ConfirmDialog {
    /// Build a dialog with the web defaults (`Confirm` label, not danger).
    #[must_use]
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            confirm_label: "Confirm".into(),
            danger: false,
        }
    }

    /// Set the confirm button label.
    #[must_use]
    pub fn confirm_label(mut self, label: impl Into<String>) -> Self {
        self.confirm_label = label.into();
        self
    }

    /// Mark the confirm action as destructive (red button).
    #[must_use]
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    /// Render the dialog.
    pub fn ui(&mut self, ctx: &egui::Context) -> DialogOutcome {
        let mut outcome = DialogOutcome::Open;
        let mut open = true;
        let mut close_clicked = false;
        let t = design::tokens_for(ctx);
        egui::Window::new(self.title.clone())
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .fixed_size([440.0, 172.0])
            .collapsible(false)
            .resizable(false)
            .fade_in(false)
            .title_bar(false)
            .frame(
                egui::Frame::new()
                    .fill(t.card)
                    .stroke(egui::Stroke::new(1.0, t.border))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::same(18))
                    .shadow(egui::Shadow {
                        offset: [0, 8],
                        blur: 32,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(60),
                    }),
            )
            .show(ctx, |ui| {
                // Header: title + close affordance.
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(self.title.as_str())
                            .size(15.5)
                            .strong()
                            .color(t.foreground),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if design::icon_button(ui, Icon::X, ButtonVariant::Ghost, "close").clicked()
                        {
                            close_clicked = true;
                        }
                    });
                });
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(self.message.clone())
                        .size(13.5)
                        .color(t.foreground),
                );
                ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                    ui.add_space(10.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let confirm_variant = if self.danger {
                            ButtonVariant::Destructive
                        } else {
                            ButtonVariant::Primary
                        };
                        if design::button(ui, self.confirm_label.as_str(), confirm_variant)
                            .clicked()
                        {
                            outcome = DialogOutcome::Confirmed;
                        }
                        if design::button(ui, "Cancel", ButtonVariant::Outline).clicked() {
                            outcome = DialogOutcome::Cancelled;
                        }
                    });
                });
            });
        if close_clicked {
            open = false;
        }
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            // Window close button / Escape cancel, never confirm.
            if outcome == DialogOutcome::Open {
                outcome = DialogOutcome::Cancelled;
            }
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_builders_set_the_web_defaults() {
        let plain = ConfirmDialog::new("Delete?", "This cannot be undone.");
        assert_eq!(plain.title, "Delete?");
        assert_eq!(plain.message, "This cannot be undone.");
        assert_eq!(plain.confirm_label, "Confirm");
        assert!(!plain.danger);

        let danger = ConfirmDialog::new("Delete?", "Really?")
            .confirm_label("Delete")
            .danger();
        assert_eq!(danger.confirm_label, "Delete");
        assert!(danger.danger);
    }

    #[test]
    fn outcome_lifecycle_matches_the_web_contract() {
        assert!(!DialogOutcome::Open.is_done());
        assert!(DialogOutcome::Confirmed.is_done());
        assert!(DialogOutcome::Cancelled.is_done());
        // Cancel and confirm are distinguishable.
        assert_ne!(DialogOutcome::Confirmed, DialogOutcome::Cancelled);
    }
}
