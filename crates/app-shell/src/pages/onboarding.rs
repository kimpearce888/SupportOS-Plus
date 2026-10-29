//! Onboarding page.
//!
//! RS-20 reskin: rebuilt onto the DC widget set (`crate::design`) per the
//! conformed lab's `onboarding` sheet (DS "stack" archetype): the Setup
//! status card renders the lab's DS.row anatomy — the DS-C-06 state dot
//! (the lab's okFlag: conversations > 0, "true", "receiving") + the
//! strong key left, the mono value pushed right — over the same five
//! status keys; the toolbar (Run initial sync primary + Mark complete
//! outline) and the conditional hints stay. Status from the onboarding
//! route logic (first-run flag, conversation count, Help Scout credential
//! presence, sync state) and the action buttons: run the initial sync
//! (`/api/sync/initial`, dispatched through the shared sync engine) and
//! mark onboarding complete (`/api/onboarding/complete`).
//!
//! Since 1.2.0 a fresh install is empty until Help Scout is connected:
//! nothing is pre-seeded, nothing is stand-in content.

use std::sync::Arc;

use eframe::egui;
use rusqlite::Connection;
use spp_core::db::DbHandle;
use spp_core::sync_engine::SyncEngine;

use crate::design::{self, ButtonVariant};
use crate::pages::sync_health::SyncAction;

/// Onboarding status (typed port of the `/api/onboarding` route).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OnboardingStatus {
    pub step: String,
    pub completed: bool,
    pub conversations: i64,
    pub hs_configured: bool,
    pub sync_state: String,
}

/// Load the onboarding status (the route's SQL, in-process).
#[must_use]
pub fn load_status(conn: &Connection) -> OnboardingStatus {
    let first_run = spp_core::settings::first_run_done(conn).unwrap_or(false);
    let conv_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
        .unwrap_or(0);
    // 1.2.0: credentials count as "configured" the moment the user
    // saves them (the encrypted envelope), not only after a token
    // exists — the 1.1.x check missed the freshly-connected state the
    // user actually starts from.
    let credentials_configured = spp_core::helpscout_real::HsCredentials::resolve(conn)
        .0
        .is_configured();
    let token_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM oauth_tokens WHERE access_token IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let sync_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sync_runs", [], |r| r.get(0))
        .unwrap_or(0);
    let webhook_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM webhook_events", [], |r| r.get(0))
        .unwrap_or(0);
    let sync_state = if webhook_count > 0 {
        "receiving"
    } else if sync_count > 0 {
        "registered"
    } else {
        "new"
    };
    OnboardingStatus {
        step: if first_run { "complete" } else { "welcome" }.into(),
        completed: first_run,
        conversations: conv_count,
        hs_configured: credentials_configured || token_count > 0,
        sync_state: sync_state.into(),
    }
}

/// The interactive onboarding page state.
pub struct OnboardingPage {
    status: Option<OnboardingStatus>,
    message: Option<String>,
}

