use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use crate::ai_lm_studio::{ChatTool, CopilotWireMessage, LmStudioProvider, OpenAiCompatibleClient};
use crate::ai_provider::{ChatMessage, LocalAiProvider};

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
    assert_eq!(body["response_format"]["type"], "json_schema");
    assert_eq!(
        body["response_format"]["json_schema"]["name"],
        "structured_output"
    );
    assert_eq!(
        body["response_format"]["json_schema"]["schema"]["type"],
        "object"
    );
}

#[test]
fn golden_set_passes_against_an_lm_studio_strict_response_format_server() {
    let mock =
        crate::mockserver::MockServer::start(Arc::new(|req: &crate::mockserver::MockRequest| {
            if req.method == "POST" && req.path == "/v1/chat/completions" {
                let body: serde_json::Value =
                    serde_json::from_str(&req.body).unwrap_or(serde_json::Value::Null);
                let ty = body["response_format"]["type"].as_str().unwrap_or("");
                if !body["response_format"].is_null() && ty != "json_schema" && ty != "text" {
                    return crate::mockserver::MockResponse::json(
                        400,
                        r#"{"error":"response_format.type' must be 'json_schema' or 'text'"}"#,
                    );
                }
                return crate::mockserver::MockResponse::json(
                    200,
                    chat_body(
                        r#"{"summary":"Customer asks how to reset the password.",
                            "intent":"account_access",
                            "customer_goal":"reset the password"}"#,
                    ),
                );
            }
            crate::mockserver::MockResponse::json(404, r#"{"error":"no"}"#)
        }))
        .expect("mock server");

    let backend = crate::ai_pipeline::AiBackend::LmStudio {
        client: OpenAiCompatibleClient::new(mock.url()),
        model: Some("test-model".into()),
        endpoint_id: None,
    };
    let mut conn = rusqlite::Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");

    let rt = runtime();
    let summary = rt
        .block_on(crate::ai_eval::run_golden_set(&conn, &backend))
        .expect("golden set runs");
    assert_eq!(
        summary.get("passed").and_then(|v| v.as_i64()),
        Some(9),
        "all 9 seeded cases pass once the server accepts the request: {summary}"
    );
    assert_eq!(
        summary.get("failed").and_then(|v| v.as_i64()),
        Some(0),
        "zero failures: {summary}"
    );
    let runs = crate::ai_eval::latest_golden_runs(&conn, 1);
    assert_eq!(runs.len(), 1, "one persisted run row");
    assert_eq!(runs[0].backend, "test-model");
    assert_eq!(runs[0].passed, 9);
    assert_eq!(runs[0].failed, 0);
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
}

#[test]
fn connection_refused_reports_cleanly_and_client_recovers() {
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
    let mock =
        crate::mockserver::MockServer::start(Arc::new(|_req: &crate::mockserver::MockRequest| {
            crate::mockserver::MockResponse::json(200, chat_body("recovered"))
        }))
        .expect("mock server");
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

#[test]
fn audp2_request_timeout_budget_survives_clone_and_covers_every_lane() {
    let mock = crate::mockserver::MockServer::start(Arc::new(
        move |req: &crate::mockserver::MockRequest| {
            std::thread::sleep(Duration::from_millis(800));
            if req.path.ends_with("/embeddings") {
                return crate::mockserver::MockResponse::json(
                    200,
                    json!({"data": [{"embedding": [0.1, 0.2], "index": 0}]}).to_string(),
                );
            }
            crate::mockserver::MockResponse::json(200, chat_body("late"))
        },
    ))
    .expect("mock server");
    let client = OpenAiCompatibleClient::new_with_timeout(mock.url(), 150);
    let cloned = client.clone();

    let started = std::time::Instant::now();
    let err = runtime()
        .block_on(async { cloned.chat("m", &[user_message("hi")]).await })
        .expect_err("the slow chat must time out");
    assert!(
        started.elapsed() < Duration::from_millis(1_500),
        "the chat lane respects the budget: {:?}",
        started.elapsed()
    );
    assert!(
        err.to_string().contains("LM Studio chat failed"),
        "a labeled timeout error: {err}"
    );

    let started = std::time::Instant::now();
    let err = runtime()
        .block_on(async { cloned.embed("m", "hello").await })
        .expect_err("the slow embed must time out");
    assert!(
        started.elapsed() < Duration::from_millis(1_500),
        "the embed lane respects the budget: {:?}",
        started.elapsed()
    );
    assert!(
        err.to_string().contains("LM Studio embed failed"),
        "a labeled timeout error: {err}"
    );

    let started = std::time::Instant::now();
    assert!(
        !runtime().block_on(async {
            use crate::ai_provider::LocalAiProvider as _;
            LmStudioProvider::with_client(client).is_available().await
        }),
        "the availability probe is budgeted too"
    );
    assert!(
        started.elapsed() < Duration::from_millis(1_500),
        "the models lane respects the budget: {:?}",
        started.elapsed()
    );
}
