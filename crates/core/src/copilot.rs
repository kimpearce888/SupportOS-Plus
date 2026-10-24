use crate::catalog::copilot::{
    CopilotTool, MAX_TOOL_CALLS, MAX_TOOL_ROUNDS, TOOL_RESULT_MAX_CHARS,
};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool: CopilotTool,
    pub args: String,
    pub output: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopilotResult {
    pub response: String,
    pub tool_calls: Vec<ToolResult>,
    pub rounds_used: u32,
    pub terminated_by_bound: bool,
}

#[must_use]
pub fn truncate_result(output: &str) -> (String, bool) {
    // Audit item 121: the cap sliced at a raw BYTE offset
    // (`&output[..TOOL_RESULT_MAX_CHARS]`), so any tool output whose
    // TOOL_RESULT_MAX_CHARS-th byte sat inside a multi-byte character
    // panicked. The cap is a display bound — it counts CHARACTERS now.
    let char_count = output.chars().count();
    if char_count <= TOOL_RESULT_MAX_CHARS {
        (output.to_string(), false)
    } else {
        let truncated: String = output.chars().take(TOOL_RESULT_MAX_CHARS).collect();
        (format!("{truncated}… [truncated]"), true)
    }
}

#[must_use]
pub fn should_stop(rounds: u32, tool_calls: u32) -> bool {
    rounds >= MAX_TOOL_ROUNDS || tool_calls >= MAX_TOOL_CALLS
}

pub fn validate_tool_allowed(tool: &CopilotTool) -> Result<()> {
    if CopilotTool::ALL.contains(tool) {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "tool {tool:?} is not in the read-only allowlist"
        )))
    }
}

#[must_use]
pub fn tool_allowlist() -> &'static [CopilotTool] {
    &CopilotTool::ALL
}

pub const fn max_tool_rounds() -> u32 {
    MAX_TOOL_ROUNDS
}

pub const fn max_tool_calls() -> u32 {
    MAX_TOOL_CALLS
}

pub const fn tool_result_max_chars() -> usize {
    TOOL_RESULT_MAX_CHARS
}

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::ai_lm_studio::{CopilotWireMessage, CopilotWireToolCall, CopilotWireToolFunction};
use crate::ai_pipeline::{self, AiBackend, LmStudioError};
use crate::ai_prompts::{
    COPILOT_SYSTEM, PROMPT_VERSIONS_COPILOT_CHAT, build_copilot_context_block,
};
use crate::ai_tools;

pub fn ensure_copilot_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS copilot_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            conversation_id INTEGER REFERENCES conversations(id) ON DELETE CASCADE,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_copilot_sessions_conversation
            ON copilot_sessions(conversation_id, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_copilot_sessions_updated
            ON copilot_sessions(updated_at DESC);

        CREATE TABLE IF NOT EXISTS copilot_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id INTEGER NOT NULL REFERENCES copilot_sessions(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            citations TEXT,
            tool_name TEXT,
            tool_calls INTEGER NOT NULL DEFAULT 0,
            latency_ms INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_copilot_messages_session
            ON copilot_messages(session_id, created_at);",
    )?;
    ai_tools::ensure_tool_tables(conn)?;
    Ok(())
}

struct ExecutedToolCall {
    name: String,
    args: String,
    result: Value,
}

