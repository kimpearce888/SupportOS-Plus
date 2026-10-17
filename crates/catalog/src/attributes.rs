use serde::{Deserialize, Serialize};

pub const AI_ATTRIBUTE_SCHEMA_VERSION: &str = "attributes_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiAttributeKey {
    Intent,
    Product,
    Feature,
    Issue,
    Urgency,
    FrustrationCues,
    TechnicalFamiliarity,
    CustomerGoal,
    QuestionCount,
    Risk,
    KnownIssue,
    IssueCluster,
    ResponseStyle,
    EscalationSignal,
}

impl AiAttributeKey {
    pub const ALL: [Self; 14] = [
        Self::Intent,
        Self::Product,
        Self::Feature,
        Self::Issue,
        Self::Urgency,
        Self::FrustrationCues,
        Self::TechnicalFamiliarity,
        Self::CustomerGoal,
        Self::QuestionCount,
        Self::Risk,
        Self::KnownIssue,
        Self::IssueCluster,
        Self::ResponseStyle,
        Self::EscalationSignal,
    ];

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().find(|k| k.as_str() == s).copied()
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Intent => "intent",
            Self::Product => "product",
            Self::Feature => "feature",
            Self::Issue => "issue",
            Self::Urgency => "urgency",
            Self::FrustrationCues => "frustration_cues",
            Self::TechnicalFamiliarity => "technical_familiarity",
            Self::CustomerGoal => "customer_goal",
            Self::QuestionCount => "question_count",
            Self::Risk => "risk",
            Self::KnownIssue => "known_issue",
            Self::IssueCluster => "issue_cluster",
            Self::ResponseStyle => "response_style",
            Self::EscalationSignal => "escalation_signal",
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Intent => "Intent",
            Self::Product => "Product",
            Self::Feature => "Feature",
            Self::Issue => "Issue type",
            Self::Urgency => "Urgency",
            Self::FrustrationCues => "Frustration cues",
            Self::TechnicalFamiliarity => "Technical familiarity",
            Self::CustomerGoal => "Customer goal",
            Self::QuestionCount => "Question count",
            Self::Risk => "Risk",
            Self::KnownIssue => "Known issue",
            Self::IssueCluster => "Issue cluster",
            Self::ResponseStyle => "Response style",
            Self::EscalationSignal => "Escalation signal",
        }
    }

    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::Intent => {
                "What kind of request this is (question, bug report, \
                feature request, billing, how-to...)."
            }
            Self::Product => "Product the ticket is about, when identifiable from evidence.",
            Self::Feature => "Feature area the ticket touches, when identifiable from evidence.",
            Self::Issue => "Detected issue type (from AI ticket analysis when available).",
            Self::Urgency => {
                "Observable urgency cues in customer messages (deterministic by default)."
            }
            Self::FrustrationCues => {
                "Observable frustration cues in customer messages (deterministic by default)."
            }
            Self::TechnicalFamiliarity => {
                "Technical language level used by the customer (deterministic by default)."
            }
            Self::CustomerGoal => "One-line statement of what the customer is trying to achieve.",
            Self::QuestionCount => {
                "Number of questions the customer asked in this ticket (deterministic)."
            }
            Self::Risk => {
                "Composite churn/escalation risk from deterministic signals \
                (never a personality claim)."
            }
            Self::KnownIssue => "Whether the ticket is linked to a known issue.",
            Self::IssueCluster => "Issue cluster the ticket belongs to, when clustered.",
            Self::ResponseStyle => "Recommended response style for this interaction.",
            Self::EscalationSignal => "Whether the customer explicitly signals escalation intent.",
        }
    }

    #[must_use]
    pub fn value_type(self) -> AttributeValueType {
        match self {
            Self::Intent
            | Self::Urgency
            | Self::FrustrationCues
            | Self::TechnicalFamiliarity
            | Self::Risk
            | Self::ResponseStyle => AttributeValueType::Enum,
            Self::Product
            | Self::Feature
            | Self::Issue
            | Self::CustomerGoal
            | Self::IssueCluster => AttributeValueType::Text,
            Self::QuestionCount => AttributeValueType::Number,
            Self::KnownIssue | Self::EscalationSignal => AttributeValueType::Boolean,
        }
    }

    #[must_use]
    pub fn values(self) -> &'static [&'static str] {
        match self {
            Self::Intent => &[
                "question",
                "bug_report",
                "feature_request",
                "billing",
                "how_to",
                "account_management",
                "feedback",
                "other",
            ],
            Self::Urgency => &["none", "low", "moderate", "high"],
            Self::FrustrationCues => &["none", "possible", "moderate", "strong"],
            Self::TechnicalFamiliarity => {
                &["non_technical", "mixed", "technical", "highly_technical"]
            }
            Self::Risk => &["low", "medium", "high"],
            Self::ResponseStyle => &[
                "concise",
                "detailed",
                "step_by_step",
                "technical",
                "conversational",
                "outcome_focused",
            ],
            _ => &[],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeValueType {
    Enum,
    Text,
    Number,
    Boolean,
}

impl AttributeValueType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enum => "enum",
            Self::Text => "text",
            Self::Number => "number",
            Self::Boolean => "boolean",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "enum" => Some(Self::Enum),
            "text" => Some(Self::Text),
            "number" => Some(Self::Number),
            "boolean" => Some(Self::Boolean),
            _ => None,
        }
    }
}
