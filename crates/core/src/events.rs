use serde::{Deserialize, Serialize};

pub(crate) type RatingValue = Option<String>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RatingReceivedEvent {
    pub rating: RatingValue,
    pub conversation_id: Option<i64>,
    pub conversation_number: Option<i64>,
    pub customer_id: Option<i64>,
    pub customer_name: Option<String>,
    pub comments: Option<String>,
    pub at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SyncTrigger {
    #[default]
    Manual,
    Auto,
    Refresh,
}

impl SyncTrigger {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
            Self::Refresh => "refresh",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncCompletedEvent {
    pub kind: String,
    pub processed: u32,
    pub errors: u32,
    #[serde(default)]
    pub trigger: SyncTrigger,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationUpdatedEvent {
    pub conversation_id: Option<i64>,
    pub conversation_number: Option<i64>,
    pub mailbox_id: Option<i64>,
    pub subject: Option<String>,
    pub reason: String,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotificationReceivedEvent {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub severity: String,
    pub title: String,
    pub conversation_id: Option<i64>,
    pub conversation_number: Option<i64>,
    pub customer_id: Option<i64>,
    pub target_user_local_id: Option<i64>,
    pub unread_count: u32,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorOccurredEvent {
    pub severity: String,
    pub message: String,
    pub service: String,
    pub conversation_id: Option<i64>,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgressEvent {
    pub stage: String,
    pub done: u32,
    pub total: u32,
    pub processed: u64,
    pub stage_records: u32,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OutboundJobFinishedEvent {
    pub job_id: i64,
    pub kind: String,
    pub conversation_id: Option<i64>,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorRefreshedEvent {
    pub connector_id: i64,
    pub name: String,
    pub ok: bool,
    pub rows: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerEvent {
    RatingReceived(RatingReceivedEvent),
    SyncCompleted(SyncCompletedEvent),
    SyncProgress(SyncProgressEvent),
    ConversationUpdated(ConversationUpdatedEvent),
    NotificationReceived(NotificationReceivedEvent),
    ErrorOccurred(ErrorOccurredEvent),
    OutboundJobFinished(OutboundJobFinishedEvent),
    ConnectorRefreshed(ConnectorRefreshedEvent),
}

impl ServerEvent {
    #[must_use]
    pub fn wire_name(&self) -> &'static str {
        match self {
            Self::RatingReceived { .. } => "ratings",
            Self::SyncCompleted { .. } => "sync",
            Self::SyncProgress { .. } => "sync",
            Self::ConversationUpdated { .. } => "conversation",
            Self::NotificationReceived { .. } => "notification",
            Self::ErrorOccurred { .. } => "error",
            Self::OutboundJobFinished { .. } => "outbound",
            Self::ConnectorRefreshed { .. } => "connectors",
        }
    }

    fn now_iso() -> String {
        chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string()
    }

    #[must_use]
    pub fn conversation_updated(
        conversation_id: Option<i64>,
        conversation_number: Option<i64>,
        mailbox_id: Option<i64>,
        subject: Option<String>,
        reason: &str,
    ) -> Self {
        Self::ConversationUpdated(ConversationUpdatedEvent {
            conversation_id,
            conversation_number,
            mailbox_id,
            subject,
            reason: reason.to_string(),
            at: Self::now_iso(),
        })
    }

    #[must_use]
    pub fn sync_completed(kind: &str, processed: u32, errors: u32, trigger: SyncTrigger) -> Self {
        Self::SyncCompleted(SyncCompletedEvent {
            kind: kind.to_string(),
            processed,
            errors,
            trigger,
            at: Self::now_iso(),
        })
    }

    #[must_use]
    pub fn sync_progress(
        stage: &str,
        done: u32,
        total: u32,
        processed: u64,
        stage_records: u32,
    ) -> Self {
        Self::SyncProgress(SyncProgressEvent {
            stage: stage.to_string(),
            done,
            total,
            processed,
            stage_records,
            at: Self::now_iso(),
        })
    }

    #[must_use]
    pub fn outbound_job_finished(
        job_id: i64,
        kind: &str,
        conversation_id: Option<i64>,
        success: bool,
        error: Option<&str>,
    ) -> Self {
        Self::OutboundJobFinished(OutboundJobFinishedEvent {
            job_id,
            kind: kind.to_string(),
            conversation_id,
            success,
            error: error.map(str::to_string),
            at: Self::now_iso(),
        })
    }

    #[must_use]
    pub fn rating_received(
        rating: RatingValue,
        conversation_id: Option<i64>,
        conversation_number: Option<i64>,
        customer_id: Option<i64>,
        customer_name: Option<String>,
        comments: Option<String>,
    ) -> Self {
        Self::RatingReceived(RatingReceivedEvent {
            rating,
            conversation_id,
            conversation_number,
            customer_id,
            customer_name,
            comments,
            at: Self::now_iso(),
        })
    }

    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn notification_received(
        id: i64,
        kind: &str,
        severity: &str,
        title: &str,
        conversation_id: Option<i64>,
        conversation_number: Option<i64>,
        customer_id: Option<i64>,
        target_user_local_id: Option<i64>,
        unread_count: u32,
    ) -> Self {
        Self::NotificationReceived(NotificationReceivedEvent {
            id,
            kind: kind.to_string(),
            severity: severity.to_string(),
            title: title.to_string(),
            conversation_id,
            conversation_number,
            customer_id,
            target_user_local_id,
            unread_count,
            at: Self::now_iso(),
        })
    }

    #[must_use]
    pub fn error_occurred(
        severity: &str,
        message: &str,
        service: &str,
        conversation_id: Option<i64>,
    ) -> Self {
        Self::ErrorOccurred(ErrorOccurredEvent {
            severity: severity.to_string(),
            message: message.to_string(),
            service: service.to_string(),
            conversation_id,
            at: Self::now_iso(),
        })
    }

    #[must_use]
    pub fn connector_refreshed(
        connector_id: i64,
        name: &str,
        ok: bool,
        rows: u32,
        error: Option<&str>,
    ) -> Self {
        Self::ConnectorRefreshed(ConnectorRefreshedEvent {
            connector_id,
            name: name.to_string(),
            ok,
            rows,
            error: error.map(str::to_string),
            at: Self::now_iso(),
        })
    }
}

#[cfg(test)]
mod batch20_audit23_tests {
    use super::*;

    #[test]
    fn audit23_sync_completed_carries_and_roundtrips_the_trigger() {
        for trigger in [SyncTrigger::Manual, SyncTrigger::Auto, SyncTrigger::Refresh] {
            let event = ServerEvent::sync_completed("incremental", 7, 0, trigger);
            let ServerEvent::SyncCompleted(e) = &event else {
                panic!("wrong variant")
            };
            assert_eq!(e.trigger, trigger);
            assert_eq!(e.trigger.as_str(), trigger.as_str());

            let json = serde_json::to_string(e).expect("serialize");
            assert!(
                json.contains(&format!("\"trigger\":\"{}\"", trigger.as_str())),
                "wire payload must carry the trigger: {json}"
            );
            let back: SyncCompletedEvent = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back.trigger, trigger);
            assert_eq!(back, *e);
        }
    }

    #[test]
    fn audit23_legacy_payload_without_trigger_defaults_to_manual() {
        let legacy = r#"{"kind":"initial","processed":25,"errors":0,"at":"2026-01-01T00:00:00Z"}"#;
        let e: SyncCompletedEvent = serde_json::from_str(legacy).expect("legacy deserializes");
        assert_eq!(e.trigger, SyncTrigger::Manual);
        assert_eq!(e.processed, 25);
    }
}
