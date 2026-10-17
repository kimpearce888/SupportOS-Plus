use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopilotTool {
    SearchConversations,
    SearchKnowledge,
    SearchKnownIssues,
    SearchSavedReplies,
    GetConversation,
    GetSupportMetrics,
    GetConversationContext,
    GetCustomerHistory,
    GetSimilarConversations,
    GetIssueClusters,
    GetAiAnalysis,
    GetSupportosMetadata,
    GetAiAttributes,
    SearchIncidents,
    SearchCustomObjects,
    GetCustomerTimeline,
    SearchConnectorData,
    GetKnowledgeGaps,
    GetFrictionReport,
    GetGraphNeighbors,
    GetGraphStats,
    GetCustomerMemory,
}

impl CopilotTool {
    pub const ALL: [Self; 22] = [
        Self::SearchConversations,
        Self::SearchKnowledge,
        Self::SearchKnownIssues,
        Self::SearchSavedReplies,
        Self::GetConversation,
        Self::GetSupportMetrics,
        Self::GetConversationContext,
        Self::GetCustomerHistory,
        Self::GetSimilarConversations,
        Self::GetIssueClusters,
        Self::GetAiAnalysis,
        Self::GetSupportosMetadata,
        Self::GetAiAttributes,
        Self::SearchIncidents,
        Self::SearchCustomObjects,
        Self::GetCustomerTimeline,
        Self::SearchConnectorData,
        Self::GetKnowledgeGaps,
        Self::GetFrictionReport,
        Self::GetGraphNeighbors,
        Self::GetGraphStats,
        Self::GetCustomerMemory,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SearchConversations => "search_conversations",
            Self::SearchKnowledge => "search_knowledge",
            Self::SearchKnownIssues => "search_known_issues",
            Self::SearchSavedReplies => "search_saved_replies",
            Self::GetConversation => "get_conversation",
            Self::GetSupportMetrics => "get_support_metrics",
            Self::GetConversationContext => "get_conversation_context",
            Self::GetCustomerHistory => "get_customer_history",
            Self::GetSimilarConversations => "get_similar_conversations",
            Self::GetIssueClusters => "get_issue_clusters",
            Self::GetAiAnalysis => "get_ai_analysis",
            Self::GetSupportosMetadata => "get_supportos_metadata",
            Self::GetAiAttributes => "get_ai_attributes",
            Self::SearchIncidents => "search_incidents",
            Self::SearchCustomObjects => "search_custom_objects",
            Self::GetCustomerTimeline => "get_customer_timeline",
            Self::SearchConnectorData => "search_connector_data",
            Self::GetKnowledgeGaps => "get_knowledge_gaps",
            Self::GetFrictionReport => "get_friction_report",
            Self::GetGraphNeighbors => "get_graph_neighbors",
            Self::GetGraphStats => "get_graph_stats",
            Self::GetCustomerMemory => "get_customer_memory",
        }
    }

    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::SearchConversations => "Search conversations by text (FTS5).",
            Self::SearchKnowledge => "Search knowledge base documents.",
            Self::SearchKnownIssues => "Search known issues by name or status.",
            Self::SearchSavedReplies => "Search saved reply templates.",
            Self::GetConversation => "Get a single conversation by id.",
            Self::GetSupportMetrics => "Get current support metrics (response times, volumes).",
            Self::GetConversationContext => "Get conversation context (customer, mailbox, tags).",
            Self::GetCustomerHistory => "Get a customer's past conversations.",
            Self::GetSimilarConversations => "Find similar conversations via vector search.",
            Self::GetIssueClusters => "Get clusters of similar issues.",
            Self::GetAiAnalysis => "Get AI analysis for a conversation.",
            Self::GetSupportosMetadata => "Get SupportOS++ version and configuration metadata.",
            Self::GetAiAttributes => "Get AI-derived attributes for a conversation.",
            Self::SearchIncidents => "Search incidents by status or severity.",
            Self::SearchCustomObjects => "Search custom object types.",
            Self::GetCustomerTimeline => "Get a customer's activity timeline.",
            Self::SearchConnectorData => "Search data from configured connectors.",
            Self::GetKnowledgeGaps => "Get identified knowledge gaps.",
            Self::GetFrictionReport => "Get friction metrics for support interactions.",
            Self::GetGraphNeighbors => "Get neighbors of a node in the support graph.",
            Self::GetGraphStats => "Get statistics about the support graph.",
            Self::GetCustomerMemory => "Get AI-derived memory for a customer.",
        }
    }
}

pub const MAX_TOOL_ROUNDS: u32 = 5;

pub const MAX_TOOL_CALLS: u32 = 8;

pub const TOOL_RESULT_MAX_CHARS: usize = 4000;
