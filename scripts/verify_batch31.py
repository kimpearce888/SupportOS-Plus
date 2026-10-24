#!/usr/bin/env python3
"""Batch 31 verification harness (clean-audit items 117-126).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..30 harness protocol.

Items:
  117  refresh_replies counts the campaign's own message under clock skew
       (outreach.rs)
  118  duplicate graph edges for mutual links (support_graph.rs)
  119  tag-filter case-folding asymmetry: Rust Unicode vs SQL ASCII
       LOWER (saved_views.rs)
  120  entity decode order double-decodes; &apos; missing (knowledge_store.rs)
  121  byte-unsafe slicing helpers (knowledge_store.rs html helpers,
       custom_objects.rs valid_iso_date, copilot.rs truncate_result)
  122  numeric customer_property filters CAST text to 0 (saved_views.rs)
  123  freshness_low_usage_days setting dead (knowledge_store.rs)
  124  pre-release vs pre-release compares as Same (version_check.rs)
  125  sweep windows drop the oldest of a burst (DESC+stamp)
       (notification_sweep.rs, 6 lanes)
  126  parse_csv header/data index misalignment on empty headers
       (connectors.rs)
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
    """Source without comment lines — for checks whose needle may
    legitimately appear inside a comment describing the pre-fix defect."""
    return "\n".join(
        line
        for line in read(root, p).splitlines()
        if not line.lstrip().startswith("//")
    )


def function_body(src: str, needle: str, end: str = "\n}\n") -> str:
    start = src.index(needle)
    end_idx = src.index(end, start)
    return src[start:end_idx]


# ----------------------------------------------------------------- item 117
def verify_item117() -> None:
    outreach = read_code(CORE, "outreach.rs")
    body = function_body(outreach, "pub fn refresh_replies")
    check(
        "117: the local-clock guard is gone",
        "julianday(COALESCE(?2, '1970-01-01'))" not in body
        and "julianday(created_at) > julianday(COALESCE(?2" not in body,
    )
    check(
        "117: the anchor is the mirrored campaign thread (first thread)",
        "ORDER BY created_at ASC, id ASC LIMIT 1" in body
        and "SELECT id, created_at FROM conversation_threads" in body,
    )
    check(
        "117: a reply is a customer thread strictly after the campaign's own",
        "julianday(created_at) > julianday(?2)" in body
        and "type = 'customer'" in body,
    )
    check(
        "117: the create payload still sends the campaign message as a "
        "customer-type thread (the hazard the fix anchors around)",
        '"type": "customer"' in read_code(CORE, "helpscout_real.rs"),
    )


# ----------------------------------------------------------------- item 118
def verify_item118() -> None:
    graph = read_code(CORE, "support_graph.rs")
    body = function_body(graph, "pub fn subgraph_json")
    check(
        "118: pushed edges dedupe through a canonical unordered key",
        "edge_keys.insert(edge_key(&edge))" in body
        and "if sk <= tk" in body,
    )
    check(
        "118: the key covers relation + both endpoints",
        'e["relation"]' in body and 'e["source"]' in body and 'e["target"]' in body,
    )


# ----------------------------------------------------------------- item 119
def verify_item119() -> None:
    views = read_code(CORE, "saved_views.rs")
    body = function_body(views, "ViewNode::Tags { tags, mode }")
    check(
        "119: the Rust fold is ASCII-only (SQL LOWER parity)",
        "t.to_ascii_lowercase()" in body and "t.to_lowercase()" not in body,
    )
    check(
        "119: the SQL side still folds through LOWER",
        "LOWER(t.name)" in body,
    )


# ----------------------------------------------------------------- item 120
def verify_item120() -> None:
    store = read_code(CORE, "knowledge_store.rs")
    body = function_body(store, "pub fn html_to_text_keep_structure")
    amp = body.find('("&amp;", "&")')
    lt = body.find('("&lt;", "<")')
    check(
        "120: &amp; decodes LAST (single-decode semantics)",
        amp > lt >= 0,
    )
    check(
        "120: &apos; decodes",
        '("&apos;", "\'")' in body,
    )
    check(
        "120: the authored &amp;lt; stays literal (single-decode test)",
        'html_to_text_keep_structure("a &amp;lt;script&amp;gt; b")' in store,
    )


# ----------------------------------------------------------------- item 121
def verify_item121() -> None:
    store = read_code(CORE, "knowledge_store.rs")
    for fn in ("fn strip_tag_block", "fn replace_ci", "fn extract_html_title"):
        body = function_body(store, fn)
        check(
            f"121: {fn} folds ASCII-only (offset map stays 1:1)",
            "to_ascii_lowercase()" in body and "to_lowercase()" not in body,
        )
    objects = read_code(CORE, "custom_objects.rs")
    iso = function_body(objects, "fn valid_iso_date")
    check(
        "121: valid_iso_date slices by characters, not bytes",
        "s.chars().take(10).collect()" in iso and "s[..10]" not in iso,
    )
    copilot = read_code(CORE, "copilot.rs")
    tr = function_body(copilot, "pub fn truncate_result")
    check(
        "121: truncate_result counts characters",
        "output.chars().count()" in tr and "&output[..TOOL_RESULT_MAX_CHARS]" not in tr,
    )


# ----------------------------------------------------------------- item 122
def verify_item122() -> None:
    views = read_code(CORE, "saved_views.rs")
    body = function_body(views, '"gt" | "gte" | "lt" | "lte" =>')
    check(
        "122: the numeric comparison is gated on a numeric-shaped value",
        "GLOB '*[0-9]*'" in body and "NOT GLOB '*[^0-9.+-eE]*'" in body,
    )
    check(
        "122: the bare CAST comparison is gone",
        "AND CAST(cp.value AS REAL) {op_sql} ?)\"" not in body.split("GLOB")[0],
    )


# ----------------------------------------------------------------- item 123
def verify_item123() -> None:
    store = read_code(CORE, "knowledge_store.rs")
    body = function_body(store, "pub fn freshness_report")
    check(
        "123: the low_usage flag consults the low_usage_days grace period",
        "low_usage_days" in body.split("let low_usage =")[1].split(";")[0]
        if "let low_usage =" in body
        else False,
    )
    check(
        "123: the setting is read (not dropped)",
        'get_int(conn, "freshness_low_usage_days", 60)' in body,
    )
    check(
        "123: usage alone no longer decides the flag",
        "let low_usage = usage_count == 0;" not in body,
    )


# ----------------------------------------------------------------- item 124
def verify_item124() -> None:
    vc = read_code(CORE, "version_check.rs")
    body = function_body(vc, "pub fn compare_versions")
    check(
        "124: same-core pre-releases compare through compare_prerelease",
        "compare_prerelease(&a, &b)" in body,
    )
    check(
        "124: numeric identifiers sort below alphanumeric ones",
        "(Some(_), None) => std::cmp::Ordering::Less," in vc,
    )
    check(
        "124: a larger identifier set outranks its prefix",
        "(None, Some(_)) => return std::cmp::Ordering::Less," in vc,
    )


# ----------------------------------------------------------------- item 125
def verify_item125() -> None:
    sweep = read_code(CORE, "notification_sweep.rs")
    for lane, col in (
        ("failed_jobs", "completed_at"),
        ("campaign_replies", "r.replied_at"),
        ("known_issues", "created_at"),
        ("issue_spikes", "updated_at"),
        ("ai_escalations", "a.created_at"),
        ("ratings", "remote_created_at"),
    ):
        check(
            f"125: the {lane} lane filters its stamp in SQL before the cap",
            f"datetime({col}) > datetime(?1)" in sweep,
        )
    check(
        "125: no DESC+LIMIT lane filters its stamp in Rust anymore",
        "before_or_equal(" not in sweep,
    )


# ----------------------------------------------------------------- item 126
def verify_item126() -> None:
    connectors = read_code(CORE, "connectors.rs")
    body = function_body(connectors, "pub fn parse_csv")
    check(
        "126: empty headers keep their positional slot",
        "Vec<Option<&str>>" in body and "(!t.is_empty()).then_some(t)" in body,
    )
    check(
        "126: the positional filter is gone",
        ".filter(|h| !h.is_empty())" not in body,
    )


def main() -> int:
    for verify in (
        verify_item117,
        verify_item118,
        verify_item119,
        verify_item120,
        verify_item121,
        verify_item122,
        verify_item123,
        verify_item124,
        verify_item125,
        verify_item126,
    ):
        verify()
    print()
    if FAILURES:
        print(f"{len(FAILURES)} check(s) FAILED:")
        for failure in FAILURES:
            print(f"  - {failure}")
        return 1
    print("All batch-31 checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
