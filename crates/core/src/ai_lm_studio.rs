use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::ai_provider::{
    ChatMessage, ChatResponse, EmbedResponse, LocalAiProvider, ModelInfo, ModelRole, TokenUsage,
};
use crate::error::{Error, Result};

pub const LM_STUDIO_BASE_URL: &str = "http://127.0.0.1:1234/v1";

pub const LM_STUDIO_DEFAULT_TIMEOUT_MS: u64 = 120_000;

#[must_use]
pub fn normalize_v1_base_url(url: &str) -> String {
    format!("{}/v1", crate::embeddings::normalize_lm_base_url(url))
}

#[derive(Debug, Clone)]
pub struct LmStudioProvider {
    client: OpenAiCompatibleClient,
}

impl LmStudioProvider {
    #[must_use]
    pub fn new() -> Self {
        Self::with_base_url(LM_STUDIO_BASE_URL)
    }

    #[must_use]
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            client: OpenAiCompatibleClient::new(base_url),
        }
    }

    #[must_use]
    /// Wrap an existing client (the provider hub hands out clients that
    /// already carry the endpoint base URL, timeout and optional API key).
    pub fn with_client(client: OpenAiCompatibleClient) -> Self {
        Self { client }
    }

    pub fn with_base_url_and_timeout(base_url: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            client: OpenAiCompatibleClient::new_with_timeout(base_url, timeout_ms),
        }
    }
}

