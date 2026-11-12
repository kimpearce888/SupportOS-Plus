use std::sync::Arc;

use eframe::egui;
use rusqlite::Connection;
use spp_core::db::DbHandle;
use spp_core::sync_engine::SyncEngine;

use crate::design::{self, ButtonVariant};
use crate::pages::sync_health::SyncAction;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct OnboardingStatus {
    pub step: String,
    pub completed: bool,
    pub conversations: i64,
    pub hs_configured: bool,
    pub sync_state: String,
}

#[must_use]
pub(crate) fn load_status(conn: &Connection) -> OnboardingStatus {
    let first_run = spp_core::settings::first_run_done(conn).unwrap_or(false);
    let conv_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
        .unwrap_or(0);
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

    pub(crate) fn invalidate(&mut self) {
        self.status = None;
    }

    fn reload(&mut self, db: &DbHandle) {
        self.status = Some(db.with_read(load_status));
    }

    pub fn complete(&mut self, db: &DbHandle) {
        db.with_write(|conn| {
            let _ = spp_core::settings::mark_first_run_done(conn);
        });
        self.message = Some("Onboarding complete.".into());
        self.reload(db);
    }

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
        // UX-4: present setup as a step checklist instead of a raw key–value
        // status dump — same data, human progression.
        let credentials_step = status.hs_configured;
        let sync_step = status.conversations > 0 || status.sync_state == "receiving";
        let complete_step = status.completed;
        design::card_titled(
            ui,
            "Setup checklist",
            Some("three steps and the workspace is yours"),
            |ui| {
                let t = design::tokens(ui);
                let steps: [(&str, &str, bool, &str); 3] = [
                    (
                        "1. Connect Help Scout",
                        "Add your app credentials in Settings → Help Scout.",
                        credentials_step,
                        "Done — credentials are configured.",
                    ),
                    (
                        "2. Pull your history",
                        "Run the initial sync to mirror conversations, customers \
                         and docs into this machine.",
                        sync_step,
                        "Done — your Help Scout history is in the local mirror.",
                    ),
                    (
                        "3. Finish setup",
                        "Mark onboarding complete once the first sync looks right.",
                        complete_step,
                        "Done — setup is complete.",
                    ),
                ];
                for (title, todo, done, done_note) in steps {
                    ui.horizontal(|ui| {
                        design::status_dot(
                            ui,
                            if done {
                                crate::design::BadgeTone::Success
                            } else {
                                crate::design::BadgeTone::Neutral
                            },
                            false,
                        );
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(title)
                                    .size(14.0)
                                    .strong()
                                    .color(t.foreground),
                            );
                            ui.label(
                                egui::RichText::new(if done { done_note } else { todo })
                                    .size(design::scale::text::SMALL)
                                    .color(t.muted_foreground),
                            );
                        });
                    });
                    ui.add_space(6.0);
                }
            },
        );

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
        // AUD-H6: the two status lines land in ONE quiet card — grouped,
        // padded, and inset from the page edge instead of two bare lines
        // flush against the fold.
        if !status.completed || !status.hs_configured {
            design::card(ui, |ui| {
                if !status.completed {
                    design::hint_text(ui, "Onboarding is not marked complete yet.");
                }
                if !status.hs_configured {
                    design::hint_text(
                        ui,
                        "No Help Scout credentials yet — add them in Settings → Help Scout first.",
                    );
                }
            });
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
