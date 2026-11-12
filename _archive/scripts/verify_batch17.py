#!/usr/bin/env python3
"""Batch 17 verification harness (clean-audit items 6-10).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16 harness protocol
(scripts/verify_gate_fix.py).

Items:
  6  (A1)     worker-restart tick-guard stranding  -> TickGuard RAII + start() resets
  7  (A2+J2)  writer lane across LLM round-trips  -> DbLongOpGuard at every pipeline site
  8  (A3+B2)  duplicate sends on ambiguous failure -> replay-safety policy in request_inner
  9  (C1)     raw_json_hash never written          -> upsert writer + all-thread signature fold
  10 (D1)     LM Studio probes without API key     -> bearer_auth on is_available/list_models
"""

import re
import sys
from pathlib import Path

REPO = Path("/home/z/my-project/spp-dev/crates/core/src")
FAILURES: list[str] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    status = "OK " if ok else "FAIL"
    print(f"[{status}] {name}" + (f" — {detail}" if detail and not ok else ""))
    if not ok:
        FAILURES.append(f"{name}: {detail}")


def read(p: str) -> str:
    return (REPO / p).read_text(encoding="utf-8")


# ---------------------------------------------------------------- item 6 (A1)
def verify_item6() -> None:
    src = read("workers.rs")
    check("6: TickGuard RAII struct defined", "struct TickGuard<'a>" in src)
    check(
        "6: TickGuard::acquire sets via swap and returns None when already set",
        "(!flag.swap(true, Ordering::SeqCst)).then_some(Self { flag })" in src,
    )
    check(
        "6: TickGuard Drop clears the flag (panic/abort-safe release)",
        re.search(
            r"impl Drop for TickGuard<'_> \{\s*"
            r"fn drop\(&mut self\) \{\s*self\.flag\.store\(false, Ordering::SeqCst\);",
            src,
        )
        is not None,
        "Drop impl must store(false)",
    )
    start = src[src.index("pub fn start(") : src.index("pub fn stop(")]
    for flag in (
        "processing",
        "sweeping_notifications",
        "sweeping_customer_events",
        "refreshing_connectors",
    ):
        check(
            f"6: start() resets {flag}",
            f"self.{flag}.store(false, Ordering::SeqCst);" in start,
        )
    # The old happy-path-only pattern must be gone from every tick entry:
    for fn, flag in (
        ("pub async fn tick(&self)", "processing"),
        ("pub fn notification_sweep_tick(&self)", "sweeping_notifications"),
        ("pub fn customer_event_sweep_tick(&self)", "sweeping_customer_events"),
        ("async fn connector_refresh_tick(&self)", "refreshing_connectors"),
    ):
        body = src[src.index(fn) : src.index(fn) + 900]
        check(
            f"6: {fn.split('(')[0].split()[-1]} acquires via TickGuard::acquire(&self.{flag})",
            f"TickGuard::acquire(&self.{flag})" in body,
            "entry must use the RAII latch",
        )
        check(
            f"6: {fn.split('(')[0].split()[-1]} has no orphan swap(true) guard",
            f"self.{flag}.swap(true, Ordering::SeqCst)" not in body,
        )
        check(
            f"6: {fn.split('(')[0].split()[-1]} has no happy-path-only trailing store(false)",
            f"self.{flag}.store(false, Ordering::SeqCst);" not in body,
        )


# ------------------------------------------------------------- item 7 (A2+J2)
def verify_item7() -> None:
    src = read("workers.rs")
    db = read("db.rs")
    check("7: DbLongOpGuard defined in db.rs", "pub struct DbLongOpGuard<'a>" in db)
    check(
        "7: long_op_guard() opens a fresh tuned side connection for file-backed DBs",
        "long_op_guard" in db and "let Ok(conn) = open(&path)" in db,
    )
    check(
        "7: DbLongOpGuard Derefs to Connection (call sites unchanged)",
        "impl Deref for DbLongOpGuard<'_>" in db and "impl DerefMut for DbLongOpGuard<'_>" in db,
    )
    # Every former pipeline-lane site now uses long_op_guard.
    long_sites = src.count("db.long_op_guard()")
    check(
        "7: all 14 writer-lane-across-work sites now use db.long_op_guard()",
        long_sites == 14,
        f"found {long_sites} sites, expected 14",
    )
    # No pipeline site still takes the UI writer lane.
    check(
        "7: zero remaining 'db.write_guard()' acquisitions in workers.rs",
        re.search(r"(?<!self\.)db\.write_guard\(\)", src) is None,
        "every background long-op site must be off the writer lane",
    )
    check(
        "7: WorkerManager::lock (short UI-lane ops) still uses the writer lane",
        "self.db.write_guard()" in src,
    )
    # Every long_op_guard site must sit directly above a block_ai_pipeline
    # call or the deterministic gap scan — i.e. no other pattern introduced.
    pairs = len(re.findall(r"let (?:mut )?conn = db\.long_op_guard\(\);", src))
    check(
        "7: guard variable binding intact at all sites",
        pairs == 14,
        f"bound sites: {pairs}, expected 14",
    )


