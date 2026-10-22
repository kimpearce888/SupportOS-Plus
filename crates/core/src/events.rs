use serde::{Deserialize, Serialize};

pub type RatingValue = Option<String>;

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

/// What started a sync run (clean-audit item 23): the completed event
/// used to be indistinguishable between the user's button press and the
/// ~30-second adaptive auto-sync timer, so the app toasted "sync
/// finished" roughly once a minute forever. Emitters must say who asked
/// for the run; the toast lane only fires for [`SyncTrigger::Manual`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SyncTrigger {
    /// A user action asked for the run (Sync Now buttons, activation,
    /// onboarding, the enqueued initial-sync job).
    #[default]
    Manual,
    /// The adaptive background timer fired on its own.
    Auto,
    /// Not a real sync at all — a post-write mirror re-read nudging the
    /// UI to refresh (saved-reply writes, webhook health checks).
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
    /// Who asked for the run (audit item 23). Defaults to `manual` when
    /// absent so old serialized payloads keep their visible outcome.
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
pub struct CampaignUpdatedEvent {
    pub campaign_id: i64,
    pub status: String,
    pub sent: u32,
    pub failed: u32,
    pub unknown: u32,
    pub remaining: u32,
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

/// Mid-run sync progress: emitted per resource (and per page of
/// conversations) so the UI can show a live "X of Y" instead of one
/// terminal event at the end of a multi-hour run (the 1.2.0 freeze).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgressEvent {
    /// Resource or stage label ("conversations", "customers", …).
    pub stage: String,
    /// Resources completed so far in this run.
    pub done: u32,
    /// Total resources planned for this run.
    pub total: u32,
    /// Cumulative records processed so far.
    pub processed: u64,
    /// Records in the just-completed stage (0 for a pure heartbeat).
    pub stage_records: u32,
    pub at: String,
}

/// An outbound job (reply / note / status change / assignment) reached a
/// terminal state. Success means the Help Scout API accepted the write —
/// the very thing that never happened before 1.3.0.
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
pub enum ServerEvent {
    RatingReceived(RatingReceivedEvent),
    SyncCompleted(SyncCompletedEvent),
    SyncProgress(SyncProgressEvent),
    ConversationUpdated(ConversationUpdatedEvent),
    CampaignUpdated(CampaignUpdatedEvent),
    NotificationReceived(NotificationReceivedEvent),
    ErrorOccurred(ErrorOccurredEvent),
    OutboundJobFinished(OutboundJobFinishedEvent),
}

impl ServerEvent {
    #[must_use]
    pub fn wire_name(&self) -> &'static str {
        match self {
            Self::RatingReceived { .. } => "ratings",
            Self::SyncCompleted { .. } => "sync",
            Self::SyncProgress { .. } => "sync",
            Self::ConversationUpdated { .. } => "conversation",
            Self::CampaignUpdated { .. } => "campaign",
            Self::NotificationReceived { .. } => "notification",
            Self::ErrorOccurred { .. } => "error",
            Self::OutboundJobFinished { .. } => "outbound",
        }
    }

    #[must_use]
    pub fn channel(&self) -> &'static str {
        match self {
            Self::RatingReceived { .. } => "spp://rating/arrived",
            Self::SyncCompleted { .. } | Self::SyncProgress { .. } => "spp://sync/updated",
            Self::ConversationUpdated { .. } => "spp://sync/updated",
            Self::CampaignUpdated { .. } => "spp://campaign/updated",
            Self::NotificationReceived { .. } => "spp://notification/received",
            Self::ErrorOccurred { .. } => "spp://error/occurred",
            Self::OutboundJobFinished { .. } => "spp://outbound/finished",
        }
    }

    #[must_use]
    pub fn payload_json(&self) -> String {
        match self {
            Self::RatingReceived(e) => serde_json::to_string(e).unwrap_or_else(|_| "{}".into()),
            Self::SyncCompleted(e) => serde_json::to_string(e).unwrap_or_else(|_| "{}".into()),
            Self::ConversationUpdated(e) => {
                serde_json::to_string(e).unwrap_or_else(|_| "{}".into())
            }
            Self::CampaignUpdated(e) => serde_json::to_string(e).unwrap_or_else(|_| "{}".into()),
            Self::SyncProgress(e) => serde_json::to_string(e).unwrap_or_else(|_| "{}".into()),
            Self::OutboundJobFinished(e) => {
                serde_json::to_string(e).unwrap_or_else(|_| "{}".into())
            }
            Self::NotificationReceived(e) => {
                serde_json::to_string(e).unwrap_or_else(|_| "{}".into())
            }
            Self::ErrorOccurred(e) => serde_json::to_string(e).unwrap_or_else(|_| "{}".into()),
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
    pub fn campaign_updated(
        campaign_id: i64,
        status: &str,
        sent: u32,
        failed: u32,
        unknown: u32,
        remaining: u32,
    ) -> Self {
        Self::CampaignUpdated(CampaignUpdatedEvent {
            campaign_id,
            status: status.to_string(),
            sent,
            failed,
            unknown,
            remaining,
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
}

pub trait EventEmitter: Send + Sync {
    fn emit(&self, event: &ServerEvent) -> Result<(), String>;
}

#[cfg(test)]
mod batch20_audit23_tests {
    use super::*;

    /// Audit item 23: the trigger is part of the event. Manual runs are
    /// the toastable ones; auto timer ticks and post-write refreshes say
    /// who they are on the wire.
    #[test]
    fn audit23_sync_completed_carries_and_roundtrips_the_trigger() {
        for trigger in [SyncTrigger::Manual, SyncTrigger::Auto, SyncTrigger::Refresh] {
            let event = ServerEvent::sync_completed("incremental", 7, 0, trigger);
            let ServerEvent::SyncCompleted(e) = &event else {
                panic!("wrong variant")
            };
            assert_eq!(e.trigger, trigger);
            assert_eq!(e.trigger.as_str(), trigger.as_str());

            // The wire JSON speaks camelCase strings and round-trips.
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

    /// Audit item 23: payloads serialized before the field existed (no
    /// `trigger` key) deserialize to `manual` — old records keep their
    /// visible outcome instead of failing the whole event.
    #[test]
    fn audit23_legacy_payload_without_trigger_defaults_to_manual() {
        let legacy = r#"{"kind":"initial","processed":25,"errors":0,"at":"2026-01-01T00:00:00Z"}"#;
        let e: SyncCompletedEvent = serde_json::from_str(legacy).expect("legacy deserializes");
        assert_eq!(e.trigger, SyncTrigger::Manual);
        assert_eq!(e.processed, 25);
    }
}
