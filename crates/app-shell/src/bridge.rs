//! Background-task bridge → `egui::Context::request_repaint`
//! (migration map row 10: the SSE/EventBus/CORS layer maps onto the
//! native event loop).
//!
//! eframe repaints reactively: when no input arrives, no frames are
//! produced, so a UI that merely polls a channel inside `App::ui` would
//! starve. The bridge instead runs a dedicated OS thread that blocks on
//! the core [`EventBus`] broadcast channel; every event that arrives is
//! queued for the next frame and wakes the loop through a repaint
//! callback (`egui::Context::request_repaint` in production). `App::logic`
//! then drains the queue once per frame — and also while the window is
//! hidden, which is exactly when background events matter most.
//!
//! The queue is pushed *before* the repaint fires, so a woken frame is
//! guaranteed to observe the event that triggered it.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use eframe::egui;
use spp_core::eventing::EventBus;
use spp_core::events::ServerEvent;
use tokio::sync::broadcast::error::RecvError;

/// Bridge between the core event bus and the eframe event loop.
pub struct TaskBridge {
    queue: Arc<Mutex<VecDeque<ServerEvent>>>,
    /// Kept to join on drop if possible; detaching is also fine (the
    /// thread exits when the bus closes).
    _worker: std::thread::JoinHandle<()>,
}

impl TaskBridge {
    /// Subscribe to `bus` and wake `ctx` on every arriving event.
    ///
    /// The worker thread lives until the bus's senders are all dropped.
    #[must_use]
    pub fn spawn(bus: &EventBus, ctx: &egui::Context) -> Self {
        let ctx = ctx.clone();
        Self::spawn_with(bus, move || ctx.request_repaint())
    }

    /// Testable core of [`Self::spawn`]: the wake-up is a plain callback.
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
                            // Wake the UI thread; the event itself is read
                            // out of the queue in the next logic() pass.
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

    /// Take all events that arrived since the last call.
    pub fn drain(&self) -> Vec<ServerEvent> {
        let mut guard = lock(&self.queue);
        guard.drain(..).collect()
    }

    /// Number of events waiting to be drained.
    #[must_use]
    pub fn pending(&self) -> usize {
        lock(&self.queue).len()
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

    /// Wait until the bridge has at least `want` pending events.
    fn wait_for_pending(bridge: &TaskBridge, want: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while bridge.pending() < want {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {want} events ({} pending)",
                bridge.pending()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Wait until `flag` reaches at least `want`.
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

        let first = ServerEvent::sync_completed("test", 3, 0);
        bus.emit(&first);
        bus.emit(&ServerEvent::sync_completed("test", 4, 0));
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
        assert_eq!(bridge.pending(), 0);
    }

    #[test]
    fn bridge_stays_alive_after_capacity_pressure() {
        // Capacity 1: a burst larger than the channel forces the worker
        // through the Lagged path; it must keep bridging afterwards.
        let bus = EventBus::new(1);
        let repaints = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&repaints);
        let bridge = TaskBridge::spawn_with(&bus, move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });

        for i in 0..8u32 {
            bus.emit(&ServerEvent::sync_completed("burst", i, 0));
        }
        wait_for_pending(&bridge, 1);
        assert!(!bridge.drain().is_empty());

        // The bridge must still deliver events emitted after the lag.
        bus.emit(&ServerEvent::sync_completed("after", 99, 0));
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
        // The production path (egui::Context::request_repaint) is exercised
        // end-to-end here: the worker must accept the context closure and
        // keep delivering into the queue.
        let bus = EventBus::new(16);
        let ctx = egui::Context::default();
        let bridge = TaskBridge::spawn(&bus, &ctx);
        bus.emit(&ServerEvent::sync_completed("ctx", 1, 0));
        wait_for_pending(&bridge, 1);
        assert_eq!(bridge.drain().len(), 1);
    }
}