impl Default for LmStudioProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LocalAiProvider for LmStudioProvider {
    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        self.client.list_models().await
    }

    async fn chat(&self, model: &str, messages: &[ChatMessage]) -> Result<ChatResponse> {
        self.client.chat(model, messages).await
    }

    async fn embed(&self, model: &str, text: &str) -> Result<EmbedResponse> {
        self.client.embed(model, text).await
    }

    async fn is_available(&self) -> bool {
        self.client.is_available().await
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleClient {
    base_url: String,
    http: reqwest::Client,
    api_key: Option<String>,
}

impl OpenAiCompatibleClient {
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self::new_with_timeout(base_url, LM_STUDIO_DEFAULT_TIMEOUT_MS)
    }

    #[must_use]
    pub fn new_with_timeout(base_url: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            base_url: normalize_v1_base_url(&base_url.into()),
            http: build_http_client(timeout_ms),
            api_key: None,
        }
    }

    #[must_use]
    pub fn with_timeout_ms(self, timeout_ms: u64) -> Self {
        Self {
            base_url: self.base_url,
            http: build_http_client(timeout_ms),
            api_key: self.api_key,
        }
    }

    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn is_available(&self) -> bool {
        match self
            .http
            .get(format!("{}/models", self.base_url))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let resp = self
            .http
            .get(format!("{}/models", self.base_url))
            .send()
            .await
            .map_err(|e| Error::Config(format!("LM Studio list_models failed: {e}")))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| Error::Config(format!("LM Studio list_models read failed: {e}")))?;
        if !status.is_success() {
            let snippet: String = text.chars().take(300).collect();
            return Err(Error::Config(format!(
                "LM Studio list_models request failed (HTTP {status}): {snippet}"
            )));
        }
        let body: ModelsResponse = serde_json::from_str(&text)
            .map_err(|e| Error::Config(format!("LM Studio list_models parse failed: {e}")))?;
        Ok(body.data.into_iter().map(infer_model_role).collect())
    }

    pub async fn chat(&self, model: &str, messages: &[ChatMessage]) -> Result<ChatResponse> {
        let request = build_chat_request_body(model, messages);
        let mut req_builder = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .json(&request);
        if let Some(ref key) = self.api_key {
            req_builder = req_builder.bearer_auth(key);
        }
        let resp = req_builder
            .send()
            .await
            .map_err(|e| Error::Config(format!("LM Studio chat failed: {e}")))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| Error::Config(format!("LM Studio chat read failed: {e}")))?;
        if !status.is_success() {
            let snippet: String = text.chars().take(300).collect();
            return Err(Error::Config(format!(
                "LM Studio chat request failed (HTTP {status}): {snippet}"
            )));
        }
        let body: ChatCompletionResponse = serde_json::from_str(&text)
            .map_err(|e| Error::Config(format!("LM Studio chat parse failed: {e}")))?;
        Ok(parse_chat_response(body))
    }

    pub async fn chat_with_opts(
        &self,
        model: Option<&str>,
        messages: &[ChatMessage],
        opts: ChatOpts,
    ) -> Result<ChatOptsResult> {
        let mut request = ChatCompletionRequest {
            model: model.unwrap_or_default().to_string(),
            messages: messages
                .iter()
                .map(|m| ChatCompletionMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                })
                .collect(),
            temperature: opts.temperature,
            max_tokens: opts.max_tokens,
            response_format: if opts.json_mode {
                Some(serde_json::json!({ "type": "json_object" }))
            } else {
                None
            },
            stream: false,
        };
        if model.is_none() {
            request.model = String::new();
            let shim = serde_json::to_value(&request)?;
            let mut map = shim.as_object().cloned().unwrap_or_default();
            map.remove("model");
            return self.post_chat_body(serde_json::Value::Object(map)).await;
        }
        self.post_chat_body(serde_json::to_value(&request)?).await
    }

    async fn post_chat_body(&self, body: serde_json::Value) -> Result<ChatOptsResult> {
        let started = std::time::Instant::now();
        let mut req_builder = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .json(&body);
        if let Some(ref key) = self.api_key {
            req_builder = req_builder.bearer_auth(key);
        }
        let resp = req_builder
            .send()
            .await
            .map_err(|e| Error::Config(format!("LM Studio chat failed: {e}")))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| Error::Config(format!("LM Studio chat read failed: {e}")))?;
        if !status.is_success() {
            let snippet: String = text.chars().take(300).collect();
            return Err(Error::Config(format!(
                "LM Studio chat request failed (HTTP {status}): {snippet}"
            )));
        }
        let body: ChatCompletionResponse = serde_json::from_str(&text)
            .map_err(|e| Error::Config(format!("LM Studio chat parse failed: {e}")))?;
        let latency_ms = started.elapsed().as_millis() as u64;
        let content = body.choices.first().and_then(|c| c.message.content.clone());
        let tool_calls = body
            .choices
            .first()
            .map(|c| {
                c.message
                    .tool_calls
                    .iter()
                    .flatten()
                    .map(|tc| ToolCall {
                        id: tc.id.clone(),
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let usage = body.usage.map(|u| crate::ai_provider::TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });
        Ok(ChatOptsResult {
            content,
            model: body.model,
            latency_ms,
            tool_calls,
            usage,
        })
    }

    pub async fn chat_with_tools(
        &self,
        model: Option<&str>,
        messages: &[CopilotWireMessage],
        tools: &[ChatTool],
        temperature: f64,
        max_tokens: u32,
    ) -> Result<ChatOptsResult> {
        let mut body = serde_json::json!({
            "messages": messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
            "stream": false,
        });
        if !tools.is_empty() {
            body["tools"] = serde_json::to_value(
                tools
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "type": "function",
                            "function": {
                                "name": t.name,
                                "description": t.description,
                                "parameters": t.parameters,
                            },
                        })
                    })
                    .collect::<Vec<_>>(),
            )?;
        }
        if let Some(model) = model {
            body["model"] = serde_json::Value::String(model.to_string());
        }
        self.post_chat_body(body).await
    }

    pub async fn embed(&self, model: &str, text: &str) -> Result<EmbedResponse> {
        let request = build_embed_request_body(model, text);
        let mut req_builder = self
            .http
            .post(format!("{}/embeddings", self.base_url))
            .json(&request);
        if let Some(ref key) = self.api_key {
            req_builder = req_builder.bearer_auth(key);
        }
        let resp = req_builder
            .send()
            .await
            .map_err(|e| Error::Config(format!("LM Studio embed failed: {e}")))?;
        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .map_err(|e| Error::Config(format!("LM Studio embed read failed: {e}")))?;
        if !status.is_success() {
            let snippet: String = body_text.chars().take(300).collect();
            return Err(Error::Config(format!(
                "LM Studio embed request failed (HTTP {status}): {snippet}"
            )));
        }
        let body: EmbeddingResponse = serde_json::from_str(&body_text)
            .map_err(|e| Error::Config(format!("LM Studio embed parse failed: {e}")))?;
        Ok(parse_embed_response(body))
    }
}

