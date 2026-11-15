use super::*;

#[derive(Clone, Copy)]
enum ToolAction {
    Review,
    Rewrite(&'static str),
    Translate,
}

fn composer_tool_row(
    ui: &mut egui::Ui,
    busy: bool,
    body_empty: bool,
    gen_ok: bool,
    translate_lang: &mut String,
) -> Option<ToolAction> {
    let mut tool: Option<ToolAction> = None;
    if !busy && !body_empty {
        if design::chip_with_icon(ui, false, Icon::Sparkles, "Review Draft")
            .on_hover_text(
                "run the 9 deterministic coaching checks (plus the AI layer when a \
                 model is routed) — advisory only",
            )
            .clicked()
        {
            tool = Some(ToolAction::Review);
        }
        if gen_ok {
            for (instruction, label) in [
                ("shorten", "Shorten"),
                ("expand", "Expand"),
                ("warmer", "Warmer"),
            ] {
                if design::chip(ui, false, label)
                    .on_hover_text("rewrite the composer text with the Draft role model")
                    .clicked()
                {
                    tool = Some(ToolAction::Rewrite(instruction));
                }
            }
            let lang_label = translate_lang.clone();
            design::select(ui, "composer-translate-lang", &lang_label, 110.0, |ui| {
                for (code, name) in spp_core::translation::SUPPORTED_LANGUAGES {
                    ui.selectable_value(
                        translate_lang,
                        code.to_string(),
                        format!("{code} — {name}"),
                    );
                }
            });
            if design::chip(ui, false, "Translate")
                .on_hover_text(
                    "translate the composer text to the picked language (cached \
                     locally)",
                )
                .clicked()
            {
                tool = Some(ToolAction::Translate);
            }
        }
    }
    tool
}

impl InboxPage {
    pub(super) fn thread_ui(&mut self, ui: &mut egui::Ui, detail: &ConversationDetail) {
        if detail.thread.is_empty() {
            design::empty_state_slim(
                ui,
                Icon::Inbox,
                "No messages yet",
                "The thread has no entries in the local mirror yet.",
            );
            return;
        }
        let mut last_day: Option<String> = None;
        for entry in &detail.thread {
            let day: String = entry.created_at.chars().take(10).collect();
            if last_day.as_deref() != Some(day.as_str()) {
                ui.add_space(design::scale::space::SP2);
                design::day_divider(ui, &day);
                ui.add_space(design::scale::space::SP2);
                last_day = Some(day);
            }
            let (role, role_label) = message_role_of(&entry.thread_type);
            let actor = entry
                .actor_name
                .clone()
                .unwrap_or_else(|| entry.actor_type.clone());
            let time = compact_time(&entry.created_at);
            let spans = spp_core::richtext::html_to_spans(entry.body.as_deref().unwrap_or(""));
            design::message_card_with(
                ui,
                role,
                &actor,
                role_label,
                &time,
                move |ui| render_rich_spans(ui, &spans),
                |_ui| {},
            );
            ui.add_space(design::scale::space::SP2);
        }
    }

