#![deny(unsafe_code, rust_2018_idioms)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

pub mod app;
pub(crate) mod chart;
pub mod design;
pub mod fatal;
pub mod icons;
pub(crate) mod mention_popup;
pub(crate) mod overlays;
pub mod pages;
pub(crate) mod palette;
#[allow(unsafe_code)]
pub mod single_instance;
pub mod smoke;
pub(crate) mod task_bridge;
#[cfg(test)]
pub(crate) fn build_sync_engine(db: &DbHandle, bus: &EventBus) -> Arc<SyncEngine> {
    build_boot_services(db, bus).engine
}

#[cfg(test)]
mod testsupport;
pub(crate) mod theme;
pub(crate) mod toasts;
pub mod workers_host;

use crate::workers_host::WorkersHost;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use spp_core::db::DbHandle;
use spp_core::event_bus::EventBus;
use spp_core::provider_handle::ProviderHandle;
use spp_core::sync_engine::SyncEngine;

use crate::app::ShellApp;

pub struct ShellOptions {
    pub db: DbHandle,
    pub bus: EventBus,
    pub data_dir: PathBuf,
    pub sync: Option<Arc<SyncEngine>>,
    pub handle: Option<Arc<ProviderHandle>>,
    pub workers: Option<Arc<WorkersHost>>,
    pub vector_index: Option<Arc<spp_core::vectorstore_sqlitevec::VectorIndex>>,
}

pub fn open_db(path: &Path) -> spp_core::error::Result<DbHandle> {
    let mut conn = spp_core::db::open(path)?;
    spp_core::bootstrap::apply_all(&mut conn)?;
    Ok(DbHandle::new(conn))
}

pub struct BootServices {
    pub mode: BootMode,
    pub handle: Arc<ProviderHandle>,
    pub provider: Arc<dyn spp_core::helpscout::HelpScoutProvider>,
    pub engine: Arc<SyncEngine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    Real,
    Empty,
}

#[must_use]
pub fn build_boot_services(db: &DbHandle, bus: &EventBus) -> BootServices {
    let credentials = db.with_read(|conn| spp_core::helpscout_real::HsCredentials::resolve(conn).0);
    let mode = if credentials.is_configured() {
        BootMode::Real
    } else {
        BootMode::Empty
    };
    let previous_kind = db.with_read(|conn| {
        spp_core::settings::get_string(conn, "provider_kind")
            .ok()
            .flatten()
    });
    let handle = match mode {
        BootMode::Real => {
            let real = Arc::new(spp_core::helpscout_real::RealHelpScoutProvider::new(
                db.clone(),
                credentials,
            ));
            Arc::new(ProviderHandle::new_real(real))
        }
        BootMode::Empty => Arc::new(ProviderHandle::new_empty()),
    };
    let current_kind = match mode {
        BootMode::Real => "real",
        BootMode::Empty => "empty",
    };
    {
        let conn = db.write_guard();
        spp_core::sync_engine::on_provider_kind_transition(
            &conn,
            previous_kind.as_deref(),
            current_kind,
        );
        let _ = spp_core::settings::set_string(&conn, "provider_kind", current_kind);
    }
    let provider: Arc<dyn spp_core::helpscout::HelpScoutProvider> = handle.clone();
    let engine = Arc::new(SyncEngine::new(db.clone(), provider.clone()).with_bus(bus.clone()));
    tracing::info!(mode = ?mode, "boot services built");
    BootServices {
        mode,
        handle,
        provider,
        engine,
    }
}

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
    native_options.renderer = resolve_renderer();
    #[cfg(feature = "ui-screenshot")]
    {
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
        let again = open_db(&path).expect("second open_db");
        let version2 = again.with_read(spp_core::db::latest_version);
        assert!(version2.expect("latest_version 2") >= 40);
        drop(again);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

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
    fn boot_serves_the_empty_provider_on_a_fresh_install() {
        let db = crate::testsupport::empty_db();
        let boot = build_boot_services(&db, &spp_core::event_bus::EventBus::default());
        assert_eq!(boot.mode, BootMode::Empty);
        assert!(
            !boot.handle.is_real(),
            "no credentials, no real provider — and nothing is simulated"
        );
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
        let boot = build_boot_services(&db, &spp_core::event_bus::EventBus::default());
        assert_eq!(boot.mode, BootMode::Real);
        assert!(boot.handle.is_real(), "credentials resolve at boot");
        assert!(boot.handle.real().is_some());
    }

    #[test]
    fn r2_empty_session_then_credentials_resets_the_sync_state() {
        install_master_key();
        let db = crate::testsupport::empty_db();
        let boot = build_boot_services(&db, &spp_core::event_bus::EventBus::default());
        assert_eq!(boot.mode, BootMode::Empty);
        db.with_write(|conn| {
            spp_core::sync_engine::set_state(conn, "LIVE");
            spp_core::sync_engine::record_success(conn, "customers", 0);
            let kind = spp_core::settings::get_string(conn, "provider_kind")
                .ok()
                .flatten();
            assert_eq!(kind.as_deref(), Some("empty"));
        });
        db.with_write(|conn| {
            spp_core::helpscout_real::HsCredentials::store_in_envelope(
                conn,
                "cid-reset",
                "secret-reset",
                "",
                "",
            )
            .expect("store");
        });
        let boot = build_boot_services(&db, &spp_core::event_bus::EventBus::default());
        assert_eq!(boot.mode, BootMode::Real);
        let state = db.with_read(spp_core::sync_engine::get_state);
        assert_eq!(state, "NEW", "the poisoned LIVE watermark is forgotten");
    }
}
