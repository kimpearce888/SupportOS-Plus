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
pub trait LocalAiProvider: Send + Sync {
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;

    async fn chat(&self, model: &str, messages: &[ChatMessage]) -> Result<ChatResponse>;

    async fn embed(&self, model: &str, text: &str) -> Result<EmbedResponse>;

    async fn is_available(&self) -> bool;
}

#[derive(Debug)]
pub struct FakeAiProvider {
    embedding_dim: usize,
    models: Vec<ModelInfo>,
}

impl FakeAiProvider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            embedding_dim: 8,
            models: vec![
                ModelInfo {
                    id: "fake-chat-model".into(),
                    role: ModelRole::Chat,
                },
                ModelInfo {
                    id: "fake-embedding-model".into(),
                    role: ModelRole::Embedding,
                },
            ],
        }
    }

    #[must_use]
    pub fn with_embedding_dim(mut self, dim: usize) -> Self {
        self.embedding_dim = dim;
        self
    }

    #[must_use]
    pub fn with_models(mut self, models: Vec<ModelInfo>) -> Self {
        self.models = models;
        self
    }

    fn input_hash(text: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    }

    fn deterministic_embedding(&self, text: &str) -> Vec<f32> {
        let hash = Self::input_hash(text);
        (0..self.embedding_dim)
            .map(|i| {
                let mixed = hash.wrapping_mul(31).wrapping_add(i as u64);
                (mixed % 10_000) as f32 / 5_000.0 - 1.0
            })
            .collect()
    }
}

impl Default for FakeAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LocalAiProvider for FakeAiProvider {
    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(self.models.clone())
    }

    async fn chat(&self, model: &str, messages: &[ChatMessage]) -> Result<ChatResponse> {
        let last_user_msg = messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or("(empty)");

        let content = format!(
            "[Fake AI demo response] I received your message: '{last_user_msg}'. \
             This is a deterministic placeholder — no real AI model is running. \
             Configure LM Studio or Ollama to get real AI responses."
        );

        let prompt_tokens: u64 = messages.iter().map(|m| m.content.len() as u64 / 4).sum();
        let completion_tokens = content.len() as u64 / 4;

        Ok(ChatResponse {
            content,
            model: model.to_string(),
            usage: Some(TokenUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            }),
            finish_reason: Some("stop".into()),
        })
    }

    async fn embed(&self, model: &str, text: &str) -> Result<EmbedResponse> {
        let vector = self.deterministic_embedding(text);
        let dim = vector.len();
        Ok(EmbedResponse {
            vector,
            dim,
            model: model.to_string(),
            usage: Some(TokenUsage {
                prompt_tokens: text.len() as u64 / 4,
                completion_tokens: 0,
                total_tokens: text.len() as u64 / 4,
            }),
        })
    }

    async fn is_available(&self) -> bool {
        true
    }
}

#[derive(Debug, Default)]
pub struct NoopAiProvider;

#[async_trait]
impl LocalAiProvider for NoopAiProvider {
    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(Vec::new())
    }

    async fn chat(&self, _model: &str, _messages: &[ChatMessage]) -> Result<ChatResponse> {
        Ok(ChatResponse {
            content: "Unknown".into(),
            model: "none".into(),
            usage: None,
            finish_reason: Some("no_provider".into()),
        })
    }

    async fn embed(&self, _model: &str, _text: &str) -> Result<EmbedResponse> {
        Ok(EmbedResponse {
            vector: Vec::new(),
            dim: 0,
            model: "none".into(),
            usage: None,
        })
    }

    async fn is_available(&self) -> bool {
        false
    }
}