impl OnboardingPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            status: None,
            message: None,
        }
    }

    /// Live-update invalidation.
    pub fn invalidate(&mut self) {
        self.status = None;
    }

    fn reload(&mut self, db: &DbHandle) {
        self.status = Some(db.with_read(load_status));
    }

    /// Mark onboarding complete (the `/api/onboarding/complete` route logic).
    pub fn complete(&mut self, db: &DbHandle) {
        db.with_write(|conn| {
            let _ = spp_core::settings::mark_first_run_done(conn);
        });
        self.message = Some("Onboarding complete.".into());
        self.reload(db);
    }

    // Clean-audit item 145: the `save_step` route logic (writing the
    // `onboarding_step` setting) had no caller and nothing ever read the
    // setting — the page's step is derived from `first_run_done`. The
    // dead writer and its unread row are removed.

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, engine: Option<&Arc<SyncEngine>>) {
        if self.status.is_none() {
            self.reload(db);
        }

        design::page_header(
            ui,
            "Welcome to SupportOS+",
            Some(
                "Connect your Help Scout account in Settings to pull your \
                 real conversations. The app starts empty: no sample \
                 content, no stand-ins — only your real history.",
            ),
            |_| {},
        );

        let status = self.status.clone().unwrap_or_default();
        design::card_titled(ui, "Setup status", None, |ui| {
            let t = design::tokens(ui);
            // The lab's DS.row anatomy: the DS-C-06 state dot (the lab's
            // okFlag — conversations > 0, "true", "receiving") + the
            // strong key left, the mono value pushed right.
            let rows = [
                ("step:", status.step.clone(), status.step == "complete"),
                (
                    "onboarding complete:",
                    status.completed.to_string(),
                    status.completed,
                ),
                (
                    "conversations:",
                    status.conversations.to_string(),
                    status.conversations > 0,
                ),
                (
                    "Help Scout credentials:",
                    status.hs_configured.to_string(),
                    status.hs_configured,
                ),
                (
                    "sync state:",
                    status.sync_state.clone(),
                    status.sync_state == "receiving",
                ),
            ];
            for (key, value, ok) in rows {
                ui.horizontal(|ui| {
                    design::status_dot(
                        ui,
                        if ok {
                            crate::design::BadgeTone::Success
                        } else {
                            crate::design::BadgeTone::Neutral
                        },
                        false,
                    );
                    ui.label(
                        egui::RichText::new(key)
                            .size(13.5)
                            .strong()
                            .color(t.foreground),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.monospace(egui::RichText::new(value).color(t.foreground));
                    });
                });
            }
        });

        if let Some(message) = &self.message {
            design::success_text(ui, &format!("status: {message}"));
        }

        ui.horizontal(|ui| {
            if design::button(ui, "Run initial sync", ButtonVariant::Primary).clicked() {
                match engine {
                    Some(engine) => {
                        if engine.is_running() {
                            self.message = Some("A sync is already running.".into());
                        } else {
                            // Same dispatch as the Sync Health page.
                            let mut sync_page = crate::pages::sync_health::SyncHealthPage::new();
                            sync_page.run_sync_action(db, engine, SyncAction::Initial);
                            self.message = Some(
                                "Initial sync started. Watch Sync Health or Diagnostics \
                                 for progress and errors."
                                    .into(),
                            );
                        }
                    }
                    None => {
                        self.message =
                            Some("The sync engine is not available in this build.".into());
                    }
                }
            }
            if design::button(ui, "Mark complete", ButtonVariant::Outline).clicked() {
                self.complete(db);
            }
        });
        if !status.completed {
            design::hint_text(ui, "Onboarding is not marked complete yet.");
        }
        if !status.hs_configured {
            design::hint_text(
                ui,
                "No Help Scout credentials yet — add them in Settings → Help Scout first.",
            );
        }
    }
}

impl Default for OnboardingPage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn fresh_database_reports_welcome_state() {
        let db = fixture(Vec::new());
        let status = db.with_read(load_status);
        assert_eq!(status.step, "welcome");
        assert!(!status.completed);
        assert_eq!(status.conversations, 0);
        assert!(!status.hs_configured);
        assert_eq!(status.sync_state, "new");
    }

    #[test]
    fn saved_credentials_count_as_configured_before_any_token_exists() {
        // The 1.1.x status only saw oauth_tokens, so a user who had just
        // saved credentials (and had not completed a connect) was told
        // Help Scout was not configured.
        crate::testsupport::install_master_key();
        let db = fixture(Vec::new());
        let before = db.with_read(load_status);
        assert!(!before.hs_configured);
        db.with_write(|conn| {
            spp_core::helpscout_real::HsCredentials::store_in_envelope(
                conn, "cid", "secret", "", "",
            )
            .expect("store credentials");
        });
        let after = db.with_read(load_status);
        assert!(after.hs_configured, "saved credentials must count");
    }

    #[test]
    fn sync_activity_advences_the_sync_state() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            spp_core::sync_engine::start_run(conn, "initial").expect("run");
        });
        let status = db.with_read(load_status);
        assert_eq!(status.sync_state, "registered");

        db.with_write(|conn| {
            conn.execute(
                "INSERT INTO webhook_events (event_hash, event_id, event_type, payload, received_at, processing_state, attempts)
                 VALUES ('e1', 'e1', 'convo.created', '{}', strftime('%Y-%m-%dT%H:%M:%fZ','now'), 'processed', 1)",
                [],
            )
            .expect("event");
        });
        let status2 = db.with_read(load_status);
        assert_eq!(status2.sync_state, "receiving");
    }

    #[test]
    fn complete_marks_first_run_done() {
        let db = fixture(Vec::new());
        let mut page = OnboardingPage::new();
        page.complete(&db);
        let status = page.status.clone().expect("status");
        assert!(status.completed);
        assert_eq!(status.step, "complete");
    }
}
