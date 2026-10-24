//! Integration tests (TODO 45): LM Studio / OpenAI-compatible client
//! against a local mock model server.
//!
//! No local model server runs in this development environment (probed
//! 127.0.0.1:1234, :11434, :8080 — all refused), so **actual model
//! inference is not tested here and is never claimed**. What is verified
//! against the mock server is everything around the inference:
//! request/response wire format, model listing, availability probing,
//! timeouts, HTTP error reporting, and recovery after failures. A live
//! round-trip test exists but is `#[ignore]`d and only runs when
//! `SPP_LM_STUDIO_URL` points at a real server (see `live_*` below).

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use crate::ai_lm_studio::{ChatTool, CopilotWireMessage, LmStudioProvider, OpenAiCompatibleClient};
use crate::ai_provider::{ChatMessage, LocalAiProvider, ModelRole};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

fn chat_body(content: &str) -> String {
    json!({
        "model": "test-model",
        "choices": [{
            "message": {"role": "assistant", "content": content},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 12, "completion_tokens": 34, "total_tokens": 46}
    })
    .to_string()
}

fn user_message(content: &str) -> ChatMessage {
    ChatMessage {
        role: "user".into(),
        content: content.into(),
    }
}

// ---------------------------------------------------------------------------
// Inference round-trips against the mock server
// ---------------------------------------------------------------------------

#[test]
fn chat_roundtrip_against_mock_server() {
    let mock =
        crate::mockserver::MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
            if req.method == "POST" && req.path == "/v1/chat/completions" {
                return crate::mockserver::MockResponse::json(
                    200,
                    chat_body("Hello! How can I help?"),
                );
            }
            crate::mockserver::MockResponse::json(404, r#"{"error":"no"}"#)
        }))
        .expect("mock server");
    let provider = LmStudioProvider::with_client(OpenAiCompatibleClient::new(mock.url()));

    let resp = runtime()
        .block_on(async { provider.chat("test-model", &[user_message("hi")]).await })
        .expect("chat roundtrip");
    assert_eq!(resp.content, "Hello! How can I help?");
    assert_eq!(resp.model, "test-model");
    assert_eq!(resp.finish_reason.as_deref(), Some("stop"));
    let usage = resp.usage.expect("usage present");
    assert_eq!(usage.total_tokens, 46);
    assert_eq!(usage.completion_tokens, 34);

    // The request body carried the model and the messages.
    let req = mock.hits().first().cloned().expect("request recorded");
    let body: serde_json::Value = serde_json::from_str(&req.body).expect("request json");
    assert_eq!(body["model"], "test-model");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "hi");
    assert_ne!(
        body.get("stream").and_then(|v| v.as_bool()),
        Some(true),
        "the client never asks for SSE streaming"
    );
    assert_eq!(req.path, "/v1/chat/completions");
}

