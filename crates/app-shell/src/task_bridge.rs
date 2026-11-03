use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use eframe::egui;
use spp_core::event_bus::EventBus;
use spp_core::events::ServerEvent;
use tokio::sync::broadcast::error::RecvError;

pub(crate) struct TaskBridge {
    queue: Arc<Mutex<VecDeque<ServerEvent>>>,
    _worker: std::thread::JoinHandle<()>,
}

impl TaskBridge {
    #[must_use]
    pub(crate) fn spawn(bus: &EventBus, ctx: &egui::Context) -> Self {
        let ctx = ctx.clone();
        Self::spawn_with(bus, move || ctx.request_repaint())
    }

    fn spawn_with(bus: &EventBus, repaint: impl Fn() + Send + Sync + 'static) -> Self {
        let mut rx = bus.subscribe();
        let queue: Arc<Mutex<VecDeque<ServerEvent>>> = Arc::new(Mutex::new(VecDeque::new()));
        let worker_queue = Arc::clone(&queue);
        let worker = std::thread::Builder::new()
            .name("spp-task-bridge".to_string())
            .spawn(move || {
                loop {
                    match rx.blocking_recv() {
                        Ok(event) => {
                            lock(&worker_queue).push_back(event);
                            repaint();
                        }
                        Err(RecvError::Lagged(skipped)) => {
                            tracing::warn!(skipped, "task bridge lagged behind the event bus");
                        }
                        Err(RecvError::Closed) => break,
                    }
                }
            })
            .expect("failed to spawn the task bridge thread");
        Self {
            queue,
            _worker: worker,
        }
    }

    pub(crate) fn drain(&self) -> Vec<ServerEvent> {
        let mut guard = lock(&self.queue);
        guard.drain(..).collect()
    }
}

fn lock(queue: &Mutex<VecDeque<ServerEvent>>) -> MutexGuard<'_, VecDeque<ServerEvent>> {
    queue
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    fn pending(bridge: &TaskBridge) -> usize {
        // inline of the deleted lib accessor (test-only observer)
        lock(&bridge.queue).len()
    }

    fn wait_for_pending(bridge: &TaskBridge, want: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while pending(bridge) < want {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {want} events ({} pending)",
                pending(bridge)
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn wait_for_counter(flag: &AtomicUsize, want: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while flag.load(Ordering::SeqCst) < want {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {want} repaint callbacks"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn events_flow_to_drain_and_wake_the_loop() {
        let bus = EventBus::new(16);
        let repaints = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&repaints);
        let bridge = TaskBridge::spawn_with(&bus, move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });

        let first =
            ServerEvent::sync_completed("test", 3, 0, spp_core::events::SyncTrigger::Manual);
        bus.emit(&first);
        bus.emit(&ServerEvent::sync_completed(
            "test",
            4,
            0,
            spp_core::events::SyncTrigger::Manual,
        ));
        wait_for_pending(&bridge, 2);
        wait_for_counter(&repaints, 2);

        let drained = bridge.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0], first);
        assert!(
            matches!(&drained[1], ServerEvent::SyncCompleted(e) if e.processed == 4),
            "second event must be the second emit"
        );
        assert!(bridge.drain().is_empty());
        assert_eq!(pending(&bridge), 0);
    }

    #[test]
    fn bridge_stays_alive_after_capacity_pressure() {
        let bus = EventBus::new(1);
        let repaints = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&repaints);
        let bridge = TaskBridge::spawn_with(&bus, move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });

        for i in 0..8u32 {
            bus.emit(&ServerEvent::sync_completed(
                "burst",
                i,
                0,
                spp_core::events::SyncTrigger::Auto,
            ));
        }
        wait_for_pending(&bridge, 1);
        assert!(!bridge.drain().is_empty());

        bus.emit(&ServerEvent::sync_completed(
            "after",
            99,
            0,
            spp_core::events::SyncTrigger::Auto,
        ));
        let deadline = Instant::now() + Duration::from_secs(5);
        let delivered = loop {
            let drained = bridge.drain();
            if drained
                .iter()
                .any(|e| matches!(e, ServerEvent::SyncCompleted(ev) if ev.processed == 99))
            {
                break true;
            }
            assert!(Instant::now() < deadline, "post-lag event never arrived");
            std::thread::sleep(Duration::from_millis(5));
        };
        assert!(delivered);
        assert!(repaints.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn production_spawn_wires_egui_context() {
        let bus = EventBus::new(16);
        let ctx = egui::Context::default();
        let bridge = TaskBridge::spawn(&bus, &ctx);
        bus.emit(&ServerEvent::sync_completed(
            "ctx",
            1,
            0,
            spp_core::events::SyncTrigger::Manual,
        ));
        wait_for_pending(&bridge, 1);
        assert_eq!(bridge.drain().len(), 1);
    }
}
