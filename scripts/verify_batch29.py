#!/usr/bin/env python3
"""Batch 29 verification harness (clean-audit items 97-106).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..28 harness protocol.

Items:
  97   rewrite paths bypass redaction (ai_pipeline.rs)
  98   tool args as_i64 rejects schema-valid floats (ai_tools.rs)
  99   get_issue_clusters returns 3x the documented limit with a query
  100  knowledge chunks: no attempts cap -> infinite retry (embeddings.rs)
  101  malformed 200 -> literal "Unknown" content (ai_lm_studio.rs)
  102  draft signature picks an arbitrary agent's name (ai_local.rs)
  103  org member_count capped by LIMIT 100 (people_store.rs)
  104  last-activity/resolution subqueries miss deleted_at (people_store.rs)
  105  org name linking case-sensitive vs NOCASE elsewhere (people_store.rs)
  106  CSV export lacks formula-injection guard (backup_service.rs)
"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORE = ROOT / "crates" / "core" / "src"
FAILURES: list[str] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    status = "OK " if ok else "FAIL"
    print(f"[{status}] {name}" + (f" — {detail}" if detail and not ok else ""))
    if not ok:
        FAILURES.append(f"{name}: {detail}")


def read(root: Path, p: str) -> str:
    return (root / p).read_text(encoding="utf-8")


def read_code(root: Path, p: str) -> str:
    """Source without doc-comment lines (///) — for checks whose needle
    may legitimately appear inside a comment describing the pre-fix
    defect."""
    return "\n".join(
        line for line in read(root, p).splitlines() if not line.lstrip().startswith("///")
    )


def function_body(src: str, needle: str, end: str = "\n}\n") -> str:
    start = src.index(needle)
    end_idx = src.index(end, start)
    return src[start:end_idx]


# ----------------------------------------------------------------- item 97
def verify_item97() -> None:
    pipeline = read_code(CORE, "ai_pipeline.rs")
    check(
        "97: the rewrite lanes share a redaction-boundary helper",
        "fn redact_for_model(conn: &Connection, text: &str) -> String" in pipeline,
    )
    check(
        "97: the helper honors the redaction_enabled switch",
        'crate::settings::get_bool(conn, "redaction_enabled", true)' in pipeline,
    )
    check(
        "97: findings are logged, not silently dropped",
        '"redaction boundary applied to the rewrite prompt"' in pipeline,
    )
    composer = function_body(pipeline, "pub async fn rewrite_composer_text")
    check(
        "97: the composer rewrite sends the redacted text",
        "let safe_text = redact_for_model(conn, text);" in composer
        and "safe_text\n            ),\n        },\n    ];" in composer.replace("    ", "    "),
    )
    check(
        "97: the composer rewrite no longer sends raw text",
        "rewrite_instruction(instruction),\n                text" not in composer,
    )
    stored = function_body(pipeline, "pub async fn rewrite_draft")
    check(
        "97: the stored-draft rewrite sends the redacted content",
        "let safe_content = redact_for_model(conn, &draft.content);" in stored,
    )
    check(
        "97: the stored-draft rewrite no longer sends draft.content raw",
        'format!("{instructions}\\n\\nReply to rewrite:\\n\\"\\"\\"\\n{}\\n\\"\\"\\"", draft.content)'
        not in stored,
    )


# ----------------------------------------------------------------- item 98
def verify_item98() -> None:
    tools = read_code(CORE, "ai_tools.rs")
    check(
        "98: the lenient integral reader exists",
        "fn arg_i64(args: &serde_json::Map<String, Value>, key: &str) -> Option<i64>" in tools,
    )
    check(
        "98: integral floats decode through as_f64",
        "let f = n.as_f64()?;" in tools,
    )
    check(
        "98: non-integral floats are never truncated",
        "f.fract() == 0.0" in tools,
    )
    check(
        "98: every raw tool-arg as_i64 read is gone",
        ".and_then(Value::as_i64)"
        not in tools[: tools.index("#[cfg(test)]")],
        "the schema-valid-float reject must have no remaining call sites in shipped code",
    )
    check(
        "98: the argument sites route through arg_i64",
        tools.count("arg_i64(&args,") >= 12,
    )
    check(
        "98: clamp_limit reads through the same lens",
        "let n = arg_i64(args, \"limit\").unwrap_or(5);" in tools,
    )


# ----------------------------------------------------------------- item 99
def verify_item99() -> None:
    tools = read_code(CORE, "ai_tools.rs")
    clusters = function_body(tools, '"get_issue_clusters" =>')
    check(
        "99: the documented limit is the SQL limit — the 3x multiplier is gone",
        "limit * if q.is_empty() { 1 } else { 3 }" not in clusters,
    )
    check(
        "99: the LIMIT binds the clamped limit alone",
        "ORDER BY ic.id DESC LIMIT {}\",\n                if q.is_empty()" in clusters.replace(
            "    ", "    "
        ),
    )
    check(
        "99: the schema still documents the cap honestly",
        '"Max clusters (default 5, max 10)"' in read(CORE, "ai_tools.rs"),
    )


# ----------------------------------------------------------------- item 100
def verify_item100() -> None:
    embeddings = read_code(CORE, "embeddings.rs")
    check(
        "100: the knowledge pending query caps on attempts",
        "WHERE (c.embedding_state = 'not_indexed' OR c.embedding_state = 'failed')\n"
        "               AND c.embedding_attempts < 5\n"
        "             LIMIT {EMBED_PASS_LIMIT}" in embeddings,
    )
    check(
        "100: mark_chunk_failed counts attempts for every family",
        '"UPDATE {table} SET embedding_state = \'failed\',\n'
        "             embedding_attempts = embedding_attempts + 1 WHERE id = ?1\"" in embeddings,
    )
    check(
        "100: the knowledge-only state-reset branch is gone",
        'conn.execute(\n            &format!("UPDATE {table} SET embedding_state = \'failed\' WHERE id = ?1"),'
        not in embeddings,
    )
    check(
        "100: the v2 re-embed migration resets knowledge attempts too",
        "UPDATE knowledge_chunks SET embedding_state = 'not_indexed', embedding_attempts = 0"
        in embeddings,
    )
    check(
        "100: migration m061 exists and adds the column",
        "pub fn apply_m061_knowledge_chunk_attempts(conn: &Connection) -> Result<()>" in embeddings
        and "ADD COLUMN embedding_attempts INTEGER NOT NULL DEFAULT 0" in embeddings,
    )
    bootstrap = read_code(CORE, "bootstrap.rs")
    check(
        "100: boot step 52 registers m061",
        'label: "m061_knowledge_chunk_attempts"' in bootstrap
        and "apply: crate::embeddings::apply_m061_knowledge_chunk_attempts," in bootstrap,
    )
    check(
        "100: boot step 52 follows step 51",
        bootstrap.index("version: 51,") < bootstrap.index("version: 52,"),
    )


# ----------------------------------------------------------------- item 101
def verify_item101() -> None:
    lm = read_code(CORE, "ai_lm_studio.rs")
    check(
        "101: the parse returns Result, never fabricated content",
        "pub fn parse_chat_response(resp: ChatCompletionResponse) -> Result<ChatResponse>" in lm,
    )
    check(
        "101: the literal Unknown content is gone",
        '"Unknown".into()' not in lm,
    )
    check(
        "101: empty choices are a labeled error",
        '"LM Studio chat returned no choices (malformed 200 body)"' in lm,
    )
    check(
        "101: null content is a labeled error",
        '"LM Studio chat returned no message content (malformed 200 body)"' in lm,
    )
    chat = function_body(lm, "pub async fn chat(&self, model: &str, messages: &[ChatMessage])")
    check(
        "101: the chat lane propagates the parse result",
        "parse_chat_response(body)\n    }" in chat,
    )


# ----------------------------------------------------------------- item 102
def verify_item102() -> None:
    local = read_code(CORE, "ai_local.rs")
    check(
        "102: the arbitrary lowest-id agent subquery is gone",
        "(SELECT u.first_name FROM users u ORDER BY u.id LIMIT 1)" not in local,
    )
    check(
        "102: the signature resolves the conversation's assignee",
        "(SELECT u.first_name FROM users u WHERE u.id = c.assignee_local_id)" in local,
    )
    check(
        "102: the honest Support fallback still speaks when unassigned",
        'signature.unwrap_or("Support")' in read(CORE, "ai_local.rs"),
    )


# ----------------------------------------------------------------- item 103
def verify_item103() -> None:
    people = read_code(CORE, "people_store.rs")
    detail = function_body(people, "pub fn get_organization_detail")
    check(
        "103: the exact totals come from an unbounded aggregate",
        "let (member_total, open_total, conversation_total): (i64, i64, i64)" in detail,
    )
    check(
        "103: the aggregate counts every member, not the list page",
        "COALESCE(SUM((SELECT COUNT(*) FROM conversations c" in detail,
    )
    check(
        "103: member_count no longer reads the capped list length",
        "let member_count = members.len() as i64;" not in people,
    )
    check(
        "103: the open total no longer sums the list rows",
        'm.get("open_count").and_then(|v| v.as_i64())' not in people,
    )
    check(
        "103: the members list stays a display list (LIMIT 100)",
        "LIMIT 100\"" in detail,
    )


# ----------------------------------------------------------------- item 104
def verify_item104() -> None:
    people = read_code(CORE, "people_store.rs")
    check(
        "104: the list row's last activity filters deleted conversations",
        "(SELECT MAX(cv.last_activity_at) FROM conversations cv\n"
        "                  WHERE cv.customer_local_id = c.id AND cv.deleted_at IS NULL) AS last_activity_at"
        in people,
    )
    check(
        "104: the recency sort filters deleted conversations too",
        "(SELECT MAX(cv.last_activity_at) FROM conversations cv\n"
        "              WHERE cv.customer_local_id = c.id AND cv.deleted_at IS NULL),"
        in people,
    )
    check(
        "104: the resolution subquery ignores deleted threads",
        "AND t.state = 'published' AND t.deleted_at IS NULL" in people,
    )
    check(
        "104: the pre-fix resolution subquery is gone",
        "AND t.state = 'published'\n                  ORDER BY COALESCE(t.remote_created_at, t.created_at) DESC LIMIT 1"
        not in people,
    )


# ----------------------------------------------------------------- item 105
def verify_item105() -> None:
    people = read_code(CORE, "people_store.rs")
    check(
        "105: the members SQL links names case-insensitively",
        "cu.organization = (SELECT name FROM organizations WHERE id = ?1) COLLATE NOCASE"
        in people,
    )
    check(
        "105: create_customer links NOCASE",
        "WHERE o.name = ?5 COLLATE NOCASE AND o.deleted_at IS NULL LIMIT 1" in people,
    )
    check(
        "105: update_customer relinks NOCASE",
        "WHERE o.name = customers.organization COLLATE NOCASE AND o.deleted_at IS NULL LIMIT 1"
        in people,
    )
    check(
        "105: the timeline backfill links NOCASE",
        "UPDATE customers SET organization_id = (\n"
        "            SELECT o.id FROM organizations o\n"
        "             WHERE o.name = customers.organization COLLATE NOCASE AND o.deleted_at IS NULL LIMIT 1)"
        in people,
    )
    check(
        "105: the org rename propagates NOCASE",
        "AND organization = (SELECT name FROM organizations WHERE id = ?1) COLLATE NOCASE"
        in people,
    )
    check(
        "105: the org list counts name-linked members NOCASE",
        people.count("cu.organization = o.name COLLATE NOCASE") >= 4,
    )


# ----------------------------------------------------------------- item 106
def verify_item106() -> None:
    backup = read_code(CORE, "backup_service.rs")
    check(
        "106: the guarded CSV cell writer exists",
        "fn csv_cell(v: &Option<String>) -> String {" in backup,
    )
    check(
        "106: the OWASP trigger set is complete",
        "raw.starts_with(['=', '+', '-', '@', '\\t', '\\r'])" in backup,
    )
    check(
        "106: the guard prefixes a single quote",
        "format!(\"'{raw}\")" in backup,
    )
    check(
        "106: the export lane uses the guarded writer",
        "let esc = |v: &Option<String>| -> String { csv_cell(v) };" in backup,
    )
    check(
        "106: the pre-fix bare quoting closure is gone",
        "let s = v.as_deref().unwrap_or(\"\").replace('\"', \"\\\"\\\"\");" not in backup,
    )


def main() -> int:
    for verify in (
        verify_item97,
        verify_item98,
        verify_item99,
        verify_item100,
        verify_item101,
        verify_item102,
        verify_item103,
        verify_item104,
        verify_item105,
        verify_item106,
    ):
        verify()
    print()
    if FAILURES:
        print(f"{len(FAILURES)} check(s) FAILED:")
        for failure in FAILURES:
            print(f"  - {failure}")
        return 1
    print("All batch-29 checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
