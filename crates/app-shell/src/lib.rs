//! SupportOS+ native desktop shell (decision D1: eframe/egui).
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
pub mod design;
pub mod fatal;
pub mod icons;
pub mod mention;
pub mod overlays;
pub mod pages;
pub mod palette;
pub mod workers_host;
// The only unsafe code in this crate: two libc::flock FFI calls that hold
// and release the single-instance advisory lock.
#[allow(unsafe_code)]
pub mod single_instance;
#[cfg(test)]
mod testsupport;
pub mod theme;
pub mod toasts;

use crate::workers_host::WorkersHost;

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
    /// The real Help Scout provider when boot resolved credentials
    /// (`None` in demo/empty sessions). The Settings page needs it for
    /// the client-credentials connect button and to detect that the
    /// OAuth callback listener is actually armed this session (1.1.4).
    pub real: Option<Arc<spp_core::helpscout_real::RealHelpScoutProvider>>,
    /// The background worker host started by `main` (job drain, webhook
    /// drain, embedding pass, maintenance). `None` in tests, where the
    /// background timers must not run.
    pub workers: Option<Arc<WorkersHost>>,
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
    /// Which of the three providers boot built (diagnostics, the
    /// loopback listener's demo flag, and tests all read it).
    pub mode: BootMode,
    /// The real provider when Help Scout credentials are configured,
    /// `None` in demo mode.
    pub real: Option<Arc<spp_core::helpscout_real::RealHelpScoutProvider>>,
    /// The provider the sync engine drives (real, demo, or empty).
    pub provider: Arc<dyn spp_core::helpscout::HelpScoutProvider>,
    /// The shared sync engine.
    pub engine: Arc<SyncEngine>,
}

/// The provider boot selected (1.1.4). Before 1.1.4 "no credentials"
/// silently meant "demo data": the one-click demo purge could never
/// stick, because the next boot rebuilt the demo provider and re-synced
/// the whole fake dataset. The explicit third state fixes that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    /// Real Help Scout credentials resolved (env or the encrypted
    /// envelope written by the settings form) and demo mode is off.
    Real,
    /// Demo mode is on (setting or `LOCAL_DEMO_MODE`): the fake
    /// provider serves a simulated account.
    Demo,
    /// The user purged the demo content (Settings → Help Scout →
    /// Demo data) and no credentials are configured yet: the empty
    /// provider serves nothing until Help Scout is connected.
    Empty,
}

