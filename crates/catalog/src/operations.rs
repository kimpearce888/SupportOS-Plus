use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationsTileKey {
    Unassigned,
    NeedsFirstResponse,
    CustomerWaiting,
    WaitingOverThreshold,
    Urgent,
    SlaAtRisk,
    SlaBreached,
    HighEffort,
    RepeatedIssue,
    KnownIssue,
    AiEscalation,
    IssueSpike,
    AutomationApprovals,
    FailedJobs,
    SyncProblems,
    CampaignActivity,
}

impl OperationsTileKey {
    pub const ALL: [Self; 16] = [
        Self::Unassigned,
        Self::NeedsFirstResponse,
        Self::CustomerWaiting,
        Self::WaitingOverThreshold,
        Self::Urgent,
        Self::SlaAtRisk,
        Self::SlaBreached,
        Self::HighEffort,
        Self::RepeatedIssue,
        Self::KnownIssue,
        Self::AiEscalation,
        Self::IssueSpike,
        Self::AutomationApprovals,
        Self::FailedJobs,
        Self::SyncProblems,
        Self::CampaignActivity,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unassigned => "unassigned",
            Self::NeedsFirstResponse => "needs_first_response",
            Self::CustomerWaiting => "customer_waiting",
            Self::WaitingOverThreshold => "waiting_over_threshold",
            Self::Urgent => "urgent",
            Self::SlaAtRisk => "sla_at_risk",
            Self::SlaBreached => "sla_breached",
            Self::HighEffort => "high_effort",
            Self::RepeatedIssue => "repeated_issue",
            Self::KnownIssue => "known_issue",
            Self::AiEscalation => "ai_escalation",
            Self::IssueSpike => "issue_spike",
            Self::AutomationApprovals => "automation_approvals",
            Self::FailedJobs => "failed_jobs",
            Self::SyncProblems => "sync_problems",
            Self::CampaignActivity => "campaign_activity",
        }
    }

    #[must_use]
    pub fn severity(self) -> &'static str {
        match self {
            Self::Unassigned
            | Self::CustomerWaiting
            | Self::HighEffort
            | Self::RepeatedIssue
            | Self::KnownIssue
            | Self::CampaignActivity => "info",
            Self::NeedsFirstResponse
            | Self::WaitingOverThreshold
            | Self::Urgent
            | Self::SlaAtRisk
            | Self::AiEscalation
            | Self::IssueSpike
            | Self::AutomationApprovals
            | Self::FailedJobs
            | Self::SyncProblems => "warning",
            Self::SlaBreached => "critical",
        }
    }
}
