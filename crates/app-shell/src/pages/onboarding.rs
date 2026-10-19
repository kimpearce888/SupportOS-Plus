//! Onboarding page (port of the web `pages/onboarding.rs`).
//!
//! Status from the `/api/onboarding` route logic (first-run flag,
//! conversation count, Help Scout credential presence, sync state) and the
//! action buttons: enable demo mode (`/api/demo/enable`), run the initial
//! sync (`/api/sync/initial`, dispatched through the shared sync engine),
//! and mark onboarding complete (`/api/onboarding/complete`).

use std::sync::Arc;

use eframe::egui;
use rusqlite::Connection;
use spp_core::db::DbHandle;
use spp_core::sync_engine::SyncEngine;

use crate::pages::sync_health::SyncAction;

/// Onboarding status (typed port of the `/api/onboarding` route).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OnboardingStatus {
    pub step: String,
    pub completed: bool,
    pub demo_mode: bool,
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
    let hs_configured: i64 = conn
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
    let demo_mode = spp_core::settings::get_bool(conn, "demo_mode", false).unwrap_or(false);
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
        demo_mode,
        conversations: conv_count,
        hs_configured: hs_configured > 0,
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

    /// Enable demo mode and complete onboarding
    /// (the `/api/demo/enable` route logic).
    pub fn enable_demo_mode(&mut self, db: &DbHandle) {
        db.with_write(|conn| {
            let _ = spp_core::settings::set_bool(conn, "demo_mode", true);
            let _ = spp_core::settings::mark_first_run_done(conn);
        });
        self.message = Some(
            "Demo mode active with a simulated Help Scout account. Run the initial sync \
             from Sync Health to populate the demo database."
                .into(),
        );
        self.reload(db);
    }

    /// Mark onboarding complete (the `/api/onboarding/complete` route logic).
    pub fn complete(&mut self, db: &DbHandle) {
        db.with_write(|conn| {
            let _ = spp_core::settings::mark_first_run_done(conn);
        });
        self.message = Some("Onboarding complete.".into());
        self.reload(db);
    }

    /// Save the current step (the `/api/onboarding/step` route logic).
    pub fn save_step(&mut self, db: &DbHandle, step: &str) {
        db.with_write(|conn| {
            let _ = spp_core::settings::set_string(conn, "onboarding_step", step);
        });
        self.reload(db);
    }

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle, engine: Option<&Arc<SyncEngine>>) {
        if self.status.is_none() {
            self.reload(db);
        }

        ui.heading("Welcome to SupportOS+");
        ui.weak(
            "Connect Help Scout, or start in demo mode with a simulated account. \
             Everything stays in your local database.",
        );
        ui.separator();

        let status = self.status.clone().unwrap_or_default();
        egui::Grid::new("onboarding-status")
            .num_columns(2)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                ui.strong("step");
                ui.monospace(status.step.as_str());
                ui.end_row();
                ui.strong("onboarding complete");
                ui.monospace(status.completed.to_string());
                ui.end_row();
                ui.strong("demo mode");
                ui.monospace(status.demo_mode.to_string());
                ui.end_row();
                ui.strong("conversations");
                ui.monospace(status.conversations.to_string());
                ui.end_row();
                ui.strong("Help Scout credentials");
                ui.monospace(status.hs_configured.to_string());
                ui.end_row();
                ui.strong("sync state");
                ui.monospace(status.sync_state.as_str());
                ui.end_row();
            });
        ui.separator();

        if let Some(message) = &self.message {
            ui.label(format!("status: {message}"));
            ui.separator();
        }

        ui.horizontal(|ui| {
            if ui.button("Enable demo mode").clicked() {
                self.enable_demo_mode(db);
            }
            if ui.button("Run initial sync").clicked() {
                match engine {
                    Some(engine) => {
                        if engine.is_running() {
                            self.message = Some("A sync is already running.".into());
                        } else {
                            // Same dispatch as the Sync Health page.
                            let mut sync_page = crate::pages::sync_health::SyncHealthPage::new();
                            sync_page.run_sync_action(db, engine, SyncAction::Initial);
                            self.message = Some(
                                "Initial sync started. Watch Sync Health for progress.".into(),
                            );
                        }
                    }
                    None => {
                        self.message =
                            Some("The sync engine is not available in this build.".into());
                    }
                }
            }
            if ui.button("Mark complete").clicked() {
                self.complete(db);
            }
        });
        if !status.completed {
            ui.weak("Onboarding is not marked complete yet.");
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

    fn fake_engine(db: &DbHandle) -> Arc<SyncEngine> {
        Arc::new(SyncEngine::new(
            db.clone(),
            Arc::new(spp_core::helpscout::FakeHelpScoutProvider::new_demo()),
        ))
    }

    #[test]
    fn fresh_database_reports_welcome_state() {
        let db = fixture(Vec::new());
        let status = db.with_read(load_status);
        assert_eq!(status.step, "welcome");
        assert!(!status.completed);
        assert!(!status.demo_mode);
        assert_eq!(status.conversations, 0);
        assert!(!status.hs_configured);
        assert_eq!(status.sync_state, "new");
    }

    #[test]
    fn demo_mode_enable_flips_status_and_completes() {
        let db = fixture(Vec::new());
        let mut page = OnboardingPage::new();
        page.reload(&db);
        page.enable_demo_mode(&db);
        let status = page.status.clone().expect("status");
        assert!(status.demo_mode);
        assert!(status.completed);
        assert_eq!(status.step, "complete");
        assert!(page.message.is_some());
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

    #[test]
    fn save_step_persists_the_setting() {
        let db = fixture(Vec::new());
        let mut page = OnboardingPage::new();
        page.save_step(&db, "connect");
        let stored = db.with_read(|conn| {
            spp_core::settings::get_string(conn, "onboarding_step")
                .ok()
                .flatten()
        });
        assert_eq!(stored.as_deref(), Some("connect"));
    }

    #[test]
    fn initial_sync_through_the_engine_populates_demo_data() {
        let db = fixture(Vec::new());
        let engine = fake_engine(&db);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            engine.initial_sync().await.expect("initial sync");
        });
        let status = db.with_read(load_status);
        assert!(
            status.conversations > 0,
            "demo provider seeds conversations"
        );
        assert_eq!(status.sync_state, "registered");
    }
}
