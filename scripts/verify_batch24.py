#!/usr/bin/env python3
"""Batch 24 verification harness (clean-audit items 46-55).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..23 harness protocol.

Items:
  46  knowledge-chunk batch poisoned by one empty vector (qdrant lane
      gated on the FIRST vector only, upserted empties as 'indexed')
  47  customer email patch deleted ALL stored emails, errors swallowed
  48  verify_bundle leaked the decrypted plaintext temp on error paths;
      no sweep of crashed-run leftovers
  49  workspace reset left downloaded attachment files on disk
  50  CSS blocklist bypassable via comments/escapes (ur/*x*/l(, \\75 rl()
  51  blocking DNS resolution inside async contexts (connectors)
  52  shared_tag display query joined a conversation's tags to itself
  53  waiting_stats.at_risk counted breached-vs-first-response for
      replied tickets
  54  radar trend precedence made "new" unreachable for >=3 clusters
  55  backlog_jump compared against a false week-ago baseline
      (currently-open-only filter forgot tickets closed during the week)
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


# ----------------------------------------------------------------- item 46
def verify_item46() -> None:
    src = read("embeddings.rs")
    fn = src[src.index('if table == "knowledge_chunks"') :]
    fn = fn[: fn.index("let mut embedded = 0_usize;\n    for (i, chunk) in pending.iter().enumerate() {\n        match vectors.get(i) {\n            Some(v) if !v.is_empty() => {\n                update_chunk_embedding")]
    check(
        "46: the batch is partitioned per chunk (indexed/failed id lists)",
        "indexed_ids" in fn and "failed_ids" in fn,
    )
    check(
        "46: only chunks with a real vector are upserted",
        'Some(v) if !v.is_empty() => {\n                        points.push(vector_point(table, chunk, v, &model)?);'
        in fn,
    )
    check(
        "46: empty-vector chunks are marked 'failed' and stay retryable",
        fn.count('set_chunk_state(conn, table, *id, "failed", None)?') == 2,
    )
    check(
        "46: the first-vector gate is gone (no vectors.first() in the lane)",
        "vectors.first()" not in fn,
    )
    check(
        "46: the empty-vector substitution is gone (no unwrap_or(&empty))",
        "unwrap_or(&empty)" not in fn,
    )


# ----------------------------------------------------------------- item 47
def verify_item47() -> None:
    src = read("people_store.rs")
    fn = src[src.index("pub fn update_customer") :]
    fn = fn[: fn.index("fn patch_field_present")]
    check(
        "47: the old primary email is captured before the UPDATE",
        "SELECT email FROM customers WHERE id = ?1" in fn,
    )
    check(
        "47: the delete targets the replaced VALUE, never the whole set",
        "DELETE FROM customer_emails"
        in fn and "WHERE customer_id = ?1 AND value = ?2" in fn,
    )
    check(
        "47: no delete-all of customer_emails remains in the patch path",
        "DELETE FROM customer_emails WHERE customer_id = ?1" not in fn,
    )
    check(
        "47: email-set write failures propagate (no swallowed let _)",
        "let _ = conn.execute" not in fn,
    )
    check(
        "47: clearing the primary leaves the stored set untouched",
        "Some(None)" in src and "audit47" in src,
    )


# ----------------------------------------------------------------- item 48
def verify_item48() -> None:
    src = read("encrypted_sync.rs")
    dec = src[src.index("fn decrypt_to_temp") :]
    dec = dec[: dec.index("fn integrity_check")]
    check(
        "48: every decrypt entry point sweeps stale plaintext temps",
        "sweep_stale_import_temps(bundles_dir);" in dec,
    )
    sweep = src[src.index("fn sweep_stale_import_temps") :]
    sweep = sweep[: sweep.index("fn integrity_check")]
    check(
        "48: the sweep is age-bounded via the embedded timestamp",
        ".import-" in sweep and "STALE_AFTER_MS" in sweep,
    )
    check(
        "48: the sweep only touches .import-*.tmp files",
        'strip_prefix(".import-")' in sweep and 'strip_suffix(".tmp")' in sweep,
    )
    ver = src[src.index("pub fn verify_bundle") :]
    ver = ver[: ver.index("pub fn import_bundle")]
    close_pos = ver.index("let _ = fs::remove_file(&temp);")
    closure_end = ver.index("})();")
    check(
        "48: verify_bundle removes the temp on EVERY path (after the "
        "closure, before the error mapping)",
        close_pos > closure_end,
    )
    check(
        "48: the snapshot connection is dropped before removal "
        "(Windows open-handle safety)",
        "drop(snap);" in ver,
    )


# ----------------------------------------------------------------- item 49
def verify_item49() -> None:
    src = read("data_reset.rs")
    reset = src[src.index("pub fn reset_workspace_data") :]
    reset = reset[: reset.index("fn purge_in_transaction")]
    check(
        "49: the reset empties data_dir/attachments",
        'purge_attachment_files(&dir.join("attachments"))' in reset,
    )
    purge = src[src.index("fn purge_attachment_files") :]
    purge = purge[: purge.index("fn purge_in_transaction")]
    check(
        "49: only regular files are removed (directories left alone)",
        "is_file()" in purge,
    )
    check(
        "49: the purge is best-effort with a warning, never fatal",
        "tracing::warn!" in purge and "?;" not in purge,
    )
    check(
        "49: the report counts removed attachment files",
        "attachment_files_deleted" in src,
    )


# ----------------------------------------------------------------- item 50
def verify_item50() -> None:
    src = read("security.rs")
    norm = src[src.index("fn normalize_css_for_blocklist") :]
    norm = norm[: norm.index("const IMG_START_TAG_RE")]
    check(
        "50: CSS comments are stripped before the blocklist runs",
        'r"(?s)/\\*.*?\\*/"' in norm,
    )
    check(
        "50: CSS backslash escapes are decoded (hex + literal forms)",
        'r"\\\\(?:[0-9A-Fa-f]{1,6}[ \\t\\r\\n]?|[\\s\\S])"' in norm,
    )
    check(
        "50: escape layering past the decode bound is rejected (None)",
        "CSS_DECODE_PASSES" in norm and "None" in norm,
    )
    filt = src[src.index("builder.attribute_filter") :]
    filt = filt[: filt.index("const IMG_START_TAG_RE")]
    check(
        "50: the blocklist runs on the DECODED form, not the raw value",
        "let decoded = match normalize_css_for_blocklist(&clip)" in filt
        and "dangerous_css.is_match(&decoded)" in filt,
    )
    check(
        "50: the raw-value match is gone",
        "dangerous_css.is_match(value)" not in filt,
    )


# ----------------------------------------------------------------- item 51
def verify_item51() -> None:
    src = read("security.rs")
    check(
        "51: the async SSRF guard exists and rides the blocking pool",
        "pub async fn validate_ssrf_full_async" in src
        and "check_url_resolved(raw_url).await" in src,
    )
    check(
        "51: check_url_resolved resolves DNS via spawn_blocking",
        "tokio::task::spawn_blocking" in src[src.index("pub async fn check_url_resolved") :],
    )
    conn = read("connectors.rs")
    vc = conn[conn.index("pub async fn validate_config") :]
    vc = vc[: vc.index("type NormalizedRow")]
    check(
        "51: connectors::validate_config (async) uses the async guard",
        "validate_ssrf_full_async(url).await" in vc,
    )
    check(
        "51: no blocking sync guard call remains in validate_config",
        "validate_ssrf_full(url)" not in vc,
    )
    fr = conn[conn.index("async fn fetch_rows") :]
    check(
        "51: fetch_rows (async refresh pipeline) uses the async guard",
        "validate_ssrf_full_async(url).await" in fr,
    )
    check(
        "51: no blocking sync guard call remains in fetch_rows",
        "validate_ssrf_full(url)" not in fr,
    )


# ----------------------------------------------------------------- item 52
def verify_item52() -> None:
    src = read("quality.rs")
    fn = src[src.index("let shared_tag: Option<String> = conn") :]
    fn = fn[: fn.index("let evidence: Vec<Value> = related")]
    check(
        "52: the shared tag must exist on the current conversation",
        "ct1.conversation_id = ?1" in fn,
    )
    check(
        "52: the shared tag must also exist on a related conversation "
        "of the same customer in the 90-day window",
        "ct2.tag_id = tg.id" in fn
        and "c2.customer_local_id = ?2" in fn
        and "datetime('now', '-90 days')" in fn,
    )
    check(
        "52: the tag-to-itself self-join is gone",
        "JOIN conversation_tags ct2 ON ct2.conversation_id = ?1" not in fn,
    )


# ----------------------------------------------------------------- item 53
def verify_item53() -> None:
    src = read("sla.rs")
    fn = src[src.index("fn waiting_stats") :]
    fn = fn[: fn.index("#[derive(Debug, Clone)]\nstruct AlertCandidate")]
    check(
        "53: the replied flag rides along in the waiting query",
        "AS replied" in fn,
    )
    check(
        "53: the target is chosen per conversation by replied-ness",
        "let target = if *replied {" in fn
        and "resolution_target" in fn
        and "first_response_target" in fn,
    )
    check(
        "53: the single first-response-only count is gone",
        "businesses.iter().filter(|b| **b > t as f64).count()" not in fn,
    )


# ----------------------------------------------------------------- item 54
def verify_item54() -> None:
    src = read("analytics.rs")
    fn = src[src.index("let trend = if") :]
    fn = fn[: fn.index("let customer_count")]
    new_pos = fn.index('previous == 0 && recent > 0')
    rising_pos = fn.index("(recent as f64) > previous as f64 * 1.3")
    check(
        "54: 'new' is tested before the rising band",
        new_pos < rising_pos,
    )
    check(
        "54: the rising band keeps its >= 3 floor and 1.3 ratio",
        "recent >= 3" in fn and "* 1.3" in fn,
    )
    check(
        "54: the falling band keeps its 0.7 ratio with a prior window",
        "previous > 0" in fn and "* 0.7" in fn,
    )


# ----------------------------------------------------------------- item 55
def verify_item55() -> None:
    src = read("digest.rs")
    fn = src[src.index("let backlog_week_ago: i64 = conn") :]
    fn = fn[: fn.index("if backlog_now >= 10")]
    check(
        "55: the baseline counts tickets still open at the period start "
        "(closed_at IS NULL branch)",
        "closed_at IS NULL AND status IN ('active','pending')" in fn,
    )
    check(
        "55: tickets closed DURING the period count as open at the start",
        "closed_at IS NOT NULL AND closed_at >= ?1" in fn,
    )
    check(
        "55: only conversations that existed before the start count",
        "COALESCE(remote_created_at, created_at) < ?1" in fn,
    )
    check(
        "55: the current-status-only filter is gone",
        "WHERE status IN ('active','pending') AND deleted_at IS NULL\n                AND COALESCE"
        not in fn,
    )


def main() -> int:
    for verify in (
        verify_item46,
        verify_item47,
        verify_item48,
        verify_item49,
        verify_item50,
        verify_item51,
        verify_item52,
        verify_item53,
        verify_item54,
        verify_item55,
    ):
        verify()
    print()
    if FAILURES:
        print(f"{len(FAILURES)} check(s) FAILED:")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("All batch-24 checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
