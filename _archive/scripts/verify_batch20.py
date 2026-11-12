#!/usr/bin/env python3
"""Batch 20 verification harness (clean-audit items 21-25).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..19 harness protocol.

Items:
  21  vector point-id collisions across entity families silently
      overwrite vectors in the single collection
  22  `rebuild_activity` job never enqueued → derived response-state
      columns permanently NULL/defaults
  23  toast pushed for every auto-sync completion (~one/minute forever)
  24  inbox AI-transform poll fills the composer from a stale/foreign
      historical run
  25  "waiting" status option is not in the platform's own status
      vocabulary
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


def read(base: Path, p: str) -> str:
    return (base / p).read_text(encoding="utf-8")


# ----------------------------------------------------------------- item 21
def verify_item21() -> None:
    store = read(CORE, "vectorstore_sqlitevec.rs")
    check(
        "21: namespaced point-id helper with the three family bands",
        'pub fn namespaced_chunk_point_id(' in store
        and '"knowledge_chunk" => Some(0)' in store
        and '"docs_chunk" => Some(1)' in store
        and '"conversation_chunk" => Some(2)' in store,
    )
    check(
        "21: the band stride bounds every family (overflow is loud)",
        "const POINT_ID_STRIDE: i64 = 1_000_000_000;" in store
        and "exceeds the" in store,
    )
    upsert = store[store.index("fn upsert(") : store.index("fn delete(")]
    check(
        "21: the store refuses the silent cross-family overwrite",
        "cross-family point-id collision" in upsert
        and "SELECT entity_type FROM vec_payload" in upsert,
    )

    emb = read(CORE, "embeddings.rs")
    vp = emb[emb.index("fn vector_point(") : emb.index("fn set_chunk_state(")]
    check(
        "21: vector_point derives the id through the namespacing helper",
        "namespaced_chunk_point_id(" in vp and "id: chunk.id," not in vp,
    )
    check(
        "21: payloads stamp index_version 2 (the v2 id schema)",
        '"index_version": 2' in vp and '"index_version": 1' not in vp,
    )
    check(
        "21: both upsert sites propagate mapping errors instead of "
        "collecting them away",
        emb.count(".collect::<Result<Vec<_>>>()?;") >= 2,
    )
    mig = emb[emb.index("fn ensure_vector_id_schema_v2(") :]
    mig = mig[: mig.index("fn vector_point(")]
    check(
        "21: the one-time migration exists and is marker-gated",
        'get_i64(conn, "vector_id_schema", 0)' in mig
        and 'set_i64(conn, "vector_id_schema", VERSION)' in mig,
    )
    check(
        "21: the migration only runs while the vector store is live "
        "(a disabled index cannot consume it)",
        "qdrant.is_some_and(|q| q.drop_collection())" in mig,
    )
    check(
        "21: the migration resets every chunk family for re-embed",
        "UPDATE knowledge_chunks SET embedding_state = 'not_indexed'" in mig
        and "UPDATE docs_chunks SET embedding_state = 'not_indexed', "
        "embedding_attempts = 0" in mig
        and "UPDATE conversation_chunks" in mig
        and "embedding_attempts = 0" in mig,
    )
    check(
        "21: the migration restarts the re-embed pipeline for all families",
        '"embed_knowledge_chunks"' in mig
        and '"embed_docs_chunks"' in mig
        and '"embed_conversation_chunks"' in mig,
    )
    chunk_fn = emb[emb.index("fn embed_pending_chunks(") : emb.index(
        "fn ensure_vector_id_schema_v2("
    )]
    check(
        "21: the migration runs before the pending list is read",
        chunk_fn.index("ensure_vector_id_schema_v2(conn, qdrant)?")
        < chunk_fn.index("list_pending_chunks(conn, table)?"),
    )


# ----------------------------------------------------------------- item 22
def verify_item22() -> None:
    workers = read(CORE, "workers.rs")
    after = workers[workers.index("fn on_after_initial_sync(") :]
    after = after[: after.index("fn run_rewrite_job(")]
    check(
        "22: on_after_initial_sync enqueues rebuild_activity",
        'enqueue_on(&conn, "sync", "rebuild_activity"' in after,
    )
    repair = workers[
        workers.index("fn enqueue_activity_rebuild_repair_once(") :
    ]
    repair = repair[: repair.index("fn parse_iso_to_unix(")]
    check(
        "22: the one-time repair exists for pre-fix mirrors",
        "activity_rebuild_repaired" in repair
        and "SELECT COUNT(*) FROM conversations" in repair
        and 'enqueue_on(conn, "sync", "rebuild_activity"' in repair,
    )
    check(
        "22: the repair only fires for databases that carry conversations",
        "conversations > 0 && repaired == 0" in repair,
    )
    maint = workers[workers.index("async fn maintenance(&self)") :]
    maint = maint[: maint.index("async fn run_rewrite_job(")]
    check(
        "22: the maintenance sweep runs the repair",
        "enqueue_activity_rebuild_repair_once(&conn);" in maint,
    )
    # The handler itself must still exist (the job enqueued above is real).
    check(
        "22: the rebuild_activity worker arm is still wired",
        '"rebuild_activity" =>' in workers
        and "crate::activity::rebuild_all(&conn)" in workers,
    )


# ----------------------------------------------------------------- item 23
def verify_item23() -> None:
    events = read(CORE, "events.rs")
    check(
        "23: SyncTrigger exists with manual/auto/refresh",
        "pub enum SyncTrigger" in events
        and "Manual," in events
        and "Auto," in events
        and "Refresh," in events,
    )
    completed = events[
        events.index("pub struct SyncCompletedEvent") :
    ]
    completed = completed[: completed.index("pub struct ConversationUpdatedEvent")]
    check(
        "23: SyncCompletedEvent carries the trigger (serde default for "
        "legacy payloads)",
        "pub trigger: SyncTrigger" in completed
        and "#[serde(default)]" in completed,
    )
    check(
        "23: the constructor takes the trigger",
        "pub fn sync_completed(kind: &str, processed: u32, errors: u32, "
        "trigger: SyncTrigger)" in events,
    )

    engine = read(CORE, "sync_engine.rs")
    check(
        "23: every engine entry point takes the trigger",
        "pub async fn initial_sync(\n        &self,\n        trigger: crate::"
        "events::SyncTrigger," in engine
        and "pub async fn incremental_sync(\n        &self,\n        trigger: "
        "crate::events::SyncTrigger," in engine
        and "pub async fn sync_docs_only(\n        &self,\n        trigger: "
        "crate::events::SyncTrigger," in engine,
    )
    check(
        "23: emit_sync_completed forwards the trigger on all three kinds",
        engine.count("self.emit_sync_completed(") == 3
        and 'self.emit_sync_completed("initial", processed, failed, trigger);'
        in engine
        and "self.emit_sync_completed(\"incremental\", processed, failed, "
        "trigger);" in engine
        and 'self.emit_sync_completed("docs", processed, failed, trigger);'
        in engine,
    )

    workers = read(CORE, "workers.rs")
    auto = workers[workers.index("async fn auto_sync(&self)") :]
    auto = auto[: auto.index("async fn maintenance(&self)")]
    check(
        "23: the adaptive timer runs with the Auto trigger",
        "initial_sync(crate::events::SyncTrigger::Auto)" in auto
        and "incremental_sync(crate::events::SyncTrigger::Auto)" in auto,
    )
    check(
        "23: the job lane runs with the Manual trigger",
        "initial_sync(crate::events::SyncTrigger::Manual)" in workers
        and "incremental_sync(crate::events::SyncTrigger::Manual)" in workers,
    )
    check(
        "23: both mirror-refresh emitters say Refresh",
        workers.count("SyncTrigger::Refresh") == 2,
    )

    app = read(SHELL, "app.rs")
    toast = app[app.index("fn sync_completed_toast(") :]
    toast = toast[: toast.index("const NAV_GROUPS")]
    check(
        "23: the toast mapper stays silent for non-manual triggers",
        "Option<(crate::toasts::ToastKind, String, Option<String>)>" in toast
        and "if e.trigger != spp_core::events::SyncTrigger::Manual"
        " return None;" in toast.replace("        return None;", " return None;")
        or "if e.trigger != spp_core::events::SyncTrigger::Manual" in toast,
    )
    check(
        "23: the shell only pushes a toast when the mapper speaks",
        "if let Some((kind, title, detail)) = sync_completed_toast(e)" in app,
    )

    eventing = read(CORE, "eventing.rs")
    check(
        "23: notify_sync_completed carries the trigger",
        "trigger: crate::events::SyncTrigger," in eventing,
    )
    settings = read(SHELL, "pages/settings.rs")
    sync_health = read(SHELL, "pages/sync_health.rs")
    check(
        "23: the manual UI paths (settings activation, sync buttons, docs "
        "sync) declare themselves manual",
        settings.count("SyncTrigger::Manual") == 2
        and sync_health.count("SyncTrigger::Manual") == 3,
    )


# ----------------------------------------------------------------- item 24
def verify_item24() -> None:
    jobs = read(CORE, "jobs.rs")
    check(
        "24: fresh jobs schema ships the result column",
        "error         TEXT,\n            result        TEXT," in jobs,
    )
    ensure = jobs[jobs.index("pub fn ensure_jobs_table(") :]
    ensure = ensure[: ensure.index("pub fn enqueue(")]
    check(
        "24: old databases heal the result column lazily",
        "pragma_table_info('jobs') WHERE name = 'result'" in ensure
        and "ALTER TABLE jobs ADD COLUMN result TEXT" in ensure,
    )
    check(
        "24: complete_with_result pins the output to the job row",
        "pub fn complete_with_result(conn: &Connection, id: i64, "
        "result: &str)" in jobs
        and "result = ?2" in jobs
        and "params![id, result]," in jobs,
    )
    check(
        "24: QueueJob surfaces the result",
        "pub result: Option<String>," in jobs
        and 'result: r.get("result")?,' in jobs,
    )

    workers = read(CORE, "workers.rs")
    dispatch = workers[
        workers.index("let outcome: Result<()> = async {") :
    ]
    dispatch = dispatch[: dispatch.index("if let Err(e) = outcome")]
    check(
        "24: the dispatcher captures the composer transforms' output",
        "let mut composer_result: Option<String> = None;" in dispatch
        and "composer_result = self.run_rewrite_job(&payload).await?;"
        in dispatch
        and "composer_result = self.run_translate_job(&payload).await?;"
        in dispatch,
    )
    check(
        "24: completion routes through complete_with_result",
        "match composer_result {" in dispatch
        and "jobs::complete_with_result(&conn, job_id, &text)?"
        in dispatch,
    )
    check(
        "24: both handlers return their output (Ok(None) = honest "
        "no-output)",
        "async fn run_rewrite_job(&self, payload: &serde_json::Value) "
        "-> Result<Option<String>>" in workers
        and "async fn run_translate_job(&self, payload: &serde_json::Value) "
        "-> Result<Option<String>>" in workers
        and workers.count("return Ok(None);") >= 4,
    )

    inbox = read(SHELL, "pages/inbox.rs")
    poll = inbox[inbox.index("fn poll_pending_ai(&mut self, db: &DbHandle)") :]
    poll = poll[: poll.index("fn poll_assistant(")]
    check(
        "24: the poll reads the job's own status/error/result",
        "SELECT status, error, result FROM jobs WHERE id = ?1" in poll,
    )
    check(
        "24: the ai_runs scrape is gone from the composer branch",
        "FROM ai_runs" not in poll
        and "ORDER BY id DESC LIMIT 1" not in poll
        and "conversation_id IS NULL" not in poll,
    )
    check(
        "24: a completed job without a result says so honestly",
        "produced no result" in poll,
    )


# ----------------------------------------------------------------- item 25
def verify_item25() -> None:
    settings = read(SHELL, "pages/settings.rs")
    combo = settings[
        settings.index('ComboBox::from_id_salt("settings-reply-status-after")') :
    ]
    combo = combo[: combo.index("ui.end_row();")]
    check(
        "25: the dropdown offers the platform's open status for "
        "await-agent",
        '("open", "open (await agent)")' in combo,
    )
    check(
        "25: the phantom waiting option is gone from the dropdown",
        '("waiting"' not in combo,
    )
    loader = settings[settings.index("reply_status_after: spp_core::settings::"
                                     "get_string(conn") :]
    loader = loader[: loader.index("log_level:")]
    check(
        "25: the loader maps a stored legacy waiting onto open",
        '"waiting" => "open".to_string(),' in loader,
    )
    check(
        "25: the loader still accepts pending/open/keep",
        '"pending" | "open" | "keep" => v,' in loader,
    )

    inbox = read(SHELL, "pages/inbox.rs")
    submit = inbox[inbox.index("pub fn submit_reply(") :]
    submit = submit[: submit.index("pub fn submit_note(")]
    check(
        "25: submit_reply maps open (and legacy waiting) onto the real "
        "status",
        '"open" | "waiting" => json!("open"),' in submit,
    )
    check(
        '25: no "waiting" ever rides the wire',
        'json!("waiting")' not in submit,
    )

    loopback = read(CORE, "helpscout.rs")
    reply = loopback[
        loopback.rindex("async fn create_reply_thread(") :
    ]
    reply = reply[: reply.index("async fn create_note_thread(")]
    check(
        "25: the loopback world maps open/waiting onto its active status",
        'Some("open" | "waiting") => "active".into(),' in reply,
    )


def main() -> int:
    verify_item21()
    verify_item22()
    verify_item23()
    verify_item24()
    verify_item25()
    print()
    if FAILURES:
        print(f"batch 20 harness: {len(FAILURES)} check(s) failed")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("batch 20 harness: all checks pass")
    return 0


if __name__ == "__main__":
    sys.exit(main())
