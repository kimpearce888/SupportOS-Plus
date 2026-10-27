#!/usr/bin/env python3
"""Batch 19 verification harness (clean-audit items 16-20).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16/17/18 harness protocol.

Items:
  16  outreach recipient `attempts` never incremented → retry storm
  17  no claim/idempotency across campaign sends → duplicate emails
  18  UI-created rules poison the engine's trigger deserialization
  19  approved automation write actions never reach Help Scout
  20  activity events keyed by REMOTE id in a LOCAL-id column
"""

import sys
from pathlib import Path

CORE = Path(__file__).resolve().parents[1] / "crates" / "core" / "src"
FAILURES: list[str] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    status = "OK " if ok else "FAIL"
    print(f"[{status}] {name}" + (f" — {detail}" if detail and not ok else ""))
    if not ok:
        FAILURES.append(f"{name}: {detail}")


def read(p: str) -> str:
    return (CORE / p).read_text(encoding="utf-8")


# ----------------------------------------------------------------- item 16
def verify_item16() -> None:
    src = read("outreach.rs")
    start = src.index("fn record_outcome(")
    end = src.index("fn datetime_now()")
    body = src[start:end]
    for arm in ("state = 'sent'", "state = 'failed'", "state = 'queued'",
                "state = 'unknown'"):
        seg = body[body.index(arm) : body.index("WHERE id = ?", body.index(arm))]
        check(
            f"16: record_outcome `{arm}` spends an attempt",
            "attempts = attempts + 1" in seg,
            seg,
        )
    check(
        "16: every outcome refreshes the staleness clock",
        body.count("updated_at = datetime('now')") >= 4,
        "expected four updated_at refreshes",
    )
    check(
        "16: classify_error still reads attempts for the 429/5xx cap",
        "attempts + 1 >= SEND_MAX_ATTEMPTS" in src,
    )


# ----------------------------------------------------------------- item 17
def verify_item17() -> None:
    src = read("outreach.rs")
    check(
        "17: fresh schema ships the updated_at column",
        "updated_at      TEXT NOT NULL DEFAULT (strftime"
        in src[src.index("CREATE TABLE IF NOT EXISTS outreach_recipients") :],
    )
    pick = src[src.index("fn pick_batch(") : src.index("fn classify_error(")]
    check(
        "17: pick_batch atomically claims rows into 'sending'",
        "SET state = 'sending', updated_at = datetime('now')" in pick,
    )
    check(
        "17: the claim only wins while the row is still pickable",
        "WHERE id = ?1 AND state IN ('selected','queued')" in pick,
    )
    ensure = src[src.index("fn ensure_recipient_updated_at(") :]
    ensure = ensure[: ensure.index("pub async fn send_batch(")]
    check(
        "17: the lazy column heal exists (old databases)",
        "ALTER TABLE outreach_recipients ADD COLUMN updated_at" in ensure,
    )
    check(
        "17: the ALTER uses a constant default + backfill (SQLite rejects "
        "expression defaults on ADD COLUMN)",
        "ADD COLUMN updated_at TEXT;" in ensure
        and "SET updated_at = COALESCE(created_at" in ensure,
    )
    rec = src[src.index("fn reconcile_recipient_states(") : src.index(
        "pub fn campaign_reconcile("
    )]
    check(
        "17: the stale sweep inlines the threshold (the bound-string "
        "comparison never matched)",
        "AND updated_at < datetime('now', '-10 minutes')" in rec
        and 'stale_threshold = "datetime' not in rec,
    )
    check(
        "17: a stale claim probes the mirror before requeueing",
        "stale sending recovered as sent from the platform mirror" in rec,
    )
    check(
        "17: the unknown arm returns provable non-delivery to 'queued' "
        "(was the contradictory 'failed')",
        "state = 'queued',\n                         last_error = 'Reconciled: "
        "not delivered, safe to retry'" in rec,
    )
    check(
        "17: exhausted unknowns terminate honestly in 'failed'",
        "not delivered and attempts exhausted" in rec,
    )
    send = src[src.index("pub async fn send_batch(") : src.index(
        "#[derive(Default)]\nstruct BatchCounts"
    )]
    check(
        "17: send_batch heals before picking (recovery is wired in)",
        "reconcile_recipient_states(&c, campaign_id)" in send,
    )
    check(
        "17: send_batch enqueues no continuation itself (no double-queue)",
        "campaign_reconcile(&c, campaign_id)" not in send,
    )


