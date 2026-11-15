use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ComposerMode {
    #[default]
    Reply,
    Note,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComposerAct {
    Send(ComposerMode, String),
    SavedReply(usize),
    AiInsert,
}

pub fn composer(
    ui: &mut egui::Ui,
    id_salt: &str,
    state: &mut ComposerState,
    saved_replies: &[String],
    before_body: impl FnOnce(&mut egui::Ui),
    after_body: impl FnOnce(&mut egui::Ui),
) -> Option<ComposerAct> {
    let t = tokens(ui);
    let mut act = None;
    let note = state.mode == ComposerMode::Note;

    card(ui, |ui| {
        ui.set_width(ui.available_width());

        let header_act = composer_header_row(ui, id_salt, state, saved_replies, t, note);
        if header_act.is_some() {
            act = header_act;
        }
        ui.add_space(scale::space::SP2);
        divider(ui);

        before_body(ui);

        let edit = egui::TextEdit::multiline(&mut state.draft)
            .id_salt(format!("spp-composer-{id_salt}"))
            .hint_text(if note {
                "add an internal note…"
            } else {
                "write a reply…"
            })
            .desired_width(ui.available_width())
            .min_size(egui::vec2(0.0, 96.0))
            .font(egui::TextStyle::Body);
        let edit = if note {
            edit.background_color(t.warning_soft)
        } else {
            edit
        };
        let out = edit.show(ui);
        state.focused = out.response.has_focus();
        state.caret = out.cursor_range.map(|range| range.primary.index.0);

        after_body(ui);

        let footer_act = composer_footer_row(ui, state, t, note);
        if footer_act.is_some() {
            act = footer_act;
        }
    });
    act
}

fn composer_header_row(
    ui: &mut egui::Ui,
    id_salt: &str,
    state: &mut ComposerState,
    saved_replies: &[String],
    t: &'static Tokens,
    note: bool,
) -> Option<ComposerAct> {
    use egui::RichText as R;
    let mut act = None;
    ui.horizontal(|ui| {
        let mode_idx = usize::from(note);
        if let Some(i) = segmented(
            ui,
            &format!("composer-mode-{id_salt}"),
            &["Reply", "Note"],
            mode_idx,
        ) {
            state.mode = if i == 0 {
                ComposerMode::Reply
            } else {
                ComposerMode::Note
            };
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if state.draft_saved {
                ui.label(
                    R::new("Draft saved")
                        .size(scale::text::META)
                        .color(t.muted_foreground),
                );
            }
            if button_with_icon(ui, Icon::Sparkles, "AI Draft", ButtonVariant::Outline)
                .on_hover_text("insert the latest LM Studio draft for this conversation")
                .clicked()
            {
                act = Some(ComposerAct::AiInsert);
            }
            let sr = button_with_icon(ui, Icon::Mail, "Saved Replies", ButtonVariant::Outline)
                .on_hover_text("pick a saved reply template");
            let popup_id = ui.make_persistent_id(("spp-sr-menu", id_salt));
            egui::Popup::menu(&sr)
                .id(popup_id)
                .width(280.0)
                .frame(
                    egui::Frame::new()
                        .fill(t.card)
                        .stroke(egui::Stroke::new(1.0, t.border))
                        .corner_radius(egui::CornerRadius::same(10))
                        .inner_margin(egui::Margin::same(4))
                        .shadow(egui::Shadow {
                            offset: [0, 8],
                            blur: 24,
                            spread: 0,
                            color: egui::Color32::from_black_alpha(60),
                        }),
                )
                .show(|ui| {
                    ui.set_min_width(270.0);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    if saved_replies.is_empty() {
                        ui.label(
                            R::new("No saved replies yet")
                                .size(scale::text::SMALL_HALF)
                                .color(t.muted_foreground),
                        );
                    }
                    for (i, reply) in saved_replies.iter().enumerate() {
                        if ui
                            .button(R::new(reply).size(scale::text::SMALL_HALF))
                            .clicked()
                        {
                            act = Some(ComposerAct::SavedReply(i));
                        }
                    }
                });
        });
    });
    act
}

