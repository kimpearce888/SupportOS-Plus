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

pub(crate) fn ensure_tool_tables(conn: &Connection) -> Result<()> {
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

pub(crate) fn definitions() -> Vec<ChatTool> {
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

mod execute;

pub use execute::execute;

#[cfg(test)]
mod batch23_audit_tests {
    use super::execute::execute;
    use super::*;
    use rusqlite::Connection;

    fn db_with_memory_rows() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn.execute(
            "INSERT INTO customers (remote_id, first_name, last_name)
             VALUES (80, 'Grace', 'Hopper')",
            [],
        )
        .expect("customer");
        let customer = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, customer_local_id)
             VALUES (9501, 95, 'Export failing', 'preview', 'active', ?1)",
            rusqlite::params![customer],
        )
        .expect("conversation");
        conn.execute(
            "INSERT INTO customer_memory (customer_id, memory_key, memory_value, evidence_excerpt,
                     source_conversation_id, confidence, provenance, source, last_seen_at)
             VALUES (?1, 'vip tier', 'gold — hand-written by support', '',
                     NULL, 'high', 'human_local', 'human', datetime('now'))",
            rusqlite::params![customer],
        )
        .expect("human memory");
        conn.execute(
            "INSERT INTO customer_memory (customer_id, memory_key, memory_value, evidence_excerpt,
                     source_conversation_id, confidence, provenance, source, last_seen_at)
             VALUES (?1, 'environment', 'staging cluster (extracted from a thread)', 'from threads',
                     NULL, 'medium', 'ai_generated', 'ai', datetime('now', '-1 minute'))",
            rusqlite::params![customer],
        )
        .expect("ai memory");
        conn
    }

    #[test]
    fn customer_memory_note_no_longer_claims_human_only() {
        let conn = db_with_memory_rows();
        let out = execute(&conn, None, "get_customer_memory", r#"{"number": 95}"#);

        let note = out.get("note").and_then(Value::as_str).unwrap_or_default();
        assert!(
            !note.contains("the only persisted rows"),
            "the false human-only claim is gone: {note}"
        );
        assert!(
            note.contains("'ai' rows are machine-extracted"),
            "the note explains the ai provenance: {note}"
        );
        assert!(
            note.contains("not human-verified"),
            "the note says ai rows are unverified: {note}"
        );

        let entries = out
            .get("sections")
            .and_then(|s| s.as_array())
            .expect("sections")
            .iter()
            .find(|s| s.get("section").and_then(Value::as_str) == Some("notes"))
            .and_then(|s| s.get("entries"))
            .and_then(|e| e.as_array())
            .expect("the notes section")
            .clone();
        assert_eq!(entries.len(), 2, "both memory rows are relayed");
        let human = entries
            .iter()
            .find(|e| e.get("source").and_then(Value::as_str) == Some("human"))
            .expect("the human row is present with its source");
        assert_eq!(human.get("title").and_then(Value::as_str), Some("vip tier"));
        let ai = entries
            .iter()
            .find(|e| e.get("source").and_then(Value::as_str) == Some("ai"))
            .expect("the ai row is present, honestly labeled");
        assert_eq!(ai.get("title").and_then(Value::as_str), Some("environment"));
    }
}
