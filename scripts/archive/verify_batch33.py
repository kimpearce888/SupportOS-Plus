#!/usr/bin/env python3
"""Batch 33 verification harness (clean-audit items 137-146).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..32 harness protocol.

Items:
  137  filter/assign refs load once per session (inbox.rs)
  138  log_level saved-never-read wired; display_timezone dead control
       removed (logging.rs, main.rs, settings.rs)
  139  qdrant_enabled hint references a nonexistent control; Integrations
       tab hint wrong (settings.rs, ai_center.rs)
  140  unconfirmed destructive deletes route through the item-70 dialog
  141  business-hours `days` unvalidated, silently ignored
       (settings.rs + sla.rs)
  142  ops drill-down layers onto stale filters (app.rs)
  143  `#778` number search silently dropped (inbox.rs)
  144  support-graph invalidation clears results nothing refetches
  145  onboarding save_step dead; setting unread (onboarding.rs)
  146  recipient snapshot cap 5000, matched discarded (outreach.rs)
"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SHELL = ROOT / "crates" / "app-shell" / "src"
CORE = ROOT / "crates" / "core" / "src"
PAGES = SHELL / "pages"
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


# ----------------------------------------------------------------- item 137
def verify_item137() -> None:
    inbox = read_code(PAGES, "inbox.rs")
    body = function_body(inbox, "pub fn invalidate(&mut self)")
    check(
        "137: invalidation clears the refs caches",
        "self.users.clear();" in body
        and "self.mailboxes.clear();" in body
        and "self.tags.clear();" in body
        and "self.directory = Directory::default();" in body,
    )
    check(
        "137: the regression test ships",
        "fn invalidation_refreshes_the_filter_refs" in inbox,
    )


# ----------------------------------------------------------------- item 138
def verify_item138() -> None:
    logging = read_code(CORE, "logging.rs")
    check(
        "138: the filter sits behind a reload layer with a live handle",
        "reload::Layer::new(env_filter)" in logging
        and "pub fn apply_log_level" in logging,
    )
    main = read_code(SHELL, "main.rs")
    check(
        "138: boot applies the stored log_level to the live subscriber",
        'spp_core::settings::get_string(conn, "log_level")' in main
        and "spp_core::logging::apply_log_level(&level);" in main,
    )
    settings = read_code(PAGES, "settings.rs")
    save = function_body(settings, "fn save_general")
    check(
        "138: the settings save applies the level live",
        "spp_core::logging::apply_log_level(&general.log_level);" in save,
    )
    check(
        "138: the dead display_timezone control is removed",
        "display_timezone" not in settings,
    )


# ----------------------------------------------------------------- item 139
def verify_item139() -> None:
    settings = read_code(PAGES, "settings.rs")
    vector_ui = function_body(settings, "fn vector_index_ui")
    check(
        "139: the Vector-index tab owns a real qdrant_enabled toggle",
        'spp_core::settings::get_bool(conn, "qdrant_enabled", true)' in vector_ui
        and 'spp_core::settings::set_bool(conn, "qdrant_enabled", enabled)' in vector_ui,
    )
    general = function_body(settings, "pub struct GeneralSettings")
    check(
        "139: the General form no longer blind-saves qdrant_enabled",
        "qdrant_enabled" not in general,
    )
    ai_center = read_code(PAGES, "ai_center.rs")
    check(
        "139: the LM Studio hint names the real Settings tab",
        "Settings → LM Studio" in ai_center
        and "Settings → Integrations" not in ai_center,
    )


# ----------------------------------------------------------------- item 140
def verify_item140() -> None:
    modrs = read_code(PAGES, "mod.rs")
    for action in (
        "DeleteKnowledgeDoc",
        "DeleteCustomType",
        "DeleteCustomObject",
        "DeleteConnector",
        "DeleteSegment",
        "ClearDiagnosticsLog",
    ):
        check(f"140: DestructiveAction carries {action}", action in modrs)
    for request in (
        "pub fn delete_knowledge_doc",
        "pub fn delete_custom_type",
        "pub fn delete_custom_object",
        "pub fn delete_connector",
        "pub fn delete_segment",
        "pub fn clear_diagnostics_log",
    ):
        check(f"140: ConfirmRequest::{request.split('pub fn ')[1]} exists", request in modrs)

    app = read_code(SHELL, "app.rs")
    exec_body = function_body(app, "fn execute_confirmed")
    for call in (
        "self.knowledge.delete_document(&self.db, id)",
        "self.custom_objects.delete_type(&self.db, id)",
        "self.custom_objects.delete_object(&self.db, id)",
        "self.connectors.delete(&self.db, id)",
        "self.outreach.delete_segment(&self.db, id)",
        "self.diagnostics.clear_log(&self.db)",
    ):
        check(f"140: execute_confirmed delegates to {call.split('(')[0]}", call in exec_body)

    knowledge = read_code(PAGES, "knowledge.rs")
    check(
        "140: the knowledge delete button arms the dialog",
        "ConfirmRequest::delete_knowledge_doc(" in knowledge,
    )
    custom = read_code(PAGES, "custom_objects.rs")
    check(
        "140: the type/object deletes arm the dialog",
        "ConfirmRequest::delete_custom_type(" in custom
        and "ConfirmRequest::delete_custom_object(" in custom,
    )
    connectors = read_code(PAGES, "connectors.rs")
    check(
        "140: the connector delete arms the dialog",
        "ConfirmRequest::delete_connector(id, &name)" in connectors,
    )
    outreach = read_code(PAGES, "outreach.rs")
    check(
        "140: the segment delete arms the dialog",
        "ConfirmRequest::delete_segment(" in outreach,
    )
    diagnostics = read_code(PAGES, "diagnostics.rs")
    check(
        "140: the clear-log button arms the dialog",
        "ConfirmRequest::clear_diagnostics_log(" in diagnostics,
    )


# ----------------------------------------------------------------- item 141
def verify_item141() -> None:
    settings = read_code(PAGES, "settings.rs")
    check(
        "141: the weekday list is parsed, range-checked and normalized on save",
        "fn parse_weekdays" in settings
        and "let days = parse_weekdays(&row.days)?;" in settings,
    )
    sla = read_code(CORE, "sla.rs")
    body = function_body(sla, "fn effective_config")
    check(
        "141: out-of-range days degrade to the default schedule, not disable",
        "let in_range: Vec<i64> = parsed" in body
        and "if !in_range.is_empty()" in body,
    )
    check(
        "141: the regression tests ship",
        "fn business_hours_days_are_validated_and_normalized" in settings
        and "fn audit141_out_of_range_days_fall_back_to_the_default_schedule" in sla,
    )


# ----------------------------------------------------------------- item 142
def verify_item142() -> None:
    app = read_code(SHELL, "app.rs")
    body = function_body(app, "NavRequest::InboxTile(tile)")
    check(
        "142: the drill-down resets the bar instead of layering",
        "FilterBar {\n                    ops: Some(tile)," in body
        and "..crate::pages::inbox::FilterBar::default()" in body,
    )
    check(
        "142: the regression test ships",
        "fn audit142_ops_drill_down_resets_stale_bar_filters" in app,
    )


# ----------------------------------------------------------------- item 143
def verify_item143() -> None:
    inbox = read_code(PAGES, "inbox.rs")
    check(
        "143: the # field strips the hash before parsing",
        "pub fn parse_number_entry" in inbox
        and "self.bar.number = parse_number_entry(&self.number_edit);" in inbox,
    )
    check(
        "143: the regression test ships",
        "fn hash_prefixed_number_entries_parse" in inbox,
    )


# ----------------------------------------------------------------- item 144
def verify_item144() -> None:
    graph = read_code(PAGES, "support_graph.rs")
    check(
        "144: invalidation schedules the results refetch",
        "self.results_stale = true;" in graph,
    )
    check(
        "144: the next frame refetches the cleared search",
        "fn refetch_if_stale" in graph and "self.refetch_if_stale(db);" in graph,
    )
    check(
        "144: the regression test ships",
        "fn invalidation_refetches_the_cleared_search" in graph,
    )


# ----------------------------------------------------------------- item 145
def verify_item145() -> None:
    onboarding = read_code(PAGES, "onboarding.rs")
    check(
        "145: the dead save_step writer is removed",
        "save_step" not in onboarding and "onboarding_step" not in onboarding,
    )


# ----------------------------------------------------------------- item 146
def verify_item146() -> None:
    outreach = read_code(PAGES, "outreach.rs")
    create = function_body(outreach, "fn create_campaign")
    check(
        "146: the fixed 5000-row snapshot cap is gone",
        "SNAPSHOT_LIMIT" not in create and "engine.preview(&tree, 1, i64::MAX)" in create,
    )
    check(
        "146: the matched total is no longer discarded",
        "let (matched, rows) = engine_result;" in create,
    )
    check(
        "146: the regression test ships",
        "fn campaign_snapshot_carries_every_matched_recipient_past_the_cap" in outreach,
    )


def main() -> int:
    for verify in (
        verify_item137,
        verify_item138,
        verify_item139,
        verify_item140,
        verify_item141,
        verify_item142,
        verify_item143,
        verify_item144,
        verify_item145,
        verify_item146,
    ):
        verify()
    print()
    if FAILURES:
        print(f"{len(FAILURES)} check(s) FAILED:")
        for failure in FAILURES:
            print(f"  - {failure}")
        return 1
    print("All batch-33 checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
