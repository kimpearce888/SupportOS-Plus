use super::*;

impl SettingsPage {
    pub(super) fn helpscout_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.poll_connect_result(db);
        self.poll_doctor_result();
        let Some(status) = self.helpscout.clone() else {
            ui.spinner();
            return;
        };
        match spp_core::loopback::bound_port() {
            Some(port) => design::hint_text(
                ui,
                &format!(
                    "loopback listener answering on :{port} (webhook ingress + OAuth callback)"
                ),
            ),
            None => design::error_text(
                ui,
                "loopback listener NOT running — webhook ingress and the OAuth \
                 callback are unavailable; check the startup log for the bind error",
            ),
        }
        ui.add_space(4.0);
        self.hs_connection_card_ui(ui, db, &status);
        ui.add_space(10.0);

        self.connection_doctor_ui(ui);
        ui.add_space(10.0);

        self.hs_credentials_card_ui(ui, db, &status);
        ui.add_space(10.0);

        self.webhook_relay_card_ui(ui, db);
        ui.add_space(10.0);

        self.workspace_purge_card_ui(ui, db);
    }

    fn hs_connection_card_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        status: &HelpScoutStatus,
    ) {
        design::card_titled_wide(
            ui,
            "Help Scout connection",
            Some(
                "Connect with client credentials (recommended — no browser, \
                 no redirect URL) or the advanced browser flow",
            ),
            |ui| {
                design::form_grid(ui, "settings-helpscout", |ui| {
                    design::field_label(ui, "Credentials stored");
                    ui.horizontal(|ui| {
                        design::badge(
                            ui,
                            if status.configured {
                                "connected"
                            } else if status.credentials {
                                "credentials saved"
                            } else {
                                "not configured"
                            },
                            if status.configured {
                                BadgeTone::Success
                            } else {
                                BadgeTone::Neutral
                            },
                        );
                    });
                    ui.end_row();
                    design::field_label(ui, "Redirect URI");
                    ui.horizontal(|ui| {
                        ui.monospace(
                            egui::RichText::new(status.redirect_uri.as_str())
                                .size(12.5)
                                .color(design::tokens(ui).foreground),
                        )
                        .on_hover_text(
                            "only for the browser flow: register this EXACT URL as \
                             the callback URL of your Help Scout custom app \
                             (Profile → Apps & Integrations). Help Scout matches \
                             the redirect exactly. The local listener binds its \
                             port when SupportOS+ starts; change it with \
                             HELPSCOUT_REDIRECT_URI.",
                        );
                    });
                    ui.end_row();
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if status.configured
                        && design::button(ui, "Disconnect", ButtonVariant::Destructive)
                            .on_hover_text("drops the stored tokens; local data is fully preserved")
                            .clicked()
                    {
                        self.disconnect_helpscout(db);
                    }
                    if !status.configured
                        && status.credentials
                        && !self.hs_connecting
                        && design::button(
                            ui,
                            "Connect with Client Credentials (Recommended)",
                            ButtonVariant::Primary,
                        )
                        .on_hover_text(
                            "no browser round-trip: verifies the stored client \
                             id/secret against Help Scout and stores the token \
                             directly",
                        )
                        .clicked()
                    {
                        self.connect_client_credentials();
                    }
                    if self.hs_connecting {
                        ui.spinner();
                    }
                });
                if status.credentials {
                    ui.add_space(4.0);
                    ui.collapsing(
                        egui::RichText::new("advanced: browser authorization flow")
                            .small()
                            .color(design::tokens(ui).muted_foreground),
                        |ui| {
                            design::hint_text(
                                ui,
                                "Help Scout redirects the browser to the callback URL \
                                 registered on your custom app — it must match the \
                                 redirect URI above EXACTLY, and loopback \
                                 (localhost) redirects are undocumented on the \
                                 platform. This flow signs in as your Help Scout \
                                 user; the client-credentials path above is the \
                                 recommended default.",
                            );
                            ui.add_space(4.0);
                            if design::button(
                                ui,
                                "Open the Authorization Page in the Browser",
                                ButtonVariant::Outline,
                            )
                            .clicked()
                            {
                                self.begin_oauth(db);
                            }
                        },
                    );
                }
                if let Some(url) = &self.authorize_url {
                    ui.add_space(6.0);
                    ui.hyperlink_to(
                        egui::RichText::new("open the Help Scout authorization page").size(13.0),
                        url.clone(),
                    );
                    ui.add_space(2.0);
                    design::hint_text(
                        ui,
                        "after approving, Help Scout redirects the browser to the \
                         redirect URI above; leave SupportOS+ running.",
                    );
                }
            },
        );
    }

    fn hs_credentials_card_ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        status: &HelpScoutStatus,
    ) {
        design::card_titled_wide(
            ui,
            "Help Scout credentials",
            Some(
                "Stored encrypted on this machine. Save them and the app \
                 connects immediately — the Connection Doctor verifies the \
                 chain and the first sync starts by itself. Environment \
                 variables override these values.",
            ),
            |ui| {
                let mask = !self.hs_show_secrets;
                design::form_grid(ui, "settings-hs-creds", |ui| {
                    let cid_label = design::field_label(ui, "Client id");
                    ui.add_sized(
                        [320.0, design::scale::CONTROL_H],
                        egui::TextEdit::singleline(&mut self.hs_client_id)
                            .hint_text("Help Scout custom app — client ID"),
                    )
                    .labelled_by(cid_label.id);
                    ui.end_row();
                    let secret_label = design::field_label(ui, "Client secret");
                    ui.add_sized(
                        [320.0, design::scale::CONTROL_H],
                        egui::TextEdit::singleline(&mut self.hs_client_secret)
                            .password(mask)
                            .hint_text("leave blank to keep the stored value"),
                    )
                    .labelled_by(secret_label.id);
                    ui.end_row();
                    let whs_label = design::field_label(ui, "Webhook secret");
                    ui.add_sized(
                        [320.0, design::scale::CONTROL_H],
                        egui::TextEdit::singleline(&mut self.hs_webhook_secret)
                            .password(mask)
                            .hint_text("verifies incoming Help Scout webhooks"),
                    )
                    .labelled_by(whs_label.id)
                    .on_hover_text(
                        "Help Scout signs each delivery with the full secret you \
                         set there, and SupportOS+ verifies against the full \
                         secret too — paste the exact same value on both sides.",
                    );
                    ui.end_row();
                    let docs_label = design::field_label(ui, "Docs API key");
                    ui.add_sized(
                        [320.0, design::scale::CONTROL_H],
                        egui::TextEdit::singleline(&mut self.hs_docs_api_key)
                            .password(mask)
                            .hint_text("optional — leave blank to keep the stored value"),
                    )
                    .labelled_by(docs_label.id);
                    ui.end_row();
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let next = design::switch(ui, self.hs_show_secrets);
                    if next != self.hs_show_secrets {
                        self.hs_show_secrets = next;
                    }
                    ui.label("Show secrets");
                    if design::button(ui, "Save Credentials", ButtonVariant::Primary).clicked() {
                        self.save_helpscout_credentials(db);
                    }
                    let any_stored = status.sources.client_id.is_set()
                        || status.sources.client_secret.is_set()
                        || status.sources.webhook_secret.is_set()
                        || status.sources.docs_api_key.is_set();
                    if any_stored
                        && design::button(
                            ui,
                            "Clear Stored Credentials",
                            ButtonVariant::Destructive,
                        )
                        .clicked()
                    {
                        self.clear_helpscout_credentials(db);
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let sources = status.sources;
                    for (label, source) in [
                        ("client id", sources.client_id),
                        ("client secret", sources.client_secret),
                        ("webhook secret", sources.webhook_secret),
                        ("docs api key", sources.docs_api_key),
                    ] {
                        design::hint_text(ui, &format!("{label}:"));
                        design::badge(
                            ui,
                            source.label(),
                            if source.is_set() {
                                BadgeTone::Success
                            } else {
                                BadgeTone::Neutral
                            },
                        );
                    }
                });
            },
        );
    }

    fn webhook_relay_card_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card_titled_wide(
            ui,
            "Webhook relay & health",
            Some(
                "the listener stays loopback-only by design; a user-run relay \
                 bridges the NAT, and the health check re-registers the hook \
                 when the platform silently drops it",
            ),
            |ui| {
                let relay_label = design::field_label(ui, "Public relay URL");
                ui.add(
                    egui::TextEdit::singleline(&mut self.webhook_public_url)
                        .hint_text("https://your-relay → forwards to this app's loopback listener"),
                )
                .labelled_by(relay_label.id);
                ui.horizontal(|ui| {
                    if design::button(ui, "Save Relay URL", ButtonVariant::Outline).clicked() {
                        self.save_webhook_relay_url(db);
                    }
                    if design::button(ui, "Check Now", ButtonVariant::Primary).clicked() {
                        self.queue_webhook_health_check(db);
                    }
                });
                design::hint_text(
                    ui,
                    "relay options: cloudflared tunnel --url http://127.0.0.1:PORT · \
                     ngrok http 127.0.0.1:PORT · ssh -N -R 80:127.0.0.1:PORT serveo.net \
                     — the registered URL must forward to the loopback listener",
                );
                design::hint_text(
                    ui,
                    "without a relay the app is polling-only: conversations and \
                     customers still arrive on the polling cadence, but beacon live \
                     chats and satisfaction ratings cannot arrive at all — Help Scout \
                     has no list endpoint for them, the webhook is the only data path",
                );
                design::hint_text(
                    ui,
                    "prove the chain from outside: curl -X POST <relay>/webhook -d '{}' \
                     must answer 401 (signature check) — anything else means the relay \
                     is not reaching the app",
                );
                design::hint_text(
                    ui,
                    "one honest limit: the API never reveals a hook's stored secret, so \
                     rotating the local webhook secret needs a re-register — the check \
                     does that for you when it finds the hook stale",
                );
                match &self.wh_outcome {
                    Some(outcome) => {
                        let tone = if outcome.is_healthy() {
                            BadgeTone::Success
                        } else {
                            BadgeTone::Neutral
                        };
                        ui.horizontal(|ui| {
                            design::field_label(ui, "Last check");
                            design::badge(ui, &outcome.verdict, tone);
                            ui.label(format!("· {}", outcome.checked_at));
                        });
                        design::hint_text(ui, &outcome.detail);
                    }
                    None => {
                        design::hint_text(
                            ui,
                            "the webhook guard has not run yet (it checks every ~6 hours, \
                             or press check now)",
                        );
                    }
                }
            },
        );
    }

    fn workspace_purge_card_ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        design::card_titled_wide(
            ui,
            "Danger zone — Workspace data",
            Some("irreversible"),
            |ui| {
                design::hint_text(
                    ui,
                    "Deletes every conversation, customer, thread, rating, \
                     knowledge document, incident, report and AI record — all \
                     synced content (and anything left over from the seeded \
                     content that shipped before 1.2.0) — and arms a fresh first sync. \
                     Keeps: credentials, settings, business hours, automation \
                     rules, saved views and replies, custom object types.",
                );
                if !self.purge_armed {
                    if design::button(ui, "Delete All Workspace Data", ButtonVariant::Destructive)
                        .clicked()
                    {
                        self.purge_armed = true;
                    }
                } else {
                    design::error_text(
                        ui,
                        "This cannot be undone. Everything listed above will be deleted.",
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if design::button(ui, design::CANCEL_LABEL, ButtonVariant::Outline)
                            .clicked()
                        {
                            self.purge_armed = false;
                        }
                        if design::button(
                            ui,
                            "Really Delete Everything",
                            ButtonVariant::Destructive,
                        )
                        .clicked()
                        {
                            self.reset_workspace(db);
                        }
                    });
                }
            },
        );
    }

    fn save_webhook_relay_url(&mut self, db: &DbHandle) {
        let url = self.webhook_public_url.trim().to_string();
        let saved =
            db.with_write(|conn| spp_core::settings::set_string(conn, "webhook_public_url", &url));
        match saved {
            Ok(()) => {
                self.message =
                    Some("relay URL saved — the next health check registers the hook there".into());
            }
            Err(e) => self.error = Some(format!("could not save the relay URL: {e}")),
        }
    }
}
