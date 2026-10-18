//! SupportOS++ native desktop shell (decision D1: eframe/egui).
//!
//! This crate is the Phase 2 skeleton of the ported UI: an eframe window
//! whose theme is persisted in the core `settings` table, a flock-based
//! single-instance guard, and a background-task bridge that wakes the
//! event loop via [`egui::Context::request_repaint`] when the core
//! [`EventBus`] fires. Pages are ported on top of this shell in Phase 3.
//!
//! The shell holds the database through the [`DbHandle`] facade (decision
//! D3) so the connection strategy can change behind it after cutover.

#![deny(unsafe_code, rust_2018_idioms)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

pub mod app;
pub mod bridge;
pub mod chart;
pub mod mention;
pub mod overlays;
pub mod pages;
pub mod palette;
// The only unsafe code in this crate: two libc::flock FFI calls that hold
// and release the single-instance advisory lock.
#[allow(unsafe_code)]
pub mod single_instance;
#[cfg(test)]
mod testsupport;
pub mod theme;
pub mod toasts;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use spp_core::db::DbHandle;
use spp_core::eventing::EventBus;
use spp_core::sync_engine::SyncEngine;

use crate::app::ShellApp;

/// Everything the shell needs at startup.
pub struct ShellOptions {
    /// Database handle with all migrations applied (see [`open_db`]).
    pub db: DbHandle,
    /// Core event bus; background workers emit onto it and the shell
    /// repaints in response.
    pub bus: EventBus,
    /// Application data directory (lock file, vector store, logs).
    pub data_dir: PathBuf,
    /// Shared sync engine, built by the boot code (demo or real provider).
    pub sync: Option<Arc<SyncEngine>>,
}

/// Open the database and apply all boot migrations, returning a handle.
///
/// Mirrors the boot sequence of the Tauri shell during the side-by-side
/// port period: `db::open` + `bootstrap::apply_all` (which also
/// initializes the secrets master key).
pub fn open_db(path: &Path) -> spp_core::error::Result<DbHandle> {
    let mut conn = spp_core::db::open(path)?;
    spp_core::bootstrap::apply_all(&mut conn)?;
    Ok(DbHandle::new(conn))
}

/// Provider + engine built together.
///
/// The loopback listener needs the *real* provider handle (OAuth code
/// exchange, `get_me`) while the sync engine drives the provider behind the
/// `HelpScoutProvider` trait, so the boot code hands both out of one
/// construction.
pub struct BootServices {
    /// The real provider when Help Scout credentials are configured,
    /// `None` in demo mode.
    pub real: Option<Arc<spp_core::helpscout_real::RealHelpScoutProvider>>,
    /// The provider the sync engine drives (real or demo).
    pub provider: Arc<dyn spp_core::helpscout::HelpScoutProvider>,
    /// The shared sync engine.
    pub engine: Arc<SyncEngine>,
}

/// Build the boot services the way the Tauri shell did: demo mode or real
/// Help Scout credentials decide the provider.
///
/// # Panics
/// Never; a missing provider degrades to the demo provider.
#[must_use]
pub fn build_boot_services(db: &DbHandle, bus: &EventBus) -> BootServices {
    let demo_mode = db
        .with_read(|conn| spp_core::settings::get_bool(conn, "demo_mode", false).unwrap_or(false))
        || std::env::var("LOCAL_DEMO_MODE").as_deref() == Ok("true");
    let credentials = spp_core::helpscout_real::HsCredentials::from_env();
    let use_real = !demo_mode && credentials.is_configured();
    let (real, provider): (
        Option<Arc<spp_core::helpscout_real::RealHelpScoutProvider>>,
        Arc<dyn spp_core::helpscout::HelpScoutProvider>,
    ) = if use_real {
        let real = Arc::new(spp_core::helpscout_real::RealHelpScoutProvider::new(
            db.clone(),
            credentials,
        ));
        let as_trait: Arc<dyn spp_core::helpscout::HelpScoutProvider> = real.clone();
        (Some(real), as_trait)
    } else {
        (
            None,
            Arc::new(spp_core::helpscout::FakeHelpScoutProvider::new_demo()),
        )
    };
    let engine = Arc::new(SyncEngine::new(db.clone(), provider.clone()).with_bus(bus.clone()));
    BootServices {
        real,
        provider,
        engine,
    }
}

/// Build only the shared sync engine (convenience over
/// [`build_boot_services`] for tests).
#[must_use]
pub fn build_sync_engine(db: &DbHandle, bus: &EventBus) -> Arc<SyncEngine> {
    build_boot_services(db, bus).engine
}

/// Run the native shell window. Blocks until the window is closed.
///
/// # Errors
/// Returns [`eframe::Error`] when the graphics context cannot be set up.
pub fn run(options: ShellOptions) -> Result<(), eframe::Error> {
    let viewport = eframe::egui::ViewportBuilder::default()
        .with_title("SupportOs-Plus")
        .with_inner_size([1180.0, 760.0]);
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "SupportOs-Plus",
        native_options,
        Box::new(move |cc| Ok(Box::new(ShellApp::new(cc, options)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_db_bootstraps_schema_and_is_idempotent() {
        let path =
            std::env::temp_dir().join(format!("spp-shell-open-db-{}.db", std::process::id()));
        let handle = open_db(&path).expect("first open_db");
        let version = handle.with_read(spp_core::db::latest_version);
        assert!(
            version.expect("latest_version") >= 40,
            "schema must reach v40"
        );
        let reported = handle.file_path().expect("file path");
        assert!(reported.ends_with(&path));
        drop(handle);
        // Second open on the same file must be a no-op migration-wise.
        let again = open_db(&path).expect("second open_db");
        let version2 = again.with_read(spp_core::db::latest_version);
        assert!(version2.expect("latest_version 2") >= 40);
        drop(again);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }
}
