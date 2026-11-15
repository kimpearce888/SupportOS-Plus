use super::*;

impl ReportsPage {
    pub(super) fn sla_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        let Some(report) = &self.sla else {
            design::empty_state(
                ui,
                Icon::Clock,
                "no SLA report",
                "No SLA data is available for the selected range.",
            );
            return;
        };
        if !report.unconfigured_mailboxes.is_empty() {
            design::card(ui, |ui| {
                let t = design::tokens(ui);
                let (id, rect) = ui.allocate_space(egui::vec2(16.0, 16.0));
                Icon::AlertCircle.paint_at(ui.painter(), rect, t.warning_foreground);
                ui.advance_cursor_after_rect(rect);
                let _ = id;
                ui.label(
                    egui::RichText::new(format!(
                        "unconfigured mailboxes: {}",
                        report.unconfigured_mailboxes.join(", ")
                    ))
                    .small()
                    .color(t.warning_foreground),
                );
            });
            ui.add_space(8.0);
        }

        let bars: Vec<(&str, i64)> = report
            .mailboxes
            .iter()
            .filter_map(|m| {
                m.first_response
                    .avg_business_min
                    .map(|v| (m.mailbox_name.as_str(), v))
            })
            .collect();
        design::card_titled(ui, "First response avg (business minutes)", None, |ui| {
            if bars.is_empty() {
                design::hint_text(ui, "(no first-response data in range)");
            } else {
                let max = bars.iter().map(|(_, v)| *v).max().unwrap_or(1);
                for (label, value) in &bars {
                    design::bar_row(ui, label, *value, max, true);
                }
                design::hint_text(ui, &format!("first response avg · max {max}"));
            }
        });
        ui.add_space(10.0);

