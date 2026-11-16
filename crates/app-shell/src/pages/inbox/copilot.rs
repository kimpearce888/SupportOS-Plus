use super::*;

impl InboxPage {
    pub(super) fn copilot_pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        detail: Option<&ConversationDetail>,
    ) {
        ui.horizontal(|ui| {
            Icon::Sparkles.show(ui, 14.0, design::tokens(ui).ai);
            ui.label(
                egui::RichText::new("Copilot")
                    .size(design::scale::text::CONTROL)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if design::icon_button(ui, Icon::X, ButtonVariant::Ghost, "close the copilot panel")
                    .clicked()
                {
                    self.copilot_open = false;
                }
            });
        });
        design::hint_text(ui, "local · no configuration needed");
        ui.add_space(design::scale::space::SP2);
        let Some(detail) = detail else {
            design::hint_text(
                ui,
                "Select a conversation — the copilot reads the local mirror for it.",
            );
            return;
        };
        egui::ScrollArea::vertical()
            .id_salt("inbox-copilot-scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if let Some(err) = &self.copilot_error {
                    design::error_text(ui, &format!("copilot: {err}"));
                    return;
                }
                let Some(report) = self.copilot.clone() else {
                    design::hint_text(ui, "Analyzing the conversation…");
                    return;
                };
                self.copilot_cards_ui(ui, &report);
                self.copilot_lmstudio_ui(ui, db, detail);
                self.assistant_ui(ui, db, detail);
            });
    }

    fn copilot_cards_ui(&mut self, ui: &mut egui::Ui, report: &spp_core::ai_local::CopilotReport) {
        let a = &report.analysis;

        design::card_titled(ui, "Signals", None, |ui| {
            ui.horizontal_wrapped(|ui| {
                design::badge(ui, &a.intent, design::BadgeTone::Info);
                let urgency_tone = match a.urgency.as_str() {
                    "high" => design::BadgeTone::Destructive,
                    "medium" => design::BadgeTone::Warning,
                    _ => design::BadgeTone::Neutral,
                };
                design::badge(ui, &format!("urgency: {}", a.urgency), urgency_tone);
                if a.escalation {
                    design::badge(ui, "escalation risk", design::BadgeTone::Destructive);
                }
            });
        });
        ui.add_space(design::scale::space::SP2);

        if !a.summary.is_empty() || !a.questions.is_empty() {
            design::card_titled(ui, "Summary", None, |ui| {
                for line in &a.summary {
                    ui.label(
                        egui::RichText::new(format!(
                            "• {}",
                            spp_core::richtext::break_long_tokens(line, 40)
                        ))
                        .small()
                        .color(design::tokens(ui).foreground),
                    );
                }
                for q in &a.questions {
                    ui.label(
                        egui::RichText::new(format!(
                            "? {}",
                            spp_core::richtext::break_long_tokens(q, 40)
                        ))
                        .small()
                        .color(design::tokens(ui).muted_foreground),
                    );
                }
            });
            ui.add_space(design::scale::space::SP2);
        }

        design::hint_text(
            ui,
            &format!(
                "{} past conversation{} on file{}",
                report.history.previous_conversations,
                if report.history.previous_conversations == 1 {
                    ""
                } else {
                    "s"
                },
                report
                    .history
                    .last_contact
                    .as_deref()
                    .map(|t| format!(" · last contact {t}"))
                    .unwrap_or_default()
            ),
        );
        ui.add_space(design::scale::space::SP1);

        if report.similar.is_empty() {
            design::hint_text(ui, "No similar past conversations found yet.");
        } else {
            design::card_titled(ui, "Similar past issues", None, |ui| {
                for s in report.similar.iter().take(3) {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("#{} {}", s.number, s.subject))
                                    .small()
                                    .strong()
                                    .color(design::tokens(ui).foreground),
                            )
                            .truncate(),
                        );
                        design::badge(
                            ui,
                            &format!("{}%", (s.score * 100.0) as u32),
                            design::BadgeTone::Ai,
                        );
                    });
                    ui.label(
                        egui::RichText::new(format!(
                            "resolved with: {}",
                            ellipsize(&s.resolution, 160)
                        ))
                        .small()
                        .color(design::tokens(ui).muted_foreground),
                    );
                    if !s.why.is_empty() {
                        ui.label(
                            egui::RichText::new(format!("why: {}", s.why.join(", ")))
                                .small()
                                .weak(),
                        );
                    }
                    ui.add_space(design::scale::space::SP1);
                }
            });
            ui.add_space(design::scale::space::SP2);
        }

        if !report.articles.is_empty() {
            design::card_titled(ui, "Relevant help articles", None, |ui| {
                for art in report.articles.iter().take(3) {
                    ui.label(
                        egui::RichText::new(format!(
                            "• {} — {}",
                            art.name,
                            ellipsize(&art.snippet, 120)
                        ))
                        .small()
                        .color(design::tokens(ui).muted_foreground),
                    );
                }
            });
            ui.add_space(design::scale::space::SP2);
        }

        if design::cop_suggest(
            ui,
            "Suggested reply",
            std::slice::from_ref(&report.suggested_reply),
            &["Insert into reply"],
        )
        .is_some()
        {
            self.composer_mode = ComposerMode::Reply;
            self.inserted_draft = None;
            self.composer_body = report.suggested_reply.clone();
            self.mention_caret = 0;
            self.mention_active = 0;
            self.draft_saved = false;
        }
        ui.add_space(design::scale::space::SP2);
    }

    fn copilot_lmstudio_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        detail: &ConversationDetail,
    ) {
        let lmstudio_ok = db.with_read(spp_core::ai_local::generative_backend_available);
        if lmstudio_ok {
            let drafting = self
                .pending_ai
                .as_ref()
                .is_some_and(|p| p.kind == "generate_draft");
            if design::button(
                ui,
                if drafting {
                    "Drafting with LM Studio…"
                } else {
                    "Draft reply with LM Studio"
                },
                ButtonVariant::Ai,
            )
            .on_hover_text(
                "read the full thread, the knowledge base, the help center and similar \
                 resolutions, then draft a grounded reply — review it and Insert before \
                 sending",
            )
            .clicked()
                && self.pending_ai.is_none()
            {
                self.enqueue_composer_ai(
                    db,
                    "generate_draft",
                    json!({ "conversationId": detail.id }),
                    detail.id,
                );
            }
        } else {
            design::hint_text(
                ui,
                "Reply drafting needs a routed Draft role (AI Center > Model-role routing).",
            );
        }
        ui.add_space(design::scale::space::SP2);
        if let Some((draft_id, draft, model)) =
            db.with_read(|conn| spp_core::ai_local::latest_ai_draft_row(conn, detail.id))
        {
            match design::cop_suggest(
                ui,
                &format!("LM Studio draft ({model})"),
                std::slice::from_ref(&draft),
                &["Insert LM Studio draft", "Discard draft"],
            ) {
                Some(0) => {
                    self.composer_mode = ComposerMode::Reply;
                    self.inserted_draft = Some((draft_id, draft.clone()));
                    self.composer_body = draft.clone();
                    self.mention_caret = 0;
                    self.mention_active = 0;
                    self.draft_saved = false;
                }
                Some(1) => {
                    let rejected = db.with_write(|conn| {
                        spp_core::ai_pipeline::record_feedback(
                            conn, draft_id, &draft, &draft, false,
                        )
                        .and_then(|()| {
                            spp_core::ai_pipeline::set_draft_state(conn, draft_id, "rejected")
                        })
                    });
                    match rejected {
                        Ok(()) => self.set_status_message(
                            "Draft marked rejected — feedback recorded.",
                            false,
                        ),
                        Err(e) => self.set_status_message(format!("Reject failed: {e}"), true),
                    }
                }
                _ => {}
            }
            if self
                .inserted_draft
                .as_ref()
                .is_some_and(|(id, _)| *id == draft_id)
            {
                design::badge(ui, "in composer", BadgeTone::Info);
            }
        }
        ui.add_space(design::scale::space::SP2);
    }

    fn assistant_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, detail: &ConversationDetail) {
        design::divider(ui);
        design::section_title(ui, "Ask the assistant");
        let (feature_on, agent_ok) = db.with_read(|conn| {
            (
                spp_core::ai_hub::ai_feature_enabled(conn, "copilot"),
                !matches!(
                    spp_core::ai_hub::backend_for_role(conn, spp_core::ai_hub::AiRole::Agent,),
                    spp_core::ai_pipeline::AiBackend::Disabled,
                ),
            )
        });
        if !feature_on {
            design::hint_text(ui, "Copilot is switched off (AI Center > Features).");
        } else if !agent_ok {
            design::hint_text(
                ui,
                "Route the Agent role to an endpoint to ask questions (AI Center > Model-role routing).",
            );
        } else {
            if let Some(err) = self.assistant_error.clone() {
                design::error_text(ui, &format!("assistant: {err}"));
            }
            if !self.assistant_messages.is_empty() {
                egui::ScrollArea::vertical()
                    .id_salt("assistant-messages")
                    .max_height(180.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for message in &self.assistant_messages {
                            let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("?");
                            if role == "tool" {
                                continue;
                            }
                            let content = message
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            design::cop_chat(ui, role == "assistant", content);
                        }
                    });
            }
            let busy = self.assistant_pending.is_some();
            ui.horizontal(|ui| {
                let mut question = self.assistant_input.clone();
                let response = ui.add_sized(
                    [ui.available_width() - 64.0, 26.0],
                    egui::TextEdit::singleline(&mut question)
                        .hint_text("ask about this conversation…"),
                );
                if response.changed() {
                    self.assistant_input = question;
                }
                if (design::button(
                    ui,
                    if busy { "Thinking…" } else { "Ask" },
                    ButtonVariant::Ai,
                )
                .clicked()
                    && !busy)
                    || (ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && response.lost_focus()
                        && !busy)
                {
                    let question = self.assistant_input.trim().to_string();
                    if !question.is_empty() {
                        let payload = json!({
                            "question": question,
                            "conversationId": detail.id,
                            "sessionId": self.assistant_session,
                        });
                        let job = db.with_write(|conn| {
                            spp_core::jobs::enqueue_on(
                                conn,
                                "ai",
                                "copilot_chat",
                                &payload.to_string(),
                                3,
                                2,
                            )
                        });
                        match job {
                            Ok(job_id) => {
                                self.assistant_pending = Some(job_id);
                                self.assistant_input.clear();
                            }
                            Err(e) => self.assistant_error = Some(e.to_string()),
                        }
                    }
                }
            });
            let starters =
                db.with_read(|conn| spp_core::copilot::starter_questions(conn, detail.id));
            if !starters.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for (question, hint) in starters.iter().take(4) {
                        if design::chip(ui, false, &ellipsize(question, 34))
                            .on_hover_text(hint)
                            .clicked()
                            && self.assistant_pending.is_none()
                        {
                            self.assistant_input = question.clone();
                        }
                    }
                });
            }
        }
    }

    pub(super) fn insert_ai_draft(&mut self, db: &DbHandle, conversation_id: i64) {
        self.composer_mode = ComposerMode::Reply;
        if let Some((draft_id, draft, _model)) =
            db.with_read(|conn| spp_core::ai_local::latest_ai_draft_row(conn, conversation_id))
        {
            self.inserted_draft = Some((draft_id, draft.clone()));
            self.composer_body = draft;
            self.mention_caret = 0;
            self.mention_active = 0;
            self.set_status_message("Latest AI draft inserted — review before sending.", false);
        } else if let Some(text) = self.copilot.as_ref().map(|r| r.suggested_reply.clone()) {
            if text.trim().is_empty() {
                self.set_status_message("The copilot has no suggested reply yet.", false);
                return;
            }
            self.inserted_draft = None;
            self.composer_body = text;
            self.mention_caret = 0;
            self.mention_active = 0;
            self.set_status_message("Suggested reply inserted — review before sending.", false);
        } else {
            self.set_status_message(
                "No AI draft yet — open Copilot to analyze the conversation first.",
                false,
            );
        }
        self.draft_saved = false;
    }

    pub(super) fn enqueue_composer_ai(
        &mut self,
        db: &DbHandle,
        kind: &'static str,
        payload: serde_json::Value,
        conversation_id: i64,
    ) {
        let job = db.with_write(|conn| {
            spp_core::jobs::enqueue_on(conn, "ai", kind, &payload.to_string(), 3, 2)
        });
        match job {
            Ok(job_id) => {
                self.pending_ai = Some(PendingComposerAi {
                    job_id,
                    kind,
                    conversation_id,
                    origin_body: self.composer_body.clone(),
                });
                self.set_status_message(
                    format!("{kind} queued — the result lands here when it finishes."),
                    false,
                );
            }
            Err(e) => {
                self.set_status_message(format!("Could not queue {kind}: {e}"), true);
            }
        }
    }

    pub(super) fn poll_pending_ai(&mut self, db: &DbHandle) {
        let Some(p) = self.pending_ai.clone() else {
            return;
        };
        let (status, error, result): (String, Option<String>, Option<String>) =
            db.with_read(|conn| {
                conn.query_row(
                    "SELECT status, error, result FROM jobs WHERE id = ?1",
                    rusqlite::params![p.job_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap_or(("missing".into(), None, None))
            });
        match status.as_str() {
            "queued" | "running" => {}
            "completed" => {
                self.pending_ai = None;
                match p.kind {
                    "rewrite_composer" | "translate_composer" => {
                        if self.composer_body != p.origin_body {
                            self.set_status_message(
                                "Transform finished, but you edited the draft while it ran — \
                                 your text was kept.",
                                false,
                            );
                            return;
                        }
                        match result {
                            Some(text) if !text.trim().is_empty() => {
                                self.composer_body = text;
                                self.mention_caret = 0;
                                self.mention_active = 0;
                                self.set_status_message(
                                    "Composer updated by the AI transform.",
                                    false,
                                );
                            }
                            _ => {
                                self.set_status_message(
                                    "The AI transform produced no result — the feature is off or no Draft role endpoint is routed.",
                                    true,
                                );
                            }
                        }
                    }
                    "review_draft" => {
                        self.coaching_review = db.with_read(|conn| {
                            spp_core::coaching::get(conn, p.conversation_id)
                                .ok()
                                .flatten()
                        });
                        self.set_status_message(
                            "Draft review finished — see the coaching card.",
                            false,
                        );
                    }
                    "qa_ai_layer" => {
                        self.qa_review = db.with_read(|conn| {
                            spp_core::quality::get_qa(conn, p.conversation_id, true)
                        });
                        self.set_status_message("Post-resolution QA AI review finished.", false);
                    }
                    "generate_draft" => {
                        self.set_status_message(
                            "LM Studio draft ready — review the draft card, then Insert into \
                             the composer before sending.",
                            false,
                        );
                    }
                    _ => {}
                }
            }
            _ => {
                self.pending_ai = None;
                self.set_status_message(
                    format!(
                        "AI job failed: {}",
                        error.unwrap_or_else(|| "unknown error".into())
                    ),
                    true,
                );
            }
        }
    }

    pub(super) fn poll_assistant(&mut self, db: &DbHandle) {
        let Some(job_id) = self.assistant_pending else {
            return;
        };
        let (status, error): (String, Option<String>) = db.with_read(|conn| {
            conn.query_row(
                "SELECT status, error FROM jobs WHERE id = ?1",
                rusqlite::params![job_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or(("missing".into(), None))
        });
        let Some(conv_id) = self.selected else {
            self.assistant_pending = None;
            return;
        };
        match status.as_str() {
            "queued" | "running" => {}
            "completed" => {
                self.assistant_pending = None;
                self.assistant_error = None;
                let (session, messages) = db.with_read(|conn| {
                    let session: Option<i64> = conn
                        .query_row(
                            "SELECT id FROM copilot_sessions WHERE conversation_id = ?1
                              ORDER BY updated_at DESC LIMIT 1",
                            rusqlite::params![conv_id],
                            |r| r.get(0),
                        )
                        .ok();
                    let messages = session
                        .map(|sid| spp_core::copilot::list_messages(conn, sid))
                        .unwrap_or_default();
                    (session, messages)
                });
                self.assistant_session = session;
                self.assistant_messages = messages;
            }
            _ => {
                self.assistant_pending = None;
                self.assistant_error = Some(
                    error.unwrap_or_else(|| "the assistant job failed without an error".into()),
                );
            }
        }
    }
}
