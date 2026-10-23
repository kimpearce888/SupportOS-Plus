#!/usr/bin/env python3
"""Batch 22 verification harness (clean-audit items 31-35).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..21 harness protocol.

Items:
  31  `api_concurrency` setting parsed/saved but never applied — every
      provider queue ran the hard-coded default forever
  32  webhook secret capped to 40 chars at registration but verified
      with the full secret — every delivery over the cap failed the
      signature check until the platform deactivated the hook
  33  team id written into the user-assignee slot (wrong user on id
      collision, phantom 'unassigned' otherwise); `assigned_team_id`
      parsed and dropped
  34  attribute extraction parses raw model output without the pipeline's
      fence-stripping extractor or json_mode — fenced/prose answers lost
  35  attribute extraction bypasses the prompt-redaction boundary every
      other AI path crosses
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


# ----------------------------------------------------------------- item 31
def verify_item31() -> None:
    real = read(CORE, "helpscout_real.rs")
    new = real[real.index("impl RealHelpScoutProvider {") :]
    new = new[: new.index("fn http_client")]
    check(
        "31: the provider constructor resolves the stored api_concurrency",
        "stored_api_concurrency(&db)" in new,
        "construction must start both queues at the configured width",
    )
    check(
        "31: the stored-setting resolver honours the env default",
        "fn stored_api_concurrency" in real
        and "load_env_config()" in real[real.index("fn stored_api_concurrency") :][:900],
    )
    setter = real[real.index("pub fn set_api_concurrency") :]
    setter = setter[: setter.index("pub fn api_concurrency")]
    check(
        "31: a live resize covers BOTH queues (docs included)",
        "self.queue.set_concurrency(n);" in setter
        and "self.docs_queue.set_concurrency(n);" in setter,
        "the docs queue used to be unresizeable",
    )
    queue = read(CORE, "api_queue.rs")
    check(
        "31: the queue's concurrency is observable",
        "pub fn concurrency(&self) -> usize" in queue,
    )
    settings = read(SHELL, "pages/settings.rs")
    save = settings[settings.index("fn save_general") :]
    save = save[: save.index("fn save_lmstudio")]
    check(
        "31: a changed setting reaches the live provider in-session",
        "real.set_api_concurrency(" in save,
        "save_general must apply the new width without a restart",
    )
    check(
        "31: the pre-save value is captured for change detection",
        "concurrency_before" in save,
    )


# ----------------------------------------------------------------- item 32
def verify_item32() -> None:
    webhook = read(CORE, "webhook.rs")
    check(
        "32: one canonical effective_secret lives at the signature boundary",
        "pub fn effective_secret(secret: &str) -> String" in webhook
        and "pub const SECRET_CAP: usize = 40" in webhook,
    )
    loopback = read(CORE, "loopback.rs")
    handle = loopback[loopback.index("fn handle_webhook") :]
    handle = handle[: handle.index("if let WebhookProcessResult::SecretNotConfigured")]
    check(
        "32: loopback verification resolves through the canonical cap",
        "crate::webhook::effective_secret(&secret)" in handle,
        "verification used the full stored secret while registration capped",
    )
    health = read(CORE, "webhook_health.rs")
    check(
        "32: registration delegates to the same canonical definition",
        "fn capped_secret(secret: &str) -> String {\n    crate::webhook::effective_secret(secret)\n}"
        in health.replace("\r\n", "\n"),
        "the two sides must share one definition to stay in agreement",
    )
    settings = read(SHELL, "pages/settings.rs")
    check(
        "32: the settings form says the truncation out loud",
        "platform caps webhook secrets at" in settings,
    )


# ----------------------------------------------------------------- item 33
def verify_item33() -> None:
    real = read(CORE, "helpscout_real.rs")
    mapper = real[real.index("fn map_conversation") :]
    mapper = mapper[: mapper.index("customer_id: v[")]
    check(
        "33: a team-typed assignee never lands in the user slot",
        "let assignee_is_team = assignee_type.as_deref() == Some(\"team\");" in mapper
        and "assignee_id: if assignee_is_team {\n                None" in mapper.replace("\r\n", "\n"),
    )
    sync = read(CORE, "sync.rs")
    upsert = sync[sync.index("pub fn upsert_conversation") :]
    upsert = upsert[: upsert.index("struct PreviousRow")]
    check(
        "33: the sync resolves the team's local id",
        "assigned_team_local" in upsert and 'local_id(conn, "teams", rid)' in upsert,
    )
    write = sync[sync.index("INSERT INTO conversations (remote_id") :]
    write = write[: write.index("ON CONFLICT")][:400]
    check(
        "33: the conversation row carries assigned_team_local_id",
        "assigned_team_local_id" in write,
    )
    inbox = read(CORE, "inbox.rs")
    check(
        "33: the inbox surfaces the team by name (list + detail)",
        inbox.count("LEFT JOIN teams tm ON tm.id = c.assigned_team_local_id") == 2
        and "assigned_team_name" in inbox,
    )
    page = read(SHELL, "pages/inbox.rs")
    check(
        "33: the assign combo shows 'team: <name>' instead of 'unassigned'",
        'format!("team: {t}")' in page,
    )


# ----------------------------------------------------------------- item 34
def verify_item34() -> None:
    attrs = read(CORE, "ai_attributes.rs")
    finish = attrs[attrs.index("pub fn finish_computation") :]
    finish = finish[: finish.index("fn merge_extraction")]
    check(
        "34: fresh output parses through the canonical fence-stripping extractor",
        "crate::ai_pipeline::extract_json(&raw)" in finish,
        "the strict serde_json::from_str on the raw answer lost fenced/prose output",
    )
    check(
        "34: the strict raw parse is gone",
        "serde_json::from_str::<Value>(&raw)" not in finish,
    )
    provider = read(CORE, "ai_provider.rs")
    check(
        "34: LocalAiProvider carries a structured (json_mode) lane",
        "async fn chat_structured(" in provider,
    )
    lm = read(CORE, "ai_lm_studio.rs")
    impl_block = lm[lm.index("impl LocalAiProvider for LmStudioProvider") :]
    impl_block = impl_block[: impl_block.index("pub struct OpenAiCompatibleClient")]
    check(
        "34: the LM Studio provider maps the structured lane onto json_mode",
        "async fn chat_structured" in impl_block and "json_mode: true" in impl_block,
    )
    run = attrs[attrs.index("pub async fn run_extraction") :]
    run = run[: run.index("pub fn finish_computation")]
    check(
        "34: the extraction rides the structured lane",
        ".chat_structured(" in run and ".chat(\n" not in run,
    )


# ----------------------------------------------------------------- item 35
def verify_item35() -> None:
    attrs = read(CORE, "ai_attributes.rs")
    run = attrs[attrs.index("pub async fn run_extraction") :]
    run = run[: run.index("pub fn finish_computation")]
    check(
        "35: the extraction prompt crosses the redaction boundary",
        "crate::security::redact_text(prompt, redaction_enabled)" in run,
        "the prompt carried raw customer messages straight to the model",
    )
    check(
        "35: the boundary honors the redaction_enabled setting (default on)",
        'get_bool(conn, "redaction_enabled", true)' in run,
    )
    check(
        "35: the redacted prompt is what the model receives",
        "content: safe_prompt" in run,
    )


# ----------------------------------------------------------------- tests
def verify_tests() -> None:
    attrs = read(CORE, "ai_attributes.rs")
    check(
        "tests: fenced/prose extraction + redaction boundary regressions exist",
        "fenced_and_prose_wrapped_extraction_outputs_are_parsed" in attrs
        and "run_extraction_crosses_the_redaction_boundary" in attrs,
    )
    webhook = read(CORE, "webhook.rs")
    check(
        "tests: the canonical cap is asserted at the signature boundary",
        "effective_secret_applies_the_platform_cap" in webhook,
    )
    loopback = read(CORE, "loopback.rs")
    check(
        "tests: over-the-cap verification agrees with registration",
        "webhook_secret_over_the_cap_verifies_against_the_truncation" in loopback,
    )
    real = read(CORE, "helpscout_real.rs")
    check(
        "tests: both queues + the team/user mapper regressions exist",
        "api_concurrency_setting_reaches_both_provider_queues" in real
        and "map_conversation_keeps_team_ids_out_of_the_user_slot" in real,
    )
    sync = read(CORE, "sync.rs")
    check(
        "tests: the end-to-end team-column regression exists",
        "team_assignment_lands_in_its_own_column_not_the_user_slot" in sync,
    )
    settings = read(SHELL, "pages/settings.rs")
    check(
        "tests: the in-session concurrency application regression exists",
        "api_concurrency_save_reaches_the_live_provider" in settings,
    )
    digest = read(CORE, "digest.rs")
    check(
        "tests: the pre-existing 00:00-09:00 UTC volume-spike flake is fixed",
        "let now_iso = chrono::Utc::now().format" in digest,
    )


def main() -> int:
    verify_item31()
    verify_item32()
    verify_item33()
    verify_item34()
    verify_item35()
    verify_tests()
    print()
    if FAILURES:
        print(f"{len(FAILURES)} check(s) failed:")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("all batch-22 checks pass")
    return 0


if __name__ == "__main__":
    sys.exit(main())
