use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::ai_lm_studio::ChatTool;
use crate::catalog::GraphNodeKind;
use crate::error::Result;

const FRICTION_FINDINGS_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS friction_findings (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
        customer_local_id INTEGER REFERENCES customers(id) ON DELETE SET NULL,
        kind TEXT NOT NULL,
        severity TEXT NOT NULL DEFAULT 'low',
        evidence TEXT NOT NULL DEFAULT '[]',
        detail TEXT,
        computed_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE UNIQUE INDEX IF NOT EXISTS uq_friction_findings_conv_kind
        ON friction_findings(conversation_id, kind);
    CREATE INDEX IF NOT EXISTS idx_friction_findings_kind_severity
        ON friction_findings(kind, severity);
    CREATE INDEX IF NOT EXISTS idx_friction_findings_customer_ref
        ON friction_findings(customer_local_id);
"#;

pub fn ensure_tool_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(FRICTION_FINDINGS_SQL)?;
    Ok(())
}

fn tool(name: &str, description: &str, parameters: Value) -> ChatTool {
    ChatTool {
        name: name.to_string(),
        description: description.to_string(),
        parameters,
    }
}

type HistoryTicket = (
    i64,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
);
type GapRow = (i64, Option<String>, String, i64, String, Option<String>);
type MemoryRow = (
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
);

fn prop(name: &str, kind: &str, description: &str) -> Value {
    json!({ name: { "type": kind, "description": description } })
}

pub fn definitions() -> Vec<ChatTool> {
    vec![
        tool(
            "search_conversations",
            "Search the local conversation archive by keyword. Returns matching tickets with subject and preview.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Keywords to search for" },
                    "limit": { "type": "number", "description": "Max results (default 5, max 10)" }
                },
                "required": ["query"]
            }),
        ),
        tool(
            "search_knowledge",
            "Search local knowledge documents.",
            json!({
                "type": "object",
                "properties": prop("query", "string", ""),
                "required": ["query"]
            }),
        ),
        tool(
            "search_known_issues",
            "Search known issues by keywords.",
            json!({
                "type": "object",
                "properties": prop("query", "string", ""),
                "required": ["query"]
            }),
        ),
        tool(
            "search_saved_replies",
            "Search saved replies by keywords.",
            json!({
                "type": "object",
                "properties": prop("query", "string", ""),
                "required": ["query"]
            }),
        ),
        tool(
            "get_conversation",
            "Get a conversation summary by conversation number.",
            json!({
                "type": "object",
                "properties": prop("number", "number", ""),
                "required": ["number"]
            }),
        ),
        tool(
            "get_support_metrics",
            "Get local support volume metrics for the last N days.",
            json!({
                "type": "object",
                "properties": prop("days", "number", "Look-back window in days (default 30)")
            }),
        ),
        tool(
            "get_conversation_context",
            "Get the full context of one conversation by number: subject, status, mailbox, tags, participants, the thread messages (customer + agent, bounded), and derived activity facts (response state, ages). Use this for \"what is this customer asking\" / \"what already provided\" style questions.",
            json!({
                "type": "object",
                "properties": prop("number", "number", "Conversation number"),
                "required": ["number"]
            }),
        ),
        tool(
            "get_customer_history",
            "Get a customer's previous tickets, resolved from a conversation number of theirs. Returns past conversations with status, dates, subjects and resolution notes when available. Use for \"what happened in their previous tickets\" / \"what changed since the last interaction\".",
            json!({
                "type": "object",
                "properties": {
                    "number": { "type": "number", "description": "Any conversation number belonging to the customer" },
                    "limit": { "type": "number", "description": "Max tickets (default 5, max 10)" }
                },
                "required": ["number"]
            }),
        ),
        tool(
            "get_similar_conversations",
            "Get conversations similar to a given conversation (relevance-ranked from local evidence). Use for \"have we seen this issue before\" / \"what solved previous cases\".",
            json!({
                "type": "object",
                "properties": {
                    "number": { "type": "number", "description": "Conversation number" },
                    "limit": { "type": "number", "description": "Max results (default 5, max 10)" }
                },
                "required": ["number"]
            }),
        ),
        tool(
            "get_issue_clusters",
            "List local issue clusters (grouped recurring issues) with sizes and member conversation numbers, optionally filtered by keyword.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Optional keyword filter on cluster title/summary" },
                    "limit": { "type": "number", "description": "Max clusters (default 5, max 10)" }
                }
            }),
        ),
        tool(
            "get_ai_analysis",
            "Get the latest local AI analysis for a conversation: intent, questions, urgency, sentiment, missing information, confidence, plus the latest draft verification outcome. Use for \"why is this urgent\" style questions.",
            json!({
                "type": "object",
                "properties": prop("number", "number", "Conversation number"),
                "required": ["number"]
            }),
        ),
        tool(
            "get_supportos_metadata",
            "Get SupportOS-local ticket metadata by conversation number: priority, custom ticket state, response state, waiting/first-response ages, assignee, mailbox, SLA status. These are locally derived fields, not Help Scout fields.",
            json!({
                "type": "object",
                "properties": prop("number", "number", "Conversation number"),
                "required": ["number"]
            }),
        ),
        tool(
            "get_ai_attributes",
            "Get the current local AI attribute snapshot for a conversation: intent, urgency, frustration cues, technical familiarity, question count, risk, escalation signal, known issue link etc., each with confidence and evidence excerpts. Attributes without values are listed as unknown.",
            json!({
                "type": "object",
                "properties": prop("number", "number", "Conversation number"),
                "required": ["number"]
            }),
        ),
        tool(
            "search_incidents",
            "List local incidents (master issues) with status, severity and affected counts, optionally filtered by keyword. Use for \"is there an ongoing incident\" / \"why are customers writing in about X\" questions.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Optional keyword filter on code/title" },
                    "limit": { "type": "number", "description": "Max incidents (default 5, max 10)" }
                }
            }),
        ),
        tool(
            "search_custom_objects",
            "Search local custom objects (user-defined records like accounts, subscriptions, deployments) by keyword, optionally narrowed to a type slug (e.g. \"account\"). Returns titles, types and property values. These are locally defined records, not Help Scout data.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Keywords to search for" },
                    "type": { "type": "string", "description": "Optional type slug (e.g. account, deployment)" },
                    "limit": { "type": "number", "description": "Max results (default 5, max 10)" }
                }
            }),
        ),
        tool(
            "get_customer_timeline",
            "Get the recent local event timeline for the customer of a conversation: signups, support conversations, first messages, campaign sends/replies, ratings, incident exposure and custom object events. Use for \"what happened with this customer over time\".",
            json!({
                "type": "object",
                "properties": {
                    "number": { "type": "number", "description": "Any conversation number belonging to the customer" },
                    "limit": { "type": "number", "description": "Max events (default 10, max 20)" }
                }
            }),
        ),
        tool(
            "search_connector_data",
            "Search rows from an APPROVED local data connector. Only connectors explicitly marked AI-visible are searchable - data that is not explicitly allowed stays private. Use for account/deployment/release data the operator has connected and approved.",
            json!({
                "type": "object",
                "properties": {
                    "connector": { "type": "string", "description": "Connector name" },
                    "query": { "type": "string", "description": "Keyword filter on row contents" },
                    "limit": { "type": "number", "description": "Max rows (default 5, max 10)" }
                },
                "required": ["connector"]
            }),
        ),
        tool(
            "get_knowledge_gaps",
            "List local knowledge-gap candidates: repeated questions without coverage, questions existing docs did not solve, conflicting documents, missing troubleshooting steps, undocumented issues. Candidates await human approval - nothing auto-publishes.",
            json!({
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "description": "Optional gap kind filter (repeated_question_uncovered, repeated_question_unsolved, conflicting_knowledge, missing_troubleshooting_steps, new_issue_undocumented)" },
                    "limit": { "type": "number", "description": "Max candidates (default 5, max 10)" }
                }
            }),
        ),
        tool(
            "get_friction_report",
            "Local conversation-friction summary: how often customers repeat explanations, agents re-ask questions, troubleshooting loops, repeated handoffs, duplicated information requests occur. Deterministic heuristics over the local mirror - patterns, not judgments about people.",
            json!({
                "type": "object",
                "properties": prop("days", "number", "Lookback window in days (default 30, max 365)")
            }),
        ),
        tool(
            "get_graph_neighbors",
            "Explore the local support graph around one node: which customers, organizations, conversations, issues, incidents, knowledge, agents, campaigns, products, custom objects or connector rows are connected to it, with the relation and provenance of every edge. Derived edges are computed live from the local mirror.",
            json!({
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "description": "Node kind (customer, organization, conversation, known_issue, issue_cluster, incident, knowledge_document, agent, campaign, product, custom_object, connector_data)" },
                    "local_id": { "type": "number", "description": "Node local id" },
                    "limit": { "type": "number", "description": "Max edges (default 5, max 10)" }
                },
                "required": ["kind", "local_id"]
            }),
        ),
        tool(
            "get_graph_stats",
            "Support graph overview: live node counts per kind and derived edge counts per relation. Use for \"how connected is our support data\" style questions.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "get_customer_memory",
            "Get the composed support memory for the customer of a conversation: known issue history, previous resolutions, communication preferences, recurring patterns, campaign history and human-written notes. Composed live from the local mirror; psychological/personality judgments are never included.",
            json!({
                "type": "object",
                "properties": prop("number", "number", "Any conversation number belonging to the customer"),
                "required": ["number"]
            }),
        ),
    ]
}