# ----------------------------------------------------------------- item 18
def verify_item18() -> None:
    src = read("automation.rs")
    check(
        "18: trigger parsing accepts the modern quoted-string shape",
        'serde_json::from_str::<String>(raw)' in src,
    )
    check(
        "18: trigger parsing maps the legacy M007 UI enum",
        "Trigger::TagAdded" in src
        and '"tag_added".to_string()' in src,
    )
    listing = src[src.index("pub fn list_rule_records(") : src.index(
        "#[derive(Debug, Clone, PartialEq)]\npub enum SqlValue"
    )]
    check(
        "18: one poisoned row no longer aborts the whole engine",
        "skipping unparsable automation rule" in listing
        and "map(parse_rule_record).collect()" not in listing,
    )
    check(
        "18: the fire loop still swallows nothing that kills the engine",
        "for rule in list_rule_records(conn)?" in src,
    )


# ----------------------------------------------------------------- item 19
def verify_item19() -> None:
    src = read("automation.rs")
    approved = src[src.index("pub fn execute_approved_action(") : src.index(
        "pub fn record_approved_run("
    )]
    check(
        "19: approved set_status queues the outbound push",
        '"update_status"' in approved and "automation_status_{conversation_id}" in approved,
    )
    check(
        "19: approved assign queues the outbound push",
        '"assign"' in approved and "automation_assign_{conversation_id}" in approved,
    )
    check(
        "19: both pushes use the inbox's keyed enqueue primitive",
        "create_outbound_job_keyed" in approved,
    )
    nondest = src[src.index("fn execute_non_destructive(") : src.index(
        "pub fn execute_approved_action("
    )]
    check(
        "19: the add_tag arm queues the tag push",
        '"update_tags"' in nondest and "automation_tags_{conversation_id}" in nondest,
    )
    workers = read("workers.rs")
    check(
        "19: the outbound lane understands update_tags",
        '"update_tags" =>' in workers,
    )
    check(
        "19: the audit trail names the tag push honestly",
        '"update_tags" => "tags_updated"' in workers,
    )


# ----------------------------------------------------------------- item 20
def verify_item20() -> None:
    top = read("ticket_ops.rs")
    check(
        "20: ticket_ops resolves the LOCAL conversation id",
        "fn conversation_local_id(" in top,
    )
    check(
        "20: the remote-keyed existence probe is gone",
        "fn conversation_exists(" not in top,
    )
    for arm in ("'assign'", "'status_change'", "'note'"):
        i = top.index(arm)
        seg = top[i : top.index("params![", i)]
        check(
            f"20: the {arm} event insert binds the local id",
            top[top.index("params![", i) : top.index(")", top.index("params![", i))].startswith(
                "params![local_id"
            ),
            arm,
        )
        _ = seg
    check(
        "20: derived columns refresh through the by-local-id reader",
        top.count("update_derived_columns_by_local_id(conn, local_id)") == 3,
        f"found {top.count('update_derived_columns_by_local_id(conn, local_id)')}",
    )
    check(
        "20: NULL actor ids no longer null out the dedup key",
        top.count("COALESCE(?3, -1)") == 3,
    )
    check(
        "20: the remote-keyed derived reader is gone from ticket_ops",
        "update_derived_columns(conn, remote_id)" not in top,
    )
    act = read("activity.rs")
    check(
        "20: the remote-keyed derived machinery is deleted",
        "pub fn derive_response_state(" not in act
        and "pub fn update_derived_columns(" not in act,
    )
    check(
        "20: the by-local-id reader is the single public entry point",
        "pub fn update_derived_columns_by_local_id(" in act,
    )


def main() -> int:
    verify_item16()
    verify_item17()
    verify_item18()
    verify_item19()
    verify_item20()
    print()
    if FAILURES:
        print(f"batch 19 harness: {len(FAILURES)} FAILED")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("batch 19 harness: all checks pass")
    return 0


if __name__ == "__main__":
    sys.exit(main())
