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
    if output.len() <= TOOL_RESULT_MAX_CHARS {
        (output.to_string(), false)
    } else {
        let truncated = &output[..TOOL_RESULT_MAX_CHARS];
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

    let (client, backend_model) = match backend {
        AiBackend::LmStudio { client, model, .. } => (Some(client), model.clone()),
        AiBackend::Disabled => (None, None),
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
                    break;
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
        let _ = ai_pipeline::complete_run(
            conn,
            run_id,
            &json!({
                "question": question.chars().take(500).collect::<String>(),
                "tool_calls": executed.len(),
                "tool_rounds": rounds,
                "model": model,
            }),
            latency_ms as u64,
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
    let _ = conn.execute(
        "INSERT INTO copilot_messages (session_id, role, content, citations, tool_name, tool_calls, latency_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![session_id, role, content, citations, tool_name, tool_calls, latency_ms],
    );
    json!({
        "id": conn.last_insert_rowid(),
        "session_id": session_id,
        "role": role,
        "content": content,
        "citations": serde_json::from_str::<Value>(citations).ok()
            .and_then(|c| c.as_array().cloned())
            .unwrap_or_default(),
        "tool_name": tool_name,
        "tool_calls": tool_calls,
        "latency_ms": latency_ms,
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