pub async fn chat(
    conn: &Connection,
    backend: &AiBackend,
    index: Option<&crate::vectorstore_sqlitevec::VectorIndex>,
    question: &str,
    conversation_id: Option<i64>,
    session_id: Option<i64>,
) -> std::result::Result<Value, LmStudioError> {
    ensure_copilot_schema(conn).map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let question: String = question.trim().chars().take(4000).collect();
    if question.is_empty() {
        return Err(LmStudioError::new("The question is empty.", false));
    }

    let mut session = session_id.and_then(|id| get_session(conn, id));
    if session_id.is_some() && session.is_none() {
        return Err(LmStudioError::new("Copilot session not found.", false));
    }
    if session.is_none() {
        let conv_ok = conversation_id.is_none()
            || conn
                .query_row(
                    "SELECT 1 FROM conversations WHERE id = ?1 AND deleted_at IS NULL",
                    params![conversation_id],
                    |_| Ok(()),
                )
                .is_ok();
        if !conv_ok {
            return Err(LmStudioError::new("Conversation not found.", false));
        }
        let title: String = if question.chars().count() > 80 {
            let mut t: String = question.chars().take(77).collect();
            t.push_str("...");
            t
        } else {
            question.clone()
        };
        conn.execute(
            "INSERT INTO copilot_sessions (title, conversation_id) VALUES (?1, ?2)",
            params![title, conversation_id],
        )
        .map_err(|e| LmStudioError::new(e.to_string(), true))?;
        let new_id = conn.last_insert_rowid();
        session = get_session(conn, new_id);
    }
    let Some(session) = session else {
        return Err(LmStudioError::new("Copilot session not found.", false));
    };
    let session_conv_id = session["conversation_id"].as_i64();

    let history = list_messages(conn, session["id"].as_i64().unwrap_or_default());
    let user_message = insert_message(
        conn,
        session["id"].as_i64().unwrap_or_default(),
        "user",
        &question,
        "[]",
        None,
        0,
        None,
    );

    let context_block = context_block(conn, session_conv_id);
    let mut messages: Vec<CopilotWireMessage> = vec![CopilotWireMessage {
        role: "system".into(),
        content: format!("{COPILOT_SYSTEM}\n\n{context_block}"),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];
    for m in history.iter().rev().take(8).rev() {
        messages.push(CopilotWireMessage {
            role: if m["role"] == "tool" {
                "user".into()
            } else {
                m["role"].as_str().unwrap_or("user").into()
            },
            content: m["content"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(2000)
                .collect(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
    }
    messages.push(CopilotWireMessage {
        role: "user".into(),
        content: question.clone(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });
    let tools = ai_tools::definitions();

    let mut executed: Vec<ExecutedToolCall> = Vec::new();
    let started = std::time::Instant::now();
    let mut rounds: u32 = 0;
    let mut answer: Option<String> = None;
    let mut model = String::from("unknown");
    // Clean-audit item 43: the copilot run used to be completed without
    // token usage or endpoint — every copilot chat was invisible to the
    // cost ledger (`usage_ledger` prices `ai_runs.token_usage` by
    // `ai_runs.endpoint_id`). Usage is accumulated across every chat
    // round (tool rounds + the final answer) and the run row now
    // carries both.
    let mut usage_total: Option<crate::ai_provider::TokenUsage> = None;

    let (client, backend_model, endpoint_id) = match backend {
        AiBackend::LmStudio {
            client,
            model,
            endpoint_id,
        } => (Some(client), model.clone(), endpoint_id.as_deref()),
        AiBackend::Disabled => (None, None, None),
    };
    let Some(client) = client else {
        return Err(LmStudioError::new(
            "AI is disabled in Settings. Enable LM Studio in Settings > LM Studio to use AI features.",
            false,
        ));
    };

    while rounds < MAX_TOOL_ROUNDS {
        rounds += 1;
        let res = client
            .chat_with_tools(backend_model.as_deref(), &messages, &tools, 0.2, 1600)
            .await
            .map_err(|e| LmStudioError::new(e.to_string(), true))?;
        model = res.model;
        if let Some(u) = res.usage {
            match usage_total.as_mut() {
                Some(t) => {
                    t.prompt_tokens += u.prompt_tokens;
                    t.completion_tokens += u.completion_tokens;
                    t.total_tokens += u.total_tokens;
                }
                None => usage_total = Some(u),
            }
        }
        if !res.tool_calls.is_empty() {
            messages.push(CopilotWireMessage {
                role: "assistant".into(),
                content: res.content.clone().unwrap_or_default(),
                tool_calls: Some(
                    res.tool_calls
                        .iter()
                        .map(|tc| CopilotWireToolCall {
                            id: tc.id.clone(),
                            r#type: "function",
                            function: CopilotWireToolFunction {
                                name: tc.name.clone(),
                                arguments: tc.arguments.clone(),
                            },
                        })
                        .collect(),
                ),
                tool_call_id: None,
                name: None,
            });
            for tc in res.tool_calls.iter() {
                if executed.len() >= MAX_TOOL_CALLS as usize {
                    // Clean-audit item 44: every assistant tool_call must
                    // carry a matching tool response — strict
                    // OpenAI-compatible servers reject the follow-up
                    // request with a 400 when a tool_call id dangles.
                    // The un-executed calls now get an explicit
                    // budget-refusal result instead of silence.
                    messages.push(CopilotWireMessage {
                        role: "tool".into(),
                        content: "Tool budget reached - this call was not executed. Answer from the evidence you already have.".into(),
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                        name: Some(tc.name.clone()),
                    });
                    continue;
                }
                let result = ai_tools::execute(conn, index, &tc.name, &tc.arguments);
                executed.push(ExecutedToolCall {
                    name: tc.name.clone(),
                    args: tc.arguments.clone(),
                    result: result.clone(),
                });
                let bounded = bound_result(&result);
                let content: String = bounded
                    .to_string()
                    .chars()
                    .take(TOOL_RESULT_MAX_CHARS)
                    .collect();
                messages.push(CopilotWireMessage {
                    role: "tool".into(),
                    content,
                    tool_calls: None,
                    tool_call_id: Some(tc.id.clone()),
                    name: Some(tc.name.clone()),
                });
            }
            if executed.len() >= MAX_TOOL_CALLS as usize {
                messages.push(CopilotWireMessage {
                    role: "user".into(),
                    content: "Tool budget reached. Answer now from the evidence you already have, and say plainly which parts you could not verify.".into(),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
                let final_res = client
                    .chat_with_tools(backend_model.as_deref(), &messages, &[], 0.2, 1600)
                    .await
                    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
                model = final_res.model;
                if let Some(u) = final_res.usage {
                    match usage_total.as_mut() {
                        Some(t) => {
                            t.prompt_tokens += u.prompt_tokens;
                            t.completion_tokens += u.completion_tokens;
                            t.total_tokens += u.total_tokens;
                        }
                        None => usage_total = Some(u),
                    }
                }
                answer = Some(final_res.content.unwrap_or_default().trim().to_string());
                break;
            }
            continue;
        }
        answer = Some(res.content.unwrap_or_default().trim().to_string());
        break;
    }
    let answer = match answer {
        Some(a) if !a.is_empty() => a,
        _ => {
            if rounds >= MAX_TOOL_ROUNDS {
                "I reached the tool-call limit before producing an answer. Please rephrase the question more specifically.".to_string()
            } else {
                "The local model returned an empty answer. Try rephrasing or check the LM Studio connection.".to_string()
            }
        }
    };

    let citations = citations_from(conn, &executed);
    let latency_ms = started.elapsed().as_millis() as i64;
    let sid = session["id"].as_i64().unwrap_or_default();

    if !executed.is_empty() {
        let trace: Vec<Value> = executed
            .iter()
            .map(
                |e| json!({ "tool": e.name, "args": e.args.chars().take(300).collect::<String>() }),
            )
            .collect();
        let trace_str: String = serde_json::to_string(&trace).unwrap_or_default();
        insert_message(
            conn,
            sid,
            "tool",
            &trace_str,
            "[]",
            Some(
                &executed
                    .iter()
                    .map(|e| e.name.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            executed.len() as i64,
            None,
        );
    }
    let assistant_message = insert_message(
        conn,
        sid,
        "assistant",
        &answer,
        &serde_json::to_string(&citations).unwrap_or_else(|_| "[]".into()),
        None,
        executed.len() as i64,
        Some(latency_ms),
    );
    let _ = conn.execute(
        "UPDATE copilot_sessions SET updated_at = datetime('now') WHERE id = ?1",
        params![sid],
    );

    let run_id = ai_pipeline::start_run(
        conn,
        "copilot_chat",
        session_conv_id,
        Some(&model),
        PROMPT_VERSIONS_COPILOT_CHAT,
        None,
        &serde_json::json!([question.chars().take(500).collect::<String>()]),
    );
    if let Ok(run_id) = run_id {
        let _ = ai_pipeline::complete_run_with_usage(
            conn,
            run_id,
            &json!({
                "question": question.chars().take(500).collect::<String>(),
                "tool_calls": executed.len(),
                "tool_rounds": rounds,
                "model": model,
            }),
            latency_ms as u64,
            usage_total.as_ref(),
            endpoint_id,
        );
    }
    let _ = crate::audit::audit(
        conn,
        &crate::audit::AuditEntry {
            actor: "user",
            action: "copilot_chat".into(),
            conversation_id: session_conv_id,
            before_state: None,
            after_state: None,
            remote_operation: None,
            remote_result: None,
            ai_involvement: true,
            job_id: None,
            correlation_id: None,
        },
    );

    let fresh = get_session(conn, sid).unwrap_or(session);
    Ok(json!({
        "session": fresh,
        "user_message": user_message,
        "assistant_message": assistant_message,
        "tool_rounds": rounds,
        "citations": citations,
        "model": model,
        "latency_ms": latency_ms,
    }))
}

fn citations_from(conn: &Connection, executed: &[ExecutedToolCall]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for e in executed {
        let label = citation_label(&e.name, &e.result);
        let reference = conversation_ref(conn, &e.result);
        out.push(json!({
            "index": out.len() + 1,
            "tool": e.name,
            "label": label,
            "conversation_id": reference.as_ref().and_then(|r| r.0),
            "conversation_number": reference.as_ref().and_then(|r| r.1),
            "customer_id": Value::Null,
        }));
    }
    out
}

fn citation_label(tool: &str, result: &Value) -> String {
    if let Some(error) = result.get("error").and_then(Value::as_str) {
        return format!("{tool} (no result: {error})");
    }
    if let Some(number) = result.get("number").and_then(Value::as_i64) {
        return format!("{tool} — conversation #{number}");
    }
    if result.get("analysis").is_some_and(|a| a.is_object()) {
        return format!("{tool} — AI analysis");
    }
    if let Some(arr) = result.as_array() {
        let first = arr.first();
        if first.is_some_and(|f| f.get("number").is_some_and(Value::is_i64)) {
            return format!("{tool} — {} conversation(s)", arr.len());
        }
        if first.is_some_and(|f| f.get("title").is_some()) {
            return format!("{tool} — {} match(es)", arr.len());
        }
        return format!("{tool} — {} result(s)", arr.len());
    }
    if let Some(total) = result.get("total_previous_tickets").and_then(Value::as_i64) {
        return format!("{tool} — {total} previous ticket(s)");
    }
    if result.get("attributes").and_then(Value::as_array).is_some() {
        return format!("{tool} — AI attributes");
    }
    tool.to_string()
}

fn conversation_ref(conn: &Connection, result: &Value) -> Option<(Option<i64>, Option<i64>)> {
    let id_for_number = |number: i64| -> Option<i64> {
        conn.query_row(
            "SELECT id FROM conversations WHERE number = ?1",
            params![number],
            |r| r.get(0),
        )
        .ok()
    };
    if let Some(cid) = result.get("conversation_id").and_then(Value::as_i64) {
        return Some((Some(cid), result.get("number").and_then(Value::as_i64)));
    }
    if let (Some(id), Some(number)) = (
        result.get("id").and_then(Value::as_i64),
        result.get("number").and_then(Value::as_i64),
    ) {
        return Some((Some(id), Some(number)));
    }
    if let Some(arr) = result.as_array()
        && let Some(first) = arr.first()
    {
        if let Some(cid) = first.get("conversation_id").and_then(Value::as_i64) {
            return Some((Some(cid), first.get("number").and_then(Value::as_i64)));
        }
        if let (Some(id), Some(number)) = (
            first.get("id").and_then(Value::as_i64),
            first.get("number").and_then(Value::as_i64),
        ) {
            return Some((Some(id), Some(number)));
        }
        if let Some(number) = first.get("number").and_then(Value::as_i64) {
            return Some((id_for_number(number), Some(number)));
        }
    }
    if let Some(number) = result.get("number").and_then(Value::as_i64) {
        return Some((id_for_number(number), Some(number)));
    }
    None
}

fn bound_result(result: &Value) -> Value {
    let s = serde_json::to_string(result).unwrap_or_default();
    if s.len() <= TOOL_RESULT_MAX_CHARS {
        return result.clone();
    }
    json!({
        "truncated": true,
        "preview": s.chars().take(TOOL_RESULT_MAX_CHARS).collect::<String>(),
    })
}

fn context_block(conn: &Connection, conversation_id: Option<i64>) -> String {
    let mut subject: Option<String> = None;
    let mut customer_name: Option<String> = None;
    let mut number: Option<i64> = None;
    if let Some(cid) = conversation_id {
        let row = conn
            .query_row(
                "SELECT c.number, c.subject,
                        (SELECT TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, ''))
                           FROM customers cu WHERE cu.id = c.customer_local_id)
                   FROM conversations c WHERE c.id = ?1 AND c.deleted_at IS NULL",
                params![cid],
                |r| {
                    Ok((
                        r.get::<_, Option<i64>>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .ok();
        if let Some((n, s, c)) = row {
            number = n;
            subject = s;
            customer_name = c;
        }
    }
    let today = crate::activity::now_iso();
    build_copilot_context_block(number, subject.as_deref(), customer_name.as_deref(), &today)
}

pub fn get_session(conn: &Connection, id: i64) -> Option<Value> {
    conn.query_row(
        "SELECT s.id, s.title, s.conversation_id, s.created_at, s.updated_at, c.number,
                (SELECT COUNT(*) FROM copilot_messages m WHERE m.session_id = s.id)
           FROM copilot_sessions s LEFT JOIN conversations c ON c.id = s.conversation_id
          WHERE s.id = ?1",
        params![id],
        |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "title": r.get::<_, String>(1)?,
                "conversation_id": r.get::<_, Option<i64>>(2)?,
                "conversation_number": r.get::<_, Option<i64>>(5)?,
                "message_count": r.get::<_, i64>(6)?,
                "created_at": r.get::<_, String>(3)?,
                "updated_at": r.get::<_, String>(4)?,
            }))
        },
    )
    .ok()
}

pub fn list_sessions(conn: &Connection, limit: i64) -> Vec<Value> {
    conn.prepare(
        "SELECT s.id, s.title, s.conversation_id, s.created_at, s.updated_at, c.number,
                (SELECT COUNT(*) FROM copilot_messages m WHERE m.session_id = s.id)
           FROM copilot_sessions s LEFT JOIN conversations c ON c.id = s.conversation_id
          ORDER BY s.updated_at DESC LIMIT ?1",
    )
    .and_then(|mut stmt| {
        stmt.query_map(params![limit.clamp(1, 200)], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "title": r.get::<_, String>(1)?,
                "conversation_id": r.get::<_, Option<i64>>(2)?,
                "conversation_number": r.get::<_, Option<i64>>(5)?,
                "message_count": r.get::<_, i64>(6)?,
                "created_at": r.get::<_, String>(3)?,
                "updated_at": r.get::<_, String>(4)?,
            }))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
    })
    .unwrap_or_default()
}

pub fn list_messages(conn: &Connection, session_id: i64) -> Vec<Value> {
    conn.prepare(
        "SELECT id, session_id, role, content, citations, tool_name, tool_calls, latency_ms, created_at
           FROM copilot_messages WHERE session_id = ?1 ORDER BY id ASC",
    )
    .and_then(|mut stmt| {
        stmt.query_map(params![session_id], |r| {
            let citations: Option<String> = r.get(4)?;
            let citations: Vec<Value> = citations
                .and_then(|c| serde_json::from_str::<Value>(&c).ok())
                .and_then(|c| c.as_array().cloned())
                .unwrap_or_default();
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "session_id": r.get::<_, i64>(1)?,
                "role": r.get::<_, String>(2)?,
                "content": r.get::<_, String>(3)?,
                "citations": citations.iter().take(20).cloned().collect::<Vec<_>>(),
                "tool_name": r.get::<_, Option<String>>(5)?,
                "tool_calls": r.get::<_, i64>(6)?,
                "latency_ms": r.get::<_, Option<i64>>(7)?,
                "created_at": r.get::<_, String>(8)?,
            }))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
    })
    .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn insert_message(
    conn: &Connection,
    session_id: i64,
    role: &str,
    content: &str,
    citations: &str,
    tool_name: Option<&str>,
    tool_calls: i64,
    latency_ms: Option<i64>,
) -> Value {
    // Clean-audit item 45: the INSERT failure used to be swallowed and
    // `last_insert_rowid()` echoed the id of whatever row was last
    // inserted on this connection — the response presented a stale id
    // pointing at a different message. A failed insert now reports a
    // null id and the error text instead of a fabricated one.
    let inserted = conn.execute(
        "INSERT INTO copilot_messages (session_id, role, content, citations, tool_name, tool_calls, latency_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![session_id, role, content, citations, tool_name, tool_calls, latency_ms],
    );
    let (id, persist_error) = match inserted {
        Ok(1) => (json!(conn.last_insert_rowid()), Value::Null),
        Ok(_) => (
            Value::Null,
            json!("insert affected an unexpected number of rows"),
        ),
        Err(e) => {
            tracing::warn!(
                operation = "copilot_insert_message",
                role,
                session_id,
                error = %e,
                "copilot message insert failed - reporting a null id"
            );
            (Value::Null, json!(e.to_string()))
        }
    };
    json!({
        "id": id,
        "session_id": session_id,
        "role": role,
        "content": content,
        "citations": serde_json::from_str::<Value>(citations).ok()
            .and_then(|c| c.as_array().cloned())
            .unwrap_or_default(),
        "tool_name": tool_name,
        "tool_calls": tool_calls,
        "latency_ms": latency_ms,
        "persist_error": persist_error,
        "created_at": crate::activity::now_iso(),
    })
}

pub fn delete_session(conn: &Connection, id: i64) -> bool {
    conn.execute("DELETE FROM copilot_sessions WHERE id = ?1", params![id])
        .map(|n| n > 0)
        .unwrap_or(false)
}

pub fn starter_questions(conn: &Connection, conversation_id: i64) -> Vec<(String, String)> {
    let facts: Option<i64> = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM conversations c2
                      WHERE c2.customer_local_id = c.customer_local_id
                        AND c2.deleted_at IS NULL AND c2.id != c.id)
               FROM conversations c WHERE c.id = ?1 AND c.deleted_at IS NULL",
            params![conversation_id],
            |r| r.get(0),
        )
        .ok();
    let Some(prior_tickets) = facts else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = vec![
        (
            "What is this customer asking?".into(),
            "Summarizes the current conversation".into(),
        ),
        (
            "What should I check before replying?".into(),
            "Pre-reply checklist from local evidence".into(),
        ),
        (
            "What information has already been provided?".into(),
            "Avoids asking the customer twice".into(),
        ),
        (
            "Why is this ticket currently considered urgent?".into(),
            "Explains urgency from AI attributes + analysis".into(),
        ),
    ];
    if prior_tickets > 0 {
        out.push((
            "What happened in their previous tickets?".into(),
            format!("{prior_tickets} previous ticket(s) in the local archive"),
        ));
        out.push((
            "What changed since the last interaction?".into(),
            "Compares with the last conversation".into(),
        ));
        out.push((
            "Summarize the last three conversations.".into(),
            "Recent history digest".into(),
        ));
    }
    out.push((
        "Have we seen this issue before?".into(),
        "Searches similar conversations".into(),
    ));
    out.push((
        "What solved the previous cases?".into(),
        "Resolutions from similar tickets".into(),
    ));
    out.push((
        "Has another customer had the same problem?".into(),
        "Cross-customer search".into(),
    ));
    out.push((
        "What documentation applies?".into(),
        "Local knowledge base search".into(),
    ));
    out.push((
        "Show evidence for that answer.".into(),
        "Citations from real tool results".into(),
    ));
    out
}

#[cfg(test)]
mod batch23_audit_tests {
    use super::*;
    use serde_json::json;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn backend_for(mock: &crate::mockserver::MockServer) -> AiBackend {
        AiBackend::LmStudio {
            client: crate::ai_lm_studio::OpenAiCompatibleClient::new_with_timeout(
                mock.url(),
                5_000,
            ),
            model: Some("test-model".into()),
            endpoint_id: Some("lm-hub-1".into()),
        }
    }

    fn chat_body(
        content: Option<&str>,
        tool_calls: serde_json::Value,
        usage: (u64, u64, u64),
    ) -> crate::mockserver::MockResponse {
        crate::mockserver::MockResponse::json(
            200,
            json!({
                "model": "test-model",
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": content,
                        "tool_calls": tool_calls
                    },
                    "finish_reason": if tool_calls.is_null() { "stop" } else { "tool_calls" }
                }],
                "usage": {"prompt_tokens": usage.0, "completion_tokens": usage.1, "total_tokens": usage.2}
            })
            .to_string(),
        )
    }

    fn tool_call(id: &str, name: &str) -> serde_json::Value {
        json!({
            "id": id,
            "type": "function",
            "function": {"name": name, "arguments": "{}"}
        })
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    /// Item 45: a failed INSERT reports a null id and the error — the
    /// pre-fix code echoed `last_insert_rowid()`, presenting the id of
    /// whatever row was last inserted as the new message.
    #[test]
    fn insert_message_reports_null_id_on_failure() {
        let conn = db();
        ensure_copilot_schema(&conn).expect("schema");
        conn.execute(
            "INSERT INTO copilot_sessions (title, conversation_id) VALUES ('t', NULL)",
            [],
        )
        .expect("session");
        let sid = conn.last_insert_rowid();

        // Happy path: the id is the row just inserted.
        let ok = insert_message(&conn, sid, "user", "hello", "[]", None, 0, None);
        assert_eq!(ok["id"].as_i64(), Some(conn.last_insert_rowid()));
        assert!(ok["persist_error"].is_null(), "{ok}");

        // Failure path: the table is gone, the INSERT fails — the id must
        // be null, never a stale rowid.
        conn.execute("DROP TABLE copilot_messages", [])
            .expect("drop");
        let failed = insert_message(&conn, sid, "user", "hello", "[]", None, 0, None);
        assert!(
            failed["id"].is_null(),
            "no fabricated id on failure: {failed}"
        );
        assert!(
            failed["persist_error"]
                .as_str()
                .is_some_and(|e| !e.is_empty()),
            "the failure is reported: {failed}"
        );
    }

    /// Item 43: the copilot run lands in the cost ledger with its token
    /// usage and endpoint — usage accumulated across the tool round AND
    /// the final answer round.
    #[test]
    fn copilot_chat_records_usage_and_endpoint_in_the_ledger() {
        let conn = db();
        // Round 1: one tool call (usage 10/5/15). Round 2: the answer
        // (usage 7/3/10) — the run must total 17/8/25.
        let mock = crate::mockserver::MockServer::start(crate::mockserver::scripted(vec![
            chat_body(
                None,
                json!([tool_call("call_1", "search_conversations")]),
                (10, 5, 15),
            ),
            chat_body(
                Some("The invite queue was cleared."),
                serde_json::Value::Null,
                (7, 3, 10),
            ),
        ]))
        .expect("mock");
        let backend = backend_for(&mock);

        let out = runtime()
            .block_on(chat(
                &conn,
                &backend,
                None,
                "why did invites bounce?",
                None,
                None,
            ))
            .expect("chat");

        assert_eq!(out["model"], "test-model");

        let (status, usage, endpoint, run_type): (String, Option<String>, Option<String>, String) =
            conn.query_row(
                "SELECT status, token_usage, endpoint_id, type FROM ai_runs
                  WHERE type = 'copilot_chat' ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("the copilot run row exists");
        assert_eq!(run_type, "copilot_chat");
        assert_eq!(status, "completed");
        assert_eq!(endpoint.as_deref(), Some("lm-hub-1"), "endpoint attributed");
        let usage = usage.expect("usage recorded");
        assert!(
            usage.contains("\"prompt_tokens\":17")
                && usage.contains("\"completion_tokens\":8")
                && usage.contains("\"total_tokens\":25"),
            "usage accumulated across both rounds: {usage}"
        );

        // The ledger sees the row with its tokens.
        let ledger = crate::ai_hub::usage_ledger(&conn, 7).expect("ledger");
        let row = ledger
            .iter()
            .find(|r| r.feature == "copilot_chat")
            .expect("copilot chats are ledger-visible now");
        assert_eq!(row.prompt_tokens, 17);
        assert_eq!(row.completion_tokens, 8);
        assert_eq!(row.runs, 1);
    }

    /// Item 44: when the tool budget cuts a round short, every assistant
    /// tool_call id still carries a matching tool response — strict
    /// OpenAI-compatible servers 400 the follow-up request otherwise.
    #[test]
    fn tool_budget_refusal_answers_every_dangling_tool_call_id() {
        let conn = db();
        // Round 1 asks for 10 tool calls at once; the budget is
        // MAX_TOOL_CALLS (8), so 8 run and 2 must be refused explicitly.
        let calls: Vec<serde_json::Value> = (1..=10)
            .map(|i| tool_call(&format!("call_{i}"), "search_conversations"))
            .collect();
        let mock = crate::mockserver::MockServer::start(crate::mockserver::scripted(vec![
            chat_body(None, serde_json::Value::Array(calls), (10, 5, 15)),
            chat_body(
                Some("Answer from the evidence gathered."),
                serde_json::Value::Null,
                (4, 2, 6),
            ),
        ]))
        .expect("mock");
        let backend = backend_for(&mock);

        let out = runtime()
            .block_on(chat(
                &conn,
                &backend,
                None,
                "look up everything",
                None,
                None,
            ))
            .expect("chat");
        assert_eq!(
            out["assistant_message"]["tool_calls"].as_i64(),
            Some(8),
            "the budget held at 8 executed calls"
        );

        // The follow-up request (mock hit #2) must pair every assistant
        // tool_call id with a tool-role response.
        let body: serde_json::Value =
            serde_json::from_str(&mock.hits()[1].body).expect("follow-up request json");
        let messages = body["messages"].as_array().expect("messages");
        let assistant_ids: Vec<String> = messages
            .iter()
            .filter(|m| m["role"] == "assistant" && !m["tool_calls"].is_null())
            .flat_map(|m| {
                m["tool_calls"]
                    .as_array()
                    .expect("tool_calls array")
                    .iter()
                    .filter_map(|tc| tc["id"].as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(assistant_ids.len(), 10, "the assistant asked for 10 calls");
        let answered_ids: Vec<String> = messages
            .iter()
            .filter(|m| m["role"] == "tool")
            .filter_map(|m| m["tool_call_id"].as_str().map(String::from))
            .collect();
        for id in &assistant_ids {
            assert!(
                answered_ids.contains(id),
                "tool_call {id} must carry a tool response (no dangling ids)"
            );
        }
        // The two refused calls say so in plain words.
        let refusals: Vec<&str> = messages
            .iter()
            .filter(|m| m["role"] == "tool")
            .filter_map(|m| m["content"].as_str())
            .filter(|c| c.contains("Tool budget reached"))
            .collect();
        assert_eq!(
            refusals.len(),
            2,
            "the 2 over-budget calls are refused explicitly"
        );
    }
}

#[cfg(test)]
mod batch31_tests {
    use super::*;

    /// Audit item 121: the truncation cap sliced at a raw BYTE offset —
    /// an é landing across the cap's byte index panicked. The cap counts
    /// characters now.
    #[test]
    fn truncate_result_is_char_safe_at_the_cap() {
        let short = "abc";
        let (out, truncated) = truncate_result(short);
        assert_eq!(out, "abc");
        assert!(!truncated);

        // 3999 ASCII bytes, then a 2-byte é across byte 4000, then more.
        let s = format!("{}é{}", "a".repeat(3999), "b".repeat(20));
        let (out, truncated) = truncate_result(&s);
        assert!(truncated);
        assert_eq!(out.chars().count(), 4000 + "… [truncated]".chars().count());
        assert!(out.starts_with(&"a".repeat(3999)));
        assert!(out.contains('é'), "the multi-byte character survives whole");
    }
}