/// Build the boot services the way the Tauri shell did — with one fix
/// (1.1.4): the provider is chosen explicitly instead of "whatever is
/// not real is demo".
///
/// * **Real** when demo mode is off and client credentials resolve
///   (environment first, then the encrypted envelope).
/// * **Empty** when the user has purged the demo content
///   ([`spp_core::data_reset::PURGED_FLAG`]) and no credentials are
///   configured — the purge must stick, so boot must NOT resurrect the
///   fake dataset.
/// * **Demo** otherwise (the out-of-the-box experience: a fresh
///   install without credentials runs against the simulated account).
///
/// `LOCAL_DEMO_MODE=true` is an explicit developer override and wins
/// over the purge flag.
///
/// # Panics
/// Never; a missing provider degrades to the demo provider.
#[must_use]
pub fn build_boot_services(db: &DbHandle, bus: &EventBus) -> BootServices {
    let env_demo = std::env::var("LOCAL_DEMO_MODE").as_deref() == Ok("true");
    let demo_mode = db
        .with_read(|conn| spp_core::settings::get_bool(conn, "demo_mode", false).unwrap_or(false))
        || env_demo;
    // Credentials resolve per field: environment first (CI, power
    // users), then the encrypted envelope written by the in-app
    // settings form (1.1.3) — so the app connects without any
    // environment variables.
    let credentials = db.with_read(|conn| spp_core::helpscout_real::HsCredentials::resolve(conn).0);
    let purged = !env_demo
        && db.with_read(|conn| {
            spp_core::settings::get_bool(conn, spp_core::data_reset::PURGED_FLAG, false)
                .unwrap_or(false)
        });
    let mode = if !demo_mode && credentials.is_configured() {
        BootMode::Real
    } else if !demo_mode && purged {
        BootMode::Empty
    } else {
        BootMode::Demo
    };
    let (real, provider): (
        Option<Arc<spp_core::helpscout_real::RealHelpScoutProvider>>,
        Arc<dyn spp_core::helpscout::HelpScoutProvider>,
    ) = match mode {
        BootMode::Real => {
            let real = Arc::new(spp_core::helpscout_real::RealHelpScoutProvider::new(
                db.clone(),
                credentials,
            ));
            let as_trait: Arc<dyn spp_core::helpscout::HelpScoutProvider> = real.clone();
            (Some(real), as_trait)
        }
        BootMode::Empty => (
            None,
            Arc::new(spp_core::helpscout::FakeHelpScoutProvider::new_empty()),
        ),
        BootMode::Demo => (
            None,
            Arc::new(spp_core::helpscout::FakeHelpScoutProvider::new_demo()),
        ),
    };
    let engine = Arc::new(SyncEngine::new(db.clone(), provider.clone()).with_bus(bus.clone()));
    tracing::info!(mode = ?mode, "boot services built");
    BootServices {
        mode,
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

/// Pick the rendering backend for this run.
///
/// Default since 1.1.1: the GL-based `glow` renderer. eframe 0.37's own
/// default is `wgpu`, which needs a working Vulkan driver or a GLES
/// stack — neither is guaranteed outside CI (which installs
/// `mesa-vulkan-drivers`), and 1.1.0 died before showing a window on
/// machines without them (VMs, nouveau, older Mesa, minimal desktops).
/// `glow` opens wherever GLX or EGL desktop GL exists, including
/// llvmpipe software GL on GPU-less machines. `SPP_RENDERER=wgpu` opts
/// back into the eframe default for systems where GL is the broken
/// half instead; unrecognized values fall back to `glow` with a warning.
fn resolve_renderer() -> eframe::Renderer {
    let requested = std::env::var("SPP_RENDERER").unwrap_or_default();
    if requested.eq_ignore_ascii_case("wgpu") {
        tracing::info!(renderer = "wgpu", "renderer: wgpu (SPP_RENDERER=wgpu)");
        eframe::Renderer::Wgpu
    } else {
        if !requested.is_empty() && !requested.eq_ignore_ascii_case("glow") {
            tracing::warn!(
                value = %requested,
                "SPP_RENDERER: unrecognized value, falling back to glow"
            );
        }
        tracing::info!(
            renderer = "glow",
            "renderer: glow (default; set SPP_RENDERER=wgpu to switch)"
        );
        eframe::Renderer::Glow
    }
}

/// Run the native shell window. Blocks until the window is closed.
///
/// # Errors
/// Returns [`eframe::Error`] when the graphics context cannot be set up.
pub fn run(options: ShellOptions) -> Result<(), eframe::Error> {
    #[cfg_attr(
        not(feature = "ui-screenshot"),
        expect(unused_mut, reason = "only mutated by the ui-screenshot harness")
    )]
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("SupportOS+")
        .with_inner_size([1180.0, 760.0]);
    #[cfg(feature = "ui-screenshot")]
    {
        // Screenshot harness: window size from the environment.
        if let (Ok(w), Ok(h)) = (std::env::var("SPP_SHOT_W"), std::env::var("SPP_SHOT_H"))
            && let (Ok(w), Ok(h)) = (w.parse::<f32>(), h.parse::<f32>())
        {
            viewport = viewport.with_inner_size([w, h]);
        }
    }
    let mut native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    // v1.1.1: never trust eframe's default (wgpu) — it needs a Vulkan
    // driver or a GLES stack, neither of which is guaranteed outside
    // CI. The GL-based glow renderer is the safe default for real
    // desktops, VMs and software GL; `resolve_renderer` above says why.
    native_options.renderer = resolve_renderer();
    #[cfg(feature = "ui-screenshot")]
    {
        // The harness runs headless (Xvfb + llvmpipe software GL); the
        // glow renderer needs no GPU and the __screenshot hook rides on it.
        native_options.renderer = eframe::Renderer::Glow;
    }
    eframe::run_native(
        "SupportOS-Plus",
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

    /// Install the process-global master key for envelope tests (same
    /// pattern as the settings page tests; no test in this binary ever
    /// clears the key, so parallel tests converge on "a key is set").
    fn install_master_key() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let path =
            std::env::temp_dir().join(format!("spp-shell-boot-key-{}-{nanos}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        spp_core::settings::init_master_key(&path).expect("master key");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn boot_defaults_to_the_demo_provider_on_a_fresh_install() {
        let db = crate::testsupport::empty_db();
        let boot = build_boot_services(&db, &spp_core::eventing::EventBus::default());
        assert_eq!(boot.mode, BootMode::Demo);
        assert!(boot.real.is_none(), "no credentials, no real provider");
    }

    #[test]
    fn boot_serves_the_empty_provider_after_a_demo_purge() {
        // The 1.1.3 bug: purged data came back on the next boot because
        // "no credentials" silently meant "demo provider".
        let db = crate::testsupport::empty_db();
        db.with_write(|conn| {
            spp_core::settings::set_bool(conn, "demo_mode", false).expect("demo off");
            spp_core::settings::set_bool(conn, spp_core::data_reset::PURGED_FLAG, true)
                .expect("purge flag");
        });
        let boot = build_boot_services(&db, &spp_core::eventing::EventBus::default());
        assert_eq!(boot.mode, BootMode::Empty);
        assert!(boot.real.is_none());
    }

    #[test]
    fn boot_builds_the_real_provider_for_stored_credentials() {
        install_master_key();
        let db = crate::testsupport::empty_db();
        db.with_write(|conn| {
            spp_core::helpscout_real::HsCredentials::store_in_envelope(
                conn,
                "cid-boot",
                "secret-boot",
                "wh",
                "",
            )
            .expect("store");
        });
        let boot = build_boot_services(&db, &spp_core::eventing::EventBus::default());
        assert_eq!(boot.mode, BootMode::Real);
        assert!(boot.real.is_some(), "credentials resolve at boot");
    }

    #[test]
    fn boot_demo_mode_wins_over_stored_credentials() {
        install_master_key();
        let db = crate::testsupport::empty_db();
        db.with_write(|conn| {
            spp_core::helpscout_real::HsCredentials::store_in_envelope(
                conn,
                "cid-boot",
                "secret-boot",
                "wh",
                "",
            )
            .expect("store");
            spp_core::settings::set_bool(conn, "demo_mode", true).expect("demo on");
        });
        let boot = build_boot_services(&db, &spp_core::eventing::EventBus::default());
        assert_eq!(boot.mode, BootMode::Demo);
        assert!(
            boot.real.is_none(),
            "explicit demo mode overrides credentials"
        );
    }
}
