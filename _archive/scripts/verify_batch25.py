#!/usr/bin/env python3
"""Batch 25 verification harness (clean-audit items 57-66).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..24 harness protocol.

Items:
  57  new_conversation + customer_reply both fired on every customer
      thread, no once-guard (automation.rs + workers.rs; m060 column)
  58  knowledge FTS lane never deduped per document (chunk rows flooded
      the LIMIT; one multi-chunk doc crowded out the rest)
  59  fused RRF scores sorted against FTS-only constant-1.0 scores
      (the "semantic" hits sank to the bottom of a fused response)
  60  orphaned vectors never cleaned; stale chunk text kept matching
      (re-chunk lanes replace chunk ids; the old points stayed forever)
  61  removing a type field bricked property updates of existing objects
      (stored stale keys failed validate_properties forever)
  62  parse_mentions diverged from scan_for_mentions (ASCII-only tokens,
      no left boundary, @team: prefixed form dropped)
  63  rm_tags is a no-op in ammonia 4.x (title/textarea content leaked
      visible — the tags were stripped, their text was not)
  64  rebuild_indexes double-inserted every thread into fts_threads
  65  notification sweep advanced the cursor past unprocessed events
      (>500 backlogs were skipped forever)
  66  textutil: bare '<' swallowed the rest of the body; sequential
      entity replaces double-decoded (&amp;lt; became <)
"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORE = ROOT / "crates" / "core" / "src"
SHELL = ROOT / "crates" / "app-shell" / "src"
FAILURES: list[str] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    status = "OK " if ok else "FAIL"
    print(f"[{status}] {name}" + (f" — {detail}" if detail and not ok else ""))
    if not ok:
        FAILURES.append(f"{name}: {detail}")


def read(root: Path, p: str) -> str:
    return (root / p).read_text(encoding="utf-8")


# ----------------------------------------------------------------- item 57
def verify_item57() -> None:
    workers = read(CORE, "workers.rs")
    fn = workers[workers.index("fn on_conversation_changed") :]
    fn = fn[: fn.index("async fn run_engine")]
    check(
        "57: on_conversation_changed no longer fires BOTH triggers",
        fn.count('fire_trigger_for_conversation(') == 1,
        f"expected exactly one fire call, found {fn.count('fire_trigger_for_conversation(')}",
    )
    check(
        "57: the once-guard decision comes from the shared helper",
        "customer_thread_trigger_decision(" in fn,
    )
    check(
        "57: the fired thread id is recorded on the conversation",
        "automation_last_customer_thread_id = ?1" in fn,
    )
    check(
        "57: the AI-enqueue block still runs on customer-latest changes",
        '"analyze_ticket"' in fn,
    )
    automation = read(CORE, "automation.rs")
    check(
        "57: the decision helper exists (new_conversation once, then customer_reply)",
        'Some(_) if last_fired.is_none() => Some("new_conversation")' in automation
        and 'Some(_) => Some("customer_reply")' in automation,
    )
    check(
        "57: an already-consumed thread fires nothing",
        "Some(thread) if last_fired == Some(thread) => None" in automation,
    )
    bootstrap = read(CORE, "bootstrap.rs")
    check(
        "57: migration m060 registered as boot step 51",
        'version: 51' in bootstrap and "m060_automation_once_guard" in bootstrap,
    )
    check(
        "57: m060 adds the once-guard column",
        "automation_last_customer_thread_id" in automation
        and "apply_m060_automation_once_guard" in automation,
    )


# ----------------------------------------------------------------- item 58
def verify_item58() -> None:
    src = read(CORE, "search.rs")
    fn = src[src.index("pub fn search_knowledge_raw") :]
    fn = fn[: fn.index("pub fn search_knowledge(")]
    check(
        "58: the chunk-level LIMIT is gone from the SQL",
        "LIMIT ?{limit_idx}" not in fn,
        "the per-chunk LIMIT would still cap chunk rows, not documents",
    )
    check(
        "58: rows are consumed rank-ordered and deduped per document",
        "seen_documents" in fn and "seen_documents.insert(hit.document_id)" in fn,
    )
    check(
        "58: collection stops at the requested number of DISTINCT documents",
        "hits.len() >= limit" in fn,
    )
    check(
        "58: ORDER BY rank is preserved (best chunk speaks for its document)",
        "ORDER BY rank" in fn,
    )


# ----------------------------------------------------------------- item 59
def verify_item59() -> None:
    src = read(CORE, "search.rs")
    fn = src[src.index("let fused_any = !fused.is_empty();") :]
    fn = fn[: fn.index("pub fn universal_search")]
    check(
        "59: the FTS-only lanes are re-scored when fusion ran",
        "if fused_any {\n        rescore_fts_only_lanes_to_rrf_scale(&mut response.hits, k);\n    }"
        in fn,
    )
    helper = src[src.index("fn rescore_fts_only_lanes_to_rrf_scale") :]
    helper = helper[: helper.index("pub fn universal_search")]
    lanes = 'const FTS_ONLY_LANES: [&str; 4] = ["customers", "issues", "saved_replies", "ai"];'
    check(
        "59: exactly the four never-fused lanes are re-scored",
        lanes in helper,
    )
    check(
        "59: the re-score uses the RRF formula by lane rank",
        "1.0 / (f64::from(k) + *rank as f64 + 1.0)" in helper,
    )


# ----------------------------------------------------------------- item 60
def verify_item60() -> None:
    vec = read(CORE, "vectorstore_sqlitevec.rs")
    check(
        "60: the store grows a prune_family_except sweep",
        "pub fn prune_family_except" in vec,
    )
    check(
        "60: the sweep deletes in bounded batches",
        "orphans.chunks(500)" in vec,
    )
    check(
        "60: both halves go (vec table and payload table)",
        vec.count("DELETE FROM vec_payload") >= 2,
    )
    check(
        "60: the VectorIndex facade exposes the sweep",
        "pub fn prune_orphaned_points" in vec,
    )
    emb = read(CORE, "embeddings.rs")
    check(
        "60: the sweep runs from the embed pipeline",
        "prune_orphaned_vectors(conn, table, qdrant);" in emb,
    )
    sweep_fn = emb[emb.index("fn prune_orphaned_vectors") :]
    sweep_fn = sweep_fn[: sweep_fn.index("fn embed_pending_chunks")]
    check(
        "60: the live set is the namespaced point ids of the live chunks",
        "namespaced_chunk_point_id(entity_type, *chunk_id).ok()" in sweep_fn,
    )
    check(
        "60: the sweep runs before the pending check (pure deletions swept)",
        emb.index("prune_orphaned_vectors(conn, table, qdrant);")
        < emb.index("let pending = list_pending_chunks(conn, table)?;"),
    )
    knowledge_page = read(SHELL, "pages/knowledge.rs")
    check(
        "60: the knowledge delete enqueues an embed pass for the orphan sweep",
        '"embed_knowledge_chunks"' in knowledge_page
        and knowledge_page.index("pub fn delete_document")
        < knowledge_page.index('"embed_knowledge_chunks"', knowledge_page.index("pub fn delete_document")),
        "the UI delete path must schedule the sweep",
    )


# ----------------------------------------------------------------- item 61
def verify_item61() -> None:
    src = read(CORE, "custom_objects.rs")
    fn = src[src.index("pub fn patch_object") :]
    fn = fn[: fn.index("pub fn delete_object")]
    check(
        "61: stored keys of removed fields are dropped before validation",
        "live_fields" in fn and "merged.retain(" in fn,
    )
    check(
        "61: the retain keeps keys present in the type's current fields",
        "type_detail.fields.iter().map(|f| f.key.as_str())" in fn,
    )
    check(
        "61: the patch input itself is still merged and validated strictly",
        "validate_properties(&type_detail.fields, &merged)" in fn,
    )
    validate = src[src.index("pub fn validate_properties") :]
    validate = validate[: validate.index("fn valid_iso_date")]
    check(
        "61: unknown keys in the INPUT still fail validation",
        'return Err(format!("Unrecognized key: \\"{key}\\""));' in validate,
    )


# ----------------------------------------------------------------- item 62
def verify_item62() -> None:
    src = read(CORE, "mentions.rs")
    check(
        "62: token chars are Unicode (scanner \\w parity)",
        "c.is_alphanumeric() || c == '.' || c == '_' || c == '-'" in src
        and "is_ascii_alphanumeric() || c == '.'" not in src,
    )
    parse = src[src.index("pub fn parse_mentions") :]
    parse = parse[: parse.index("pub fn team_members")]
    check(
        "62: the left boundary check skips @ glued to word chars (emails)",
        "is_preceded_by_word_char(body, at)" in parse,
    )
    check(
        "62: the @team: prefixed form is resolved",
        'starts_with("team:")' in parse and "is_team_prefix_char" in parse,
    )
    check(
        "62: the prefixed run is consumed even when unresolved",
        "search_from = run_start + run.len();" in parse,
    )
    check(
        "62: a bare '@team' never resolves as a USER mention",
        'if lower != "team"' in parse,
    )


# ----------------------------------------------------------------- item 63
def verify_item63() -> None:
    src = read(CORE, "richtext.rs")
    fn = src[src.index("fn sanitize(html: &str)") :]
    fn = fn[: fn.index("struct ListState")]
    check(
        "63: the no-op rm_tags call is gone",
        ".rm_tags(" not in fn,
        "rm_tags only removes tags from the allowlist — a no-op here",
    )
    check(
        "63: the scripty family is routed through clean_content_tags",
        ".add_clean_content_tags([" in fn,
    )
    check(
        "63: the leaked-content tags are in the content-removal list",
        all(t in fn for t in ('"title"', '"textarea"', '"noscript"', '"select"')),
    )


# ----------------------------------------------------------------- item 64
def verify_item64() -> None:
    src = read(CORE, "search.rs")
    fn = src[src.index("fn rebuild_indexes_tx") :]
    fn = fn[: fn.index("fn rebuild_custom_objects_fts")]
    check(
        "64: the redundant bulk INSERT INTO fts_threads is gone",
        "INSERT INTO fts_threads" not in fn,
    )
    check(
        "64: the per-conversation loop remains the single thread writer",
        "index_conversation_fts(conn, *id)?;" in fn,
    )
    check(
        "64: the fts_indexed bookkeeping UPDATE stays",
        "UPDATE conversation_threads SET fts_indexed = 1" in fn,
    )


# ----------------------------------------------------------------- item 65
def verify_item65() -> None:
    src = read(CORE, "notification_sweep.rs")
    sweep = src[src.index("pub fn sweep(") :]
    sweep = sweep[: sweep.index("struct EventRow")]
    check(
        "65: the steady-state cursor advance uses the computed stop, not max_event_id",
        "crate::settings::set_i64(conn, CURSOR_KEY, next_cursor)?" in sweep,
    )
    check(
        "65: the cursor parks where the event read stopped",
        "cap.min(ceiling).max(cursor)" in sweep,
    )
    check(
        "65: the max_event_id anchor survives only as the first-run initializer",
        sweep.count("set_i64(conn, CURSOR_KEY, max_event_id(conn))?") == 1
        and sweep.index("set_i64(conn, CURSOR_KEY, max_event_id(conn))?")
        < sweep.index("set_i64(conn, CURSOR_KEY, next_cursor)?"),
    )
    events = src[src.index("fn sweep_conversation_events") :]
    events = events[: events.index("fn sweep_sla")]
    check(
        "65: the read cap is an explicit named constant",
        "const EVENT_SWEEP_CAP: usize = 500;" in src,
    )
    check(
        "65: the event read fetches one row past the cap",
        "(EVENT_SWEEP_CAP + 1) as i64" in events,
    )
    check(
        "65: a capped read truncates to the cap",
        "events.truncate(EVENT_SWEEP_CAP);" in events,
    )
    check(
        "65: the sweep reports where its read stopped",
        "Ok((created, processed_to))" in events,
    )


# ----------------------------------------------------------------- item 66
def verify_item66() -> None:
    src = read(CORE, "textutil.rs")
    check(
        "66: the sequential replace chain is gone (double decode source)",
        '.replace("&amp;", "&")' not in src,
    )
    check(
        "66: the tag-start heuristic gates tag mode",
        "starts_tag" in src and "Some(next) if starts_tag(*next) => in_tag = true" in src,
    )
    check(
        "66: a bare '<' is emitted as literal text",
        "_ => out.push('<')," in src,
    )
    check(
        "66: the entity decode is single-pass (known entities only)",
        "fn decode_entity(candidate: &str) -> Option<char>" in src,
    )
    check(
        "66: a never-closing tag is flushed back as text",
        "MAX_TAG_BYTES" in src and "out.push_str(&tag_name);" in src,
    )


def main() -> int:
    print("Batch 25 (clean-audit items 57-66) — source-level verification\n")
    verify_item57()
    verify_item58()
    verify_item59()
    verify_item60()
    verify_item61()
    verify_item62()
    verify_item63()
    verify_item64()
    verify_item65()
    verify_item66()
    print()
    if FAILURES:
        print(f"{len(FAILURES)} check(s) FAILED:")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("All checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
