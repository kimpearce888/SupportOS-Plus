use super::*;

impl ReportsPage {
    pub(super) fn insights_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let health = self.i_health.clone();
        let effectiveness = self.i_effectiveness.clone();
        if let Some(health) = &health {
            design::card_titled(
                ui,
                "Health facts",
                Some(&format!(
                    "the last {} days, over the whole mirror",
                    self.days
                )),
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        design::metric_card(
                            ui,
                            "conversations",
                            &health.total_conversations.to_string(),
                            None,
                        );
                        design::metric_card(
                            ui,
                            "SLA breaches (tracked)",
                            &health.sla_breach_count.to_string(),
                            health
                                .sla_breach_rate
                                .map(|r| format!("{:.1}% of conversations", r * 100.0))
                                .as_deref(),
                        );
                        design::metric_card(
                            ui,
                            "high friction",
                            &health.high_friction_count.to_string(),
                            health
                                .high_friction_rate
                                .map(|r| format!("{:.1}% of conversations", r * 100.0))
                                .as_deref(),
                        );
                        design::metric_card(
                            ui,
                            "avg first response",
                            &health
                                .avg_first_response_minutes
                                .map(|m| format!("{m:.0} min"))
                                .unwrap_or_else(|| design::MISSING_VALUE.into()),
                            None,
                        );
                        design::metric_card(
                            ui,
                            "resolution rate",
                            &health
                                .resolution_rate
                                .map(|r| format!("{:.1}%", r * 100.0))
                                .unwrap_or_else(|| design::MISSING_VALUE.into()),
                            None,
                        );
                    });
                },
            );
            ui.add_space(8.0);
        }

        if let Some(eff) = &effectiveness {
            design::card_titled(
                ui,
                "Post-resolution QA effectiveness",
                Some("avg QA score and the score distribution over every stored check"),
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        design::metric_card(
                            ui,
                            "avg QA score",
                            &eff.avg_qa_score
                                .map(|s| format!("{s:.2}"))
                                .unwrap_or_else(|| design::MISSING_VALUE.into()),
                            None,
                        );
                        design::metric_card(
                            ui,
                            "checks stored",
                            &eff.total_qa_checks.to_string(),
                            None,
                        );
                        for (bucket, count) in &eff.score_distribution {
                            design::badge(ui, &format!("{bucket}: {count}"), BadgeTone::Neutral);
                        }
                    });
                },
            );
            ui.add_space(8.0);
        }

        design::card_titled(
            ui,
            "Customer effort (friction)",
            Some(
                "scores persist per conversation; crossing the threshold records a customer milestone",
            ),
            |ui| {
                ui.horizontal(|ui| {
                    design::badge(
                        ui,
                        &format!("{} high-friction conversation(s)", self.i_high_friction),
                        BadgeTone::Warning,
                    );
                    if design::button(ui, "Recompute (last 100 closed)", ButtonVariant::Outline)
                        .clicked()
                    {
                        match db.with_write(|conn| {
                            spp_core::reports::recompute_recent_friction(conn, 100)
                        }) {
                            Ok((computed, high)) => {
                                self.i_note = Some((
                                    true,
                                    format!("recomputed {computed} conversation(s), {high} high-friction"),
                                ));
                                self.reload(db);
                            }
                            Err(e) => {
                                self.i_note = Some((false, format!("recompute failed: {e}")))
                            }
                        }
                    }
                    if let Some((ok, msg)) = &self.i_note {
                        if *ok {
                            design::success_text(ui, msg);
                        } else {
                            design::error_text(ui, msg);
                        }
                    }
                });
                if self.i_friction_top.is_empty() {
                    design::hint_text(
                        ui,
                        "(no friction scores stored — recompute to fill the table)",
                    );
                } else {
                    let friction_top = &self.i_friction_top;
                    design::data_table(
                        ui,
                        "insights-friction",
                        &[
                            "conversation",
                            "effort",
                            "messages",
                            "resolution",
                            "reopens",
                            "escalated",
                        ],
                        friction_top.len(),
                        24.0,
                        |ui, i| {
                            let (conv, score, factors) = &friction_top[i];
                            design::td_num(ui, &conv.to_string());
                            design::td_num(ui, &format!("{score:.2}"));
                            design::td_num(
                                ui,
                                &factors
                                    .get("message_count")
                                    .and_then(serde_json::Value::as_u64)
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|| design::MISSING_VALUE.into()),
                            );
                            design::td_num(
                                ui,
                                &factors
                                    .get("resolution_hours")
                                    .and_then(serde_json::Value::as_f64)
                                    .map(|v| format!("{v:.1} h"))
                                    .unwrap_or_else(|| design::MISSING_VALUE.into()),
                            );
                            design::td_num(
                                ui,
                                &factors
                                    .get("reopen_count")
                                    .and_then(serde_json::Value::as_u64)
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|| design::MISSING_VALUE.into()),
                            );
                            design::td_num(
                                ui,
                                factors
                                    .get("escalated")
                                    .and_then(serde_json::Value::as_bool)
                                    .map(|b| if b { "yes" } else { "no" })
                                    .unwrap_or(design::MISSING_VALUE),
                            );
                        },
                    );
                }
            },
        );
        ui.add_space(8.0);

        let nodes = self.i_graph_nodes.clone();
        design::card_titled(
            ui,
            "Graph nodes",
            Some("custom nodes over the support graph; neighbors come from the live human edges"),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    design::field_label(ui, "Kind");
                    let kinds = spp_core::catalog::GraphNodeKind::ALL;
                    let kind_label = kinds[self.i_g_kind].as_str();
                    egui::ComboBox::from_id_salt("g-kind")
                        .selected_text(kind_label)
                        .show_ui(ui, |ui| {
                            for (i, kind) in kinds.iter().enumerate() {
                                ui.selectable_value(&mut self.i_g_kind, i, kind.as_str());
                            }
                        })
                        .response
                        .widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::Button,
                                ui.is_enabled(),
                                format!("kind: {kind_label}"),
                            )
                        });
                    let label_label = design::field_label(ui, "Label");
                    ui.add(egui::TextEdit::singleline(&mut self.i_g_label).desired_width(140.0))
                        .labelled_by(label_label.id);
                    let entity_label = design::field_label(ui, "Entity id");
                    ui.add(egui::TextEdit::singleline(&mut self.i_g_entity).desired_width(80.0))
                        .labelled_by(entity_label.id);
                    let props_label = design::field_label(ui, "Properties (JSON)");
                    ui.add(egui::TextEdit::singleline(&mut self.i_g_props).desired_width(180.0))
                        .labelled_by(props_label.id);
                    if design::button(ui, "Add node", ButtonVariant::Primary).clicked() {
                        self.add_graph_node_action(db);
                    }
                });
                ui.add_space(4.0);
                if nodes.is_empty() {
                    design::hint_text(ui, "(no nodes stored)");
                } else {
                    design::data_table(
                        ui,
                        "insights-graph",
                        &["Kind", "Label", "Entity", "Created", ""],
                        nodes.len(),
                        24.0,
                        |ui, i| {
                            let node = &nodes[i];
                            ui.label(node.kind.as_str());
                            ui.label(node.label.as_deref().unwrap_or(design::MISSING_VALUE));
                            design::td_num(
                                ui,
                                &node
                                    .entity_id
                                    .map(|e| e.to_string())
                                    .unwrap_or_else(|| design::MISSING_VALUE.into()),
                            );
                            design::td_num(ui, &node.created_at);
                            ui.horizontal(|ui| {
                                if design::button(ui, "Inspect", ButtonVariant::Outline).clicked() {
                                    self.inspect_graph_node(db, node.id);
                                }
                                if design::button(ui, design::DELETE_LABEL, ButtonVariant::Ghost)
                                    .on_hover_text("delete this node and its human edges")
                                    .clicked()
                                    && self.pending_confirm.is_none()
                                    && let Some(node_id) = node.id
                                {
                                    self.pending_confirm = Some(ConfirmRequest::delete_graph_node(
                                        node_id,
                                        node.label.as_deref().unwrap_or(node.kind.as_str()),
                                    ));
                                }
                            });
                        },
                    );
                }
                if self.i_graph_pick.is_some() && !self.i_graph_neighbors.is_empty() {
                    ui.add_space(4.0);
                    design::section_title(ui, "Neighbors (live human edges)");
                    ui.horizontal_wrapped(|ui| {
                        for neighbor in &self.i_graph_neighbors {
                            design::badge(
                                ui,
                                &format!("{} #{}", neighbor.kind, neighbor.local_id),
                                BadgeTone::Info,
                            );
                        }
                    });
                }
            },
        );
    }

    pub(super) fn add_graph_node_action(&mut self, db: &DbHandle) {
        let Some(kind) = spp_core::catalog::GraphNodeKind::ALL
            .get(self.i_g_kind)
            .copied()
        else {
            return;
        };
        let label = self.i_g_label.trim().to_string();
        let props = self.i_g_props.trim().to_string();
        if !props.is_empty() && serde_json::from_str::<serde_json::Value>(&props).is_err() {
            self.i_note = Some((false, "properties must be valid JSON".into()));
            return;
        }
        let entity = if self.i_g_entity.trim().is_empty() {
            Ok(None)
        } else {
            self.i_g_entity
                .trim()
                .parse::<i64>()
                .map(Some)
                .map_err(|_| "entity id must be a number".to_string())
        };
        let entity = match entity {
            Ok(e) => e,
            Err(msg) => {
                self.i_note = Some((false, msg));
                return;
            }
        };
        match db.with_write(|conn| {
            spp_core::reports::add_graph_node(
                conn,
                kind.as_str(),
                entity,
                (!label.is_empty()).then_some(label.as_str()),
                (!props.is_empty()).then_some(props.as_str()),
            )
        }) {
            Ok(_) => {
                self.i_note = Some((true, "node added".into()));
                self.i_g_label.clear();
                self.i_g_entity.clear();
                self.i_g_props.clear();
                self.refresh_graph(db);
            }
            Err(e) => self.i_note = Some((false, format!("add failed: {e}"))),
        }
    }

    fn inspect_graph_node(&mut self, db: &DbHandle, node_id: Option<i64>) {
        let Some(node_id) = node_id else {
            return;
        };
        let picked = db.with_read(|conn| {
            spp_core::reports::get_graph_node(conn, node_id)
                .ok()
                .flatten()
        });
        self.i_graph_pick = Some(node_id);
        self.i_graph_neighbors = picked
            .as_ref()
            .filter(|n| n.entity_id.is_some())
            .map(|n| {
                db.with_read(|conn| {
                    spp_core::reports::get_graph_neighbors(conn, &n.kind, n.entity_id.unwrap_or(0))
                        .unwrap_or_default()
                })
            })
            .unwrap_or_default();
    }

    fn refresh_graph(&mut self, db: &DbHandle) {
        let pick = self.i_graph_pick;
        self.i_graph_nodes = db
            .with_read(|conn| spp_core::reports::list_graph_nodes(conn, Some(50)))
            .unwrap_or_default();
        if let Some(pick) = pick {
            self.inspect_graph_node(db, Some(pick));
        }
    }
}
