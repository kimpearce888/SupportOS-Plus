#!/usr/bin/env python3
"""Batch 36 verification harness (clean-audit items 56, 150, 151).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact
wiring landed, mirroring the batch-16..35 harness protocol. This batch
WIRES the three formerly parked surfaces (the owner's explicit "wire it"
decisions), so the needles assert live call sites, not removals.

Items:
  56   report-builder surface rendered (Builder tab: run_builder_report,
       parse_report_config, saved-report CRUD; Insights tab: health facts
       over get_dashboard_metrics, effectiveness, friction stack with the
       bounded recompute, graph nodes; per-customer support milestones
       with real writers)
  150  intelligence tracking wired to real writers (workers intelligence
       sweep persisting live SLA verdicts, sla_configs thresholds for
       unmonitored mailboxes, knowledge gaps on zero-hit searches, doc
       freshness stamps on ingest + stale maintenance)
  151  ai_settings write API wired to the AI Center provider panel
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


def read_code(root: Path, p: str) -> str:
    """Source without comment lines — for checks whose needle may
    legitimately appear inside a comment describing the pre-fix defect."""
    return "\n".join(
        line
        for line in read(root, p).splitlines()
        if not line.lstrip().startswith("//")
    )


# ----------------------------------------------------------------- item 56
def verify_item56() -> None:
    page = read_code(SHELL, "pages/reports.rs")
    core = read_code(CORE, "reports.rs")
    customers = read_code(SHELL, "pages/customers.rs")

    # The builder engine is rendered: the page calls run_builder_report
    # and the config round-trip helpers back the saved-report CRUD.
    check("56 builder: page runs run_builder_report",
          "run_builder_report(conn, &config)" in page)
    check("56 builder: page renders parse_report_config on load",
          "parse_report_config(config)" in page)
    check("56 builder: page saves reports", "save_report(conn, &name, &config)" in page)
    check("56 builder: page deletes saved reports",
          "delete_saved_report(conn, id)" in page)
    check("56 builder: page lists saved reports",
          "with_read(spp_core::reports::list_saved_reports)" in page)
    check("56 builder: Builder tab registered",
          "Tab::Builder" in page and "Self::Builder," in page)
    check("56 builder: build_config validates dates/ids/limit",
          'Err("dates use the YYYY-MM-DD form".into())' in page
          and "'{item}' is not an id" in page)

    # The dashboard surface is rendered on the Insights tab.
    check("56 insights: health facts read (dashboard metrics composed)",
          "get_health_facts(" in page
          and "u32::try_from(self.days).unwrap_or(30)" in page)
    check("56 insights: effectiveness report read",
          "get_effectiveness_report(conn).ok()" in page)
    check("56 insights: high-friction count read",
          "count_high_friction(conn).unwrap_or(0)" in page)
    check("56 insights: Insights tab registered",
          "Tab::Insights" in page and "Self::Insights," in page)

    # The friction stack has a real writer: the bounded recompute that
    # also records deduped customer_timeline milestones.
    check("56 friction: bounded recompute exists in core",
          "pub fn recompute_recent_friction" in core)
    check("56 friction: recompute writes milestone timeline events",
          'record_timeline_event(conn, customer_id, "high_friction"' in core)
    check("56 friction: milestone dedup keyed by conversation via json_extract",
          "json_extract(event_data_json, '$.conversation_id')" in core)
    check("56 friction: page recomputes through core, not inline SQL",
          "recompute_recent_friction(conn, 100)" in page)

    # The graph-node surface is live from the shell.
    check("56 graph: page lists nodes", "list_graph_nodes(conn, Some(50))" in page)
    check("56 graph: page adds nodes", "add_graph_node(" in page)
    check("56 graph: page reads a node", "get_graph_node(conn, node_id)" in page)
    check("56 graph: page reads neighbors", "get_graph_neighbors(conn, &n.kind," in page)

    # The per-customer support-milestone timeline is rendered with its
    # reader on the Customers page.
    check("56 timeline: customers page reads get_timeline",
          "get_timeline(conn, customer_id, 50)" in customers)
    check("56 timeline: milestones rendered in the profile",
          "milestones_ui(ui, &self.milestones)" in customers
          and "fn milestones_ui" in customers)


# ----------------------------------------------------------------- item 150
def verify_item150() -> None:
    intel = read_code(CORE, "intelligence_features.rs")
    workers = read_code(CORE, "workers.rs")
    kstore = read_code(CORE, "knowledge_store.rs")
    knowledge = read_code(SHELL, "pages/knowledge.rs")
    page = read_code(SHELL, "pages/reports.rs")

    # The breach log gains rows from the LIVE evaluation (single source of
    # truth), deduped, with the milestone timeline written alongside.
    check("150 sla: sweep persists the live sla_alerts verdicts",
          "crate::sla::sla_alerts(conn)?" in intel
          and "pub fn sweep_sla_breaches" in intel)
    check("150 sla: persisted rows dedupe per conversation + kind",
          "SELECT EXISTS(SELECT 1 FROM sla_breaches" in intel)
    check("150 sla: breach records the customer milestone",
          '"sla_breach"' in intel)
    check("150 sla: unmonitored sweep honors sla_configs with defaults",
          "get_sla_config(conn, mailbox_id)?" in intel
          and "None => (24.0, 72.0)," in intel)
    check("150 sla: the live evaluator stays the only breach authority",
          "target_kind.as_str()" in intel)

    # The worker owns the cadence (the tick + spawn + guard pattern).
    check("150 workers: intelligence_sweep_tick calls the combined sweep",
          "crate::intelligence_features::sweep(&conn)" in workers)
    check("150 workers: sweep spawn block with its own settings cadence",
          '"intelligence_sweep_seconds", 900' in workers)
    check("150 workers: tick guard present (struct + new + restart reset)",
          workers.count("sweeping_intelligence") >= 3)
    check("150 workers: combined sweep runs every leg despite partial failure",
          "pub fn sweep(conn: &Connection) -> Result<()>" in intel
          and "sweep_doc_freshness(conn)" in intel)

    # Knowledge gaps: zero-hit searches gain rows; the list is rendered.
    check("150 gaps: the knowledge page records zero-hit searches",
          "record_knowledge_gap(conn, &q)" in knowledge
          and "self.articles.is_empty()" in knowledge)
    check("150 gaps: the knowledge page lists the tracked gaps",
          "list_knowledge_gaps(conn, 10)" in knowledge)
    check("150 gaps: the writer dedupes by hit_count upsert",
          "SET hit_count = hit_count + 1" in intel)

    # Doc freshness: ingest stamps, the sweep maintains, the count renders.
    check("150 freshness: successful upserts stamp the freshness row",
          kstore.count("update_doc_freshness(") >= 2,
          )
    check("150 freshness: the unchanged re-ingest path stamps too",
          "update_doc_freshness(conn, *id, Some(&checksum), false)" in kstore
          and "return Ok((*id, false));" in kstore)
    check("150 freshness: sweep marks/clears stale + prunes orphans",
          "stale_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')" in intel
          and "doc_id NOT IN (SELECT id FROM knowledge_documents)" in intel)
    check("150 freshness: stale window keys on the live freshness setting",
          '"freshness_stale_days", 180' in intel)
    check("150 freshness: the tracked count renders on the knowledge page",
          "count_stale_docs(conn).unwrap_or(0)" in knowledge)

    # The sla_configs write API is reachable from the SLA tab editor.
    check("150 sla: the SLA tab saves thresholds through set_sla_config",
          "set_sla_config(conn, mailbox_id, fr, res)" in page)
    check("150 sla: picking a mailbox reads its thresholds",
          "get_sla_config(conn, mailbox_id)" in page)
    check("150 sla: the tracked breach count renders",
          "count_sla_breaches(conn).unwrap_or(0)" in page)


# ----------------------------------------------------------------- item 151
def verify_item151() -> None:
    shell = read_code(SHELL, "pages/ai_center.rs")
    core = read_code(CORE, "ai_center.rs")

    # The four write-API functions are all called from the panel's save.
    for fn in (
        "set_provider_kind(conn, kind)?",
        "set_chat_model(conn, &chat)?",
        "set_embedding_model(conn, &embed, dim)?",
        "set_base_url(conn, &base)?",
    ):
        check(f"151 ai_settings: panel calls {fn.split('(')[0]}", fn in shell)

    # The record is loaded through the core reader, and the ready badge
    # consumes ai_features_enabled (previously caller-less).
    check("151 ai_settings: the stored record loads via get_ai_status",
          "spp_core::ai_center::get_ai_status(conn).ok()" in shell)
    check("151 ai_settings: ai_features_enabled has a live caller",
          "ai_features_enabled(&record)" in shell)
    check("151 ai_settings: dirty forms survive reload, clean forms re-seed",
          "fn seed_ai_record" in shell and "self.ai_form_clean" in shell)
    check("151 ai_settings: embedding write validates a positive dim",
          "embedding dim must be a positive integer" in shell)
    check("151 ai_settings: the pipeline fallback still consumes the row",
          "status.chat_model.clone()" in read_code(CORE, "ai_pipeline.rs"))

    # Core regression pins the setters round-trip.
    check("151 ai_settings: core round-trip regression exists",
          "fn setters_round_trip_through_get_ai_status" in core)
    check("151 ai_settings: ready-indicator regression exists",
          "fn ai_features_enabled_requires_provider_and_chat_model" in core)


def main() -> int:
    for verify in [
        verify_item56,
        verify_item150,
        verify_item151,
    ]:
        verify()
    print()
    if FAILURES:
        print(f"VERIFY-BATCH36: {len(FAILURES)} CHECK(S) FAILED")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("VERIFY-BATCH36: ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
