#!/usr/bin/env python3
"""Batch 28 verification harness (clean-audit items 87-96).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..27 harness protocol.

Items:
  87  ratings upsert never re-links conversation/customer on conflict
  88  cancelled initial sync recorded LIVE; threads backfill never re-runs
  89  threads pass hardcodes failed:0 then gets stamped ok
  90  post-initial-sync AI backfill covers only first 50 conversations
  91  platform-side deletions never propagate (replies/orgs/workflows)
  92  dead "-> 404" mapping in get_rating
  93  OAuth TTL sweeper + redirect-URI verifier dead; states never expire
  94  webhook prune deletes unprocessed pending events under backlog
  95  weekly_digest completes the ledger row despite failed narrative
  96  empty {} analysis accepted+cached; eval gate disagrees
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


# ----------------------------------------------------------------- item 87
def verify_item87() -> None:
    workers = read(CORE, "workers.rs")
    check(
        "87: the conflict path re-links the conversation",
        "conversation_id = COALESCE(excluded.conversation_id, ratings.conversation_id)"
        in workers,
    )
    check(
        "87: the conflict path re-links the customer",
        "customer_local_id = COALESCE(excluded.customer_local_id, ratings.customer_local_id)"
        in workers,
    )
    check(
        "87: the conflict path re-links the acting user",
        "user_local_id = COALESCE(excluded.user_local_id, ratings.user_local_id)" in workers,
    )
    check(
        "87: the bare three-column conflict update is gone",
        "ON CONFLICT(remote_id) DO UPDATE SET\n            rating = excluded.rating,\n"
        "            comments = excluded.comments,\n            last_synced_at = datetime('now')"
        not in workers,
    )


# ----------------------------------------------------------------- item 88
def verify_item88() -> None:
    engine = read_code(CORE, "sync_engine.rs")
    check(
        "88: the initial-sync verdict checks cancellation first",
        "let cancelled = self.cancelled();" in engine,
    )
    check(
        "88: a cancelled run lands CATCHING_UP, not LIVE",
        'let new_state = if cancelled {\n                    "CATCHING_UP"\n                } else if failed == 0 {'
        in engine.replace("    ", "    ").replace("\n    ", "\n") or
        '"CATCHING_UP"\n                } else if failed == 0' in engine,
    )
    check(
        "88: the run row records CANCELLED",
        'state: Some(if cancelled {\n                            "CANCELLED"' in engine
        or '"CANCELLED"' in engine,
    )
    check(
        "88: the incremental runs the threads backfill",
        'let _ = self.finish_resource("threads", r, &mut results);' in engine,
    )
    check(
        "88: the threads stage rides after the conversations stage",
        engine.index('self.finish_resource("conversations", r, &mut results);')
        < engine.index('self.finish_resource("threads", r, &mut results);'),
        "threads stage must follow the conversations stage in incremental_sync",
    )
    check(
        "88: the incremental progress total counts the third extra stage",
        "INCREMENTAL_REFERENCE_ORDER.len() as u32 + 3" in engine,
    )
    check(
        "88: recovery counts a CANCELLED run as data-on-record",
        "'LIVE','ERROR','CATCHING_UP','CANCELLED'" in engine,
    )


# ----------------------------------------------------------------- item 89
def verify_item89() -> None:
    engine = read_code(CORE, "sync_engine.rs")
    check(
        "89: the threads pass counts failures",
        "let mut failed = 0i64;" in engine,
    )
    check(
        "89: an unreachable conversation increments the count",
        "failed += 1;" in engine,
    )
    check(
        "89: the summarized failure rides the resource result",
        '"{failed} conversation(s) could not be re-fetched for thread backfill"' in engine,
    )
    check(
        "89: the hardcoded clean result is gone",
        "            resource,\n            processed,\n            failed: 0,\n            error: None,\n        })\n    }\n\n    /// `trigger`"
        not in engine,
    )
    check(
        "89: the per-row record_failure inside the pass is gone",
        "record_failure(&conn, &resource, &e.to_string(), 1);" not in engine,
    )


# ----------------------------------------------------------------- item 90
def verify_item90() -> None:
    workers = read_code(CORE, "workers.rs")
    check(
        "90: the ticket_analysis backfill is uncapped",
        "AND a.status = 'completed')\n                        AND c.deleted_at IS NULL\n                      ORDER BY c.id\","
        in workers,
    )
    check(
        "90: the attributes backfill is uncapped",
        "AND a.superseded_at IS NULL)\n                        AND c.deleted_at IS NULL\n                      ORDER BY c.id\","
        in workers,
    )
    check(
        "90: no LIMIT 50 remains on the backfill queries",
        "ORDER BY c.id LIMIT 50" not in workers,
    )


# ----------------------------------------------------------------- item 91
def verify_item91() -> None:
    engine = read_code(CORE, "sync_engine.rs")
    check(
        "91: the shared prune helper exists",
        "fn prune_missing_rows(conn: &Connection, table: &str, seen: &[i64]) -> Result<u64>"
        in engine,
    )
    check(
        "91: the prune stamps deleted_at only on unseen rows",
        "WHERE deleted_at IS NULL AND remote_id NOT IN ({placeholders})" in engine,
    )
    check(
        "91: an empty list never wipes the mirror",
        "if seen.is_empty() {\n        return Ok(0);\n    }" in engine,
    )
    for table in ("organizations", "saved_replies", "workflows"):
        check(
            f"91: the {table} arm prunes platform-side deletions",
            f'prune_missing_rows(&conn, "{table}", &seen)?' in engine,
        )
    check(
        "91: a re-listed organization comes back to life",
        "last_seen_at = datetime('now'), last_synced_at = datetime('now'),\n           deleted_at = NULL"
        in engine,
    )
    check(
        "91: a re-listed saved reply comes back to life",
        "last_synced_at = datetime('now'), deleted_at = NULL" in engine,
    )
    hs = read_code(CORE, "helpscout_real.rs")
    check(
        "91: saved-reply listings walk every page before pruning",
        'self.list_all_v2(\n            &format!("/v2/mailboxes/{mailbox_id}/saved-replies"),\n            "savedReplies",'
        in hs,
    )


# ----------------------------------------------------------------- item 92
def verify_item92() -> None:
    hs = read_code(CORE, "helpscout_real.rs")
    check(
        "92: get_rating maps not-found through the structured check",
        "async fn get_rating(&self, rating_id: i64) -> Result<Option<HsRating>>" in hs
        and "if is_not_found(&e) {\n                    return Ok(None);\n                }" in hs,
    )
    # The dead literal mapping must not survive in get_rating's body.
    rating_start = hs.index("async fn get_rating")
    rating_end = hs.index("async fn ", rating_start + 10)
    body = hs[rating_start:rating_end]
    check(
        "92: the dead '-> 404' string match is gone from get_rating",
        'msg.contains("-> 404")' not in body,
    )


# ----------------------------------------------------------------- item 93
def verify_item93() -> None:
    oauth = read_code(CORE, "oauth_state.rs")
    check(
        "93: the state TTL constant exists",
        "pub const STATE_TTL_SECONDS: i64 = 600;" in oauth,
    )
    check(
        "93: consume_state enforces freshness",
        "AND (julianday('now') - julianday(created_at)) * 86400.0 < ?2" in oauth,
    )
    check(
        "93: the sweeper covers unconsumed states too",
        "julianday(COALESCE(consumed_at, created_at))" in oauth,
    )
    check(
        "93: the old consumed-only sweeper signature is gone",
        "fn sweep_consumed(conn: &Connection, older_than_seconds: i64)" not in oauth,
    )
    loopback = read_code(CORE, "loopback.rs")
    check(
        "93: begin_oauth wires the sweeper",
        "db.with_write(|conn| crate::oauth_state::sweep_states(conn))" in loopback,
    )
    check(
        "93: the callback verifies the recorded redirect URI",
        "crate::oauth_state::redirect_uri_for(conn, state_param)" in loopback,
    )
    check(
        "93: the mismatch rejects before the code exchange",
        loopback.index("redirect_uri_for") < loopback.index("match oauth.complete(code)"),
    )
    check(
        "93: the listener carries its own port for verification",
        "pub port: u16," in loopback,
    )


# ----------------------------------------------------------------- item 94
def verify_item94() -> None:
    webhook = read_code(CORE, "webhook.rs")
    check(
        "94: the prune exempts rows that still owe processing",
        "WHERE NOT (processing_state IN ('pending','failed') AND attempts < 5)" in webhook,
    )
    check(
        "94: the keep window still bounds history",
        "AND id NOT IN (SELECT id FROM webhook_events ORDER BY id DESC LIMIT ?1)" in webhook,
    )
    check(
        "94: the state-blind prune is gone",
        "DELETE FROM webhook_events WHERE id NOT IN (SELECT id FROM webhook_events ORDER BY id DESC LIMIT ?1)"
        not in webhook,
    )


# ----------------------------------------------------------------- item 95
def verify_item95() -> None:
    pipeline = read_code(CORE, "ai_pipeline.rs")
    digest_start = pipeline.index("pub async fn weekly_digest")
    digest_end = pipeline.index("#[cfg(test)]", digest_start)
    body = pipeline[digest_start:digest_end]
    check(
        "95: a usable narrative completes the ledger row",
        "let _ = complete_run_with_usage(\n                                conn,\n                                rid," in body
        or "complete_run_with_usage(" in body,
    )
    check(
        "95: an unusable narrative FAILS the ledger row",
        '"the model returned no usable narrative for the digest"' in body,
    )
    check(
        "95: the verdict match precedes any ledger write",
        body.index("match narrative {") < body.index("complete_run_with_usage("),
        "the completion must live inside the verdict match, never before it",
    )
    check(
        "95: the unconditional pre-verdict completion is gone",
        "let _ = complete_run_with_usage(\n                        conn,\n                        rid,\n                        &serde_json::json!({ \"narrative\": narrative }),"
        not in body,
    )


# ----------------------------------------------------------------- item 96
def verify_item96() -> None:
    pipeline = read_code(CORE, "ai_pipeline.rs")
    check(
        "96: the core-content gate helper exists",
        "fn analysis_has_core_content(a: &TicketAnalysis) -> bool" in pipeline,
    )
    check(
        "96: the gate mirrors the eval's core fields",
        "a.summary.as_deref(),\n        a.intent.as_deref(),\n        a.customer_goal.as_deref(),"
        in pipeline,
    )
    check(
        "96: the pipeline rejects an all-empty analysis",
        "if !analysis_has_core_content(&analysis) {" in pipeline,
    )
    check(
        "96: the rejected run fails with an honest reason",
        '"AI analysis returned no usable core field (summary/intent/customer_goal all empty)"'
        in pipeline,
    )
    check(
        "96: the cache path applies the same gate",
        "&& let Some(analysis) = parse_ticket_analysis(Some(&cached_json))\n        && analysis_has_core_content(&analysis)"
        in pipeline,
    )


def main() -> int:
    for verify in (
        verify_item87,
        verify_item88,
        verify_item89,
        verify_item90,
        verify_item91,
        verify_item92,
        verify_item93,
        verify_item94,
        verify_item95,
        verify_item96,
    ):
        verify()
    print()
    if FAILURES:
        print(f"{len(FAILURES)} check(s) FAILED:")
        for failure in FAILURES:
            print(f"  - {failure}")
        return 1
    print("All batch-28 checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