    pub(super) fn composer_pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        bus: &EventBus,
        detail: &ConversationDetail,
    ) {
        let conv_id = detail.id;
        let busy = self.pending_ai.is_some();
        let body_empty = self.composer_body.trim().is_empty();

        let mention_len = caret_query(&self.composer_body, self.mention_caret)
            .map_or(0, |(q, _)| filter_matches(&self.directory, &q).len());
        if mention_len > 0 && self.composer_focused {
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
                self.mention_active =
                    crate::palette::move_selection(self.mention_active, mention_len, 1);
            }
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
                self.mention_active =
                    crate::palette::move_selection(self.mention_active, mention_len, -1);
            }
        }

        let saved_reply_names: Vec<String> = self
            .saved_replies
            .iter()
            .map(|(name, _)| name.clone())
            .collect();

        let mut cc = self.composer_cc.clone();
        let mut bcc = self.composer_bcc.clone();
        let mut translate_lang = self.translate_lang.clone();
        let gen_ok = db.with_read(spp_core::ai_local::generative_backend_available);
        let mut tool: Option<ToolAction> = None;

        let mut state = ComposerState {
            mode: match self.composer_mode {
                ComposerMode::Reply => design::ComposerMode::Reply,
                ComposerMode::Note => design::ComposerMode::Note,
            },
            draft: self.composer_body.clone(),
            draft_saved: self.draft_saved,
            caret: None,
            focused: false,
        };
        let reply_mode = self.composer_mode == ComposerMode::Reply;
        let to_who = match detail.customer_email.as_deref() {
            Some(email) if !email.trim().is_empty() => format!(
                "{} <{}>",
                detail.customer_name.as_deref().unwrap_or("customer"),
                email
            ),
            _ => detail
                .customer_name
                .clone()
                .unwrap_or_else(|| "customer".into()),
        };
        let act = design::composer(
            ui,
            "inbox",
            &mut state,
            &saved_reply_names,
            |ui| {
                if reply_mode {
                    recipient_row_static(ui, "To", &to_who);
                    recipient_row_input(ui, "Cc", &mut cc);
                    recipient_row_input(ui, "Bcc", &mut bcc);
                    ui.add_space(design::scale::space::SP2 - 2.0);
                }
            },
            |ui| {
                tool = composer_tool_row(ui, busy, body_empty, gen_ok, &mut translate_lang);
            },
        );
        self.composer_cc = cc;
        self.composer_bcc = bcc;
        self.translate_lang = translate_lang;
        self.composer_focused = state.focused;
        self.composer_panel_h = Some(ui.min_rect().height() + 2.0 * design::scale::space::SP2);
        if let Some(caret) = state.caret {
            self.mention_caret = char_index_to_byte(&state.draft, caret);
        }
        self.enqueue_composer_tool(db, conv_id, tool);
        self.apply_composer_act(db, bus, conv_id, act, &state);

        self.mention_popup_ui(ui);
    }

    fn enqueue_composer_tool(&mut self, db: &DbHandle, conv_id: i64, tool: Option<ToolAction>) {
        match tool {
            Some(ToolAction::Review) => {
                self.enqueue_composer_ai(
                    db,
                    "review_draft",
                    json!({
                        "conversationId": conv_id,
                        "draft": self.composer_body,
                    }),
                    conv_id,
                );
            }
            Some(ToolAction::Rewrite(instruction)) => {
                self.enqueue_composer_ai(
                    db,
                    "rewrite_composer",
                    json!({
                        "conversationId": conv_id,
                        "text": self.composer_body,
                        "instruction": instruction,
                    }),
                    conv_id,
                );
            }
            Some(ToolAction::Translate) => {
                let text: String = self.composer_body.chars().take(8000).collect();
                self.enqueue_composer_ai(
                    db,
                    "translate_composer",
                    json!({
                        "conversationId": conv_id,
                        "text": text,
                        "targetLang": self.translate_lang,
                    }),
                    conv_id,
                );
            }
            None => {}
        }
    }

    fn apply_composer_act(
        &mut self,
        db: &DbHandle,
        bus: &EventBus,
        conv_id: i64,
        act: Option<ComposerAct>,
        state: &ComposerState,
    ) {
        match act {
            Some(ComposerAct::Send(mode, text)) => {
                self.composer_mode = match mode {
                    design::ComposerMode::Reply => ComposerMode::Reply,
                    design::ComposerMode::Note => ComposerMode::Note,
                };
                self.composer_body = text;
                self.draft_saved = false;
                self.submit_composer(db, bus);
            }
            Some(ComposerAct::SavedReply(index)) => {
                if let Some((_, text)) = self.saved_replies.get(index) {
                    let text = text.clone();
                    self.composer_edited(text);
                    self.mention_caret = 0;
                    self.mention_active = 0;
                    self.draft_saved = true;
                }
            }
            Some(ComposerAct::AiInsert) => self.insert_ai_draft(db, conv_id),
            None => {
                if state.draft != self.composer_body {
                    self.mention_active = 0;
                    let body = state.draft.clone();
                    self.composer_edited(body);
                    self.draft_saved = true;
                }
            }
        }
    }

    fn mention_popup_ui(&mut self, ui: &mut egui::Ui) {
        let body = self.composer_body.clone();
        if let Some((query, start)) = caret_query(&body, self.mention_caret) {
            let matches = filter_matches(&self.directory, &query);
            if !matches.is_empty() {
                design::section_title(ui, "@ mentions");
                egui::Grid::new("mention-popup")
                    .num_columns(1)
                    .show(ui, |ui| {
                        for (i, (token, display, _)) in matches.iter().enumerate() {
                            let picked =
                                design::chip(ui, self.mention_active == i, display.as_str())
                                    .clicked();
                            if picked {
                                let completed =
                                    apply_completion(&body, start, token, self.mention_caret);
                                self.composer_body = completed;
                                self.mention_caret = 0;
                                self.mention_active = 0;
                            }
                        }
                    });
                design::hint_text(
                    ui,
                    "Enter/Tab completes the highlighted entry when typed; click to pick",
                );
                if ui.input(|i| i.key_pressed(egui::Key::Tab) || i.key_pressed(egui::Key::Enter))
                    && self.composer_focused
                    && let Some((token, _, _)) =
                        matches.get(self.mention_active.min(matches.len() - 1))
                {
                    let completed = apply_completion(&body, start, token, self.mention_caret);
                    self.composer_body = completed;
                    self.mention_caret = 0;
                    self.mention_active = 0;
                }
            }
        }
    }
}