        design::card_titled_wide(ui, "SLA by mailbox", None, |ui| {
            design::data_table(
                ui,
                "reports-sla-table",
                &[
                    "mailbox",
                    "in range",
                    "fr met/missed",
                    "fr avg (min)",
                    "res met/missed",
                    "res avg (min)",
                    "waiting oldest (min)",
                ],
                report.mailboxes.len(),
                design::scale::CONTROL_H,
                |ui, i| {
                    let t = design::tokens(ui);
                    let row = &report.mailboxes[i];
                    ui.label(
                        egui::RichText::new(row.mailbox_name.as_str())
                            .size(13.0)
                            .strong()
                            .color(t.foreground),
                    );
                    design::td_num(ui, &row.conversations_in_range.to_string());
                    design::td_num(
                        ui,
                        &format!("{}/{}", row.first_response.met, row.first_response.missed),
                    );
                    design::td_num(
                        ui,
                        &row.first_response
                            .avg_business_min
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| design::MISSING_VALUE.into()),
                    );
                    design::td_num(
                        ui,
                        &format!("{}/{}", row.resolution.met, row.resolution.missed),
                    );
                    design::td_num(
                        ui,
                        &row.resolution
                            .avg_business_min
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| design::MISSING_VALUE.into()),
                    );
                    design::td_num(
                        ui,
                        &row.waiting
                            .oldest_business_min
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| design::MISSING_VALUE.into()),
                    );
                },
            );
        });

        let mailboxes: Vec<(i64, String)> = report
            .mailboxes
            .iter()
            .map(|m| (m.mailbox_id, m.mailbox_name.clone()))
            .collect();
        design::card_titled(
            ui,
            "Persisted breach tracking",
            Some(
                "recorded breach events; the thresholds feed the coarse \
                 tracker for mailboxes without business-hours config",
            ),
            |ui| {
                design::badge(
                    ui,
                    &format!("{} recorded breach event(s)", self.tracked_breaches),
                    BadgeTone::Info,
                );
                ui.add_space(6.0);
                design::section_title(ui, "Tracking thresholds (hours)");
                ui.horizontal_wrapped(|ui| {
                    if mailboxes.is_empty() {
                        design::hint_text(ui, "(no mailboxes synced yet)");
                    }
                    for (id, name) in &mailboxes {
                        if design::chip(ui, self.sla_cfg_pick == Some(*id), name).clicked() {
                            self.pick_sla_cfg_mailbox(db, *id);
                        }
                    }
                });
                design::form_grid(ui, "sla-track-cfg", |ui| {
                    let fr_label = design::field_label(ui, "First response hours");
                    if ui
                        .add(egui::TextEdit::singleline(&mut self.sla_cfg_fr).desired_width(120.0))
                        .labelled_by(fr_label.id)
                        .changed()
                    {
                        self.sla_cfg_note = None;
                    }
                    ui.end_row();
                    let res_label = design::field_label(ui, "Resolution hours");
                    if ui
                        .add(egui::TextEdit::singleline(&mut self.sla_cfg_res).desired_width(120.0))
                        .labelled_by(res_label.id)
                        .changed()
                    {
                        self.sla_cfg_note = None;
                    }
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if design::button(ui, "Save thresholds", ButtonVariant::Primary).clicked() {
                        self.save_sla_cfg(db);
                    }
                    if let Some((ok, msg)) = &self.sla_cfg_note {
                        if *ok {
                            design::success_text(ui, msg);
                        } else {
                            design::error_text(ui, msg);
                        }
                    }
                });
                design::hint_text(
                    ui,
                    "Mailboxes with business-hours config are tracked by the live \
                     evaluation; these plain hours apply only where it has no \
                     config (defaults 24h / 72h). Breaches persist deduped per \
                     conversation and kind, and land on the customer's milestone \
                     timeline.",
                );
            },
        );
    }

    fn pick_sla_cfg_mailbox(&mut self, db: &DbHandle, mailbox_id: i64) {
        self.sla_cfg_pick = Some(mailbox_id);
        self.sla_cfg_note = None;
        let cfg = db.with_read(|conn| {
            spp_core::intelligence_features::get_sla_config(conn, mailbox_id)
                .ok()
                .flatten()
        });
        match cfg {
            Some(cfg) => {
                self.sla_cfg_fr = cfg.first_response_hours.to_string();
                self.sla_cfg_res = cfg.resolution_hours.to_string();
            }
            None => {
                self.sla_cfg_fr = "24".into();
                self.sla_cfg_res = "72".into();
            }
        }
    }

    fn save_sla_cfg(&mut self, db: &DbHandle) {
        let Some(mailbox_id) = self.sla_cfg_pick else {
            self.sla_cfg_note = Some((false, "pick a mailbox first".into()));
            return;
        };
        let fr = self.sla_cfg_fr.trim().parse::<f64>();
        let res = self.sla_cfg_res.trim().parse::<f64>();
        let (fr, res) = match (fr, res) {
            (Ok(fr), Ok(res)) if fr > 0.0 && res > 0.0 => (fr, res),
            _ => {
                self.sla_cfg_note =
                    Some((false, "both thresholds must be positive numbers".into()));
                return;
            }
        };
        let outcome = db.with_write(|conn| {
            spp_core::intelligence_features::set_sla_config(conn, mailbox_id, fr, res)?;
            spp_core::intelligence_features::sweep_unmonitored_sla(conn)?;
            Ok::<(), spp_core::error::Error>(())
        });
        match outcome {
            Ok(()) => {
                self.sla_cfg_note = Some((true, "thresholds saved".into()));
                self.tracked_breaches = db
                    .with_read(spp_core::intelligence_features::count_sla_breaches)
                    .unwrap_or(0);
            }
            Err(e) => {
                self.sla_cfg_note = Some((false, format!("save failed: {e}")));
            }
        }
    }

    pub(super) fn build_config(
        &self,
    ) -> std::result::Result<spp_core::reports::ReportBuilderConfig, String> {
        let metric = spp_core::catalog::ReportMetricKey::ALL
            .get(self.b_metric)
            .copied()
            .ok_or_else(|| "pick a metric".to_string())?;
        let dimension = spp_core::catalog::ReportDimensionKey::ALL
            .get(self.b_dimension)
            .copied()
            .ok_or_else(|| "pick a dimension".to_string())?;
        if self.b_from.trim().len() != 10 || self.b_to.trim().len() != 10 {
            return Err("dates use the YYYY-MM-DD form".into());
        }
        let mut filters = spp_core::reports::ReportFilters::default();
        let text = |s: &str| {
            let t = s.trim();
            (!t.is_empty()).then(|| t.to_string())
        };
        filters.channel = text(&self.b_f_channel);
        filters.min_priority = text(&self.b_f_min_priority);
        filters.attribute_key = text(&self.b_f_attr_key);
        filters.attribute_value = text(&self.b_f_attr_value);
        filters.state_key = text(&self.b_f_state_key);
        let list = |s: &str| {
            let items: Vec<String> = s
                .split(',')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .collect();
            (!items.is_empty()).then_some(items)
        };
        filters.statuses = list(&self.b_f_statuses);
        filters.tags_any = list(&self.b_f_tags_any);
        filters.tags_none = list(&self.b_f_tags_none);
        let ids = |s: &str| -> std::result::Result<Option<Vec<i64>>, String> {
            let mut out = Vec::new();
            for item in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
                out.push(
                    item.parse::<i64>()
                        .map_err(|_| format!("'{item}' is not an id"))?,
                );
            }
            Ok((!out.is_empty()).then_some(out))
        };
        filters.mailbox_local_ids = ids(&self.b_f_mailbox_ids)?;
        filters.assignee_local_ids = ids(&self.b_f_assignee_ids)?;
        let limit = if self.b_limit.trim().is_empty() {
            None
        } else {
            Some(
                self.b_limit
                    .trim()
                    .parse::<i64>()
                    .map_err(|_| "limit must be a number".to_string())?,
            )
        };
        Ok(spp_core::reports::ReportBuilderConfig {
            metric,
            dimension,
            date_from: self.b_from.trim().to_string(),
            date_to: self.b_to.trim().to_string(),
            comparison: if self.b_compare {
                spp_core::reports::ReportComparison::PreviousPeriod
            } else {
                spp_core::reports::ReportComparison::None
            },
            filters,
            sort: match self.b_sort {
                1 => spp_core::reports::ReportSort::MetricAsc,
                2 => spp_core::reports::ReportSort::DimensionAsc,
                _ => spp_core::reports::ReportSort::MetricDesc,
            },
            limit,
        })
    }

    pub(super) fn builder_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        use spp_core::catalog::{ReportDimensionKey, ReportMetricKey};
        design::card(ui, |ui| {
            design::section_title(ui, "Report definition");
            ui.horizontal_wrapped(|ui| {
                design::field_label(ui, "Metric");
                let metrics = ReportMetricKey::ALL;
                let metric_label =
                    spp_core::catalog::reporting::metric_entry(metrics[self.b_metric]).label;
                egui::ComboBox::from_id_salt("b-metric")
                    .selected_text(metric_label)
                    .show_ui(ui, |ui| {
                        for (i, key) in metrics.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.b_metric,
                                i,
                                spp_core::catalog::reporting::metric_entry(*key).label,
                            );
                        }
                    })
                    .response
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            ui.is_enabled(),
                            format!("metric: {metric_label}"),
                        )
                    });
                design::field_label(ui, "Grouped by");
                let dims = ReportDimensionKey::ALL;
                let dim_label =
                    spp_core::catalog::reporting::dimension_entry(dims[self.b_dimension]).label;
                egui::ComboBox::from_id_salt("b-dim")
                    .selected_text(dim_label)
                    .show_ui(ui, |ui| {
                        for (i, key) in dims.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.b_dimension,
                                i,
                                spp_core::catalog::reporting::dimension_entry(*key).label,
                            );
                        }
                    })
                    .response
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            ui.is_enabled(),
                            format!("grouped by: {dim_label}"),
                        )
                    });
                ui.checkbox(&mut self.b_compare, "Compare with previous period");
                design::field_label(ui, "Sort");
                for (i, label) in ["metric ↓", "metric ↑", "label A→Z"].iter().enumerate() {
                    if design::chip(ui, self.b_sort == i, label).clicked() {
                        self.b_sort = i;
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                let from_label = design::field_label(ui, "From (YYYY-MM-DD)");
                ui.add(egui::TextEdit::singleline(&mut self.b_from).desired_width(110.0))
                    .labelled_by(from_label.id);
                let to_label = design::field_label(ui, "To");
                ui.add(egui::TextEdit::singleline(&mut self.b_to).desired_width(110.0))
                    .labelled_by(to_label.id);
                let limit_label = design::field_label(ui, "Limit");
                ui.add(
                    egui::TextEdit::singleline(&mut self.b_limit)
                        .hint_text("50")
                        .desired_width(60.0),
                )
                .labelled_by(limit_label.id);
            });
            ui.add_space(4.0);
            design::section_title(ui, "Filters (blank = unfiltered)");
            {
                ui.horizontal(|ui| {
                    design::field_label(ui, "Statuses");
                    for status in ["active", "pending", "closed", "spam"] {
                        let selected = self
                            .b_f_statuses
                            .split(',')
                            .map(str::trim)
                            .any(|s| s == status);
                        if design::chip(ui, selected, status)
                            .on_hover_text(if selected {
                                "click to remove this status filter"
                            } else {
                                "click to filter to this status too"
                            })
                            .clicked()
                        {
                            let mut parts: Vec<String> = self
                                .b_f_statuses
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            if selected {
                                parts.retain(|p| p != status);
                            } else {
                                parts.push(status.to_string());
                            }
                            self.b_f_statuses = parts.join(",");
                        }
                    }
                });
                ui.add_space(design::scale::space::SP1);
                ui.horizontal_wrapped(|ui| {
                    design::field_label(ui, "Mailboxes");
                    for (id, name) in self.b_mailboxes.clone() {
                        let selected = csv_contains_id(&self.b_f_mailbox_ids, id);
                        if design::chip(ui, selected, name.as_str())
                            .on_hover_text(if selected {
                                "click to remove this mailbox filter"
                            } else {
                                "click to filter to this mailbox too"
                            })
                            .clicked()
                        {
                            self.b_f_mailbox_ids = toggle_csv_id(&self.b_f_mailbox_ids, id);
                        }
                    }
                    if self.b_mailboxes.is_empty() {
                        design::hint_text(ui, "no mailboxes synced yet");
                    }
                });
                ui.add_space(design::scale::space::SP1);
                ui.horizontal_wrapped(|ui| {
                    design::field_label(ui, "Assignees");
                    for (id, name) in self.b_assignees.clone() {
                        let selected = csv_contains_id(&self.b_f_assignee_ids, id);
                        if design::chip(ui, selected, name.as_str())
                            .on_hover_text(if selected {
                                "click to remove this assignee filter"
                            } else {
                                "click to filter to this assignee too"
                            })
                            .clicked()
                        {
                            self.b_f_assignee_ids = toggle_csv_id(&self.b_f_assignee_ids, id);
                        }
                    }
                    if self.b_assignees.is_empty() {
                        design::hint_text(ui, "no synced users yet");
                    }
                });
                ui.add_space(design::scale::space::SP1);
            }
            design::form_grid(ui, "b-filters", |ui| {
                for (label, field) in [
                    ("channel:", &mut self.b_f_channel),
                    ("tags any (csv):", &mut self.b_f_tags_any),
                    ("tags none (csv):", &mut self.b_f_tags_none),
                    ("min priority:", &mut self.b_f_min_priority),
                    ("attribute key:", &mut self.b_f_attr_key),
                    ("attribute value:", &mut self.b_f_attr_value),
                    ("state key:", &mut self.b_f_state_key),
                ] {
                    let label = design::field_label(ui, label);
                    ui.add(egui::TextEdit::singleline(field).desired_width(180.0))
                        .labelled_by(label.id);
                    ui.end_row();
                }
            });
            ui.horizontal_wrapped(|ui| {
                if design::button(ui, "Run report", ButtonVariant::Primary).clicked() {
                    self.b_outcome = Some(match self.build_config() {
                        Ok(config) => db
                            .with_read(|conn| spp_core::reports::run_builder_report(conn, &config)),
                        Err(e) => Err(e),
                    });
                }
                let save_label = design::field_label(ui, "Save as");
                ui.add(
                    egui::TextEdit::singleline(&mut self.b_save_name)
                        .hint_text("name")
                        .desired_width(160.0),
                )
                .labelled_by(save_label.id);
                if design::button(ui, "Save", ButtonVariant::Outline).clicked() {
                    self.save_report_config(db);
                }
                if let Some((ok, msg)) = &self.b_note {
                    if *ok {
                        design::success_text(ui, msg);
                    } else {
                        design::error_text(ui, msg);
                    }
                }
            });
        });
        ui.add_space(8.0);

        design::card_titled(
            ui,
            "Saved reports",
            Some("load or delete a saved configuration"),
            |ui| {
                let saved = self.b_saved.clone();
                if saved.is_empty() {
                    design::hint_text(ui, "(none saved yet)");
                }
                for saved in &saved {
                    ui.horizontal(|ui| {
                        ui.monospace(&saved.name);
                        design::hint_text(ui, &saved.created_at);
                        if design::button(ui, "Load", ButtonVariant::Outline).clicked() {
                            self.load_saved_report(&saved.config);
                        }
                        if design::button(ui, design::DELETE_LABEL, ButtonVariant::Destructive)
                            .clicked()
                        {
                            self.pending_confirm =
                                Some(ConfirmRequest::delete_saved_report(saved.id, &saved.name));
                        }
                    });
                }
            },
        );
        ui.add_space(8.0);

        match &self.b_outcome {
            None => design::hint_text(ui, "run a report to see its rows"),
            Some(Err(e)) => design::error_text(ui, &format!("error: {e}")),
            Some(Ok(outcome)) => {
                design::card_titled_wide(
                    ui,
                    &format!("{} — {}", outcome.metric.label, outcome.dimension.label),
                    Some(&format!(
                        "{} → {}",
                        outcome.date_range.date_from, outcome.date_range.date_to
                    )),
                    |ui| {
                        design::badge(ui, outcome.origin, BadgeTone::Info);
                        ui.add_space(4.0);
                        if outcome.rows.is_empty() {
                            design::empty_state(
                                ui,
                                Icon::BarChart,
                                "no rows",
                                "Nothing matched in the range.",
                            );
                        } else {
                            let format = outcome.metric.format;
                            let has_comparison = outcome.comparison_rows.is_some();
                            let headers: Vec<&str> = if has_comparison {
                                vec!["group", "value", "Δ vs previous"]
                            } else {
                                vec!["group", "value"]
                            };
                            design::data_table(
                                ui,
                                "b-rows",
                                &headers,
                                outcome.rows.len(),
                                24.0,
                                |ui, i| {
                                    let row = &outcome.rows[i];
                                    ui.label(&row.dimension_label);
                                    design::td_num(
                                        ui,
                                        &Self::format_metric_value(format, row.value),
                                    );
                                    if has_comparison {
                                        let delta =
                                            outcome.comparison_rows.as_ref().and_then(|prev| {
                                                prev.iter()
                                                    .find(|p| {
                                                        p.dimension_value == row.dimension_value
                                                    })
                                                    .map(|p| row.value - p.value)
                                            });
                                        match delta {
                                            Some(d) => design::td_num(ui, &format!("{d:+.1}")),
                                            None => design::td_num(ui, design::MISSING_VALUE),
                                        }
                                    }
                                },
                            );
                        }
                        ui.add_space(4.0);
                        for note in &outcome.notes {
                            design::hint_text(ui, note);
                        }
                    },
                );
            }
        }
    }

    fn format_metric_value(format: spp_core::catalog::reporting::MetricFormat, v: f64) -> String {
        use spp_core::catalog::reporting::MetricFormat;
        match format {
            MetricFormat::Count => format!("{}", v.round() as i64),
            MetricFormat::Rate => format!("{:.1}%", v * 100.0),
            MetricFormat::Minutes => format!("{:.1} min", v),
            MetricFormat::Hours => format!("{:.1} h", v),
            MetricFormat::Score => format!("{:.2}", v),
        }
    }

    pub(super) fn save_report_config(&mut self, db: &DbHandle) {
        let name = self.b_save_name.trim().to_string();
        if name.is_empty() {
            self.b_note = Some((false, "give the report a name first".into()));
            return;
        }
        match self.build_config() {
            Ok(config) => {
                match db.with_write(|conn| spp_core::reports::save_report(conn, &name, &config)) {
                    Ok(saved) => {
                        self.b_note = Some((true, format!("saved as \"{}\"", saved.name)));
                        self.b_save_name.clear();
                        self.b_saved = db
                            .with_read(spp_core::reports::list_saved_reports)
                            .unwrap_or_default();
                    }
                    Err(e) => self.b_note = Some((false, format!("save failed: {e}"))),
                }
            }
            Err(e) => self.b_note = Some((false, e)),
        }
    }

    pub(super) fn load_saved_report(&mut self, config: &serde_json::Value) {
        match spp_core::reports::parse_report_config(config) {
            Ok(config) => {
                self.b_metric = spp_core::catalog::ReportMetricKey::ALL
                    .iter()
                    .position(|k| *k == config.metric)
                    .unwrap_or(0);
                self.b_dimension = spp_core::catalog::ReportDimensionKey::ALL
                    .iter()
                    .position(|k| *k == config.dimension)
                    .unwrap_or(0);
                self.b_from = config.date_from;
                self.b_to = config.date_to;
                self.b_compare =
                    config.comparison == spp_core::reports::ReportComparison::PreviousPeriod;
                self.b_sort = match config.sort {
                    spp_core::reports::ReportSort::MetricAsc => 1,
                    spp_core::reports::ReportSort::DimensionAsc => 2,
                    spp_core::reports::ReportSort::MetricDesc => 0,
                };
                self.b_limit = config.limit.map(|l| l.to_string()).unwrap_or_default();
                let f = config.filters;
                self.b_f_channel = f.channel.unwrap_or_default();
                self.b_f_statuses = f.statuses.unwrap_or_default().join(", ");
                self.b_f_tags_any = f.tags_any.unwrap_or_default().join(", ");
                self.b_f_tags_none = f.tags_none.unwrap_or_default().join(", ");
                self.b_f_min_priority = f.min_priority.unwrap_or_default();
                self.b_f_attr_key = f.attribute_key.unwrap_or_default();
                self.b_f_attr_value = f.attribute_value.unwrap_or_default();
                self.b_f_state_key = f.state_key.unwrap_or_default();
                self.b_f_mailbox_ids = f
                    .mailbox_local_ids
                    .map(|v| {
                        v.iter()
                            .map(|i| i.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                self.b_f_assignee_ids = f
                    .assignee_local_ids
                    .map(|v| {
                        v.iter()
                            .map(|i| i.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                self.b_note = Some((true, "loaded — run to refresh".into()));
            }
            Err(issues) => {
                let first = issues
                    .first()
                    .map(|(f, m)| format!("{f}: {m}"))
                    .unwrap_or_else(|| "invalid saved config".into());
                self.b_note = Some((false, first));
            }
        }
    }

    pub(crate) fn delete_saved_report(&mut self, db: &DbHandle, id: i64) -> spp_core::Result<()> {
        match db.with_write(|conn| spp_core::reports::delete_saved_report(conn, id)) {
            Ok(true) => {
                self.b_note = Some((true, "deleted".into()));
                self.b_saved = db
                    .with_read(spp_core::reports::list_saved_reports)
                    .unwrap_or_default();
                Ok(())
            }
            Ok(false) => {
                self.b_note = Some((false, "already gone".into()));
                Ok(())
            }
            Err(e) => {
                self.b_note = Some((false, format!("delete failed: {e}")));
                Err(e)
            }
        }
    }
}
