use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseState {
    NeedsFirstResponse,
    CustomerWaiting,
    AgentWaiting,
    Closed,
}

impl ResponseState {
    pub const ALL: [Self; 4] = [
        Self::NeedsFirstResponse,
        Self::CustomerWaiting,
        Self::AgentWaiting,
        Self::Closed,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NeedsFirstResponse => "needs_first_response",
            Self::CustomerWaiting => "customer_waiting",
            Self::AgentWaiting => "agent_waiting",
            Self::Closed => "closed",
        }
    }
}
