use super::*;

impl SettingsPage {
    pub(super) fn begin_oauth(&mut self, db: &DbHandle) {
        let live = self.provider.as_ref().and_then(|p| p.real());
        let Some(_real) = live else {
            self.error = Some(
                "the browser flow needs an active Help Scout connection. \
                 Save your client credentials in the form below first — \
                 they activate immediately, no restart needed."
                    .into(),
            );
            self.authorize_url = None;
            return;
        };
        let (creds, _) = db.with_read(spp_core::helpscout_real::HsCredentials::resolve);
        let redirect_uri = creds.redirect_uri.clone();
        self.begin_oauth_at(db, &creds, &redirect_uri);
    }

    pub(super) fn begin_oauth_at(
        &mut self,
        db: &DbHandle,
        creds: &spp_core::helpscout_real::HsCredentials,
        redirect_uri: &str,
    ) {
        let bound = spp_core::loopback::bound_port()
            .or_else(|| spp_core::loopback::port_from_redirect_uri(redirect_uri));
        self.begin_oauth_on(db, creds, redirect_uri, bound);
    }

    pub(super) fn begin_oauth_on(
        &mut self,
        db: &DbHandle,
        creds: &spp_core::helpscout_real::HsCredentials,
        redirect_uri: &str,
        bound: Option<u16>,
    ) {
        let Some(bound_port) = bound else {
            self.error = Some(
                "the SupportOS+ callback listener is not running — webhooks and \
                 the OAuth callback are unavailable. Restart SupportOS+ and \
                 check the log for the listener bind error."
                    .into(),
            );
            self.authorize_url = None;
            return;
        };
        let effective_redirect = match spp_core::loopback::port_from_redirect_uri(redirect_uri) {
            Some(port) if port == bound_port => redirect_uri.to_string(),
            Some(_) => format!("http://localhost:{bound_port}/oauth/callback"),
            None => redirect_uri.to_string(),
        };
        if !spp_core::loopback::listener_answers(bound_port) {
            self.error = Some(format!(
                "the SupportOS+ callback listener is not answering on port \
                 {bound_port}. Either SupportOS+ was not restarted after the \
                 credentials were saved, or another program is using the \
                 port. Restart SupportOS+ and try again; if the port is \
                 taken, close that program (or set HELPSCOUT_REDIRECT_URI \
                 to a free port and register the same URL on your Help \
                 Scout app)."
            ));
            self.authorize_url = None;
            return;
        }
        match spp_core::loopback::begin_oauth(db, creds, &effective_redirect) {
            Ok(url) => {
                self.message = Some(format!(
                    "Authorization URL ready — the callback listener answers on \
                     :{bound_port}. Register {effective_redirect} EXACTLY as the \
                     callback URL on your Help Scout app if it differs from what \
                     is registered. Complete the flow in the browser; the loopback \
                     listener stores the token."
                ));
                self.authorize_url = Some(url);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub(super) fn connect_client_credentials(&mut self) {
        let Some(real) = self.provider.as_ref().and_then(|p| p.real()) else {
            self.error = Some(
                "connect needs an active Help Scout connection — save your \
                 credentials in the form below first (they activate \
                 immediately, no restart)."
                    .into(),
            );
            return;
        };
        if self.hs_connecting {
            return;
        }
        self.hs_connecting = true;
        self.message = Some("connecting with client credentials…".into());
        let slot = std::sync::Arc::clone(&self.hs_connect_result);
        let bus = self.bus.clone();
        let spawned = std::thread::Builder::new()
            .name("spp-hs-client-credentials".to_string())
            .spawn(move || {
                let outcome = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("cannot start the connect runtime: {e}"))
                    .and_then(|rt| {
                        rt.block_on(async { real.client_credentials_login().await.map(|_| ()) })
                            .map_err(|e| e.to_string())
                    })
                    .map_or_else(
                        |e| {
                            if let Some(bus) = &bus {
                                spp_core::event_bus::notify_error(
                                    bus,
                                    "error",
                                    &format!("Connecting with client credentials failed: {e}"),
                                    "connect",
                                    None,
                                );
                            }
                            Err(e)
                        },
                        |()| {
                            Ok("connected with client credentials — tokens stored. \
                                The sync pulls your account automatically."
                                .to_string())
                        },
                    );
                if let Ok(mut guard) = slot.lock() {
                    *guard = Some(outcome);
                }
            });
        if let Err(e) = spawned {
            self.hs_connecting = false;
            self.error = Some(format!("cannot start the connect thread: {e}"));
        }
    }

    pub(super) fn poll_connect_result(&mut self, db: &DbHandle) {
        if !self.hs_connecting {
            return;
        }
        let outcome = self
            .hs_connect_result
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        let Some(outcome) = outcome else {
            return;
        };
        self.hs_connecting = false;
        match outcome {
            Ok(msg) => {
                self.message = Some(msg);
                self.invalidate();
                self.reload(db);
            }
            Err(e) => self.error = Some(e),
        }
    }
}
