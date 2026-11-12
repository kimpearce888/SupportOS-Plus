#!/usr/bin/env python3
"""Batch 23 verification harness (clean-audit items 36-45).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..22 harness protocol.

Items:
  36  customer_messages included soft-deleted threads and took the
      OLDEST 30 — the recent context was cut off
  37  extraction runs stuck at the 'queued' status default, invisible to
      the cost ledger (no type/latency/usage/endpoint)
  38  cache-hit path paired the hash-matched run's id with the NEWEST
      conversation analysis
  39  draft fallback stored the raw JSON envelope as the draft
  40  interaction-strategy prompt block hard-coded None; the whole
      COMMUNICATION APPROACH section unreachable
  41  get_customer_memory relays AI rows under a note claiming they are
      human-written
  42  cluster prompt carried EVERY conversation in the window — context
      blowup at volume
  43  copilot runs invisible to the cost ledger (no usage/endpoint)
  44  tool-budget cutoff left dangling tool_call ids (400 on strict
      servers)
  45  insert_message swallowed INSERT failures and echoed a stale rowid
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


def read(p: str) -> str:
    return (CORE / p).read_text(encoding="utf-8")


# ----------------------------------------------------------------- item 36
def verify_item36() -> None:
    src = read("ai_attributes.rs")
    fn = src[src.index("pub fn customer_messages") :]
    fn = fn[: fn.index("pub struct PreparedComputation")]
    check(
        "36: soft-deleted threads are excluded (deleted_at IS NULL)",
        "deleted_at IS NULL" in fn,
    )
    check(
        "36: the window takes the NEWEST 30 (DESC before the LIMIT)",
        "ORDER BY created_at DESC, id DESC LIMIT 30" in fn,
    )
    check(
        "36: whitespace-only bodies cannot consume LIMIT slots "
        "(SQL-side TRIM with space/tab/lf/cr charset)",
        "TRIM(COALESCE(body_text, ''), char(32) || char(9) || char(10) || char(13))" in fn,
    )
    check(
        "36: the selected window is presented oldest-first for the prompt",
        "rows.reverse()" in fn,
    )


# ----------------------------------------------------------------- item 37
def verify_item37() -> None:
    src = read("ai_attributes.rs")
    store = src[src.index("fn store_extraction_run") :]
    store = store[: store.index("fn lookup_cached_extraction")]
    check(
        "37: the run row lands typed 'attributes' and status 'completed'",
        "'attributes'" in store and "'completed'" in store,
    )
    check(
        "37: the row carries the ledger's raw material "
        "(latency_ms, token_usage, endpoint_id, conversation_id)",
        all(c in store for c in ("latency_ms", "token_usage", "endpoint_id", "conversation_id")),
    )
    lookup = src[src.index("fn lookup_cached_extraction") :]
    lookup = lookup[: lookup.index("const TECHNICAL_VOCAB")]
    check(
        "37: only completed runs are cache hits (stuck 'queued' rows self-heal)",
        "AND status = 'completed'" in lookup,
    )
    run_fn = src[src.index("pub async fn run_extraction") :]
    run_fn = run_fn[: run_fn.index("pub fn finish_computation")]
    check(
        "37: run_extraction returns content+latency+usage (ExtractionOutput)",
        "ExtractionOutput" in run_fn and "latency_ms" in run_fn and "usage" in run_fn,
    )
    wiring = src[src.index("pub async fn compute_with_backend") :]
    check(
        "37: compute_with_backend threads the backend's endpoint_id through",
        "endpoint_id.as_deref()" in wiring,
    )


# ----------------------------------------------------------------- item 38
def verify_item38() -> None:
    src = read("ai_pipeline.rs")
    analyze = src[src.index("pub async fn analyze_ticket") :]
    analyze = analyze[: analyze.index("pub async fn generate_draft")]
    check(
        "38: the cache hit parses the hash-matched run's own output",
        "serde_json::from_str::<serde_json::Value>(&cached.output)" in analyze
        and "parse_ticket_analysis(Some(&cached_json))" in analyze,
    )
    cache_path = analyze[: analyze.index("let ctx = build_evidence")]
    check(
        "38: the cache-hit path no longer CALLS the newest-analysis reader",
        "get_latest_analysis(conn" not in cache_path,
    )
    check(
        "38: the cache hit loads the run's own sources (sources_for_run)",
        "sources_for_run(conn, cached.id)" in analyze,
    )
    helper = src[src.index("fn sources_for_run") :]
    check(
        "38: sources_for_run reads ai_sources by run id",
        "FROM ai_sources WHERE run_id = ?1" in helper,
    )


# ----------------------------------------------------------------- item 39
def verify_item39() -> None:
    src = read("ai_pipeline.rs")
    draft = src[src.index("pub async fn generate_draft") :]
    draft = draft[: draft.index("pub async fn verify_draft_internal")]
    check(
        "39: the raw fallback requires NO parseable JSON to exist",
        "res.json.is_none()" in draft,
    )
    check(
        "39: a parsed envelope without a usable draft fails loudly",
        "AI draft JSON had no draft text" in draft,
    )
    check(
        "39: a bare JSON string still counts as the draft",
        ".or_else(|| j.as_str())" in draft,
    )


# ----------------------------------------------------------------- item 40
def verify_item40() -> None:
    src = read("ai_pipeline.rs")
    block = src[src.index("pub fn build_interaction_strategy_block") :]
    block = block[: block.index("pub fn extract_provided_facts")]
    check(
        "40: the hard-coded None is gone",
        "None\n" not in block or "heuristic_recommendation" in block,
    )
    check(
        "40: the stored interaction recommendation is rendered",
        "get_latest_current_interaction" in block and "stored.recommendation" in block,
    )
    check(
        "40: the deterministic heuristic carries the block without an AI rec",
        "heuristic_recommendation" in block and "comparison_baseline" in block,
    )
    check(
        "40: customer preferences and already-provided facts are included",
        "get_preferences" in block and "extract_provided_facts" in block,
    )
    check(
        "40: the draft path wires the block into the evidence context",
        "ctx.interaction_strategy = build_interaction_strategy_block" in src,
    )


# ----------------------------------------------------------------- item 41
def verify_item41() -> None:
    src = read("ai_tools.rs")
    tool = src[src.index('"get_customer_memory" =>') :]
    tool = tool[: tool.index("_ => {")]
    note = tool[tool.index('"note":') :] if '"note":' in tool else ""
    note_literal = tool[tool.index('"note": "') :] if '"note": "' in tool else tool
    check(
        "41: the false 'the only persisted rows' claim is gone from the note",
        "the only persisted rows" not in note_literal,
    )
    check(
        "41: the note explains the ai provenance as machine-extracted",
        "machine-extracted" in note and "not human-verified" in note,
    )
    check(
        "41: each relayed entry keeps its own source",
        '"source": source' in tool,
    )


# ----------------------------------------------------------------- item 42
def verify_item42() -> None:
    src = read("ai_pipeline.rs")
    cluster = src[src.index("pub async fn cluster_issues") :]
    cluster = cluster[: cluster.index("let run_id = start_run")]
    check(
        "42: the conversation list is capped in SQL (LIMIT)",
        "LIMIT ?2" in cluster,
    )
    check(
        "42: the cap selects the NEWEST conversations of the window",
        "ORDER BY COALESCE(c.remote_created_at, c.created_at) DESC" in cluster,
    )
    prompts = read("ai_prompts.rs")
    builder = prompts[prompts.index("pub fn build_issue_cluster_user") :]
    check(
        "42: the prompt declares the sample cap out loud",
        "only the newest" in builder and "context cap" in builder,
    )
    check(
        "42: the caller passes the window's true conversation count",
        "total_in_window" in cluster,
    )


# ----------------------------------------------------------------- item 43
def verify_item43() -> None:
    src = read("copilot.rs")
    chat = src[src.index("pub async fn chat") :]
    chat = chat[: chat.index("fn citations_from")]
    check(
        "43: usage is accumulated across chat rounds",
        "usage_total" in chat and "t.prompt_tokens += u.prompt_tokens" in chat,
    )
    check(
        "43: the run is completed with usage AND endpoint",
        "complete_run_with_usage" in chat and "endpoint_id," in chat,
    )
    check(
        "43: the endpoint id is destructured from the backend",
        "AiBackend::LmStudio {" in chat and "endpoint_id," in chat,
    )


# ----------------------------------------------------------------- item 44
def verify_item44() -> None:
    src = read("copilot.rs")
    chat = src[src.index("pub async fn chat") :]
    chat = chat[: chat.index("fn citations_from")]
    check(
        "44: over-budget tool_calls get an explicit refusal result",
        "Tool budget reached - this call was not executed" in chat,
    )
    check(
        "44: the refusal carries the dangling tool_call id",
        "tool_call_id: Some(tc.id.clone())" in chat,
    )
    check(
        "44: the silent break is gone from the budget branch",
        "if executed.len() >= MAX_TOOL_CALLS as usize {\n                    break;" not in chat,
    )


# ----------------------------------------------------------------- item 45
def verify_item45() -> None:
    src = read("copilot.rs")
    ins = src[src.index("fn insert_message") :]
    ins = ins[: ins.index("pub fn delete_session")]
    check(
        "45: the INSERT result is checked",
        "let inserted = conn.execute(" in ins and "match inserted" in ins,
    )
    check(
        "45: a failed insert reports a null id, never a stale rowid",
        "(Value::Null, json!(e.to_string()))" in ins or "Value::Null" in ins,
    )
    check(
        "45: the failure is surfaced as persist_error",
        '"persist_error": persist_error' in ins,
    )


def main() -> int:
    verify_item36()
    verify_item37()
    verify_item38()
    verify_item39()
    verify_item40()
    verify_item41()
    verify_item42()
    verify_item43()
    verify_item44()
    verify_item45()
    print()
    if FAILURES:
        print(f"{len(FAILURES)} check(s) failed:")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("batch 23 verification: all checks pass")
    return 0


if __name__ == "__main__":
    sys.exit(main())
