use std::path::PathBuf;
use std::sync::Arc;

use spp_core::db::DbHandle;
use spp_core::event_bus::EventBus;
use spp_core::helpscout::HelpScoutProvider;
use spp_core::sync_engine::SyncEngine;
use spp_core::workers::WorkerManager;

const WORKER_THREADS: usize = 2;

const STARTUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub struct WorkersHost {
    manager: Arc<WorkerManager>,
    runtime_handle: tokio::runtime::Handle,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    restart_task: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    restart_gate: Arc<tokio::sync::Mutex<()>>,
}

impl WorkersHost {
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
            restart_task: std::sync::Mutex::new(None),
            restart_gate: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    #[cfg(test)]
    pub fn manager(&self) -> &Arc<WorkerManager> {
        &self.manager
    }

    pub(crate) fn restart(&self) {
        let mut slot = self.restart_task.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(prev) = slot.take()
            && prev.is_finished()
            && let Err(join_err) = self.runtime_handle.block_on(prev)
        {
            tracing::error!(error = %join_err, "background worker restart task panicked");
        }
        let gate = Arc::clone(&self.restart_gate);
        let manager = Arc::clone(&self.manager);
        let task = self.runtime_handle.spawn(async move {
            let _guard = gate.lock().await;
            manager.stop();
            manager.start();
        });
        *slot = Some(task);
    }

    pub(crate) fn pause(&self) {
        let mut slot = self.restart_task.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(task) = slot.take() {
            task.abort();
        }
        self.manager.stop();
    }
}

impl Drop for WorkersHost {
    fn drop(&mut self) {
        if let Some(task) = self
            .restart_task
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            task.abort();
        }
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
            Arc::new(spp_core::helpscout::EmptyHelpScoutProvider::new()),
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
