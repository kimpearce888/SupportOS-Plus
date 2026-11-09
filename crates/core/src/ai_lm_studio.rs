use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::ai_provider::{
    ChatMessage, ChatResponse, EmbedResponse, LocalAiProvider, ModelInfo, ModelRole, TokenUsage,
};
use crate::error::{Error, Result};

pub(crate) const LM_STUDIO_BASE_URL: &str = "http://127.0.0.1:1234/v1";

pub(crate) const LM_STUDIO_DEFAULT_TIMEOUT_MS: u64 = 120_000;

#[must_use]
pub(crate) fn normalize_v1_base_url(url: &str) -> String {
    format!("{}/v1", crate::embeddings::normalize_lm_base_url(url))
}

#[derive(Debug, Clone)]
pub(crate) struct LmStudioProvider {
    client: OpenAiCompatibleClient,
}

impl LmStudioProvider {
    #[must_use]
    pub(crate) fn with_client(client: OpenAiCompatibleClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl LocalAiProvider for LmStudioProvider {
    async fn chat(&self, model: &str, messages: &[ChatMessage]) -> Result<ChatResponse> {
        self.client.chat(model, messages).await
    }

    async fn chat_structured(&self, model: &str, messages: &[ChatMessage]) -> Result<ChatResponse> {
        let res = self
            .client
            .chat_with_opts(
                Some(model),
                messages,
                ChatOpts {
                    temperature: Some(0.15),
                    max_tokens: Some(2048),
                    json_mode: true,
                },
            )
            .await?;
        Ok(ChatResponse {
            content: res.content.unwrap_or_default(),
            model: res.model,
            usage: res.usage,
            finish_reason: None,
        })
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
    /// AUD-P2-2 (A8): the configured request budget, re-applied on
    /// every outbound request. The client-level builder timeout stays
    /// the first line of defense, but a builder failure used to fall
    /// back to `Client::default()` — silently losing the timeout (and
    /// reqwest's `Client::default()` is itself a `build().expect`, so a
    /// broken TLS backend panicked there). Per-request timeouts make
    /// the guarantee independent of which client got built.
    request_timeout: Duration,
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
            request_timeout: Duration::from_millis(timeout_ms.max(1)),
        }
    }

    #[must_use]
    pub(crate) fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) async fn is_available(&self) -> bool {
        let mut req = self
            .http
            .get(format!("{}/models", self.base_url))
            .timeout(self.request_timeout);
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }
        match req.send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let mut req = self
            .http
            .get(format!("{}/models", self.base_url))
            .timeout(self.request_timeout);
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }
        let resp = req
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
            .timeout(self.request_timeout)
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
        parse_chat_response(body)
    }

    pub(crate) async fn chat_with_opts(
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
                // UXF-2-1: LM Studio rejects OpenAI's legacy
                // `{"type":"json_object"}` with HTTP 400 ("response_format.type'
                // must be 'json_schema' or 'text'") — every failing
                // golden_eval / ticket_analysis run in the owner's
                // screenshot report died on exactly this. The portable
                // form is `json_schema` with a permissive object schema:
                // every json_mode caller wants "a JSON object, shape up
                // to the model", which `{"type":"object"}` expresses
                // without pinning properties. `strict` is deliberately
                // omitted — LM Studio accepts the schema either way (the
                // grammar allows any object), and OpenAI-compatible
                // cloud endpoints treat an unset strict as hint-mode, so
                // one payload works across LM Studio, Ollama and BYOK.
                // The response side already tolerates fenced/free-form
                // output via `extract_json`.
                Some(serde_json::json!({
                    "type": "json_schema",
                    "json_schema": {
                        "name": "structured_output",
                        "schema": { "type": "object" }
                    }
                }))
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
            .timeout(self.request_timeout)
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

    pub(crate) async fn chat_with_tools(
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
            .timeout(self.request_timeout)
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
    // AUD-P2-2 (A8): this used to end in `.unwrap_or_default()`, which
    // "recovered" from a builder failure by silently swapping in a
    // client with NO timeout (a hung LM Studio could then hang a job
    // for the 30-minute stale-job sweep) — and reqwest's
    // `Client::default()` is itself `ClientBuilder::new().build()
    // .expect("Client::new()")`, so the only realistic builder failure
    // (TLS backend init) would have panicked right here. Keep the
    // constructor infallible but make the failure loud and retry once;
    // the timeout guarantee itself no longer depends on the
    // client-level setting because every request builder in this
    // client also applies `.timeout(self.request_timeout)`.
    let timeout = Duration::from_millis(timeout_ms.max(1));
    match reqwest::Client::builder().timeout(timeout).build() {
        Ok(client) => client,
        Err(first) => {
            tracing::error!(
                error = %first,
                "HTTP client builder failed (timeout {} ms); retrying with the same budget",
                timeout_ms
            );
            reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .unwrap_or_default()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ModelsResponse {
    pub data: Vec<OpenAiModel>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct OpenAiModel {
    pub id: String,
}

#[must_use]
pub(crate) fn infer_model_role(model: OpenAiModel) -> ModelInfo {
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
pub(crate) struct ChatCompletionRequest {
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
pub(crate) struct ChatCompletionMessage {
    pub role: String,
    pub content: String,
}

#[must_use]
pub(crate) fn build_chat_request_body(
    model: &str,
    messages: &[ChatMessage],
) -> ChatCompletionRequest {
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
pub(crate) struct ChatOptsResult {
    pub content: Option<String>,
    pub model: String,
    pub latency_ms: u64,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<crate::ai_provider::TokenUsage>,
}

#[derive(Debug, Clone)]
pub(crate) struct ChatTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CopilotWireMessage {
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
pub(crate) struct CopilotWireToolCall {
    pub id: String,
    pub r#type: &'static str,
    pub function: CopilotWireToolFunction,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CopilotWireToolFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ChatOpts {
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub json_mode: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChatCompletionResponse {
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Option<OpenAiUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChatChoice {
    pub message: ChatChoiceMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChatChoiceMessage {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<RawToolCall>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RawToolCall {
    pub id: String,
    pub function: RawToolCallFunction,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RawToolCallFunction {
    pub name: String,
    #[serde(default)]
    pub arguments: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct OpenAiUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

pub(crate) fn parse_chat_response(resp: ChatCompletionResponse) -> Result<ChatResponse> {
    let Some(first) = resp.choices.first() else {
        return Err(Error::Config(
            "LM Studio chat returned no choices (malformed 200 body)".into(),
        ));
    };
    let Some(content) = first.message.content.clone() else {
        return Err(Error::Config(
            "LM Studio chat returned no message content (malformed 200 body)".into(),
        ));
    };
    let finish_reason = first.finish_reason.clone();
    let usage = resp.usage.map(|u| TokenUsage {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
    });
    Ok(ChatResponse {
        content,
        model: resp.model,
        usage,
        finish_reason,
    })
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct EmbeddingRequest {
    pub model: String,
    pub input: String,
}

#[must_use]
pub(crate) fn build_embed_request_body(model: &str, text: &str) -> EmbeddingRequest {
    EmbeddingRequest {
        model: model.to_string(),
        input: text.to_string(),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EmbeddingResponse {
    pub model: String,
    pub data: Vec<EmbeddingData>,
    pub usage: Option<OpenAiUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EmbeddingData {
    pub embedding: Vec<f32>,
}

#[must_use]
pub(crate) fn parse_embed_response(resp: EmbeddingResponse) -> EmbedResponse {
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

#[cfg(test)]
mod batch29_audit101_tests {
    use super::*;

    fn body_with(choice_content: Option<&str>) -> ChatCompletionResponse {
        ChatCompletionResponse {
            model: "m".into(),
            choices: vec![ChatChoice {
                message: ChatChoiceMessage {
                    content: choice_content.map(str::to_string),
                    tool_calls: None,
                },
                finish_reason: Some("stop".into()),
            }],
            usage: None,
        }
    }

    #[test]
    fn audit101_empty_choices_are_rejected_not_fabricated() {
        let empty = ChatCompletionResponse {
            model: "m".into(),
            choices: Vec::new(),
            usage: None,
        };
        let err = parse_chat_response(empty).expect_err("no choices must fail");
        assert!(
            err.to_string().contains("no choices"),
            "the error must name the malformation: {err}"
        );

        let err = parse_chat_response(body_with(None)).expect_err("null content must fail");
        assert!(
            err.to_string().contains("no message content"),
            "the error must name the malformation: {err}"
        );

        let ok =
            parse_chat_response(body_with(Some("real answer"))).expect("a readable body parses");
        assert_eq!(ok.content, "real answer");
        assert_eq!(ok.finish_reason.as_deref(), Some("stop"));
    }
}
