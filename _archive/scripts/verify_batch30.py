#!/usr/bin/env python3
"""Batch 30 verification harness (clean-audit items 107-116).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..29 harness protocol.

Items:
  107  redact_string byte-slicing panic (config.rs)
  108  digest doc counts: space-format vs ISO-T lexical comparison (digest.rs)
  109  new-conv windows key on local_created_at; deleted-filter claim false
       (reports.rs)
  110  %W year-boundary week split (reports.rs)
  111  LIMIT caps silently undercount alert claims (analytics.rs)
  112  concentration denominator counts NULL-key conversations (analytics.rs)
  113  open_closed_distribution drops pending (issue_impact.rs)
  114  email_domain filter unanchored, op ignored (segment.rs)
  115  campaign_reconcile references nonexistent updated_at column
       (outreach.rs)
  116  ORDER BY remote_created_at never populated by sync (2 sites)
       (interaction_current.rs, interaction_engine.rs)
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


# ----------------------------------------------------------------- item 107
def verify_item107() -> None:
    # Batch 34 (item 153) re-point: `redact_string` moved into the
    # test-only `batch30_tests` module (its production caller,
    # HelpScoutConfig::redacted, was removed with the dead AppConfig
    # surface), so the function body is indented and closes with a
    # four-space brace.
    config = read_code(CORE, "config.rs")
    body = function_body(config, "fn redact_string", end="\n    }\n")
    check(
        "107: head/tail take CHARACTERS, not byte offsets",
        "s.chars().collect()" in body
        and 'chars[..2].iter().collect()' in body
        and "chars[chars.len() - 2..].iter().collect()" in body,
    )
    check(
        "107: the byte-slicing expressions are gone",
        "&s[..head]" not in body and "s.len() - tail" not in body,
    )
    check(
        "107: short strings mask by character count",
        "chars.len() <= 4" in body,
    )
    check(
        "107: a regression test exercises multi-byte secrets",
        "redact_string(\"🔒🔒🔒🔒🔒🔒\")" in read(CORE, "config.rs"),
    )


# ----------------------------------------------------------------- item 108
def verify_item108() -> None:
    digest = read_code(CORE, "digest.rs")
    body = function_body(digest, "fn change_facts")
    check(
        "108: the article comparison normalizes through datetime()",
        "WHERE datetime(remote_updated_at) >= datetime(?1)" in body,
    )
    check(
        "108: the article ordering normalizes through datetime()",
        "ORDER BY datetime(remote_updated_at) DESC" in body,
    )
    check(
        "108: the document count normalizes through datetime()",
        "WHERE datetime(updated_at) >= datetime(?1)" in body,
    )
    check(
        "108: no raw lexical comparison against the since bound remains",
        "remote_updated_at >= ?1" not in body and "updated_at >= ?1" not in body,
    )


# ----------------------------------------------------------------- item 109
def verify_item109() -> None:
    reports = read_code(CORE, "reports.rs")
    dash = function_body(reports, "pub fn get_dashboard_metrics")
    check(
        "109: the dashboard new-conv window keys on the creation stamp",
        "julianday(COALESCE(remote_created_at, created_at)) >= julianday('now', '-{days_back} days')"
        in dash,
    )
    check(
        "109: the dashboard new-conv window excludes deleted conversations",
        "AND deleted_at IS NULL" in dash,
    )
    check(
        "109: no report window keys on the local insert stamp",
        "julianday(local_created_at)" not in reports,
    )
    check(
        "109: the report window filters deleted rows and the limitation "
        "claim is now true",
        "AND deleted_at IS NULL" in function_body(reports, "pub fn build_report"),
        "build_report lacks the deleted_at filter",
    )
    check(
        "109: the previous-period window keys on the same stamp",
        function_body(reports, "pub fn build_report").count("julianday({created_stamp})") >= 1,
    )


# ----------------------------------------------------------------- item 110
def verify_item110() -> None:
    reports = read_code(CORE, "reports.rs")
    body = function_body(reports, "pub fn build_report")
    check(
        "110: the week dimension is ISO (%G-%V)",
        "strftime('%G-W%V', {created_stamp})" in body,
    )
    check(
        "110: the %Y-W%W year-boundary split is gone",
        "%Y-W%W" not in reports,
    )
    check(
        "110: day/month buckets share the creation stamp",
        "strftime('%Y-%m-%d', {created_stamp})" in body
        and "strftime('%Y-%m', {created_stamp})" in body,
    )


# ----------------------------------------------------------------- item 111
def verify_item111() -> None:
    analytics = read_code(CORE, "analytics.rs")
    check(
        "111: the escalation sample is not silently capped",
        "WHERE t.name = 'escalated'" in analytics
        and "LIMIT 1000" not in analytics,
        "the escalated query still carries a LIMIT",
    )
    check(
        "111: the rating overlap sees every not-good rating",
        "WHERE r.rating = 'not-good' AND r.conversation_id IS NOT NULL" in analytics
        and "LIMIT 2000" not in analytics,
        "the ratings query still carries a LIMIT/ORDER BY cap",
    )


# ----------------------------------------------------------------- item 112
def verify_item112() -> None:
    analytics = read_code(CORE, "analytics.rs")
    body = function_body(analytics, "fn radar_extensions")
    check(
        "112: the customer concentration runs on the attributed subset",
        "let attributed: Vec<&ConvFacts> = convs"
        in body
        and "customers.len() <= attributed_total / 2" in body,
    )
    check(
        "112: the inbox concentration runs on the attributed subset",
        "let with_mailbox: Vec<&ConvFacts> = convs"
        in body
        and "n as f64 / mailbox_total as f64 >= 0.7" in body,
    )
    check(
        "112: the claim texts report the attributed totals",
        body.count("attributed_total") >= 2 and body.count("mailbox_total") >= 2,
    )
    check(
        "112: the old whole-set denominators are gone",
        "customers.len() <= convs.len() / 2" not in body
        and "n as f64 / convs.len() as f64" not in body,
    )


# ----------------------------------------------------------------- item 113
def verify_item113() -> None:
    impact = read_code(CORE, "issue_impact.rs")
    body = function_body(impact, "fn compute")
    check(
        "113: open counts active AND pending",
        "SUM(CASE WHEN c.status IN ('active','pending') THEN 1 ELSE 0 END) AS open_count" in body,
    )
    check(
        "113: closed still counts only closed",
        "SUM(CASE WHEN c.status = 'closed' THEN 1 ELSE 0 END) AS closed_count" in body,
    )


# ----------------------------------------------------------------- item 114
def verify_item114() -> None:
    segment = read_code(CORE, "segment.rs")
    body = function_body(
        segment,
        '"email_domain" => {',
        end="\n            _ => {\n                let expr = match field.as_str() {",
    )
    check(
        "114: the domain part is extracted from each stored address",
        "LOWER(SUBSTR(ce.value, INSTR(ce.value, '@') + 1))" in body,
    )
    check(
        "114: the operator is honored — the standard set dispatches",
        all(
            op in body
            for op in (
                '"equals"',
                '"not_equals"',
                '"contains"',
                '"starts_with"',
                '"ends_with"',
            )
        ),
    )
    check(
        "114: the unanchored %@{domain}% pattern is gone",
        "%@{}%" not in body,
    )
    check(
        "114: is_empty / is_not_empty answer without a domain value",
        body.index('"is_empty"') < body.index("let domain = val.trim_start_matches('@')"),
    )
    check(
        "114: is_empty checks email presence, not a domain match",
        "c.id NOT IN\n                               (SELECT ce.customer_id FROM customer_emails ce)" in body,
    )


# ----------------------------------------------------------------- item 115
def verify_item115() -> None:
    outreach = read_code(CORE, "outreach.rs")
    body = function_body(outreach, "fn reconcile_recipient_states")
    check(
        "115: the reconcile path self-heals the claim/staleness clock column",
        "ensure_recipient_updated_at(conn)" in body,
    )
    check(
        "115: the self-heal failure is logged, not silent",
        '"recipient_schema_ensure_failed"' in body,
    )
    check(
        "115: the ensure runs BEFORE the unknown-row sweep",
        body.index("ensure_recipient_updated_at(conn)")
        < body.index('state = \'unknown\'"'),
    )


# ----------------------------------------------------------------- item 116
def verify_item116() -> None:
    current = read_code(CORE, "interaction_current.rs")
    engine = read_code(CORE, "interaction_engine.rs")
    check(
        "116: customer_messages orders on the COALESCE fallback",
        "ORDER BY COALESCE(remote_created_at, created_at) ASC" in current,
    )
    check(
        "116: compute_outcome orders on the COALESCE fallback",
        "ORDER BY COALESCE(remote_created_at, created_at) ASC" in engine,
    )
    check(
        "116: no bare remote_created_at ORDER BY remains in either lane",
        "ORDER BY remote_created_at" not in current
        and "ORDER BY remote_created_at" not in engine,
    )
    check(
        "116: the sync writer still does not populate remote_created_at — "
        "the fallback is the fix, not a schema change",
        "remote_created_at" not in function_body(
            read(CORE, "sync_engine.rs"), "fn upsert_thread"
        ),
    )


def main() -> int:
    for verify in (
        verify_item107,
        verify_item108,
        verify_item109,
        verify_item110,
        verify_item111,
        verify_item112,
        verify_item113,
        verify_item114,
        verify_item115,
        verify_item116,
    ):
        verify()
    print()
    if FAILURES:
        print(f"{len(FAILURES)} check(s) FAILED:")
        for failure in FAILURES:
            print(f"  - {failure}")
        return 1
    print("All batch-30 checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
