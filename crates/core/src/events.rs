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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RatingsRefreshedEvent {
    pub processed: u32,
    pub fresh: u32,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncCompletedEvent {
    pub kind: String,
    pub processed: u32,
    pub errors: u32,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerEvent {
    RatingReceived(RatingReceivedEvent),
    RatingsRefreshed(RatingsRefreshedEvent),
    SyncCompleted(SyncCompletedEvent),
    ConversationUpdated(ConversationUpdatedEvent),
    CampaignUpdated(CampaignUpdatedEvent),
    NotificationReceived(NotificationReceivedEvent),
    ErrorOccurred(ErrorOccurredEvent),
}

impl ServerEvent {
    #[must_use]
    pub fn wire_name(&self) -> &'static str {
        match self {
            Self::RatingReceived { .. } | Self::RatingsRefreshed { .. } => "ratings",
            Self::SyncCompleted { .. } => "sync",
            Self::ConversationUpdated { .. } => "conversation",
            Self::CampaignUpdated { .. } => "campaign",
            Self::NotificationReceived { .. } => "notification",
            Self::ErrorOccurred { .. } => "error",
        }
    }

    #[must_use]
    pub fn channel(&self) -> &'static str {
        match self {
            Self::RatingReceived { .. } | Self::RatingsRefreshed { .. } => "spp://rating/arrived",
            Self::SyncCompleted { .. } | Self::ConversationUpdated { .. } => "spp://sync/updated",
            Self::CampaignUpdated { .. } => "spp://campaign/updated",
            Self::NotificationReceived { .. } => "spp://notification/received",
            Self::ErrorOccurred { .. } => "spp://error/occurred",
        }
    }

    #[must_use]
    pub fn payload_json(&self) -> String {
        match self {
            Self::RatingReceived(e) => serde_json::to_string(e).unwrap_or_else(|_| "{}".into()),
            Self::RatingsRefreshed(e) => serde_json::to_string(e).unwrap_or_else(|_| "{}".into()),
            Self::SyncCompleted(e) => serde_json::to_string(e).unwrap_or_else(|_| "{}".into()),
            Self::ConversationUpdated(e) => {
                serde_json::to_string(e).unwrap_or_else(|_| "{}".into())
            }
            Self::CampaignUpdated(e) => serde_json::to_string(e).unwrap_or_else(|_| "{}".into()),
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
    pub fn sync_completed(kind: &str, processed: u32, errors: u32) -> Self {
        Self::SyncCompleted(SyncCompletedEvent {
            kind: kind.to_string(),
            processed,
            errors,
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
    pub fn ratings_refreshed(processed: u32, fresh: u32) -> Self {
        Self::RatingsRefreshed(RatingsRefreshedEvent {
            processed,
            fresh,
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
