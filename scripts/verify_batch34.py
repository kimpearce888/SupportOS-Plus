#!/usr/bin/env python3
"""Batch 34 verification harness (clean-audit items 147-149, 152-158).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..33 harness protocol.

Items:
  147  ensure_loaded-on-empty re-query pattern across the shell pages
  148  dead Runner/JobRegistry/bulk_*/EventEmitter machinery removed
  149  dead prompt surfaces removed (report_narrative, duplicate
       attribute prompt, ai_features runtime incl. mismatched translation-v1)
  152  dead copilot/embedding helper surface removed
  153  AppConfig helpscout/ai/sync/loopback fields removed
  154  dead hybrid_search fusion stack removed (RRF_K kept)
  155  legacy FTS tables and chunk_version column retired
  156  connector AI-search/auth-edit + data_tools runtime retired
  157  AppDir staging cleaned before packaging
  158  appimagetool pinned (version + sha256) and verified before execution
"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORE = ROOT / "crates" / "core" / "src"
SHELL = ROOT / "crates" / "app-shell" / "src"
XTASK = ROOT / "crates" / "xtask" / "src"
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


# ----------------------------------------------------------------- item 147
def verify_item147() -> None:
    pages = {
        "outreach.rs": "campaigns",
        "organizations.rs": "orgs",
        "connectors.rs": "connectors",
        "notifications.rs": "records",
        "incidents.rs": "rows",
        "automation.rs": "rules",
    }
    for page, list_field in pages.items():
        code = read_code(SHELL, f"pages/{page}")
        check(
            f"147: {page} no longer gates ensure_loaded on {list_field}.is_empty()",
            f"if self.{list_field}.is_empty() {{\n            self.reload" not in code
            and f"if self.{list_field}.is_empty() {{\n            self.reload_list" not in code,
        )
        check(
            f"147: {page} carries the loaded latch (field, init, set, invalidate)",
            "loaded: bool," in code
            and "loaded: false," in code
            and "self.loaded = true;" in code
            and "self.loaded = false;" in code,
        )
        check(
            f"147: {page} ensure_loaded gates on !loaded",
            "if !self.loaded {" in code,
        )

    ops = read_code(SHELL, "pages/operations.rs")
    check(
        "147: operations suggestions lane gates on suggestions_loaded",
        "suggestions_loaded: bool," in ops
        and "if self.workload.is_some() && !self.suggestions_loaded {" in ops,
    )
    search = read_code(SHELL, "pages/search.rs")
    check(
        "147: search auto-refetch guard trims the query",
        "!self.query.trim().is_empty()" in search,
    )


# ----------------------------------------------------------------- item 148
def verify_item148() -> None:
    check(
        "148: runner.rs is deleted",
        not (CORE / "runner.rs").exists(),
    )
    check(
        "148: the runner module declaration is gone",
        "pub mod runner;" not in read(CORE, "lib.rs"),
    )
    check(
        "148: the EventEmitter trait is gone",
        "pub trait EventEmitter" not in read_code(CORE, "events.rs"),
    )
    check(
        "148: the EventBus EventEmitter impl is gone",
        "impl crate::events::EventEmitter for EventBus" not in read(CORE, "eventing.rs"),
    )
    workers = read_code(CORE, "workers.rs")
    check(
        "148: the bulk_* dispatch arm is gone",
        'k if k.starts_with("bulk_")' not in workers,
    )
    check(
        "148: execute_bulk_action is gone",
        "fn execute_bulk_action" not in workers,
    )
    check(
        "148: the EventBus inherent emit stays live (workers still publish)",
        "use crate::eventing::EventBus;" in workers,
    )


# ----------------------------------------------------------------- item 149
def verify_item149() -> None:
    pipeline = read_code(CORE, "ai_pipeline.rs")
    check(
        "149: the dead report_narrative runner is gone",
        "pub async fn report_narrative" not in pipeline,
    )
    prompts = read_code(CORE, "ai_prompts.rs")
    for needle in [
        "REPORT_NARRATIVE_SYSTEM",
        "build_report_narrative_user",
        "PROMPT_VERSIONS_REPORT_NARRATIVE",
        "ATTRIBUTE_EXTRACTION_SYSTEM",
        "build_attribute_extraction_user",
        "PROMPT_VERSIONS_ATTRIBUTE_EXTRACTION",
    ]:
        check(f"149: ai_prompts no longer carries {needle}", needle not in prompts)
    attributes = read(CORE, "ai_attributes.rs")
    check(
        "149: the live attribute prompt still lives in ai_attributes.rs",
        "const ATTRIBUTE_EXTRACTION_SYSTEM" in attributes
        and "ATTRIBUTE_EXTRACTION_PROMPT_VERSION" in attributes,
    )
    features = read(CORE, "ai_features.rs")
    check(
        "149: the migrations stay wired",
        "pub fn apply_m011_to_m013" in features
        and "M011_SQL" in features
        and "M013_SQL" in features,
    )
    for needle in [
        "parse_coaching_suggestions",
        "TRANSLATION_PROMPT_VERSION",
        "build_translation_prompt",
        "pub fn store_qa",
        "pub fn get_qa",
        "parse_suggestions",
        "pub fn set_memory",
        "pub fn get_memory",
        "is_confirmed_pattern",
    ]:
        check(f"149: ai_features dead runtime {needle} is gone", needle not in features)
    check(
        "149: the mismatched translation-v1 label is gone (the live one is translation_v1)",
        "translation-v1" not in read_code(CORE, "ai_features.rs"),
    )


# ----------------------------------------------------------------- item 152
def verify_item152() -> None:
    copilot = read_code(CORE, "copilot.rs")
    for needle in [
        "pub fn truncate_result",
        "pub fn should_stop",
        "pub fn validate_tool_allowed",
        "pub fn tool_allowlist",
        "pub const fn max_tool_rounds",
        "pub const fn max_tool_calls",
        "pub const fn tool_result_max_chars",
    ]:
        check(f"152: copilot helper {needle} is gone", needle not in copilot)
    check(
        "152: the live tool-result cap still truncates by characters",
        ".chars()" in copilot and "TOOL_RESULT_MAX_CHARS" in copilot,
    )
    embeddings = read_code(CORE, "embeddings.rs")
    for needle in [
        "pub async fn embed_with_cache",
        "fn lookup_cache",
        "fn store_cache",
        "MAX_EMBED_RETRIES",
        "pub fn count_cache_entries",
        "pub fn trim_cache_per_model",
        "pub fn clear_cache",
    ]:
        check(f"152: embeddings dead surface {needle} is gone", needle not in embeddings)
    check(
        "152: the content hash stays (sync_engine consumes it)",
        "pub fn content_hash" in embeddings,
    )
    lm = read_code(CORE, "ai_lm_studio.rs")
    check(
        "152: LmStudioProvider::new / with_base_url / Default are gone",
        "pub fn new() -> Self" not in lm
        and "pub fn with_base_url(" not in lm
        and "impl Default for LmStudioProvider" not in lm,
    )
    check(
        "152: with_client stays (production constructor)",
        "pub fn with_client(" in lm,
    )
    it = read(CORE, "it_lm_studio.rs")
    check(
        "152: the integration tests build the provider through with_client",
        "LmStudioProvider::with_base_url(" not in it
        and "LmStudioProvider::with_client(OpenAiCompatibleClient::new(" in it,
    )


# ----------------------------------------------------------------- item 153
def verify_item153() -> None:
    config = read(CORE, "config.rs")
    for needle in ["pub struct AppConfig", "pub struct HelpScoutConfig",
                   "pub struct AiConfig", "impl Default for AppConfig",
                   "pub fn redacted("]:
        check(f"153: config.rs no longer carries {needle}", needle not in config)
    main_code = read_code(SHELL, "main.rs")
    check(
        "153: the shell reads default_data_dir() directly",
        "AppConfig" not in main_code and "default_data_dir()" in main_code,
    )
    check(
        "153: the item-107 redaction proof stays pinned in config.rs",
        "fn redact_string" in config,
    )


# ----------------------------------------------------------------- item 154
def verify_item154() -> None:
    hybrid = read(CORE, "hybrid_search.rs")
    check(
        "154: the shared RRF constant stays (search.rs consumes it)",
        "pub const RRF_K: u32 = 60;" in hybrid,
    )
    for needle in [
        "pub fn fuse_results",
        "pub fn merge_doc_hits",
        "pub fn cosine_similarity",
        "pub fn hybrid_search",
        "pub struct HybridResult",
        "pub struct MergedDocHit",
        "DOC_RRF_K",
    ]:
        check(f"154: dead fusion surface {needle} is gone", needle not in hybrid)
    search = read_code(CORE, "search.rs")
    check(
        "154: the live lanes still read the pinned constant",
        "crate::hybrid_search::RRF_K" in search,
    )


# ----------------------------------------------------------------- item 155
def verify_item155() -> None:
    search = read(CORE, "search.rs")
    check(
        "155: the legacy FTS tables are no longer created",
        "CREATE VIRTUAL TABLE IF NOT EXISTS conversations_fts" not in search
        and "CREATE VIRTUAL TABLE IF NOT EXISTS customers_fts" not in search,
    )
    check(
        "155: the legacy writers are gone",
        "pub fn index_conversation(" not in search
        and "pub fn index_customer(" not in search,
    )
    check(
        "155: the live per-conversation FTS writer stays",
        "pub fn index_conversation_fts(" in search,
    )
    check(
        "155: the rebuild list no longer touches the legacy tables",
        '"conversations_fts",' not in search and '"customers_fts",' not in search,
    )
    for rel in ["search.rs", "mirror_tables.rs", "sync_engine.rs", "knowledge_store.rs",
                "embeddings.rs"]:
        code = read(CORE, rel)
        check(f"155: chunk_version is gone from {rel}", "chunk_version" not in code)
    leftovers = list(ROOT.glob("crates/**/chunk_version*"))
    check("155: no chunk_version artifacts remain", not leftovers)


# ----------------------------------------------------------------- item 156
def verify_item156() -> None:
    connectors = read_code(CORE, "connectors.rs")
    for needle in [
        "pub fn search_for_ai",
        "pub fn ai_visible_connector_names",
        "fn get_by_name",
        "pub fn patch(",
    ]:
        check(f"156: connectors dead surface {needle} is gone", needle not in connectors)
    check(
        "156: the live connector surface stays (list/create/delete/refresh/rows)",
        all(f"pub fn {fn}(" in connectors
            for fn in ["list", "create", "delete", "list_rows"])
        and "pub async fn refresh(" in connectors
        and "pub async fn validate_config(" in connectors,
    )
    data_tools = read(CORE, "data_tools.rs")
    check(
        "156: the migration stays wired",
        "pub fn apply_m026_to_m027" in data_tools and "M026_TO_M027_SQL" in data_tools,
    )
    for needle in ["create_object_type", "create_connector", "export_db",
                   "export_settings", "import_settings", "validate_ssrf"]:
        check(f"156: data_tools dead runtime {needle} is gone", needle not in data_tools)


# ----------------------------------------------------------------- item 157
def verify_item157() -> None:
    package = read(XTASK, "package.rs")
    check(
        "157: stage_appdir wipes a leftover staging tree",
        "if appdir.exists() {\n        fs::remove_dir_all(appdir)?;\n    }" in package,
    )
    check(
        "157: a regression test pins the clean",
        "fn stage_appdir_cleans_a_stale_staging_tree(" in package,
    )


# ----------------------------------------------------------------- item 158
def verify_item158() -> None:
    package = read(XTASK, "package.rs")
    package_code = read_code(XTASK, "package.rs")
    check(
        "158: the URL is version-pinned (no moving continuous build)",
        "1.9.1/appimagetool-x86_64.AppImage" in package
        and "download/continuous" not in package_code,
    )
    check(
        "158: the sha256 pin is present",
        "APPIMAGETOOL_SHA256: &str =" in package
        and "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0" in package,
    )
    check(
        "158: the cached tool is verified before use and after download",
        "verify_appimagetool(&cached)" in package,
    )
    check(
        "158: a mismatched tool is refused (ensure! in the verifier)",
        "checksum mismatch" in package and "refusing to execute an unverified tool" in package,
    )
    cargo = read(ROOT, "crates/xtask/Cargo.toml")
    check("158: xtask hashes with sha2 (same major as core)", 'sha2 = "0.11"' in cargo)


def main() -> int:
    for verify in [
        verify_item147,
        verify_item148,
        verify_item149,
        verify_item152,
        verify_item153,
        verify_item154,
        verify_item155,
        verify_item156,
        verify_item157,
        verify_item158,
    ]:
        verify()
    print()
    if FAILURES:
        print(f"VERIFY-BATCH34: {len(FAILURES)} CHECK(S) FAILED")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("VERIFY-BATCH34: ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
