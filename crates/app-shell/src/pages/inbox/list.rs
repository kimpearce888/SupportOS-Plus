use super::*;

impl InboxPage {
    pub(super) fn list_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        engine: Option<&std::sync::Arc<SyncEngine>>,
        folded: bool,
    ) {
        let t = design::tokens(ui);
        if folded {
            let counts = self.view_counts;
            ui.horizontal_wrapped(|ui| {
                for (count, view) in counts.into_iter().zip(ViewFilter::ALL) {
                    let selected = self.view_pick == view;
                    let label = format!("{} {}", view.label(), count);
                    if design::chip(ui, selected, &label).clicked() {
                        self.view_pick = view;
                        self.bar.view = Some(view);
                        self.page = 0;
                        self.reload_list(db);
                    }
                }
            });
            ui.add_space(design::scale::space::SP2);
        }
        ui.label(
            egui::RichText::new(self.view_pick.label())
                .size(design::scale::text::SECTION)
                .strong()
                .color(t.foreground),
        );
        let subtitle = if self.total > INBOX_PAGE_SIZE {
            let from = self.page * INBOX_PAGE_SIZE + 1;
            let to = from - 1 + self.list.len() as u32;
            format!("{from}–{to} of {} conversations", self.total)
        } else {
            format!("{} of {} conversations", self.list.len(), self.total)
        };
        design::hint_text(ui, &subtitle);
        ui.add_space(design::scale::space::SP2);

        let apply_requested = self.search_filter_ui(ui);

        self.list_toolbar_ui(ui, db, engine);
        if apply_requested {
            self.apply_filters(db);
        }
        ui.add_space(design::scale::space::SP2);

        if let Some(id) = self.conversation_rows_ui(ui, t) {
            self.select(db, id);
        }
        let pages = self.total.div_ceil(INBOX_PAGE_SIZE);
        let delta = design::pager(ui, self.page, pages);
        if delta != 0
            && let Some(next) = self.page.checked_add_signed(delta)
            && next < pages
        {
            self.page = next;
            self.reload_list(db);
        }
    }

    fn search_filter_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut apply_entered = false;
        let apply = design::filter_bar(
            ui,
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    let mut search = self.search_edit.clone();
                    const NUMBER_W: f32 = 56.0;
                    const APPLY_W: f32 = 82.0;
                    let row_rest = NUMBER_W + APPLY_W + 2.0 * 8.0;
                    let search_w = (ui.available_width() - row_rest).clamp(120.0, 240.0);
                    let response = ui.add_sized(
                        [search_w, 32.0],
                        egui::TextEdit::singleline(&mut search).hint_text("search conversations…"),
                    );
                    if response.changed() {
                        self.search_edit = search;
                    }
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        apply_entered = true;
                    }
                    let mut number = self.number_edit.clone();
                    let number_response = ui.add_sized(
                        [NUMBER_W, 32.0],
                        egui::TextEdit::singleline(&mut number).hint_text("#"),
                    );
                    if number_response.changed() {
                        self.number_edit = number;
                    }
                    if number_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        apply_entered = true;
                    }
                    if self.show_filters {
                        let mailbox_selected = self
                            .mailbox_pick
                            .and_then(|id| {
                                self.mailboxes
                                    .iter()
                                    .find(|(mid, _)| *mid == id)
                                    .map(|(_, name)| name.clone())
                            })
                            .unwrap_or_else(|| "All mailboxes".into());
                        design::select(ui, "inbox-mailbox", &mailbox_selected, 150.0, |ui| {
                            ui.selectable_value(&mut self.mailbox_pick, None, "All mailboxes");
                            for (id, name) in self.mailboxes.clone() {
                                ui.selectable_value(&mut self.mailbox_pick, Some(id), name);
                            }
                        });
                        let channel_selected = self
                            .channel_pick
                            .clone()
                            .map(|c| match c.as_str() {
                                "email" => "Email".to_string(),
                                "chat" => "Chat".to_string(),
                                other => other.to_string(),
                            })
                            .unwrap_or_else(|| "All channels".into());
                        design::select(ui, "inbox-channel", &channel_selected, 130.0, |ui| {
                            ui.selectable_value(&mut self.channel_pick, None, "All channels");
                            ui.selectable_value(
                                &mut self.channel_pick,
                                Some("email".into()),
                                "Email",
                            );
                            ui.selectable_value(
                                &mut self.channel_pick,
                                Some("chat".into()),
                                "Chat",
                            );
                        });
                        let tag_selected =
                            self.tag_pick.clone().unwrap_or_else(|| "Any tag".into());
                        design::select(ui, "inbox-tag", &tag_selected, 130.0, |ui| {
                            ui.selectable_value(&mut self.tag_pick, None, "Any tag");
                            for tag in self.tags.clone() {
                                ui.selectable_value(&mut self.tag_pick, Some(tag.clone()), tag);
                            }
                        });
                    }
                });
            },
            "Apply",
        );
        apply.clicked() || apply_entered
    }

    fn list_toolbar_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        engine: Option<&std::sync::Arc<SyncEngine>>,
    ) {
        ui.horizontal(|ui| {
            if design::button(
                ui,
                if self.show_filters {
                    "Hide Filters"
                } else {
                    "Filters"
                },
                ButtonVariant::Text,
            )
            .clicked()
            {
                self.show_filters = !self.show_filters;
            }
            if design::button(ui, "Reset", ButtonVariant::Ghost)
                .on_hover_text("clear every filter and reload")
                .clicked()
            {
                self.bar = FilterBar::default();
                self.view_pick = ViewFilter::All;
                self.mailbox_pick = None;
                self.channel_pick = None;
                self.tag_pick = None;
                self.search_edit.clear();
                self.number_edit.clear();
                self.page = 0;
                self.reload_list(db);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let syncing = engine.is_some_and(|e| e.is_running());
                if design::button_with_icon(
                    ui,
                    Icon::Refresh,
                    if syncing { "Syncing…" } else { "Sync now" },
                    ButtonVariant::Outline,
                )
                .on_hover_text(
                    "pull new and updated conversations from Help Scout right now \
                     (also happens automatically: 30 s while busy, backing off \
                     when quiet)",
                )
                .clicked()
                    && let Some(engine) = engine
                {
                    if !crate::pages::sync_health::spawn_sync_action(
                        db,
                        engine,
                        crate::pages::sync_health::SyncAction::Incremental,
                    ) {
                        self.set_status_message(
                            "A sync is already running — watch the ring in the header.",
                            false,
                        );
                    } else {
                        self.set_status_message("Syncing…", false);
                    }
                }
            });
        });
    }

    fn conversation_rows_ui(
        &mut self,
        ui: &mut egui::Ui,
        t: &'static design::Tokens,
    ) -> Option<i64> {
        let mut clicked: Option<i64> = None;
        egui::ScrollArea::vertical()
            .id_salt("inbox-list-scroll")
            .show(ui, |ui| {
                if self.list.is_empty() {
                    design::empty_state_slim(
                        ui,
                        Icon::Inbox,
                        "No conversations here",
                        "Adjust the filters or run a sync — conversations arrive from \
                         Help Scout.",
                    );
                }
                for item in self.list.clone() {
                    let selected = self.selected == Some(item.id);
                    let row_id = egui::Id::unique(("inbox-row", item.id));
                    let hovered = ui.ctx().read_response(row_id).is_some_and(|r| r.hovered());
                    let fill = if selected {
                        t.selection
                    } else if hovered {
                        t.accent
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    let subject = format!(
                        "#{}  {}",
                        item.number,
                        item.subject
                            .as_deref()
                            .unwrap_or(spp_core::catalog::NO_SUBJECT),
                    );
                    let response = egui::Frame::new()
                        .fill(fill)
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::same(design::scale::space::SP3 as i8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(
                                            item.customer_name
                                                .as_deref()
                                                .unwrap_or("(no customer)"),
                                        )
                                        .size(design::scale::text::CONTROL)
                                        .color(t.muted_foreground),
                                    )
                                    .truncate(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(
                                                item.updated_at
                                                    .as_deref()
                                                    .map(compact_time)
                                                    .unwrap_or_default(),
                                            )
                                            .size(design::scale::text::META)
                                            .color(t.muted_foreground),
                                        );
                                    },
                                );
                            });
                            ui.add_space(design::scale::space::SP1);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&subject)
                                        .size(design::scale::text::CONTROL)
                                        .strong()
                                        .color(t.foreground),
                                )
                                .truncate(),
                            );
                            ui.add_space(design::scale::space::SP1);
                            ui.horizontal(|ui| {
                                let assignee_label = item.assignee_name.clone().or_else(|| {
                                    item.assigned_team_name
                                        .as_deref()
                                        .map(|team| format!("team: {team}"))
                                });
                                design::assignee(ui, assignee_label.as_deref());
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let (tone, label) = design::status_tone(&item.status);
                                        design::badge(ui, label, tone);
                                    },
                                );
                            });
                        })
                        .response;
                    if selected {
                        let r = response.rect;
                        ui.painter().rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(r.min.x, r.min.y),
                                egui::pos2(r.min.x + 3.0, r.max.y),
                            ),
                            egui::CornerRadius {
                                nw: 8,
                                sw: 8,
                                ne: 0,
                                se: 0,
                            },
                            t.primary,
                        );
                    }
                    let response = ui
                        .interact(response.rect, row_id, egui::Sense::click())
                        .on_hover_text(item.preview.clone().unwrap_or_else(|| subject.clone()));
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::Role::ListItem,
                            ui.is_enabled(),
                            selected,
                            subject.clone(),
                        )
                    });
                    if response.clicked() {
                        clicked = Some(item.id);
                    }
                }
            });
        clicked
    }

    fn apply_filters(&mut self, db: &DbHandle) {
        let trimmed = self.search_edit.trim();
        self.bar.query = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
        self.bar.number = parse_number_entry(&self.number_edit);
        self.bar.mailbox = self.mailbox_pick;
        self.bar.channel = self.channel_pick.clone();
        self.bar.tag = self.tag_pick.clone();
        self.page = 0;
        self.reload_list(db);
    }

    pub(super) fn refresh_view_counts(&mut self, db: &DbHandle) {
        for (slot, view) in self.view_counts.iter_mut().zip(ViewFilter::ALL) {
            let mut bar = self.bar.clone();
            bar.view = Some(view);
            let filters = db.with_read(|conn| {
                let mut f = filters_from_bar(&bar, 0, conn);
                f.limit = Some(1);
                f
            });
            *slot = db
                .with_read(|conn| {
                    spp_core::inbox::list_conversations(conn, &filters).map(|(_, total)| total)
                })
                .unwrap_or(0);
        }
    }

    pub(super) fn reader_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        bus: &EventBus,
        detail: Option<ConversationDetail>,
    ) {
        let Some(detail) = detail else {
            if let Some(err) = &self.detail_error {
                design::empty_state(
                    ui,
                    Icon::AlertCircle,
                    "This conversation could not be loaded",
                    err,
                );
                return;
            }
            design::empty_state(
                ui,
                Icon::Inbox,
                "Select a conversation",
                "Pick a conversation from the list to read the thread and reply.",
            );
            return;
        };
        self.reader_head_ui(ui, db, &detail);
        ui.add_space(design::scale::space::SP2);
        egui::ScrollArea::vertical()
            .id_salt("inbox-reader-scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                self.action_bar_ui(ui, db, bus, &detail);
                ui.add_space(design::scale::space::SP2);
                self.triage_ui(ui, db);
                if !self.briefing.is_empty() {
                    self.briefing_ui(ui);
                    ui.add_space(design::scale::space::SP2);
                }
                self.thread_ui(ui, &detail);
                ui.add_space(design::scale::space::SP2);
                self.state_history_ui(ui, db, &detail);
                if let Some(qa) = self.qa_review.clone() {
                    self.qa_card_ui(ui, db, &qa, detail.id);
                    ui.add_space(design::scale::space::SP2);
                }
                if let Some(review) = self.coaching_review.clone() {
                    self.coaching_review_ui(ui, &review);
                    ui.add_space(design::scale::space::SP2);
                }
            });
    }

    fn reader_head_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, detail: &ConversationDetail) {
        let subject = format!(
            "#{}  {}",
            detail.number,
            detail
                .subject
                .as_deref()
                .unwrap_or(spp_core::catalog::NO_SUBJECT)
        );
        let who = match detail.customer_email.as_deref() {
            Some(email) if !email.trim().is_empty() => {
                format!(
                    "{} <{}>",
                    detail.customer_name.as_deref().unwrap_or("-"),
                    email
                )
            }
            _ => detail.customer_name.as_deref().unwrap_or("-").to_string(),
        };
        let meta = format!(
            "{} · {} · Priority: {}",
            who,
            detail.mailbox_name.as_deref().unwrap_or("-"),
            detail.priority.as_deref().unwrap_or("none"),
        );
        let t = design::tokens(ui);
        let head = egui::containers::Sides::new().shrink_left().show(
            ui,
            |ui| {
                ui.vertical(|ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&subject)
                                .size(design::scale::text::SECTION)
                                .strong(),
                        )
                        .truncate(),
                    );
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&meta)
                                .size(design::scale::text::SMALL)
                                .color(t.muted_foreground),
                        )
                        .truncate(),
                    );
                });
            },
            |ui| {
                if design::icon_button(
                    ui,
                    Icon::Refresh,
                    ButtonVariant::Ghost,
                    "refresh this conversation from Help Scout",
                )
                .clicked()
                {
                    db.with_write(|conn| {
                        let _ = spp_core::jobs::enqueue(
                            conn,
                            "sync_conversation",
                            &json!({ "remoteId": detail.remote_id }).to_string(),
                        );
                    });
                    self.set_status_message("Refreshing from Help Scout…", false);
                }
                if design::chip_with_icon(ui, self.copilot_open, Icon::Sparkles, "Copilot")
                    .on_hover_text("Copilot: local summary, similar past issues, suggested reply")
                    .clicked()
                {
                    self.copilot_open = !self.copilot_open;
                    self.copilot = None;
                    self.copilot_error = None;
                    self.reload_detail(db);
                }
                let (tone, label) = design::status_tone(&detail.status);
                design::badge(ui, label, tone);
            },
        );
        let _ = head;
    }

    fn action_bar_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        bus: &EventBus,
        detail: &ConversationDetail,
    ) {
        design::card_titled(ui, "Conversation", None, |ui| {
            fn row_label(ui: &mut egui::Ui, text: &str) {
                ui.allocate_ui(egui::vec2(64.0, design::scale::CONTROL_H), |ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(text)
                                .size(design::scale::text::SMALL)
                                .color(design::tokens(ui).muted_foreground),
                        );
                    });
                });
            }
            ui.horizontal(|ui| {
                row_label(ui, "Status");
                let statuses = ["active", "pending", "closed", "spam"];
                let active_idx = statuses
                    .iter()
                    .position(|s| *s == detail.status)
                    .unwrap_or(0);
                if let Some(i) = design::segmented(ui, "inbox-status", &statuses, active_idx) {
                    let status = statuses[i];
                    self.change_status(db, bus, status);
                }
            });
            ui.add_space(design::scale::space::SP2);
            ui.horizontal(|ui| {
                row_label(ui, "Assignee");
                let assign_label: String = detail
                    .assignee_name
                    .clone()
                    .or_else(|| {
                        detail
                            .assigned_team_name
                            .as_deref()
                            .map(|team| format!("team: {team}"))
                    })
                    .unwrap_or_else(|| "Unassigned".into());
                design::select(ui, "inbox-assign", &assign_label, 200.0, |ui| {
                    ui.selectable_value(&mut self.assign_pick, None, "Unassigned");
                    for (id, name) in self.users.clone() {
                        ui.selectable_value(&mut self.assign_pick, Some(id), name);
                    }
                });
                if design::button(ui, "Assign", ButtonVariant::Secondary).clicked() {
                    self.assign(db, bus, self.assign_pick);
                }
            });
            ui.add_space(design::scale::space::SP2);
            ui.horizontal(|ui| {
                row_label(ui, "Priority");
                let priorities = ["low", "normal", "high", "urgent"];
                let active_idx = priorities
                    .iter()
                    .position(|p| Some(*p) == detail.priority.as_deref())
                    .unwrap_or(usize::MAX);
                if let Some(i) = design::segmented(ui, "inbox-priority", &priorities, active_idx)
                    && detail.priority.as_deref() != Some(priorities[i])
                {
                    self.set_priority(db, bus, priorities[i]);
                }
            });
            ui.add_space(design::scale::space::SP2);
            ui.horizontal(|ui| {
                row_label(ui, "State");
                let states = spp_core::ticket_states::TICKET_WORKFLOW_STATES;
                let active_idx = states
                    .iter()
                    .position(|s| Some(*s) == detail.workflow_state.as_deref())
                    .unwrap_or(usize::MAX);
                if let Some(i) = design::segmented(ui, "inbox-workflow-state", &states, active_idx)
                    && detail.workflow_state.as_deref() != Some(states[i])
                {
                    self.set_workflow_state(db, bus, states[i]);
                }
            });
            ui.add_space(design::scale::space::SP2);
            ui.horizontal(|ui| {
                row_label(ui, "Tags");
                for tag in &detail.tags {
                    if design::chip(ui, false, tag.as_str())
                        .on_hover_text("click to remove this tag")
                        .clicked()
                    {
                        let remaining: Vec<String> = detail
                            .tags
                            .iter()
                            .filter(|t| !t.eq_ignore_ascii_case(tag))
                            .cloned()
                            .collect();
                        self.set_tags(db, bus, remaining);
                    }
                }
                let add_clicked = ui
                    .add_sized(
                        [110.0, 26.0],
                        egui::TextEdit::singleline(&mut self.tag_edit).hint_text("add tag…"),
                    )
                    .lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if add_clicked
                    || (design::button(ui, "Add", ButtonVariant::Secondary)
                        .on_hover_text("adds the tag locally and queues the Help Scout update")
                        .clicked()
                        && !self.tag_edit.trim().is_empty())
                {
                    let tag = self.tag_edit.trim().to_string();
                    if !tag.is_empty() {
                        let mut next = detail.tags.clone();
                        if !next.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
                            next.push(tag);
                        }
                        self.set_tags(db, bus, next);
                        self.tag_edit.clear();
                    }
                }
            });
            ui.add_space(design::scale::space::SP2);
            ui.horizontal(|ui| {
                row_label(ui, "Snooze");
                if let Some(until) = detail.snoozed_until.as_deref() {
                    design::badge(ui, until, BadgeTone::Warning)
                        .on_hover_text("snoozed until this moment (Help Scout)");
                    if design::button(ui, "Unsnooze", ButtonVariant::Secondary)
                        .on_hover_text("wake the conversation up now — local and Help Scout")
                        .clicked()
                    {
                        self.unsnooze(db, bus);
                    }
                } else {
                    let now = chrono::Utc::now();
                    let presets = [
                        ("2h", now + chrono::Duration::hours(2)),
                        ("tomorrow", now + chrono::Duration::hours(24)),
                        ("3d", now + chrono::Duration::hours(72)),
                    ];
                    for (label, at) in presets {
                        if design::chip(ui, false, label)
                            .on_hover_text(format!(
                                "snooze until {} (Help Scout); a customer reply wakes it up",
                                at.format("%Y-%m-%d %H:%M UTC")
                            ))
                            .clicked()
                        {
                            let until = at.format("%Y-%m-%dT%H:%M:%SZ").to_string();
                            self.snooze(db, bus, until);
                        }
                    }
                }
            });
            ui.add_space(design::scale::space::SP2);
            self.expire_status_message();
            if let Some(msg) = &self.status_message {
                if self.status_is_error {
                    design::error_text(ui, msg);
                } else {
                    design::success_text(ui, msg);
                }
            }
            if let Some((note, is_error)) = &self.outbound_note {
                if *is_error {
                    design::error_text(ui, note);
                } else {
                    design::hint_text(ui, note);
                }
            }
        });
    }

    fn triage_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.triage.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for chip in &self.triage {
                let tone = match (chip.label, chip.value.as_str()) {
                    ("urgency", "high" | "critical") => BadgeTone::Destructive,
                    ("urgency", "moderate") => BadgeTone::Warning,
                    ("frustration cues", "strong" | "moderate") => BadgeTone::Warning,
                    ("escalation signal", "true") => BadgeTone::Destructive,
                    (_, _) => BadgeTone::Info,
                };
                let text = if chip.confidence == "heuristic" {
                    format!("{}: {}", chip.label, chip.value)
                } else if chip.source == "human" {
                    format!("{}: {} ✓", chip.label, chip.value)
                } else {
                    format!("{}: {} · {}", chip.label, chip.value, chip.confidence)
                };
                let response = design::badge(ui, &text, tone);
                let hint = format!(
                    "source: {} · confidence: {}. {}",
                    chip.source,
                    chip.confidence,
                    if chip.editable {
                        "click to correct"
                    } else {
                        "deterministic estimate — not stored"
                    },
                );
                let mut response = response.on_hover_text(hint);
                if chip.editable {
                    response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
                }
                if chip.editable && response.clicked() {
                    self.editing_chip = Some(chip.key);
                }
            }
        });
        if let Some(key) = self.editing_chip {
            let chip = self.triage.iter().find(|c| c.key == key).cloned();
            if let Some(chip) = chip {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(format!("Set {}:", chip.label.to_lowercase()));
                    if ui.button("Unknown (clear)").clicked() {
                        self.override_chip_value(db, &chip, "");
                        self.editing_chip = None;
                    }
                    for value in key.values() {
                        if ui.button(value.to_string()).clicked() {
                            self.override_chip_value(db, &chip, value);
                            self.editing_chip = None;
                        }
                    }
                    if ui.button(design::CANCEL_LABEL).clicked() {
                        self.editing_chip = None;
                    }
                });
            } else {
                self.editing_chip = None;
            }
        }
        ui.add_space(design::scale::space::CLUSTER);
    }

    fn briefing_ui(&self, ui: &mut egui::Ui) {
        let t = design::tokens(ui);
        design::card_titled(
            ui,
            "Know before you reply",
            Some("durable customer facts extracted from past conversations"),
            |ui| {
                ui.horizontal(|ui| {
                    Icon::Sparkles.show(ui, 14.0, t.muted_foreground);
                    design::badge(ui, "AI", BadgeTone::Ai);
                });
                for fact in &self.briefing {
                    ui.label(
                        egui::RichText::new(format!("· {}: {}", fact.key, fact.value))
                            .small()
                            .color(t.foreground),
                    );
                }
            },
        );
    }
}
