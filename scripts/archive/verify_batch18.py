#!/usr/bin/env python3
"""Batch 18 verification harness (clean-audit items 11-15).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16/17 harness protocol.

Items:
  11  backup restore: unverified file over live DB, no fsync,
      stale-WAL window, single unconfirmed click
  12  auto QA rebuild wipes stored AI QA layers every >=20h
  13  SLA waiting stats use updated_at instead of last-customer-message
  14  repeated_question_unsolved gap can never fire
  15  campaign sends pass the LOCAL mailbox row id as remote mailboxId
"""

import sys
from pathlib import Path

CORE = Path(__file__).resolve().parents[1] / "crates" / "core" / "src"
SHELL = Path(__file__).resolve().parents[1] / "crates" / "app-shell" / "src"
FAILURES: list[str] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    status = "OK " if ok else "FAIL"
    print(f"[{status}] {name}" + (f" — {detail}" if detail and not ok else ""))
    if not ok:
        FAILURES.append(f"{name}: {detail}")


def read(base: Path, p: str) -> str:
    return (base / p).read_text(encoding="utf-8")


# ----------------------------------------------------------------- item 11
def verify_item11() -> None:
    src = read(CORE, "backup_service.rs")
    restore = src[src.index("pub fn restore(") : src.index("pub fn export_json")]

    check(
        "11: restore refuses before touching disk when the backup fails integrity",
        "if !verify_backup(backup_path)" in restore and "integrity check" in restore,
    )
    check(
        "11: structural probe (app_state table) refuses foreign SQLite files",
        "fn looks_like_supportos_db" in src
        and "if !looks_like_supportos_db(backup_path)" in restore
        and "not a SupportOS" in restore,
    )
    check(
        "11: the staged copy is fsynced BEFORE any swap",
        restore.index("fs::File::open(&restore_tmp)")
        < restore.index("fs::rename(db_path, &prev)"),
    )
    check(
        "11: the live database is moved aside BEFORE the new file is swapped in "
        "(stale-WAL window closed)",
        restore.index("fs::rename(db_path, &prev)")
        < restore.index("fs::rename(&restore_tmp, db_path)"),
    )
    check(
        "11: the live WAL/SHM move aside WITH the database (not deleted after the swap)",
        'fs::rename(&live, format!("{prev}{ext}"))' in restore,
        "WAL/SHM must be renamed aside before the swap, never left to pair with the fresh file",
    )
    check(
        "11: swapped file and directory are fsynced after the swap",
        "fs::File::open(db_path).and_then(|f| f.sync_all())" in restore
        and "fs::File::open(parent).and_then(|d| d.sync_all())" in restore,
    )
    check(
        "11: a failed swap rolls the live database (and its WAL) back",
        "Roll back: the live database (and its WAL) return to place." in restore,
    )
    check(
        "11: no staging or previous-generation residue survives success",
        "fs::remove_file(&prev)" in restore and 'fs::remove_file(format!("{prev}-wal"))' in restore,
    )
    check(
        "11: the old copy-then-rename-then-delete-wal order is gone",
        "let restore_tmp = format!(\"{}.restore-tmp\", db_path.to_string_lossy());" not in restore,
    )

    # UI half: the single-click restore must be a two-step confirmation.
    page = read(SHELL, "pages/settings.rs")
    check(
        "11: the settings page carries the restore-armed state",
        "restore_armed: Option<String>," in page,
    )
    click = page[page.index('if design::button(ui, "restore", ButtonVariant::Outline)') :][:900]
    check(
        "11: the first restore click only ARMS (no immediate restore_backup)",
        "self.restore_armed = Some(path.display().to_string());" in click
        and "self.restore_backup(db, &path);" not in click,
    )
    confirm = page[page.index('if let Some(path) = self.restore_armed.clone()') :][:1200]
    check(
        "11: the confirm card explains the consequence and demands a second click",
        "REPLACED" in confirm and "really restore" in confirm,
    )
    check(
        "11: cancel disarms; only the confirmed click executes",
        'design::button(ui, "cancel", ButtonVariant::Ghost)' in confirm
        and 'design::button(ui, "really restore", ButtonVariant::Destructive)' in confirm
        and "self.restore_backup(db, &path);" in confirm,
    )


