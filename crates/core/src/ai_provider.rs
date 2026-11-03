use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<TokenUsage>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbedResponse {
    pub vector: Vec<f32>,
    pub dim: usize,
    pub model: String,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub role: ModelRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    Chat,
    Embedding,
}

impl ModelRole {
    pub const ALL: [Self; 2] = [Self::Chat, Self::Embedding];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Embedding => "embedding",
        }
    }
}

#[async_trait]
pub(crate) trait LocalAiProvider: Send + Sync {
    async fn chat(&self, model: &str, messages: &[ChatMessage]) -> Result<ChatResponse>;

    async fn chat_structured(&self, model: &str, messages: &[ChatMessage]) -> Result<ChatResponse> {
        self.chat(model, messages).await
    }

    async fn is_available(&self) -> bool;
}
