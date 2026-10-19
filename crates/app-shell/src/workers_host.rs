//! Background worker host — the Phase 4 cutover gap fix.
//!
//! Pre-cutover, the axum HTTP server started the core [`WorkerManager`]
//! (job drain, webhook drain, embedding pass, maintenance, sweeps). The
//! cutover deleted that server and its replacement — the minimal loopback
//! listener — never pulled the workers up, so those background timers only
//! ran inside tests. This module restores the behavior on the native
//! shell: a dedicated OS thread hosts a fixed 2-worker-thread tokio
//! runtime (plan row 12: "2-thread runtime") that owns the
//! [`WorkerManager`] for the whole process lifetime.
//!
//! The host is owned by `main` (not by `ShellApp`) so the library — and
//! therefore every page test — keeps running without background timers.
//! The settings page receives an optional handle so it can restart the
//! timers when the sync interval changes, mirroring the old
//! `PUT /api/settings` behavior (`workers.stop(); workers.start();`).

use std::path::PathBuf;
use std::sync::Arc;

use spp_core::db::DbHandle;
use spp_core::eventing::EventBus;
use spp_core::helpscout::HelpScoutProvider;
use spp_core::sync_engine::SyncEngine;
use spp_core::workers::WorkerManager;

/// Fixed size of the background runtime (plan row 12). Two worker threads
/// cover the overlapping long-poll timers (sync + connector refresh) while
/// keeping the idle footprint minimal on desktop machines.
const WORKER_THREADS: usize = 2;

/// How long `start` waits for the worker thread to report readiness.
const STARTUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Running [`WorkerManager`] plus the thread/runtime that drives it.
///
/// Dropping the host stops the manager and joins the runtime thread.
pub struct WorkersHost {
    manager: Arc<WorkerManager>,
    /// Handle onto the dedicated runtime: `restart` re-arms the timers
    /// there because `WorkerManager::start` uses `tokio::spawn`, which
    /// needs a runtime context (the settings page runs on the UI thread).
    runtime_handle: tokio::runtime::Handle,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl WorkersHost {
    /// Start the dedicated runtime thread and the background workers on
    /// it. Blocks until the manager reports itself running (or the thread
    /// fails to come up).
    ///
    /// # Errors
    /// Returns an error when the thread or the runtime cannot be created.
    pub fn start(
        db: DbHandle,
        engine: Option<Arc<SyncEngine>>,
        provider: Arc<dyn HelpScoutProvider>,
        bus: EventBus,
        data_dir: PathBuf,
        vector_index: Option<Arc<spp_core::vectorstore_sqlitevec::VectorIndex>>,
    ) -> spp_core::error::Result<Self> {
        let (ready_tx, ready_rx) =
            std::sync::mpsc::channel::<(Arc<WorkerManager>, tokio::runtime::Handle)>();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let thread = std::thread::Builder::new()
            .name("spp-workers".to_string())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(WORKER_THREADS)
                    .thread_name("spp-worker")
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to build the background worker runtime");
                        // Dropping ready_tx makes `start` fail instead of hanging.
                        return;
                    }
                };
                let runtime_handle = runtime.handle().clone();
                runtime.block_on(async move {
                    let manager = spp_core::workers::start_workers(
                        db,
                        engine,
                        provider,
                        bus,
                        data_dir,
                        vector_index,
                    );
                    let _ = ready_tx.send((manager.clone(), runtime_handle));
                    // Park until the host is dropped, then stop the
                    // manager before the runtime is torn down.
                    let _ = shutdown_rx.await;
                    manager.stop();
                });
            })
            .map_err(|e| {
                spp_core::error::Error::Config(format!(
                    "failed to spawn the background worker thread: {e}"
                ))
            })?;
        let (manager, runtime_handle) = match ready_rx.recv_timeout(STARTUP_TIMEOUT) {
            Ok(pair) => pair,
            Err(_) => {
                // The thread died (runtime build failure logged above) or
                // never reported readiness in time. Release the park first
                // so the thread can finish its current step and exit;
                // otherwise join() would deadlock on the parked future.
                drop(shutdown_tx);
                let _ = thread.join();
                return Err(spp_core::error::Error::Config(
                    "background workers failed to start (see log for details)".to_string(),
                ));
            }
        };
        Ok(Self {
            manager,
            runtime_handle,
            shutdown: Some(shutdown_tx),
            thread: Some(thread),
        })
    }

    /// The running manager (for status surfaces).
    #[must_use]
    pub fn manager(&self) -> &Arc<WorkerManager> {
        &self.manager
    }

    /// Stop and immediately restart the timers, re-reading interval
    /// settings from the database — the old settings-route behavior for a
    /// changed sync interval. The restart is spawned onto the worker
    /// runtime because `WorkerManager::start` needs a runtime context.
    pub fn restart(&self) {
        let manager = Arc::clone(&self.manager);
        // The restart task detaches: it only re-arms timers and returns.
        std::mem::drop(self.runtime_handle.spawn(async move {
            manager.stop();
            manager.start();
        }));
    }
}

impl Drop for WorkersHost {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_data_dir(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("spp-workers-host-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp data dir");
        dir
    }

    fn start_host(db: DbHandle) -> WorkersHost {
        WorkersHost::start(
            db,
            None,
            Arc::new(spp_core::helpscout::FakeHelpScoutProvider::new_demo()),
            EventBus::default(),
            temp_data_dir("boot"),
            None,
        )
        .expect("worker host starts")
    }

    #[test]
    fn host_starts_the_manager_and_drops_stop_it() {
        let db = crate::testsupport::empty_db();
        let host = start_host(db);
        assert!(
            host.manager().is_running(),
            "the manager must report running right after host start"
        );
        let manager = Arc::clone(host.manager());
        drop(host);
        assert!(
            !manager.is_running(),
            "dropping the host must stop the manager"
        );
    }

    #[test]
    fn restart_rearms_the_timers() {
        let db = crate::testsupport::empty_db();
        let host = start_host(db);
        host.restart();
        assert!(
            host.manager().is_running(),
            "restart must leave the manager running (timers re-armed)"
        );
    }
}