fn clamp_limit(args: &serde_json::Map<String, Value>) -> i64 {
    let n = args.get("limit").and_then(Value::as_i64).unwrap_or(5);
    n.clamp(1, 10)
}

fn red(conn: &Connection, text: &str) -> String {
    let enabled = crate::settings::get_bool(conn, "redaction_enabled", true).unwrap_or(true);
    crate::security::redact_text(text, enabled).0
}

fn red_chars(conn: &Connection, text: &str, n: usize) -> String {
    let taken: String = text.chars().take(n).collect();
    red(conn, &taken)
}

fn conv_by_number(conn: &Connection, number: i64) -> Option<(i64, Option<i64>)> {
    conn.query_row(
        "SELECT id, customer_local_id FROM conversations WHERE number = ?1 AND deleted_at IS NULL",
        params![number],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .ok()
}

pub fn execute(conn: &Connection, name: &str, args_json: &str) -> Value {
    let args: serde_json::Map<String, Value> = match serde_json::from_str(if args_json.is_empty() {
        "{}"
    } else {
        args_json
    }) {
        Ok(Value::Object(map)) => map,
        _ => return json!({ "error": "Invalid tool arguments" }),
    };
    let limit = clamp_limit(&args);
    match name {
        "search_conversations" => {
            let q: String = args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect();
            let hits = crate::search::search_conversations(
                conn,
                &q,
                &crate::search::SearchFilters::default(),
                limit,
            )
            .unwrap_or_default();
            json!(
                hits.iter()
                    .map(|h| {
                        let number: i64 = h
                            .title
                            .split(' ')
                            .next()
                            .map(|w| w.trim_start_matches('#'))
                            .and_then(|w| w.parse().ok())
                            .unwrap_or(h.id);
                        json!({
                            "number": number,
                            "title": h.title,
                            "snippet": red_chars(conn, &h.snippet, 300),
                        })
                    })
                    .collect::<Vec<_>>()
            )
        }
        "search_knowledge" => {
            let q: String = args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect();
            let hits = crate::search::search_knowledge(conn, &q, None, limit).unwrap_or_default();
            json!(
                hits.iter()
                    .map(|h| json!({
                        "title": h.title,
                        "snippet": red_chars(conn, &h.snippet, 300),
                    }))
                    .collect::<Vec<_>>()
            )
        }
        "search_known_issues" => {
            let q: String = args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect();
            let hits = crate::search::search_known_issues(conn, &q, limit).unwrap_or_default();
            json!(
                hits.iter()
                    .map(|h| json!({
                        "id": h.id,
                        "title": h.title,
                        "snippet": red_chars(conn, &h.snippet, 300),
                    }))
                    .collect::<Vec<_>>()
            )
        }
        "search_saved_replies" => {
            let q: String = args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect();
            let hits = crate::search::search_saved_replies(conn, &q, limit).unwrap_or_default();
            json!(
                hits.iter()
                    .map(|h| json!({
                        "id": h.id,
                        "title": h.title,
                        "snippet": red_chars(conn, &h.snippet, 300),
                    }))
                    .collect::<Vec<_>>()
            )
        }
        "get_conversation" => {
            let Some(num) = args.get("number").and_then(Value::as_i64) else {
                return json!({ "error": "Invalid number" });
            };
            let row = conn
                .query_row(
                    "SELECT id, number, subject, preview, status, COALESCE(remote_created_at, created_at)
                       FROM conversations WHERE number = ?1",
                    params![num],
                    |r| {
                        Ok(json!({
                            "id": r.get::<_, i64>(0)?,
                            "number": r.get::<_, i64>(1)?,
                            "subject": r.get::<_, Option<String>>(2)?,
                            "preview": r.get::<_, Option<String>>(3)?,
                            "status": r.get::<_, String>(4)?,
                            "remote_created_at": r.get::<_, Option<String>>(5)?,
                        }))
                    },
                )
                .ok();
            let Some(mut row) = row else {
                return json!({ "error": "Conversation not found" });
            };
            let id = row["id"].as_i64().unwrap_or_default();
            let threads = conn
                .prepare(
                    "SELECT type, COALESCE(body_text, ''), COALESCE(remote_created_at, created_at)
                       FROM conversation_threads
                      WHERE conversation_id = ?1
                      ORDER BY COALESCE(remote_created_at, created_at) DESC LIMIT 3",
                )
                .and_then(|mut stmt| {
                    stmt.query_map(params![id], |r| {
                        Ok(json!({
                            "type": r.get::<_, String>(0)?,
                            "text": red_chars(conn, &r.get::<_, String>(1)?, 400),
                            "date": r.get::<_, Option<String>>(2)?,
                        }))
                    })
                    .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                })
                .unwrap_or_default();
            row["recent_threads"] = json!(threads);
            row
        }
        "get_support_metrics" => {
            let days = args
                .get("days")
                .and_then(Value::as_i64)
                .unwrap_or(30)
                .clamp(1, 365);
            let (conversations, active, closed) = conn
                .query_row(
                    "SELECT COUNT(*),
                            SUM(CASE WHEN status='active' THEN 1 ELSE 0 END),
                            SUM(CASE WHEN status='closed' THEN 1 ELSE 0 END)
                       FROM conversations
                      WHERE deleted_at IS NULL
                        AND julianday(COALESCE(remote_created_at, created_at)) >= julianday('now', '-' || ?1 || ' days')",
                    params![days],
                    |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                            r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                        ))
                    },
                )
                .unwrap_or((0, 0, 0));
            json!({
                "window_days": days,
                "conversations": conversations,
                "active": active,
                "closed": closed,
            })
        }
        "get_conversation_context" => {
            let Some(num) = args.get("number").and_then(Value::as_i64) else {
                return json!({ "error": "Invalid number" });
            };
            let conv = conn
                .query_row(
                    "SELECT c.id, c.number, c.subject, c.status, c.type,
                            COALESCE(c.remote_created_at, c.created_at), c.remote_updated_at,
                            (SELECT m.name FROM mailboxes m WHERE m.id = c.mailbox_local_id) AS mailbox,
                            (SELECT GROUP_CONCAT(t.name) FROM conversation_tags ct
                               JOIN tags t ON t.id = ct.tag_id WHERE ct.conversation_id = c.id) AS tags,
                            (SELECT TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, ''))
                               FROM customers cu WHERE cu.id = c.customer_local_id) AS customer
                       FROM conversations c WHERE c.number = ?1 AND c.deleted_at IS NULL",
                    params![num],
                    |r| {
                        Ok(json!({
                            "id": r.get::<_, i64>(0)?,
                            "number": r.get::<_, i64>(1)?,
                            "subject": r.get::<_, Option<String>>(2)?,
                            "status": r.get::<_, String>(3)?,
                            "type": r.get::<_, Option<String>>(4)?,
                            "remote_created_at": r.get::<_, Option<String>>(5)?,
                            "remote_updated_at": r.get::<_, Option<String>>(6)?,
                            "mailbox": r.get::<_, Option<String>>(7)?,
                            "tags": r.get::<_, Option<String>>(8)?,
                            "customer": r.get::<_, Option<String>>(9)?,
                        }))
                    },
                )
                .ok();
            let Some(conv) = conv else {
                return json!({ "error": "Conversation not found" });
            };
            let id = conv["id"].as_i64().unwrap_or_default();
            let messages: Vec<(String, String, Option<String>, String)> = conn
                .prepare(
                    "SELECT COALESCE(from_name, type, 'unknown'), type,
                            COALESCE(remote_created_at, created_at), COALESCE(body_html, body_text, '')
                       FROM conversation_threads
                      WHERE conversation_id = ?1 AND deleted_at IS NULL AND state = 'published'
                      ORDER BY COALESCE(remote_created_at, created_at) ASC LIMIT 30",
                )
                .and_then(|mut stmt| {
                    stmt.query_map(params![id], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, Option<String>>(2)?,
                            r.get::<_, String>(3)?,
                        ))
                    })
                    .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                })
                .unwrap_or_default();
            let msgs: Vec<Value> = messages
                .iter()
                .map(|(author, kind, date, body)| {
                    let text: String = crate::textutil::html_to_text(body)
                        .chars()
                        .take(700)
                        .collect();
                    json!({
                        "author": author,
                        "type": kind,
                        "date": date,
                        "text": red(conn, &text),
                    })
                })
                .collect();
            let mut participants: Vec<String> = Vec::new();
            for m in &msgs {
                let label = format!(
                    "{} ({})",
                    m["author"].as_str().unwrap_or(""),
                    m["type"].as_str().unwrap_or("")
                );
                if !participants.contains(&label) {
                    participants.push(label);
                }
            }
            participants.truncate(8);
            let mut out = conv;
            out["participants"] = json!(participants);
            out["messages"] = json!(msgs);
            out
        }
        "get_customer_history" => {
            let Some(num) = args.get("number").and_then(Value::as_i64) else {
                return json!({ "error": "Invalid number" });
            };
            let Some(customer) = conn
                .query_row(
                    "SELECT customer_local_id FROM conversations WHERE number = ?1 AND deleted_at IS NULL",
                    params![num],
                    |r| r.get::<_, Option<i64>>(0),
                )
                .ok()
                .flatten()
            else {
                return json!({ "error": "Conversation not found or has no customer" });
            };
            let tickets: Vec<HistoryTicket> = conn
                .prepare(
                    "SELECT c.number, c.subject, c.status, COALESCE(c.remote_created_at, c.created_at), c.closed_at,
                            (SELECT COALESCE(t.body_text, '') FROM conversation_threads t
                              WHERE t.conversation_id = c.id AND t.deleted_at IS NULL AND t.state='published'
                              ORDER BY COALESCE(t.remote_created_at, t.created_at) ASC LIMIT 1),
                            (SELECT COALESCE(t.body_text, '') FROM conversation_threads t
                              WHERE t.conversation_id = c.id AND t.deleted_at IS NULL AND t.state='published'
                                AND t.type='reply'
                              ORDER BY COALESCE(t.remote_created_at, t.created_at) DESC LIMIT 1)
                       FROM conversations c
                      WHERE c.customer_local_id = ?1 AND c.deleted_at IS NULL
                        AND c.id != (SELECT id FROM conversations WHERE number = ?2)
                      ORDER BY COALESCE(c.remote_created_at, c.created_at) DESC LIMIT ?3",
                )
                .and_then(|mut stmt| {
                    stmt.query_map(params![customer, num, limit], |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, Option<String>>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, Option<String>>(3)?,
                            r.get::<_, Option<String>>(4)?,
                            r.get::<_, String>(5)?,
                            r.get::<_, String>(6)?,
                        ))
                    })
                    .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                })
                .unwrap_or_default();
            json!({
                "total_previous_tickets": tickets.len(),
                "tickets": tickets.iter().map(|(number, subject, status, created, closed, first_message, last_reply)| json!({
                    "number": number,
                    "subject": subject,
                    "status": status,
                    "created": created,
                    "closed": closed,
                    "first_message": red_chars(conn, first_message, 300),
                    "last_reply": red_chars(conn, last_reply, 300),
                })).collect::<Vec<_>>(),
            })
        }
        "get_similar_conversations" => {
            let Some(num) = args.get("number").and_then(Value::as_i64) else {
                return json!({ "error": "Invalid number" });
            };
            let Some((id, _)) = conv_by_number(conn, num) else {
                return json!({ "error": "Conversation not found" });
            };
            let similar =
                crate::ai_evidence::find_similar(conn, id, limit.clamp(1, 10) as usize, &[])
                    .unwrap_or_default();
            json!(
                similar
                    .iter()
                    .map(|s| json!({
                        "conversation_id": s.conversation_id,
                        "number": s.number,
                        "subject": s.subject,
                        "resolution": red_chars(conn, &s.resolution, 400),
                    }))
                    .collect::<Vec<_>>()
            )
        }
        "get_issue_clusters" => {
            let q: String = args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect();
            let like = format!(
                "%{}%",
                q.replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            );
            let sql = format!(
                "SELECT ic.id, ic.title, ic.summary, ic.category, ic.product, ic.feature, ic.ai_generated,
                        (SELECT GROUP_CONCAT(c.number) FROM issue_cluster_conversations icc
                           JOIN conversations c ON c.id = icc.conversation_id
                          WHERE icc.cluster_id = ic.id ORDER BY c.number DESC) AS numbers
                   FROM issue_clusters ic
                  {}
                  ORDER BY ic.id DESC LIMIT {}",
                if q.is_empty() {
                    String::new()
                } else {
                    "WHERE ic.title LIKE ?1 ESCAPE '\\' OR ic.summary LIKE ?1 ESCAPE '\\' OR ic.category LIKE ?1 ESCAPE '\\'".to_string()
                },
                limit * if q.is_empty() { 1 } else { 3 },
            );
            let rows: Vec<Value> = if q.is_empty() {
                conn.prepare(&sql)
                    .and_then(|mut stmt| {
                        stmt.query_map([], |r| map_cluster_row(conn, r))
                            .map(collect_rows)
                    })
                    .unwrap_or_default()
            } else {
                conn.prepare(&sql)
                    .and_then(|mut stmt| {
                        stmt.query_map(params![like], |r| map_cluster_row(conn, r))
                            .map(collect_rows)
                    })
                    .unwrap_or_default()
            };
            json!(rows)
        }
        "get_ai_analysis" => {
            let Some(num) = args.get("number").and_then(Value::as_i64) else {
                return json!({ "error": "Invalid number" });
            };
            let Some((id, _)) = conv_by_number(conn, num) else {
                return json!({ "error": "Conversation not found" });
            };
            let run = conn
                .query_row(
                    "SELECT response_json, model, latency_ms, created_at FROM ai_runs
                      WHERE conversation_id = ?1 AND type = 'ticket_analysis' AND status = 'completed'
                      ORDER BY id DESC LIMIT 1",
                    params![id],
                    |r| {
                        Ok((
                            r.get::<_, Option<String>>(0)?,
                            r.get::<_, Option<String>>(1)?,
                            r.get::<_, Option<i64>>(2)?,
                            r.get::<_, Option<String>>(3)?,
                        ))
                    },
                )
                .ok();
            let Some((output, model, latency_ms, created_at)) = run else {
                return json!({
                    "analysis": Value::Null,
                    "note": "No AI analysis has been run for this conversation yet."
                });
            };
            let analysis = output
                .as_deref()
                .and_then(|o| serde_json::from_str::<Value>(o).ok())
                .unwrap_or(Value::Null);
            let verification = conn
                .query_row(
                    "SELECT verification FROM ai_drafts
                      WHERE conversation_id = ?1 AND verification IS NOT NULL
                      ORDER BY id DESC LIMIT 1",
                    params![id],
                    |r| r.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten()
                .and_then(|v| serde_json::from_str::<Value>(&v).ok())
                .map(|v| {
                    json!({
                        "verified": v.get("verified").and_then(Value::as_bool) == Some(true),
                        "warnings": v.get("warnings").and_then(Value::as_array)
                            .map(|a| a.iter().take(5).cloned().collect::<Vec<_>>())
                            .unwrap_or_default(),
                    })
                })
                .unwrap_or(Value::Null);
            json!({
                "analysis": analysis,
                "model": model,
                "analyzed_at": created_at,
                "latency_ms": latency_ms,
                "latest_draft_verification": verification,
            })
        }
        "get_supportos_metadata" => {
            let Some(num) = args.get("number").and_then(Value::as_i64) else {
                return json!({ "error": "Invalid number" });
            };
            let row = conn
                .query_row(
                    "SELECT c.number, c.supportos_priority, c.supportos_state_id,
                            (SELECT ts.name FROM ticket_states ts WHERE ts.id = c.supportos_state_id),
                            (SELECT TRIM(COALESCE(u2.first_name, '') || ' ' || COALESCE(u2.last_name, ''))
                               FROM users u2 WHERE u2.id = c.assignee_local_id),
                            (SELECT m.name FROM mailboxes m WHERE m.id = c.mailbox_local_id),
                            c.closed_at, c.first_customer_message_at, c.first_response_at,
                            c.last_customer_reply_at, c.last_human_agent_response_at, c.customer_waiting_since,
                            (SELECT kic.known_issue_id FROM known_issue_conversations kic
                              WHERE kic.conversation_id = c.id LIMIT 1)
                       FROM conversations c WHERE c.number = ?1 AND c.deleted_at IS NULL",
                    params![num],
                    |r| {
                        Ok(json!({
                            "number": r.get::<_, i64>(0)?,
                            "supportos_priority": r.get::<_, Option<String>>(1)?,
                            "ticket_state": r.get::<_, Option<String>>(3)?,
                            "assignee": r.get::<_, Option<String>>(4)?,
                            "mailbox": r.get::<_, Option<String>>(5)?,
                            "closed_at": r.get::<_, Option<String>>(6)?,
                            "first_customer_message_at": r.get::<_, Option<String>>(7)?,
                            "first_response_at": r.get::<_, Option<String>>(8)?,
                            "last_customer_reply_at": r.get::<_, Option<String>>(9)?,
                            "last_human_agent_response_at": r.get::<_, Option<String>>(10)?,
                            "customer_waiting_since": r.get::<_, Option<String>>(11)?,
                            "known_issue_linked": r.get::<_, Option<i64>>(12)?.is_some(),
                        }))
                    },
                )
                .ok();
            let Some(mut row) = row else {
                return json!({ "error": "Conversation not found" });
            };
            row["note"] = json!(
                "SLA state is computed live against mailbox business hours; customer_waiting_since is the local waiting marker."
            );
            row
        }
        "get_ai_attributes" => {
            let Some(num) = args.get("number").and_then(Value::as_i64) else {
                return json!({ "error": "Invalid number" });
            };
            let Some((id, _)) = conv_by_number(conn, num) else {
                return json!({ "error": "Conversation not found" });
            };
            let attrs: Vec<Value> = conn
                .prepare(
                    "SELECT attribute, value, confidence, source, evidence, computed_at
                       FROM ai_attributes WHERE conversation_id = ?1 AND superseded_at IS NULL",
                )
                .and_then(|mut stmt| {
                    stmt.query_map(params![id], |r| {
                        let evidence = r
                            .get::<_, Option<String>>(4)?
                            .and_then(|e| serde_json::from_str::<Value>(&e).ok())
                            .and_then(|e| e.as_array().cloned())
                            .unwrap_or_default();
                        let excerpts: Vec<Value> = evidence
                            .iter()
                            .take(2)
                            .map(|e| {
                                json!({ "excerpt": red_chars(
                                    conn,
                                    e.get("excerpt").and_then(Value::as_str).unwrap_or(""),
                                    200,
                                )})
                            })
                            .collect();
                        Ok(json!({
                            "attribute": r.get::<_, String>(0)?,
                            "value": r.get::<_, Option<String>>(1)?,
                            "confidence": r.get::<_, Option<String>>(2)?,
                            "source": r.get::<_, Option<String>>(3)?,
                            "computed_at": r.get::<_, Option<String>>(5)?,
                            "evidence": excerpts,
                        }))
                    })
                    .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                })
                .unwrap_or_default();
            json!({
                "attributes": attrs,
                "note": "Attributes absent from this list are unknown (not yet computed or no evidence)."
            })
        }
        "search_incidents" => {
            let q: String = args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect::<String>()
                .to_lowercase();
            let rows: Vec<Value> = conn
                .prepare(
                    "SELECT i.id, i.code, i.title, i.status, i.severity, i.product, i.feature,
                            (SELECT COUNT(*) FROM incident_conversations ic WHERE ic.incident_id = i.id),
                            (SELECT COUNT(DISTINCT c.customer_local_id) FROM incident_conversations ic
                               JOIN conversations c ON c.id = ic.conversation_id
                              WHERE ic.incident_id = i.id AND c.customer_local_id IS NOT NULL AND c.deleted_at IS NULL)
                       FROM incidents i
                      ORDER BY CASE i.status WHEN 'resolved' THEN 1 ELSE 0 END, i.updated_at DESC LIMIT 20",
                )
                .and_then(|mut stmt| {
                    stmt.query_map([], |r| {
                        Ok(json!({
                            "code": r.get::<_, Option<String>>(1)?,
                            "title": r.get::<_, Option<String>>(2)?,
                            "status": r.get::<_, String>(3)?,
                            "severity": r.get::<_, String>(4)?,
                            "product": r.get::<_, Option<String>>(5)?,
                            "feature": r.get::<_, Option<String>>(6)?,
                            "conversation_count": r.get::<_, i64>(7)?,
                            "customer_count": r.get::<_, i64>(8)?,
                        }))
                    })
                    .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                })
                .unwrap_or_default();
            json!(
                rows.into_iter()
                    .filter(|r| {
                        q.is_empty()
                            || format!(
                                "{} {} {}",
                                r["code"].as_str().unwrap_or(""),
                                r["title"].as_str().unwrap_or(""),
                                r["product"].as_str().unwrap_or("")
                            )
                            .to_lowercase()
                            .contains(&q)
                    })
                    .take(limit as usize)
                    .map(|mut r| {
                        let conversations = r["conversation_count"].as_i64().unwrap_or(0);
                        let customers = r["customer_count"].as_i64().unwrap_or(0);
                        r["affected_conversations"] = json!(conversations);
                        r["affected_customers"] = json!(customers);
                        r["note"] = json!(
                            "Affected customers are distinct customers, never ticket counts."
                        );
                        r
                    })
                    .collect::<Vec<_>>()
            )
        }
        "search_custom_objects" => {
            let q: String = args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect();
            let type_slug = args
                .get("type")
                .and_then(Value::as_str)
                .map(|s| s.chars().take(60).collect::<String>());
            let type_id: Option<i64> = type_slug.as_deref().and_then(|slug| {
                conn.query_row(
                    "SELECT id FROM custom_object_types WHERE slug = ?1 AND deleted_at IS NULL",
                    params![slug],
                    |r| r.get(0),
                )
                .ok()
            });
            let tokens = crate::search::fts_query(&q.replace(['"', '*', '(', ')'], " "));
            if tokens == "\"\"" {
                return json!([]);
            }
            let sql = format!(
                "SELECT o.id, o.title, o.data_json, t.name AS type_name, t.slug AS type_slug
                   FROM fts_custom_objects f
                   JOIN custom_objects o ON o.id = f.object_id AND o.deleted_at IS NULL
                   JOIN custom_object_types t ON t.id = o.type_id
                  WHERE fts_custom_objects MATCH ?1 {}
                  ORDER BY rank LIMIT ?2",
                if type_id.is_some() {
                    "AND o.type_id = ?3"
                } else {
                    ""
                },
            );
            let rows: Vec<(i64, String, String, String)> = if let Some(tid) = type_id {
                conn.prepare(&sql)
                    .and_then(|mut stmt| {
                        stmt.query_map(params![tokens, limit, tid], |r| {
                            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                        })
                        .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                    })
                    .unwrap_or_default()
            } else {
                conn.prepare(&sql)
                    .and_then(|mut stmt| {
                        stmt.query_map(params![tokens, limit], |r| {
                            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                        })
                        .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                    })
                    .unwrap_or_default()
            };
            json!(rows
                .iter()
                .map(|(id, title, data_json, type_name)| {
                    let properties: Value = serde_json::from_str(data_json).unwrap_or(json!({}));
                    let mut clean = serde_json::Map::new();
                    if let Some(obj) = properties.as_object() {
                        for (k, v) in obj.iter().take(12) {
                            let v = match v.as_str() {
                                Some(s) => Value::String(red_chars(conn, s, 200)),
                                None => v.clone(),
                            };
                            clean.insert(k.clone(), v);
                        }
                    }
                    let _ = id;
                    json!({ "type": type_name, "title": red_chars(conn, title, 200), "properties": clean })
                })
                .collect::<Vec<_>>())
        }
        "get_customer_timeline" => {
            let Some(num) = args.get("number").and_then(Value::as_i64) else {
                return json!({ "error": "Invalid number" });
            };
            let Some(customer) = conn
                .query_row(
                    "SELECT customer_local_id FROM conversations WHERE number = ?1 AND deleted_at IS NULL",
                    params![num],
                    |r| r.get::<_, Option<i64>>(0),
                )
                .ok()
                .flatten()
            else {
                return json!({ "error": "Conversation not found or has no customer" });
            };
            let events: Vec<Value> = conn
                .prepare(
                    "SELECT event_kind, occurred_at, title, source FROM customer_events
                      WHERE customer_local_id = ?1
                      ORDER BY COALESCE(occurred_at, created_at) DESC, id DESC LIMIT ?2",
                )
                .and_then(|mut stmt| {
                    stmt.query_map(params![customer, limit.clamp(1, 20)], |r| {
                        Ok(json!({
                            "kind": r.get::<_, String>(0)?,
                            "at": r.get::<_, Option<String>>(1)?,
                            "title": red_chars(conn, &r.get::<_, String>(2)?, 160),
                            "source": r.get::<_, String>(3)?,
                        }))
                    })
                    .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                })
                .unwrap_or_default();
            json!({ "events": events })
        }
        "search_connector_data" => {
            let connector_name: String = args
                .get("connector")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect();
            if connector_name.is_empty() {
                return json!({ "error": "Invalid connector name" });
            }
            let q: String = args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect::<String>()
                .to_lowercase();
            let row = conn
                .query_row(
                    "SELECT id, name, allowed_ai, enabled FROM connectors WHERE name = ?1",
                    params![connector_name],
                    |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, i64>(3)?,
                        ))
                    },
                )
                .ok();
            let Some((id, name, allowed_ai, enabled)) = row else {
                let available: Vec<String> = conn
                    .prepare(
                        "SELECT name FROM connectors WHERE allowed_ai = 1 AND enabled = 1 LIMIT 10",
                    )
                    .and_then(|mut stmt| {
                        stmt.query_map([], |r| r.get::<_, String>(0))
                            .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                    })
                    .unwrap_or_default();
                return json!({ "error": "Connector not found", "ai_visible_connectors": available });
            };
            if allowed_ai != 1 {
                return json!({ "error": format!("Connector \"{name}\" is not marked as AI-visible. Its data stays private to the UI.") });
            }
            if enabled != 1 {
                return json!({ "error": format!("Connector \"{name}\" is disabled.") });
            }
            let data_rows: Vec<String> = conn
                .prepare("SELECT data FROM connector_rows WHERE connector_id = ?1 ORDER BY id DESC LIMIT 100")
                .and_then(|mut stmt| {
                    stmt.query_map(params![id], |r| r.get::<_, String>(0))
                        .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                })
                .unwrap_or_default();
            let results: Vec<Value> = data_rows
                .into_iter()
                .map(|d| serde_json::from_str::<Value>(&d).unwrap_or(json!({})))
                .filter(|d| q.is_empty() || d.to_string().to_lowercase().contains(&q))
                .take(limit as usize)
                .map(|d| {
                    let mut out = serde_json::Map::new();
                    if let Some(obj) = d.as_object() {
                        for (k, v) in obj.iter().take(15) {
                            let v = match v.as_str() {
                                Some(s) => Value::String(red_chars(conn, s, 300)),
                                None => v.clone(),
                            };
                            out.insert(k.clone(), v);
                        }
                    }
                    Value::Object(out)
                })
                .collect();
            json!({ "connector": name, "results": results })
        }
        "get_knowledge_gaps" => {
            let kind_filter = args
                .get("kind")
                .and_then(Value::as_str)
                .map(|s| s.chars().take(60).collect::<String>());
            let rows: Vec<GapRow> = conn
                .prepare(
                    "SELECT id, kind, query_text, occurrence_count, status, detail
                       FROM knowledge_gap_candidates
                      ORDER BY CASE status WHEN 'open' THEN 0 ELSE 1 END, occurrence_count DESC LIMIT 50",
                )
                .and_then(|mut stmt| {
                    stmt.query_map([], |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, Option<String>>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, i64>(3)?,
                            r.get::<_, String>(4)?,
                            r.get::<_, Option<String>>(5)?,
                        ))
                    })
                    .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                })
                .unwrap_or_default();
            json!(rows
                .into_iter()
                .filter(|r| {
                    kind_filter
                        .as_deref()
                        .map(|k| r.1.as_deref() == Some(k))
                        .unwrap_or(true)
                })
                .take(limit as usize)
                .map(|(id, kind, question, occurrences, status, detail)| {
                    let explanation: String = detail
                        .as_deref()
                        .and_then(|d| serde_json::from_str::<Value>(d).ok())
                        .and_then(|d| {
                            d.get("explanation").and_then(Value::as_str).map(str::to_string)
                        })
                        .unwrap_or_default();
                    json!({
                        "id": id,
                        "kind": kind,
                        "question": red_chars(conn, &question, 200),
                        "occurrences": occurrences,
                        "status": status,
                        "explanation": red_chars(conn, &explanation, 300),
                        "note": "Candidates are deterministic detections awaiting human approval; approving never publishes automatically."
                    })
                })
                .collect::<Vec<_>>())
        }
        "get_friction_report" => {
            let days = args
                .get("days")
                .and_then(Value::as_i64)
                .unwrap_or(30)
                .clamp(1, 365);
            let rows: Vec<Value> = conn
                .prepare(
                    "SELECT f.kind, COUNT(*), SUM(CASE WHEN f.severity = 'high' THEN 1 ELSE 0 END)
                       FROM friction_findings f JOIN conversations c ON c.id = f.conversation_id
                      WHERE COALESCE(julianday(c.remote_created_at), julianday(f.computed_at)) >= julianday('now', ?1)
                      GROUP BY f.kind ORDER BY 2 DESC",
                )
                .and_then(|mut stmt| {
                    stmt.query_map(params![format!("-{days} days")], |r| {
                        Ok(json!({
                            "kind": r.get::<_, String>(0)?,
                            "conversations": r.get::<_, i64>(1)?,
                            "high_severity": r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                        }))
                    })
                    .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                })
                .unwrap_or_default();
            json!({
                "window_days": days,
                "kinds": rows,
                "note": "Deterministic text-shape heuristics over the local mirror - patterns, not judgments about people. Findings exist only for analyzed conversations."
            })
        }
        "get_graph_neighbors" => {
            let kind = args
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let Some(local_id) = args
                .get("local_id")
                .and_then(Value::as_i64)
                .filter(|v| *v > 0)
            else {
                return json!({ "error": "Invalid local_id" });
            };
            let Some(kind_enum) = GraphNodeKind::ALL.into_iter().find(|k| k.as_str() == kind)
            else {
                return json!({ "error": format!("Unknown node kind: {kind}") });
            };
            let Some(node_ref) = crate::support_graph::node_ref(conn, kind_enum, local_id)
                .ok()
                .flatten()
            else {
                return json!({ "error": "Node not found" });
            };
            let edges = crate::support_graph::human_edges_touching(conn, kind_enum, local_id)
                .unwrap_or_default();
            let center_key = json!({"kind": kind, "local_id": local_id});
            let items: Vec<Value> = edges
                .iter()
                .map(|e| {
                    let source_is_center = e["source"]["kind"] == center_key["kind"]
                        && e["source"]["local_id"] == center_key["local_id"];
                    let other = if source_is_center {
                        &e["target"]
                    } else {
                        &e["source"]
                    };
                    json!({
                        "relation": e["relation"],
                        "connected_kind": other["kind"],
                        "connected": red_chars(conn, other["label"].as_str().unwrap_or(""), 120),
                        "note": e["note"],
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "node": { "kind": kind, "label": red_chars(conn, &node_ref.label, 160) },
                "total_edges": edges.len(),
                "edges": items,
                "note": "Edges are read from the local support graph store; only human-asserted edges are stored."
            })
        }
        "get_graph_stats" => {
            let nodes: Vec<Value> = conn
                .prepare("SELECT kind, COUNT(*) FROM graph_nodes GROUP BY kind ORDER BY 2 DESC")
                .and_then(|mut stmt| {
                    stmt.query_map([], |r| {
                        Ok(json!({ "kind": r.get::<_, String>(0)?, "count": r.get::<_, i64>(1)? }))
                    })
                    .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                })
                .unwrap_or_default();
            let edges: Vec<Value> = conn
                .prepare(
                    "SELECT relation, COUNT(*) FROM graph_edges GROUP BY relation ORDER BY 2 DESC",
                )
                .and_then(|mut stmt| {
                    stmt.query_map([], |r| {
                        Ok(json!({
                            "relation": r.get::<_, String>(0)?,
                            "origin": "human_local",
                            "count": r.get::<_, i64>(1)?,
                        }))
                    })
                    .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                })
                .unwrap_or_default();
            json!({
                "nodes": nodes,
                "edges": edges,
                "note": "Live counts - no denormalized totals. Connector rows have no derived edges by design."
            })
        }
        "get_customer_memory" => {
            let Some(num) = args.get("number").and_then(Value::as_i64) else {
                return json!({ "error": "Invalid number" });
            };
            let Some(customer) = conn
                .query_row(
                    "SELECT customer_local_id FROM conversations WHERE number = ?1 AND deleted_at IS NULL",
                    params![num],
                    |r| r.get::<_, Option<i64>>(0),
                )
                .ok()
                .flatten()
            else {
                return json!({ "error": "Conversation not found or has no customer" });
            };
            let profile = conn
                .query_row(
                    "SELECT COALESCE(first_name, ''), COALESCE(last_name, '') FROM customers WHERE id = ?1 AND deleted_at IS NULL",
                    params![customer],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                )
                .ok();
            let Some((first, last)) = profile else {
                return json!({ "error": "Customer not found" });
            };
            let issues: Vec<Value> = conn
                .prepare(
                    "SELECT ki.title, ki.status, MAX(kic.linked_at)
                       FROM known_issue_conversations kic
                       JOIN known_issues ki ON ki.id = kic.known_issue_id
                       JOIN conversations c ON c.id = kic.conversation_id
                      WHERE c.customer_local_id = ?1
                      GROUP BY ki.id ORDER BY 3 DESC LIMIT 5",
                )
                .and_then(|mut stmt| {
                    stmt.query_map(params![customer], |r| {
                        Ok(json!({
                            "title": red_chars(conn, &r.get::<_, String>(0)?, 140),
                            "value": r.get::<_, Option<String>>(1)?,
                            "source": "local_mirror",
                            "confidence": "high",
                            "last_seen": r.get::<_, Option<String>>(2)?,
                        }))
                    })
                    .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                })
                .unwrap_or_default();
            let mut quarantined = 0usize;
            let notes: Vec<MemoryRow> = conn
                .prepare(
                    "SELECT memory_key, memory_value, source, confidence, last_seen_at
                       FROM customer_memory WHERE customer_id = ?1
                      ORDER BY COALESCE(last_seen_at, created_at) DESC LIMIT 25",
                )
                .and_then(|mut stmt| {
                    stmt.query_map(params![customer], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, Option<String>>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, Option<String>>(3)?,
                            r.get::<_, Option<String>>(4)?,
                        ))
                    })
                    .map(|rows| rows.filter_map(|t| t.ok()).collect::<Vec<_>>())
                })
                .unwrap_or_default();
            let human: Vec<Value> = notes
                .iter()
                .filter(|(k, v, ..)| {
                    if crate::ai_memory_guard::is_quarantined(k, v.as_deref()) {
                        quarantined += 1;
                        false
                    } else {
                        true
                    }
                })
                .take(5)
                .map(|(k, v, source, confidence, last_seen)| {
                    json!({
                        "title": red_chars(conn, k, 140),
                        "value": v.as_deref().map(|v| red_chars(conn, v, 200)),
                        "source": source,
                        "confidence": confidence,
                        "last_seen": last_seen,
                    })
                })
                .collect();
            let mut sections = vec![json!({ "section": "issue_history", "entries": issues })];
            if !human.is_empty() {
                sections.push(json!({ "section": "notes", "entries": human }));
            }
            json!({
                "customer": format!("{} {}", first, last).trim(),
                "sections": sections,
                "quarantined": quarantined,
                "note": "Composed live from the local mirror - human-written entries are the only persisted rows. Psychological/personality judgments are never included."
            })
        }
        _ => {
            json!({ "error": format!("Unknown tool {name} - allowed tools are read-only search tools") })
        }
    }
}

fn map_cluster_row(conn: &Connection, r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let title: String = r.get(1)?;
    let summary: Option<String> = r.get(2)?;
    let numbers: Option<String> = r.get(7)?;
    let conversation_numbers: Vec<i64> = numbers
        .unwrap_or_default()
        .split(',')
        .filter_map(|n| n.parse().ok())
        .take(15)
        .collect();
    Ok(json!({
        "title": title,
        "summary": red_chars(conn, summary.as_deref().unwrap_or(""), 400),
        "category": r.get::<_, Option<String>>(3)?,
        "product": r.get::<_, Option<String>>(4)?,
        "feature": r.get::<_, Option<String>>(5)?,
        "conversation_numbers": conversation_numbers,
        "ai_generated": r.get::<_, Option<i64>>(6)? == Some(1),
    }))
}

fn collect_rows(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Value>>,
) -> Vec<Value> {
    rows.filter_map(|r| r.ok()).collect()
}