fn composer_footer_row(
    ui: &mut egui::Ui,
    state: &mut ComposerState,
    t: &'static Tokens,
    note: bool,
) -> Option<ComposerAct> {
    use egui::RichText as R;
    let mut act = None;
    ui.horizontal(|ui| {
        let context = if note {
            "note — visible to teammates only · Ctrl+Enter sends"
        } else {
            "reply — sent to the customer · Ctrl+Enter sends"
        };
        ui.add(
            egui::Label::new(
                R::new(context)
                    .size(scale::text::META)
                    .color(t.muted_foreground),
            )
            .truncate(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let send_label = if note { "Send Note" } else { "Send" };
            let send = if note {
                let g = egui::WidgetText::from(
                    R::new(send_label).color(t.on_ai).size(scale::text::CONTROL),
                )
                .into_galley(
                    ui,
                    Some(egui::TextWrapMode::Extend),
                    f32::INFINITY,
                    egui::TextStyle::Button,
                );
                let size = egui::vec2(g.size().x + 28.0, 32.0);
                let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
                ui.painter().rect_filled(
                    rect,
                    egui::CornerRadius::same(scale::radius::CONTROL),
                    t.warning,
                );
                ui.painter().galley(
                    egui::pos2(
                        rect.center().x - g.size().x / 2.0,
                        rect.center().y - g.size().y / 2.0,
                    ),
                    g,
                    t.on_ai,
                );
                resp.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, true, send_label.to_owned())
                });
                resp
            } else {
                button_with_icon(ui, Icon::Send, send_label, ButtonVariant::Primary)
            };
            let ctrl_enter = ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter));
            if send.clicked() || ctrl_enter {
                act = Some(ComposerAct::Send(state.mode, state.draft.clone()));
                state.draft.clear();
                state.draft_saved = false;
            }
        });
    });
    act
}

#[derive(Debug, Clone, Default)]
pub struct ComposerState {
    pub mode: ComposerMode,
    pub draft: String,
    pub draft_saved: bool,
    pub caret: Option<usize>,
    pub focused: bool,
}

impl ComposerState {
    pub fn new() -> Self {
        Self::default()
    }
}

pub fn cop_suggest(
    ui: &mut egui::Ui,
    title: &str,
    bullets: &[String],
    actions: &[&str],
) -> Option<usize> {
    let t = tokens(ui);
    let mut clicked = None;
    let resp = egui::Frame::new()
        .fill(t.ai_soft)
        .corner_radius(egui::CornerRadius::same(scale::radius::CARD))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                badge(ui, "AI", BadgeTone::Ai);
                ui.label(
                    egui::RichText::new(title)
                        .size(scale::text::META_HALF)
                        .strong()
                        .color(t.ai_foreground),
                );
            });
            ui.add_space(scale::space::SP2 - 2.0);
            for bullet in bullets {
                ui.horizontal(|ui| {
                    ui.painter().circle_filled(
                        egui::pos2(ui.min_rect().left() + 4.0, ui.cursor().top() + 8.0),
                        2.0,
                        t.border_strong,
                    );
                    ui.add_space(12.0);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(spp_core::richtext::break_long_tokens(bullet, 40))
                                .size(scale::text::SMALL_HALF)
                                .color(t.foreground),
                        )
                        .wrap(),
                    );
                });
            }
            if !actions.is_empty() {
                ui.add_space(scale::space::SP2 - 2.0);
                ui.horizontal(|ui| {
                    for (i, a) in actions.iter().enumerate() {
                        if button(ui, a, ButtonVariant::Outline).clicked() {
                            clicked = Some(i);
                        }
                    }
                });
            }
        })
        .response;
    let r = resp.rect;
    ui.painter().rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(r.min.x, r.min.y),
            egui::pos2(r.min.x + 3.0, r.max.y),
        ),
        egui::CornerRadius {
            nw: scale::radius::CARD,
            sw: scale::radius::CARD,
            ne: 0,
            se: 0,
        },
        t.ai,
    );
    clicked
}

pub fn cop_chat(ui: &mut egui::Ui, from_assistant: bool, text: &str) {
    let t = tokens(ui);
    let (bg, border) = if from_assistant {
        (t.ai_soft, t.ai_border)
    } else {
        (t.muted, t.border)
    };
    let align_right = !from_assistant;
    ui.with_layout(
        if align_right {
            egui::Layout::right_to_left(egui::Align::TOP)
        } else {
            egui::Layout::left_to_right(egui::Align::TOP)
        },
        |ui| {
            let galley = egui::WidgetText::from(egui::RichText::new(text).size(scale::text::BODY))
                .into_galley(
                    ui,
                    Some(egui::TextWrapMode::Wrap),
                    320.0,
                    egui::TextStyle::Body,
                );
            let size = galley.size() + egui::vec2(24.0, 18.0);
            let (rect, _id) = ui.allocate_exact_size(size, egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(12), bg);
            if from_assistant {
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::same(12),
                    egui::Stroke::new(1.0, border),
                    egui::StrokeKind::Inside,
                );
            }
            ui.painter().galley(
                egui::pos2(rect.min.x + 12.0, rect.min.y + 9.0),
                galley,
                t.foreground,
            );
        },
    );
}