fn build_http_client(timeout_ms: u64) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms.max(1)))
        .build()
        .unwrap_or_default()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelsResponse {
    pub data: Vec<OpenAiModel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiModel {
    pub id: String,
}

#[must_use]
pub fn infer_model_role(model: OpenAiModel) -> ModelInfo {
    let role = if model.id.to_lowercase().contains("embed") {
        ModelRole::Embedding
    } else {
        ModelRole::Chat
    };
    ModelInfo { id: model.id, role }
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatCompletionMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "is_false")]
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionMessage {
    pub role: String,
    pub content: String,
}

#[must_use]
pub fn build_chat_request_body(model: &str, messages: &[ChatMessage]) -> ChatCompletionRequest {
    ChatCompletionRequest {
        model: model.to_string(),
        messages: messages
            .iter()
            .map(|m| ChatCompletionMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect(),
        temperature: None,
        max_tokens: None,
        response_format: None,
        stream: false,
    }
}

#[derive(Debug, Clone)]
pub struct ChatOptsResult {
    pub content: Option<String>,
    pub model: String,
    pub latency_ms: u64,
    pub tool_calls: Vec<ToolCall>,
    /// Token usage as reported by the endpoint (A1 cost ledger); `None`
    /// when the endpoint does not report usage.
    pub usage: Option<crate::ai_provider::TokenUsage>,
}

#[derive(Debug, Clone)]
pub struct ChatTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CopilotWireMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<CopilotWireToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CopilotWireToolCall {
    pub id: String,
    pub r#type: &'static str,
    pub function: CopilotWireToolFunction,
}

#[derive(Debug, Clone, Serialize)]
pub struct CopilotWireToolFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ChatOpts {
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub json_mode: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponse {
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Option<OpenAiUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatChoice {
    pub message: ChatChoiceMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatChoiceMessage {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<RawToolCall>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawToolCall {
    pub id: String,
    #[serde(default)]
    pub r#type: Option<String>,
    pub function: RawToolCallFunction,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawToolCallFunction {
    pub name: String,
    #[serde(default)]
    pub arguments: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct OpenAiUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[must_use]
pub fn parse_chat_response(resp: ChatCompletionResponse) -> ChatResponse {
    let content = resp
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_else(|| "Unknown".into());
    let finish_reason = resp.choices.first().and_then(|c| c.finish_reason.clone());
    let usage = resp.usage.map(|u| TokenUsage {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
    });
    ChatResponse {
        content,
        model: resp.model,
        usage,
        finish_reason,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: String,
}

#[must_use]
pub fn build_embed_request_body(model: &str, text: &str) -> EmbeddingRequest {
    EmbeddingRequest {
        model: model.to_string(),
        input: text.to_string(),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbeddingResponse {
    pub model: String,
    pub data: Vec<EmbeddingData>,
    pub usage: Option<OpenAiUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbeddingData {
    pub embedding: Vec<f32>,
}

#[must_use]
pub fn parse_embed_response(resp: EmbeddingResponse) -> EmbedResponse {
    let vector = resp
        .data
        .first()
        .map(|d| d.embedding.clone())
        .unwrap_or_default();
    let dim = vector.len();
    let usage = resp.usage.map(|u| TokenUsage {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
    });
    EmbedResponse {
        vector,
        dim,
        model: resp.model,
        usage,
    }
}
