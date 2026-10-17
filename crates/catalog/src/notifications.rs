use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    CustomerReplied,
    TicketAssigned,
    Mentioned,
    TeamMentioned,
    SlaRisk,
    SlaBreach,
    AutomationApproval,
    AiEscalation,
    KnownIssueDetected,
    IssueSpike,
    CampaignReply,
    SyncFailure,
    JobFailure,
    CustomerEvent,
    IncidentUpdate,
}

impl NotificationType {
    pub const ALL: [Self; 15] = [
        Self::CustomerReplied,
        Self::TicketAssigned,
        Self::Mentioned,
        Self::TeamMentioned,
        Self::SlaRisk,
        Self::SlaBreach,
        Self::AutomationApproval,
        Self::AiEscalation,
        Self::KnownIssueDetected,
        Self::IssueSpike,
        Self::CampaignReply,
        Self::SyncFailure,
        Self::JobFailure,
        Self::CustomerEvent,
        Self::IncidentUpdate,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CustomerReplied => "customer_replied",
            Self::TicketAssigned => "ticket_assigned",
            Self::Mentioned => "mentioned",
            Self::TeamMentioned => "team_mentioned",
            Self::SlaRisk => "sla_risk",
            Self::SlaBreach => "sla_breach",
            Self::AutomationApproval => "automation_approval",
            Self::AiEscalation => "ai_escalation",
            Self::KnownIssueDetected => "known_issue_detected",
            Self::IssueSpike => "issue_spike",
            Self::CampaignReply => "campaign_reply",
            Self::SyncFailure => "sync_failure",
            Self::JobFailure => "job_failure",
            Self::CustomerEvent => "customer_event",
            Self::IncidentUpdate => "incident_update",
        }
    }

    #[must_use]
    pub fn default_enabled(self) -> bool {
        true
    }

    #[must_use]
    pub fn severity(self) -> &'static str {
        match self {
            Self::SlaBreach | Self::SyncFailure => "critical",
            Self::SlaRisk
            | Self::AutomationApproval
            | Self::AiEscalation
            | Self::IssueSpike
            | Self::JobFailure
            | Self::IncidentUpdate => "warning",
            _ => "info",
        }
    }
}
