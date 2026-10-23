use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::helpscout_real::HsRateLimiter;

pub const PRIORITY_USER_SEND: u8 = 0;
pub const PRIORITY_INTERACTIVE: u8 = 1;
pub const PRIORITY_SYNC: u8 = 2;
pub const PRIORITY_ANALYTICS: u8 = 3;
pub const PRIORITY_INDEXING: u8 = 4;

pub const DEFAULT_CONCURRENCY: usize = 2;

#[derive(Default)]
pub struct ApiQueueStats {
    pub queued: AtomicI64,
    pub active: AtomicI64,
    pub dispatched: AtomicI64,
    pub completed: AtomicI64,
    pub failed: AtomicI64,
    pub high_water: AtomicI64,
}

impl ApiQueueStats {
    pub fn snapshot(&self) -> Value {
        json!({
            "queued": self.queued.load(Ordering::Relaxed),
            "active": self.active.load(Ordering::Relaxed),
            "dispatched": self.dispatched.load(Ordering::Relaxed),
            "completed": self.completed.load(Ordering::Relaxed),
            "failed": self.failed.load(Ordering::Relaxed),
            "highWater": self.high_water.load(Ordering::Relaxed),
        })
    }
}

struct ActiveSlot {
    queue: Weak<ApiQueue>,
}

impl Drop for ActiveSlot {
    fn drop(&mut self) {
        if let Some(q) = self.queue.upgrade() {
            q.stats.active.fetch_sub(1, Ordering::Relaxed);
            q.pump();
        }
    }
}

struct QueueItem {
    priority: u8,
    seq: u64,
    is_write: bool,
    permit: tokio::sync::oneshot::Sender<ActiveSlot>,
}

pub struct ApiQueue {
    items: Mutex<Vec<QueueItem>>,
    stats: ApiQueueStats,
    limiter: Arc<HsRateLimiter>,
    concurrency: AtomicUsize,
    seq: AtomicU64,
    timer_armed: AtomicBool,
    self_ref: Mutex<Weak<ApiQueue>>,
}

impl ApiQueue {
    pub fn new(limiter: Arc<HsRateLimiter>, concurrency: usize) -> Arc<Self> {
        let q = Arc::new(Self {
            items: Mutex::new(Vec::new()),
            stats: ApiQueueStats::default(),
            limiter,
            concurrency: AtomicUsize::new(concurrency.max(1)),
            seq: AtomicU64::new(0),
            timer_armed: AtomicBool::new(false),
            self_ref: Mutex::new(Weak::new()),
        });
        *q.self_ref.lock().unwrap_or_else(|p| p.into_inner()) = Arc::downgrade(&q);
        q
    }

    fn weak(&self) -> Weak<ApiQueue> {
        self.self_ref
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn set_concurrency(&self, n: usize) {
        self.concurrency.store(n.max(1), Ordering::Relaxed);
        self.pump();
    }

    /// The currently effective concurrency limit (audit item 31: the
    /// `api_concurrency` setting was parsed and saved but never applied —
    /// this getter exists so the wiring is observable and testable).
    #[must_use]
    pub fn concurrency(&self) -> usize {
        self.concurrency.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> Value {
        self.stats.snapshot()
    }

    pub async fn enqueue<T>(
        &self,
        priority: u8,
        is_write: bool,
        fut: impl Future<Output = Result<T>>,
    ) -> Result<T> {
        let (permit_tx, mut permit_rx) = tokio::sync::oneshot::channel::<ActiveSlot>();
        {
            let mut items = self.items.lock().unwrap_or_else(|p| p.into_inner());
            items.push(QueueItem {
                priority,
                seq: self.seq.fetch_add(1, Ordering::Relaxed),
                is_write,
                permit: permit_tx,
            });
            self.stats
                .queued
                .store(items.len() as i64, Ordering::Relaxed);
            self.stats
                .high_water
                .fetch_max(items.len() as i64, Ordering::Relaxed);
        }
        self.pump();
        let slot = match permit_rx.try_recv() {
            Ok(s) => Some(s),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => permit_rx.await.ok(),
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => None,
        };
        let _slot = match slot {
            Some(s) => s,
            None => return Err(Error::Other("API queue shut down before dispatch".into())),
        };
        let res = fut.await;
        match res {
            Ok(_) => {
                self.stats.completed.fetch_add(1, Ordering::Relaxed);
            }
            Err(_) => {
                self.stats.failed.fetch_add(1, Ordering::Relaxed);
            }
        }
        res
    }

    fn pump(&self) {
        loop {
            let mut items = self.items.lock().unwrap_or_else(|p| p.into_inner());
            let active = self.stats.active.load(Ordering::Relaxed);
            if active >= self.concurrency.load(Ordering::Relaxed) as i64 || items.is_empty() {
                return;
            }
            items.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.seq.cmp(&b.seq)));
            let item = items.remove(0);
            let wait = self.limiter.wait_time_ms(item.is_write);
            if wait > 0 {
                items.insert(0, item);
                self.stats
                    .queued
                    .store(items.len() as i64, Ordering::Relaxed);
                if !self.timer_armed.swap(true, Ordering::Relaxed) {
                    if let Some(q) = self.weak().upgrade() {
                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_millis(wait.min(5_000) as u64)).await;
                            q.timer_armed.store(false, Ordering::Relaxed);
                            q.pump();
                        });
                    } else {
                        self.timer_armed.store(false, Ordering::Relaxed);
                    }
                }
                return;
            }
            self.stats.dispatched.fetch_add(1, Ordering::Relaxed);
            self.stats.active.fetch_add(1, Ordering::Relaxed);
            self.stats
                .queued
                .store(items.len() as i64, Ordering::Relaxed);
            drop(items);
            let slot = ActiveSlot { queue: self.weak() };
            if item.permit.send(slot).is_err() {
                self.stats.active.fetch_sub(1, Ordering::Relaxed);
            }
        }
    }
}
