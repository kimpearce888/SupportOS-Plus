use std::sync::Arc;

use tokio::sync::broadcast;

use crate::events::ServerEvent;

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