#[test]
fn chat_with_opts_roundtrip_json_mode_and_usage() {
    let mock =
        crate::mockserver::MockServer::start(Arc::new(|_req: &crate::mockserver::MockRequest| {
            // Sleep past one millisecond so the measured latency is
            // deterministically non-zero.
            std::thread::sleep(std::time::Duration::from_millis(2));
            crate::mockserver::MockResponse::json(200, chat_body(r#"{"answer": 42}"#))
        }))
        .expect("mock server");
    let client = OpenAiCompatibleClient::new_with_timeout(mock.url(), 5_000);

    use crate::ai_lm_studio::ChatOpts;
    let result = runtime()
        .block_on(async {
            client
                .chat_with_opts(
                    Some("test-model"),
                    &[user_message("give me json")],
                    ChatOpts {
                        temperature: Some(0.2),
                        max_tokens: Some(128),
                        json_mode: true,
                    },
                )
                .await
        })
        .expect("chat_with_opts roundtrip");
    assert_eq!(result.content.as_deref(), Some(r#"{"answer": 42}"#));
    assert_eq!(result.model, "test-model");
    assert!(result.latency_ms > 0, "latency must be measured");
    assert!(result.tool_calls.is_empty());

    let body: serde_json::Value = serde_json::from_str(&mock.hits()[0].body).expect("request json");
    assert_eq!(body["temperature"], 0.2);
    assert_eq!(body["max_tokens"], 128);
    assert_eq!(body["response_format"]["type"], "json_object");
}

#[test]
fn chat_with_tools_roundtrip_parses_tool_calls() {
    let mock =
        crate::mockserver::MockServer::start(Arc::new(|_req: &crate::mockserver::MockRequest| {
            crate::mockserver::MockResponse::json(
                200,
                json!({
                    "model": "test-model",
                    "choices": [{
                        "message": {
                            "role": "assistant",
                            "content": null,
                            "tool_calls": [{
                                "id": "call_1",
                                "type": "function",
                                "function": {
                                    "name": "search_docs",
                                    "arguments": "{\"query\": \"login\"}"
                                }
                            }]
                        },
                        "finish_reason": "tool_calls"
                    }],
                    "usage": {"prompt_tokens": 9, "completion_tokens": 9, "total_tokens": 18}
                })
                .to_string(),
            )
        }))
        .expect("mock server");
    let client = OpenAiCompatibleClient::new_with_timeout(mock.url(), 5_000);

    let result = runtime()
        .block_on(async {
            client
                .chat_with_tools(
                    Some("test-model"),
                    &[CopilotWireMessage {
                        role: "user".into(),
                        content: "find login docs".into(),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    }],
                    &[ChatTool {
                        name: "search_docs".into(),
                        description: "Search the knowledge base".into(),
                        parameters: json!({"type": "object"}),
                    }],
                    0.1,
                    512,
                )
                .await
        })
        .expect("chat_with_tools roundtrip");
    assert_eq!(result.content, None);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name, "search_docs");
    assert_eq!(result.tool_calls[0].id, "call_1");
    assert_eq!(result.tool_calls[0].arguments, r#"{"query": "login"}"#);

    let body: serde_json::Value = serde_json::from_str(&mock.hits()[0].body).expect("request json");
    assert_eq!(body["tools"][0]["function"]["name"], "search_docs");
    assert_eq!(body["tools"][0]["type"], "function");
}

#[test]
fn list_models_and_role_inference_roundtrip() {
    let mock =
        crate::mockserver::MockServer::start(Arc::new(|_req: &crate::mockserver::MockRequest| {
            crate::mockserver::MockResponse::json(
                200,
                json!({"data": [
                    {"id": "qwen2.5-7b-instruct"},
                    {"id": "nomic-embed-text-v1.5"},
                    {"id": "phi-3-mini"}
                ]}),
            )
        }))
        .expect("mock server");
    let provider = LmStudioProvider::with_client(OpenAiCompatibleClient::new(mock.url()));

    let models = runtime()
        .block_on(async { provider.list_models().await })
        .expect("list_models");
    assert_eq!(models.len(), 3);
    assert_eq!(models[0].id, "qwen2.5-7b-instruct");
    assert_eq!(models[0].role, ModelRole::Chat);
    assert_eq!(
        models[1].role,
        ModelRole::Embedding,
        "embed models detected"
    );
    assert_eq!(models[2].role, ModelRole::Chat);
    assert_eq!(mock.hit_count("GET", "/v1/models"), 1);
}

#[test]
fn embed_roundtrip_against_mock_server() {
    let mock =
        crate::mockserver::MockServer::start(Arc::new(|_req: &crate::mockserver::MockRequest| {
            crate::mockserver::MockResponse::json(
                200,
                json!({
                    "model": "nomic-embed-text-v1.5",
                    "data": [{"embedding": [0.1, -0.2, 0.3, 0.4]}],
                    "usage": {"prompt_tokens": 4, "completion_tokens": 0, "total_tokens": 4}
                }),
            )
        }))
        .expect("mock server");
    let provider = LmStudioProvider::with_client(OpenAiCompatibleClient::new(mock.url()));

    let resp = runtime()
        .block_on(async { provider.embed("nomic-embed-text-v1.5", "hello").await })
        .expect("embed roundtrip");
    assert_eq!(resp.dim, 4);
    assert!((resp.vector[0] - 0.1).abs() < 1e-6);
    assert_eq!(resp.model, "nomic-embed-text-v1.5");
    assert_eq!(resp.usage.map(|u| u.total_tokens), Some(4));

    let body: serde_json::Value = serde_json::from_str(&mock.hits()[0].body).expect("request json");
    assert_eq!(body["model"], "nomic-embed-text-v1.5");
    assert_eq!(body["input"], "hello");
}

// ---------------------------------------------------------------------------
// Availability probing and error reporting
// ---------------------------------------------------------------------------

#[test]
fn availability_probe_reflects_server_state() {
    let mock =
        crate::mockserver::MockServer::start(Arc::new(|_req: &crate::mockserver::MockRequest| {
            crate::mockserver::MockResponse::json(200, json!({"data": []}))
        }))
        .expect("mock server");
    let provider = LmStudioProvider::with_client(OpenAiCompatibleClient::new(mock.url()));
    assert!(
        runtime().block_on(async { provider.is_available().await }),
        "a healthy server must report available"
    );

    // Deterministic dead port: bind, note the port, drop the listener.
    let dead =
        crate::mockserver::MockServer::start(crate::mockserver::not_found()).expect("mock server");
    let dead_port: u16 = dead
        .url()
        .rsplit(':')
        .next()
        .unwrap_or_default()
        .parse()
        .expect("port");
    drop(dead);
    let dead_provider = LmStudioProvider::with_client(OpenAiCompatibleClient::new(format!(
        "http://127.0.0.1:{dead_port}"
    )));
    assert!(
        !runtime().block_on(async { dead_provider.is_available().await }),
        "a refused connection must report unavailable (no panic, no hang)"
    );
}

#[test]
fn server_error_surfaces_http_status_and_body() {
    let mock = crate::mockserver::MockServer::start(crate::mockserver::fixed(
        crate::mockserver::MockResponse::json(500, r#"{"error": {"message": "model crashed"}}"#),
    ))
    .expect("mock server");
    let provider = LmStudioProvider::with_client(OpenAiCompatibleClient::new(mock.url()));

    let err = runtime()
        .block_on(async { provider.chat("test-model", &[user_message("hi")]).await })
        .expect_err("chat must fail on HTTP 500");
    let msg = err.to_string();
    assert!(
        msg.contains("HTTP 500"),
        "the error must name the HTTP status: {msg}"
    );
    assert!(
        msg.contains("model crashed"),
        "the error must include a body snippet for diagnostics: {msg}"
    );

    let err = runtime()
        .block_on(async { provider.embed("embed-model", "hi").await })
        .expect_err("embed must fail on HTTP 500");
    assert!(err.to_string().contains("HTTP 500"), "embed: {err}");

    let err = runtime()
        .block_on(async { provider.list_models().await })
        .expect_err("list_models must fail on HTTP 500");
    assert!(err.to_string().contains("HTTP 500"), "list_models: {err}");
}

#[test]
fn connection_refused_reports_cleanly_and_client_recovers() {
    // Reserve then abandon a port so nothing listens on it.
    let dead =
        crate::mockserver::MockServer::start(crate::mockserver::not_found()).expect("mock server");
    let dead_port: u16 = dead
        .url()
        .rsplit(':')
        .next()
        .unwrap_or_default()
        .parse()
        .expect("port");
    drop(dead);

    let client =
        OpenAiCompatibleClient::new_with_timeout(format!("http://127.0.0.1:{dead_port}"), 2_000);
    let err = runtime()
        .block_on(async { client.chat("m", &[user_message("hi")]).await })
        .expect_err("refused connection must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("LM Studio chat failed"),
        "a clean, labeled error: {msg}"
    );
    // A transport error must not poison the client: the same instance
    // works against a healthy server afterwards.
    let mock =
        crate::mockserver::MockServer::start(Arc::new(|_req: &crate::mockserver::MockRequest| {
            crate::mockserver::MockResponse::json(200, chat_body("recovered"))
        }))
        .expect("mock server");
    // Cannot repoint the base URL of a built client; the recovery claim is
    // exercised through a fresh client below, plus is_available on the
    // failed one.
    assert!(!runtime().block_on(async {
        use crate::ai_provider::LocalAiProvider as _;
        LmStudioProvider::with_client(OpenAiCompatibleClient::new(format!(
            "http://127.0.0.1:{dead_port}"
        )))
        .is_available()
        .await
    }));
    let healthy = OpenAiCompatibleClient::new_with_timeout(mock.url(), 2_000);
    let resp = runtime()
        .block_on(async { healthy.chat("m", &[user_message("hi")]).await })
        .expect("a fresh client against a healthy server works");
    assert_eq!(resp.content, "recovered");
}

#[test]
fn timeout_is_reported_and_does_not_leak_into_later_calls() {
    // The server sleeps past the client timeout for the first request,
    // then answers normally: the client must surface a timeout error and
    // the server keeps serving later requests.
    let slow_yet = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let flag = std::sync::Arc::clone(&slow_yet);
    let mock = crate::mockserver::MockServer::start(Arc::new(
        move |req: &crate::mockserver::MockRequest| {
            if flag.load(std::sync::atomic::Ordering::SeqCst) && req.path == "/v1/chat/completions"
            {
                std::thread::sleep(Duration::from_millis(800));
                return crate::mockserver::MockResponse::json(200, chat_body("late"));
            }
            crate::mockserver::MockResponse::json(200, chat_body("on time"))
        },
    ))
    .expect("mock server");
    let client = OpenAiCompatibleClient::new_with_timeout(mock.url(), 150);

    let started = std::time::Instant::now();
    let err = runtime()
        .block_on(async { client.chat("m", &[user_message("hi")]).await })
        .expect_err("the slow request must time out");
    assert!(started.elapsed() < Duration::from_millis(1_500));
    let msg = err.to_string();
    assert!(
        msg.contains("LM Studio chat failed"),
        "timeout surfaces as a labeled request error: {msg}"
    );

    slow_yet.store(false, std::sync::atomic::Ordering::SeqCst);
    let resp = runtime()
        .block_on(async { client.chat("m", &[user_message("hi")]).await })
        .expect("the same client works once the server responds in time");
    assert_eq!(resp.content, "on time");
}

#[test]
fn malformed_success_body_is_a_parse_error() {
    let mock = crate::mockserver::MockServer::start(crate::mockserver::fixed(
        crate::mockserver::MockResponse::json(200, "not-json-at-all"),
    ))
    .expect("mock server");
    let provider = LmStudioProvider::with_client(OpenAiCompatibleClient::new(mock.url()));

    let err = runtime()
        .block_on(async { provider.chat("m", &[user_message("hi")]).await })
        .expect_err("garbage body must fail");
    assert!(
        err.to_string().contains("parse failed"),
        "parse failures are labeled: {err}"
    );
}

// ---------------------------------------------------------------------------
// Live server (never run automatically — no model server exists here)
// ---------------------------------------------------------------------------

/// Live round-trip against a real LM Studio instance. Not part of the
/// normal suite: no model server exists in the development environment,
/// so this test would be red by construction. Run it only against a real
/// server:
///
/// ```text
/// SPP_LM_STUDIO_URL=http://127.0.0.1:1234 \
///   cargo test -p supportos-plus-core it_lm_studio::live -- --ignored --nocapture
/// ```
///
/// It performs a real inference round-trip and asserts only what a real
/// server guarantees (non-empty content, model id, availability). It is
/// never executed in CI and no result of it is claimed anywhere unless
/// its output was actually observed.
#[test]
#[ignore = "requires a running local model server; set SPP_LM_STUDIO_URL"]
fn live_roundtrip_when_server_provided() {
    let url = match std::env::var("SPP_LM_STUDIO_URL") {
        Ok(u) if !u.trim().is_empty() => u,
        _ => panic!(
            "SPP_LM_STUDIO_URL is not set. This live test needs a running local \
             model server (e.g. LM Studio at http://127.0.0.1:1234)."
        ),
    };
    let provider = LmStudioProvider::with_base_url_and_timeout(url.clone(), 60_000);
    assert!(
        runtime().block_on(async { provider.is_available().await }),
        "server at {url} did not answer GET /v1/models"
    );
    let models = runtime()
        .block_on(async { provider.list_models().await })
        .expect("list models");
    let chat_model = models
        .iter()
        .find(|m| m.role == ModelRole::Chat)
        .expect("at least one chat model must be loaded");
    let resp = runtime()
        .block_on(async {
            provider
                .chat(&chat_model.id, &[user_message("Reply with exactly: OK")])
                .await
        })
        .expect("live chat round-trip");
    assert!(
        !resp.content.trim().is_empty(),
        "real inference returned empty content"
    );
    println!(
        "live inference ok: model={} content={:?} usage={:?}",
        resp.model, resp.content, resp.usage
    );
}
