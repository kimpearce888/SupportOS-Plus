use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphNodeKind {
    Customer,
    Organization,
    Conversation,
    KnownIssue,
    IssueCluster,
    Incident,
    KnowledgeDocument,
    Agent,
    Campaign,
    Product,
    CustomObject,
    ConnectorData,
}

impl GraphNodeKind {
    pub const ALL: [Self; 12] = [
        Self::Customer,
        Self::Organization,
        Self::Conversation,
        Self::KnownIssue,
        Self::IssueCluster,
        Self::Incident,
        Self::KnowledgeDocument,
        Self::Agent,
        Self::Campaign,
        Self::Product,
        Self::CustomObject,
        Self::ConnectorData,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Customer => "customer",
            Self::Organization => "organization",
            Self::Conversation => "conversation",
            Self::KnownIssue => "known_issue",
            Self::IssueCluster => "issue_cluster",
            Self::Incident => "incident",
            Self::KnowledgeDocument => "knowledge_document",
            Self::Agent => "agent",
            Self::Campaign => "campaign",
            Self::Product => "product",
            Self::CustomObject => "custom_object",
            Self::ConnectorData => "connector_data",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_str() == s)
    }
}