# ------------------------------------------------------------- item 8 (A3+B2)
def verify_item8() -> None:
    src = read("helpscout_real.rs")
    inner = src[src.index("async fn request_inner") :]
    check(
        "8: replay-safety policy computed (POST = non-idempotent)",
        'let idempotent = !method.eq_ignore_ascii_case("POST");' in inner,
    )
    check(
        "8: in-client transport retry only when provably undelivered or replay-safe",
        "let never_delivered = e.is_connect();" in inner
        and "(never_delivered || idempotent)" in inner,
    )
    check(
        "8: ambiguous create transport failure is NOT retried in-client",
        "msg.contains(\"timeout\")" not in inner
        and "msg.contains(\"Connection refused\")" not in inner
        and "msg.contains(\"error sending request\")" not in inner,
        "the old string-match blind retry must be gone",
    )
    check(
        "8: ambiguous create transport failure is NOT requeued (retryable=false)",
        "retryable: never_delivered || idempotent," in inner,
    )
    check(
        "8: 5xx in-client retry only for replay-safe methods",
        "is_retryable(status) && retries_left > 0 && status != 429 && idempotent" in inner,
    )
    check(
        "8: 5xx requeue only for replay-safe methods (429 stays safe)",
        "(status == 429 || idempotent)" in inner,
    )
    check(
        "8: honest user-facing ambiguity message for failed creates",
        "Delivery could not be confirmed" in inner,
    )
    workers = read("workers.rs")
    check(
        "8: outbound job layer still consumes hs.retryable for requeue decisions",
        ".map(|hs| hs.retryable)" in workers and "outbound::mark_failed" in workers,
    )


# ------------------------------------------------------------------ item 9 (C1)
def verify_item9() -> None:
    sync = read("sync_engine.rs")
    check(
        "9: upsert_thread computes a content hash of the ingested thread",
        "sha256_hex(serde_json::to_string(t).unwrap_or_default().as_bytes())" in sync,
    )
    upsert = sync[sync.index("fn upsert_thread") : sync.index("fn upsert_thread") + 2600]
    check(
        "9: INSERT writes raw_json_hash column",
        "raw_json_hash)" in upsert and "?11" in upsert,
    )
    check(
        "9: ON CONFLICT update propagates raw_json_hash on in-place edits",
        "raw_json_hash = excluded.raw_json_hash" in upsert,
    )
    pipe = read("ai_pipeline.rs")
    sig = pipe[
        pipe.index("pub fn get_analysis_signature") : pipe.index("pub struct LatestAnalysis")
    ]
    check(
        "9: signature folds EVERY live thread's hash (no LIMIT 1 latest-row read)",
        "ORDER BY id" in sig and "LIMIT 1" not in sig,
    )
    check(
        "9: signature is a hash of the folded hashes (any-thread edit invalidates)",
        "hasher.update(hash.as_bytes());" in sig,
    )
    # The signature must not be read from a single row any more.
    check(
        "9: old last_hash query shape gone",
        "ORDER BY COALESCE(remote_created_at, created_at) DESC" not in sig,
    )


# ----------------------------------------------------------------- item 10 (D1)
def verify_item10() -> None:
    src = read("ai_lm_studio.rs")
    for fn in ("pub async fn is_available", "pub async fn list_models"):
        body = src[src.index(fn) : src.index(fn) + 700]
        check(
            f"10: {fn.split()[-1]} attaches the stored API key",
            "bearer_auth(key)" in body and "if let Some(ref key) = self.api_key" in body,
        )
    chat = src[src.index("pub async fn chat") : src.index("pub async fn chat") + 500]
    check(
        "10: chat still authenticates (no regression)",
        "bearer_auth(key)" in chat,
    )


def main() -> int:
    verify_item6()
    verify_item7()
    verify_item8()
    verify_item9()
    verify_item10()
    print()
    if FAILURES:
        print(f"VERIFY-BATCH17: {len(FAILURES)} FAILED CHECK(S)")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("VERIFY-BATCH17: ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
