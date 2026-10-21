//! The in-process event bus (promoted from the retired axum
//! `http/event_bus.rs` at the Phase 4 cutover).
//!
//! A tokio broadcast channel the background engines (sync, workers,
//! sweeps) emit [`ServerEvent`]s onto and the native shell subscribes to
//! through its repaint bridge.

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::events::{ConversationUpdatedEvent, ServerEvent};

#[derive(Clone)]
pub struct EventBus {
    tx: Arc<broadcast::Sender<ServerEvent>>,
}

impl EventBus {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx: Arc::new(tx) }
    }

    #[must_use]
    pub fn default_capacity() -> usize {
        256
    }

    pub fn emit(&self, event: &ServerEvent) -> usize {
        self.tx.send(event.clone()).unwrap_or(0)
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.tx.subscribe()
    }

    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(Self::default_capacity())
    }
}

impl crate::events::EventEmitter for EventBus {
    fn emit(&self, event: &ServerEvent) -> Result<(), String> {
        let _ = EventBus::emit(self, event);
        Ok(())
    }
}

pub fn notify_conversation_updated(bus: &EventBus, event: &ConversationUpdatedEvent) {
    bus.emit(&ServerEvent::ConversationUpdated(event.clone()));
}

pub fn notify_sync_completed(bus: &EventBus, kind: &str, processed: u32, errors: u32) {
    bus.emit(&ServerEvent::sync_completed(kind, processed, errors));
}

pub fn notify_sync_progress(
    bus: &EventBus,
    stage: &str,
    done: u32,
    total: u32,
    processed: u64,
    stage_records: u32,
) {
    bus.emit(&ServerEvent::sync_progress(
        stage,
        done,
        total,
        processed,
        stage_records,
    ));
}

pub fn notify_outbound_finished(
    bus: &EventBus,
    job_id: i64,
    kind: &str,
    conversation_id: Option<i64>,
    success: bool,
    error: Option<&str>,
) {
    bus.emit(&ServerEvent::outbound_job_finished(
        job_id,
        kind,
        conversation_id,
        success,
        error,
    ));
}

/// R5 (D4): the first live emitters for `ErrorOccurred`. Background
/// failures (sync, outbound, connect, worker jobs) used to land only in
/// the Diagnostics log; now they also invalidate pages and surface as
/// toasts through the shell's event bridge.
pub fn notify_error(
    bus: &EventBus,
    severity: &str,
    message: &str,
    service: &str,
    conversation_id: Option<i64>,
) {
    bus.emit(&ServerEvent::error_occurred(
        severity,
        message,
        service,
        conversation_id,
    ));
}

pub fn notify_rating_received(
    bus: &EventBus,
    rating: Option<&str>,
    conversation_id: Option<i64>,
    conversation_number: Option<i64>,
    customer_id: Option<i64>,
    customer_name: Option<&str>,
    comments: Option<&str>,
) {
    bus.emit(&ServerEvent::rating_received(
        rating.map(String::from),
        conversation_id,
        conversation_number,
        customer_id,
        customer_name.map(String::from),
        comments.map(String::from),
    ));
}

#[allow(clippy::too_many_arguments)]
pub fn notify_campaign_updated(
    bus: &EventBus,
    campaign_id: i64,
    status: &str,
    sent: u32,
    failed: u32,
    unknown: u32,
    remaining: u32,
) {
    bus.emit(&ServerEvent::campaign_updated(
        campaign_id,
        status,
        sent,
        failed,
        unknown,
        remaining,
    ));
}

#[allow(clippy::too_many_arguments)]
pub fn notify_notification_received(
    bus: &EventBus,
    id: i64,
    kind: &str,
    severity: &str,
    title: &str,
    conversation_id: Option<i64>,
    conversation_number: Option<i64>,
    customer_id: Option<i64>,
    target_user_local_id: Option<i64>,
    unread_count: u32,
) {
    bus.emit(&ServerEvent::notification_received(
        id,
        kind,
        severity,
        title,
        conversation_id,
        conversation_number,
        customer_id,
        target_user_local_id,
        unread_count,
    ));
}
