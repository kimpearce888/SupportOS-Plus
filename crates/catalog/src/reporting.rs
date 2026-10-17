use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportMetricKey {
    Conversations,
    UniqueCustomers,
    Organizations,
    FirstResponses,
    AgentReplies,
    CustomerReplies,
    Closures,
    AvgFirstResponseMinutes,
    AvgResolutionMinutes,
    AvgWaitHours,
    SlaBreached,
    StateChanges,
    AvgStateHours,
    HighPriorityRate,
    IssueLinkedShare,
    AiAttributeShare,
    AvgCustomerEffort,
    HighFrictionRate,
    CampaignSent,
    CampaignReplies,
    CampaignReplyRate,
}

impl ReportMetricKey {
    pub const ALL: [Self; 21] = [
        Self::Conversations,
        Self::UniqueCustomers,
        Self::Organizations,
        Self::FirstResponses,
        Self::AgentReplies,
        Self::CustomerReplies,
        Self::Closures,
        Self::AvgFirstResponseMinutes,
        Self::AvgResolutionMinutes,
        Self::AvgWaitHours,
        Self::SlaBreached,
        Self::StateChanges,
        Self::AvgStateHours,
        Self::HighPriorityRate,
        Self::IssueLinkedShare,
        Self::AiAttributeShare,
        Self::AvgCustomerEffort,
        Self::HighFrictionRate,
        Self::CampaignSent,
        Self::CampaignReplies,
        Self::CampaignReplyRate,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Conversations => "conversations",
            Self::UniqueCustomers => "unique_customers",
            Self::Organizations => "organizations",
            Self::FirstResponses => "first_responses",
            Self::AgentReplies => "agent_replies",
            Self::CustomerReplies => "customer_replies",
            Self::Closures => "closures",
            Self::AvgFirstResponseMinutes => "avg_first_response_minutes",
            Self::AvgResolutionMinutes => "avg_resolution_minutes",
            Self::AvgWaitHours => "avg_wait_hours",
            Self::SlaBreached => "sla_breached",
            Self::StateChanges => "state_changes",
            Self::AvgStateHours => "avg_state_hours",
            Self::HighPriorityRate => "high_priority_rate",
            Self::IssueLinkedShare => "issue_linked_share",
            Self::AiAttributeShare => "ai_attribute_share",
            Self::AvgCustomerEffort => "avg_customer_effort",
            Self::HighFrictionRate => "high_friction_rate",
            Self::CampaignSent => "campaign_sent",
            Self::CampaignReplies => "campaign_replies",
            Self::CampaignReplyRate => "campaign_reply_rate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportDimensionKey {
    None,
    Day,
    Week,
    Month,
    Mailbox,
    Channel,
    Tag,
    Assignee,
    Team,
    Status,
    Priority,
    CustomState,
    ResponseState,
    Issue,
}

impl ReportDimensionKey {
    pub const ALL: [Self; 14] = [
        Self::None,
        Self::Day,
        Self::Week,
        Self::Month,
        Self::Mailbox,
        Self::Channel,
        Self::Tag,
        Self::Assignee,
        Self::Team,
        Self::Status,
        Self::Priority,
        Self::CustomState,
        Self::ResponseState,
        Self::Issue,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Mailbox => "mailbox",
            Self::Channel => "channel",
            Self::Tag => "tag",
            Self::Assignee => "assignee",
            Self::Team => "team",
            Self::Status => "status",
            Self::Priority => "priority",
            Self::CustomState => "custom_state",
            Self::ResponseState => "response_state",
            Self::Issue => "issue",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricFormat {
    Count,
    Rate,
    Minutes,
    Hours,
    Score,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricCatalogEntry {
    pub key: ReportMetricKey,
    pub label: &'static str,
    pub definition: &'static str,
    pub limitations: &'static str,
    pub format: MetricFormat,
    #[serde(skip_serializing_if = "is_false")]
    pub needs_attribute: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[must_use]
pub fn metric_entry(key: ReportMetricKey) -> MetricCatalogEntry {
    let (label, definition, limitations, format, needs_attribute) = match key {
        ReportMetricKey::Conversations => (
            "Conversations",
            "Conversations created in the selected range, counting each local conversation row once.",
            "Counts the local mirror; conversations deleted upstream are excluded.",
            MetricFormat::Count,
            false,
        ),
        ReportMetricKey::UniqueCustomers => (
            "Unique customers",
            "Distinct customers who started at least one conversation in the range.",
            "Counts customers with a local mirror row.",
            MetricFormat::Count,
            false,
        ),
        ReportMetricKey::Organizations => (
            "Organizations",
            "Distinct organizations of the customers who started conversations in the range.",
            "Only counts customers linked to an organization.",
            MetricFormat::Count,
            false,
        ),
        ReportMetricKey::FirstResponses => (
            "First responses",
            "Conversations whose first agent response fell inside the range.",
            "Requires the first-response timestamp, which exists only after sync observed it.",
            MetricFormat::Count,
            false,
        ),
        ReportMetricKey::AgentReplies => (
            "Agent replies",
            "Published reply threads by agents (or agent-triggered automation) in the range.",
            "Line items and internal notes are not replies.",
            MetricFormat::Count,
            false,
        ),
        ReportMetricKey::CustomerReplies => (
            "Customer replies",
            "Published customer threads in the range.",
            "Line items are not messages.",
            MetricFormat::Count,
            false,
        ),
        ReportMetricKey::Closures => (
            "Closures",
            "Conversations closed inside the range.",
            "Reopened conversations may close more than once; each closure event counts once per observed transition.",
            MetricFormat::Count,
            false,
        ),
        ReportMetricKey::AvgFirstResponseMinutes => (
            "Avg first response time (minutes)",
            "Mean minutes from conversation creation to the first agent response, over conversations that received one.",
            "Conversations still awaiting a first response are excluded, which biases the average downward.",
            MetricFormat::Minutes,
            false,
        ),
        ReportMetricKey::AvgResolutionMinutes => (
            "Avg resolution time (minutes)",
            "Mean minutes from creation to close, over conversations closed in the range.",
            "Excludes conversations still open (survivorship bias). Clock time, not working hours.",
            MetricFormat::Minutes,
            false,
        ),
        ReportMetricKey::AvgWaitHours => (
            "Avg customer waiting (hours)",
            "Mean hours a customer waited for the next agent response, estimated per conversation as close-or-now minus the last customer message, when the conversation ended in a waiting-to-close shape.",
            "Approximation from observable timestamps; ignores overlapping wait spans in the middle of a thread.",
            MetricFormat::Hours,
            false,
        ),
        ReportMetricKey::SlaBreached => (
            "SLA breached conversations",
            "Conversations in range whose current or final wait exceeded the configured SLA target for their mailbox.",
            "Uses the same expression as the SLA report; conversations without a mailbox target are not monitored.",
            MetricFormat::Count,
            false,
        ),
        ReportMetricKey::StateChanges => (
            "Custom state changes",
            "Ticket-state transitions recorded locally in the range.",
            "Pre-sync history is unknown by design (Help Scout exposes no historical state log).",
            MetricFormat::Count,
            false,
        ),
        ReportMetricKey::AvgStateHours => (
            "Avg time in custom state (hours)",
            "Mean hours conversations spent in the selected SupportOS state, from local transitions.",
            "Censored for conversations still in the state; based on local observation only.",
            MetricFormat::Hours,
            false,
        ),
        ReportMetricKey::HighPriorityRate => (
            "High-priority share",
            "Share of conversations marked high or urgent in the local SupportOS priority field.",
            "Priority is a local field; unset counts as none.",
            MetricFormat::Rate,
            false,
        ),
        ReportMetricKey::IssueLinkedShare => (
            "Issue-linked share",
            "Share of conversations linked to a local issue cluster or known issue.",
            "Linkage depends on clustering runs; unlinking removes the attribution.",
            MetricFormat::Rate,
            false,
        ),
        ReportMetricKey::AiAttributeShare => (
            "AI attribute share",
            "Share of conversations whose current local AI attribute matches the configured key/value.",
            "AI attributes require the deterministic or AI layer to have computed; missing rows count as unknown.",
            MetricFormat::Rate,
            true,
        ),
        ReportMetricKey::AvgCustomerEffort => (
            "Avg customer effort",
            "Mean effort score (0-10) from the interaction engine over analyzed conversations.",
            "Heuristic composite; conversations without customer messages have no score.",
            MetricFormat::Score,
            false,
        ),
        ReportMetricKey::HighFrictionRate => (
            "High-friction share",
            "Share of analyzed conversations with high friction.",
            "Friction is a deterministic heuristic, not a judgment about people.",
            MetricFormat::Rate,
            false,
        ),
        ReportMetricKey::CampaignSent => (
            "Campaign messages sent",
            "Outreach recipients actually sent in the range.",
            "Counts locally observed sends; unknown-state sends resolve on reconciliation.",
            MetricFormat::Count,
            false,
        ),
        ReportMetricKey::CampaignReplies => (
            "Campaign replies",
            "Outreach recipients the customer replied to in the range.",
            "Reply detection scans the local mirror of the created conversations.",
            MetricFormat::Count,
            false,
        ),
        ReportMetricKey::CampaignReplyRate => (
            "Campaign reply rate",
            "Replies divided by sent recipients for campaigns active in the range.",
            "Not email-delivery analytics; only replies observable in Help Scout conversations count.",
            MetricFormat::Rate,
            false,
        ),
    };
    MetricCatalogEntry {
        key,
        label,
        definition,
        limitations,
        format,
        needs_attribute,
    }
}

#[must_use]
pub fn all_metric_entries() -> Vec<MetricCatalogEntry> {
    ReportMetricKey::ALL
        .iter()
        .map(|k| metric_entry(*k))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionCatalogEntry {
    pub key: ReportDimensionKey,
    pub label: &'static str,
    pub definition: &'static str,
}

#[must_use]
pub fn dimension_entry(key: ReportDimensionKey) -> DimensionCatalogEntry {
    let (label, definition) = match key {
        ReportDimensionKey::None => (
            "No grouping (total)",
            "One row: the metric over the whole range.",
        ),
        ReportDimensionKey::Day => (
            "Day",
            "Calendar day of the conversation timestamp the metric is anchored to.",
        ),
        ReportDimensionKey::Week => (
            "Week",
            "ISO week starting Monday of the anchoring timestamp.",
        ),
        ReportDimensionKey::Month => ("Month", "Calendar month of the anchoring timestamp."),
        ReportDimensionKey::Mailbox => (
            "Mailbox",
            "The Help Scout mailbox the conversation belongs to.",
        ),
        ReportDimensionKey::Channel => (
            "Channel",
            "Conversation source type (email, chat, beacon...).",
        ),
        ReportDimensionKey::Tag => (
            "Tag",
            "One row per tag carried by conversations (a conversation with two tags counts in both rows).",
        ),
        ReportDimensionKey::Assignee => (
            "Assignee",
            "Assigned agent; unassigned conversations group separately.",
        ),
        ReportDimensionKey::Team => (
            "Team",
            "Team of the assigned agent, where a team assignment exists.",
        ),
        ReportDimensionKey::Status => ("Status", "Conversation status (active/closed/pending...)."),
        ReportDimensionKey::Priority => ("Priority", "Local SupportOS priority bucket."),
        ReportDimensionKey::CustomState => (
            "Custom state",
            "Local SupportOS ticket state at computation time.",
        ),
        ReportDimensionKey::ResponseState => (
            "Response state",
            "Derived response state (awaiting first response, waiting on customer...).",
        ),
        ReportDimensionKey::Issue => (
            "Issue",
            "Linked local issue cluster or known issue; unlinked conversations group separately.",
        ),
    };
    DimensionCatalogEntry {
        key,
        label,
        definition,
    }
}

#[must_use]
pub fn all_dimension_entries() -> Vec<DimensionCatalogEntry> {
    ReportDimensionKey::ALL
        .iter()
        .map(|k| dimension_entry(*k))
        .collect()
}