# ----------------------------------------------------------------- item 12
def verify_item12() -> None:
    src = read(CORE, "quality.rs")
    compute = src[src.index("pub fn compute_qa_deterministic") : src.index("fn string_gt")]
    check(
        "12: the deterministic layer is UPDATEd in place",
        "UPDATE post_resolution_qa SET deterministic = ?2, computed_at = datetime('now')"
        in compute,
    )
    check(
        "12: an INSERT happens only when the row is missing",
        "if updated.unwrap_or(0) == 0" in compute
        and "INSERT INTO post_resolution_qa (conversation_id, deterministic, computed_at)" in compute,
    )
    check(
        "12: the row-wiping DELETE is gone from the recompute",
        "DELETE FROM post_resolution_qa WHERE conversation_id = ?1" not in compute,
        "the >=20h sweep used to DELETE the whole row (wiping the AI layer)",
    )
    check(
        "12: the AI-layer writer is untouched (ai / ai_run_id / recomputed_at)",
        "UPDATE post_resolution_qa SET ai = ?2, ai_run_id = ?3," in src,
    )
    workers = read(CORE, "workers.rs")
    check(
        "12: the >=20h maintenance trigger is unchanged (the fix is on the write side)",
        "now_unix - last_run_unix >= 20 * 3600" in workers
        and "crate::quality::rebuild_qa(&conn)" in workers,
    )


# ----------------------------------------------------------------- item 13
def verify_item13() -> None:
    src = read(CORE, "sla.rs")
    start = src.index("fn waiting_stats(")
    waiting = src[start : start + 2200]
    check(
        "13: waiting runs from the newest published customer thread",
        "t.type = 'customer'" in waiting
        and "MAX(COALESCE(t.remote_created_at, t.created_at))" in waiting
        and "t.state = 'published'" in waiting
        and "t.deleted_at IS NULL" in waiting,
    )
    check(
        "13: conversations without customer threads fall back to created_at",
        "c.created_at) AS since" in waiting,
    )
    check(
        "13: the updated_at-based since is gone",
        "COALESCE(c.updated_at, c.created_at) AS since" not in waiting,
    )


# ----------------------------------------------------------------- item 14
def verify_item14() -> None:
    src = read(CORE, "quality.rs")
    friction = src[src.index("fn friction_among(") : src.index("fn seen_bounds")]
    check(
        "14: the gap arm reads the wired interaction-outcomes table",
        "FROM client_support_outcomes" in friction,
    )
    check(
        "14: real follow-up / clarification columns instead of JSON keys",
        "follow_up_count > 0 OR clarification_count > 0" in friction
        and "json_extract" not in friction,
    )
    check(
        "14: the dead friction_scores join is gone",
        "friction_scores" not in friction,
    )
    reports = read(CORE, "reports.rs")
    check(
        "14: the never-called reports::compute_friction stays untouched (not in this batch)",
        "pub fn compute_friction(" in reports,
    )


# ----------------------------------------------------------------- item 15
def verify_item15() -> None:
    src = read(CORE, "outreach.rs")
    send = src[src.index("pub async fn send_batch(") : src.index("struct BatchCounts")]
    check(
        "15: the remote mailbox id is resolved before any send",
        "SELECT remote_id FROM mailboxes WHERE id = ?1" in send,
    )
    check(
        "15: CreateConversationInput carries the REMOTE id",
        "mailbox_id: mailbox_remote_id," in send
        and "mailbox_id: mailbox_local_id," not in send,
    )
    check(
        "15: an unsynced mailbox fails the batch honestly (permanent, not retried)",
        '"mailbox not synced"' in send
        and "Sending mailbox has no remote id (not synced)" in send,
    )
    check(
        "15: both early-fail branches scope the writer guard before finalizing "
        "(no same-thread write-lane self-deadlock)",
        send.count("}\n        }\n        return finalize_if_drained(conn, campaign_id);") == 2
        and send.count("let c = conn.write_guard();") >= 3,
        "let-else keeps bindings alive to the block end; the guard must be inner-scoped "
        "(inner-scope close must precede each finalize)",
    )
    check(
        "15: the not-synced branch finalizes AFTER dropping the guard",
        send.index('"mailbox not synced"')
        < send.index("return finalize_if_drained(conn, campaign_id);", send.index('"mailbox not synced"')),
    )


def main() -> int:
    verify_item11()
    verify_item12()
    verify_item13()
    verify_item14()
    verify_item15()
    print()
    if FAILURES:
        print(f"VERIFY-BATCH18: {len(FAILURES)} FAILED CHECK(S)")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("VERIFY-BATCH18: ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
