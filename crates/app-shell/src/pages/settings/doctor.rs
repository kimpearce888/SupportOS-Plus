use super::*;

impl SettingsPage {
    pub(super) fn run_connection_doctor(&mut self) {
        let Some(handle) = self.provider.clone() else {
            self.error = Some("the provider handle is not available (test session)".into());
            return;
        };
        if self.doctor_running {
            return;
        }
        self.doctor_running = true;
        let slot = std::sync::Arc::clone(&self.doctor_result);
        let bus = self.bus.clone();
        let spawned = std::thread::Builder::new()
            .name("spp-connection-doctor".to_string())
            .spawn(move || {
                let Some(real) = handle.real() else {
                    if let Ok(mut guard) = slot.lock() {
                        *guard = Some(Vec::new());
                    }
                    return;
                };
                let outcome = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("cannot start the doctor runtime: {e}"))
                    .map(|rt| {
                        rt.block_on(async { spp_core::connection_doctor::run_doctor(&real).await })
                    });
                match outcome {
                    Ok(steps) => {
                        if let Some(step) = steps
                            .iter()
                            .find(|s| s.status == spp_core::connection_doctor::DoctorStatus::Fail)
                            && let Some(bus) = &bus
                        {
                            spp_core::event_bus::notify_error(
                                bus,
                                "error",
                                &format!(
                                    "Connection check failed: {} — {}",
                                    step.name, step.detail
                                ),
                                "connect",
                                None,
                            );
                        }
                        if let Ok(mut guard) = slot.lock() {
                            *guard = Some(steps);
                        }
                    }
                    Err(e) => {
                        if let Some(bus) = &bus {
                            spp_core::event_bus::notify_error(
                                bus,
                                "error",
                                &format!("the Connection Doctor could not run: {e}"),
                                "connect",
                                None,
                            );
                        }
                        if let Ok(mut guard) = slot.lock() {
                            *guard = Some(Vec::new());
                        }
                    }
                }
            });
        if let Err(e) = spawned {
            self.doctor_running = false;
            self.error = Some(format!("cannot start the doctor thread: {e}"));
        }
    }

    pub(super) fn connection_doctor_ui(&mut self, ui: &mut egui::Ui) {
        design::card_titled_wide(
            ui,
            "Connection Doctor",
            Some(
                "five checks: reach the API → sign in → read the user → \
                 list mailboxes → rate-limit budget",
            ),
            |ui| {
                if self.doctor_running {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("checking the connection…");
                    });
                } else {
                    let live = self.provider.as_ref().is_some_and(|p| p.is_real());
                    if live
                        && design::button(ui, "Run the Checks", ButtonVariant::Outline)
                            .on_hover_text(
                                "verifies the whole connection chain right now: \
                                 network, credentials, token, identity, mailboxes, \
                                 rate limits",
                            )
                            .clicked()
                    {
                        self.run_connection_doctor();
                    }
                    if !live {
                        design::hint_text(
                            ui,
                            "save credentials above (or press connect) and the doctor \
                             runs automatically — or run it any time while a \
                             connection is active.",
                        );
                    }
                }
                if !self.doctor_steps.is_empty() {
                    let steps = self.doctor_steps.clone();
                    ui.add_space(6.0);
                    egui::Grid::new(egui::IdSalt::new("doctor-steps"))
                        .num_columns(2)
                        .striped(true)
                        .spacing(egui::vec2(16.0, 4.0))
                        .show(ui, |ui| {
                            use spp_core::connection_doctor::DoctorStatus;
                            let dt = design::tokens(ui);
                            for step in &steps {
                                let (tone, label) = match step.status {
                                    DoctorStatus::Pass => (dt.success, "✓ ok"),
                                    DoctorStatus::Warn => (dt.warning, "! warning"),
                                    DoctorStatus::Fail => (dt.destructive, "✗ failed"),
                                };
                                ui.label(
                                    egui::RichText::new(format!("{label} — {}", step.name))
                                        .color(tone)
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(format!("{} {}", step.detail, step.remedy))
                                        .small(),
                                );
                                ui.end_row();
                            }
                        });
                }
            },
        );
    }

    pub(super) fn poll_doctor_result(&mut self) {
        if !self.doctor_running {
            return;
        }
        let done = self
            .doctor_result
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        if let Some(steps) = done {
            self.doctor_running = false;
            self.doctor_steps = steps;
        }
    }
}
