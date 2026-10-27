#!/usr/bin/env python3
"""Batch 26 verification harness (clean-audit items 67-76).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..25 harness protocol.

Items:
  67  translation_cache never pruned; retention policy omitted it
  68  audit trail never surfaced; the dead reader dropped non-JSON
      payloads to null
  69  toasts expired on wall-clock while the window was hidden — errors
      silently lost the moment the window returned
  70  the confirm-dialog system existed but was never armed; destructive
      actions ran unconfirmed
  71  SPP_SMOKE returned before the shell/renderer boot — renderer
      regressions passed CI unexercised
  72  palette Enter on zero hits defocused the input; keystrokes
      swallowed until the user clicked back in
  73  transform completion overwrote typing made while the job ran
  74  the role-model field persisted + enqueued golden_eval per keystroke
  75  copilot submitted on a global Enter with no focus check
  76  sync invalidation destroyed the open Library edit form
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


# ----------------------------------------------------------------- item 67
def verify_item67() -> None:
    maintenance = read(CORE, "maintenance.rs")
    retention = maintenance[maintenance.index("pub fn enforce_retention") :]
    retention = retention[: retention.index("#[cfg(test)]")]
    check(
        "67: enforce_retention covers translation_cache",
        '("translation_cache", "created_at")' in retention,
    )
    check(
        "67: the table list keeps the original four tables",
        all(
            f'("{t}",' in retention
            for t in ("webhook_events", "application_errors", "audit_log", "ai_runs")
        ),
    )
    translation = read(CORE, "translation.rs")
    check(
        "67: the cache schema still stamps created_at (the prune column)",
        "created_at       TEXT NOT NULL DEFAULT (datetime('now'))" in translation,
    )


# ----------------------------------------------------------------- item 68
def verify_item68() -> None:
    audit = read(CORE, "audit.rs")
    check(
        "68: non-JSON payloads surface verbatim instead of null",
        'unwrap_or_else(|_| Value::String(v.to_string()))' in audit,
    )
    check(
        "68: the old drop-to-null reader is gone",
        "s.and_then(|v| serde_json::from_str::<Value>(v).ok())" not in audit,
    )
    diagnostics = read(SHELL, "pages/diagnostics.rs")
    check(
        "68: the Diagnostics page reads the audit trail",
        "spp_core::audit::list_audit(conn, None, 50)" in diagnostics,
    )
    check(
        "68: the audit trail is a rendered card",
        '"Audit trail"' in diagnostics,
    )
    check(
        "68: audit rows reload with the page",
        "audit: Option<Vec<AuditRow>>" in diagnostics
        and "self.audit = None" in diagnostics,
    )
    # The reader must actually be reachable from the shipped UI.
    app = read(SHELL, "app.rs")
    check(
        "68: the Diagnostics page still renders through the shell",
        "PageKey::Diagnostics =>" in app,
    )


# ----------------------------------------------------------------- item 69
def verify_item69() -> None:
    toasts = read(SHELL, "toasts.rs")
    check(
        "69: toast lifetime is a remaining budget, not a wall-clock deadline",
        "pub remaining: Duration" in toasts,
    )
    check(
        "69: lifetime starts at the first rendered pass",
        "pub last_paint: Option<Instant>" in toasts,
    )
    check(
        "69: the per-pass burn is capped (hidden windows cannot burn a lifetime)",
        "const MAX_PASS_GAP: Duration = Duration::from_millis(1600);" in toasts,
    )
    check(
        "69: the burn happens in ui() with the cap applied",
        ".min(MAX_PASS_GAP)" in toasts,
    )
    check(
        "69: the wall-clock deadline field is gone",
        "expires_at" not in toasts,
    )
    app = read(SHELL, "app.rs")
    check(
        "69: the shell still renders the toast stack every ui pass",
        "self.toasts.ui(ui.ctx());" in app,
    )


# ----------------------------------------------------------------- item 70
def verify_item70() -> None:
    pages = read(SHELL, "pages/mod.rs")
    check(
        "70: NavRequest carries a Confirm variant",
        "Confirm(ConfirmRequest)," in pages,
    )
    check(
        "70: destructive actions are modeled",
        "pub enum DestructiveAction" in pages
        and "DeleteAutomationRule { id: i64 }" in pages,
    )
    check(
        "70: the request builds the shell dialog",
        "pub fn dialog(&self) -> crate::overlays::ConfirmDialog" in pages,
    )
    app = read(SHELL, "app.rs")
    check(
        "70: the shell stores the pending confirmed action",
        "pending_confirm: Option<DestructiveAction>" in app,
    )
    check(
        "70: apply_nav arms the dialog (never executes)",
        "NavRequest::Confirm(request) => {" in app
        and "self.pending_confirm = Some(request.action);" in app,
    )
    check(
        "70: only a Confirmed outcome executes the action",
        "if outcome == DialogOutcome::Confirmed {" in app
        and "self.execute_confirmed(action);" in app,
    )
    check(
        "70: execution routes the automation delete through core",
        "spp_core::automation::delete_rule(conn, id)" in app,
    )
    automation = read(SHELL, "pages/automation.rs")
    check(
        "70: the delete button raises the confirm request instead of deleting",
        "ConfirmRequest::delete_automation_rule(" in automation,
    )
    check(
        "70: the page no longer deletes directly from the button handler",
        "if let Some(id) = deleted {" not in automation,
    )
    check(
        "70: the confirmed execution half exists on the page",
        "pub fn delete_rule_confirmed(&mut self, db: &DbHandle, id: i64)" in automation,
    )


# ----------------------------------------------------------------- item 71
def verify_item71() -> None:
    main = read(SHELL, "main.rs")
    check(
        "71: SPP_SMOKE runs the shell smoke before returning",
        "spp_shell::smoke::run_shell_smoke(options)" in main,
    )
    check(
        "71: a failed smoke exits non-zero",
        'std::process::exit(1);' in main,
    )
    smoke_path = SHELL / "smoke.rs"
    check("71: the smoke module exists", smoke_path.exists())
    if smoke_path.exists():
        smoke = smoke_path.read_text(encoding="utf-8")
        check(
            "71: the smoke constructs the REAL shell",
            "ShellApp::new_with_context(&ctx, options)" in smoke,
        )
        check(
            "71: every page renders a pass",
            "for key in PageKey::all() {" in smoke,
        )
        check(
            "71: each pass runs logic + ui (the eframe order)",
            "app.logic_pass(ui.ctx());" in smoke
            and "app.ui_pass(ui);" in smoke,
        )
        check(
            "71: a blank frame fails the smoke",
            "rendered an empty frame" in smoke,
        )
    app = read(SHELL, "app.rs")
    check(
        "71: the headless constructor exists",
        "pub fn new_with_context(ctx: &egui::Context, options: ShellOptions) -> Self" in app,
    )
    check(
        "71: the eframe constructor delegates to it",
        "Self::new_with_context(&cc.egui_ctx, options)" in app,
    )
    check(
        "71: the App trait bodies are shared with the smoke harness",
        "fn logic_pass(&mut self, ctx: &egui::Context)" in app
        and "fn ui_pass(&mut self, ui: &mut egui::Ui)" in app,
    )
    lib = read(SHELL, "lib.rs")
    check("71: the smoke module is registered", "pub mod smoke;" in lib)


# ----------------------------------------------------------------- item 72
def verify_item72() -> None:
    palette = read(SHELL, "palette.rs")
    check(
        "72: a dry Enter re-grabs focus for the query field",
        # Batch 32 (item 133) widens the re-grab to also cover the
        # suppressed same-frame Enter — the zero-hits dry Enter is the
        # `|| take_hits(...)` disjunct, still re-grabbing focus.
        "if take && (query_changed || take_hits(&self.hits).is_empty()) {"
        in palette
        and "response.request_focus();" in palette,
    )
    check(
        "72: the hit-bearing Enter path is untouched",
        # Batch 32 (item 133) guards the confirm with `!query_changed` —
        # an Enter processed in the same frame as a query change must not
        # confirm the previous query's result set. The confirm itself
        # (take + selected hit -> nav + close) survives unchanged.
        "&& !query_changed\n                    && let Some(hit) = hits.get(self.selected)"
        in palette,
    )


# ----------------------------------------------------------------- item 73
def verify_item73() -> None:
    inbox = read(SHELL, "pages/inbox.rs")
    check(
        "73: the pending transform records its origin text",
        "origin_body: String," in inbox,
    )
    check(
        "73: enqueue snapshots the composer",
        "origin_body: self.composer_body.clone()," in inbox,
    )
    check(
        "73: completion guards against in-flight typing",
        "if self.composer_body != p.origin_body {" in inbox,
    )
    check(
        "73: the kept-edit decision is said out loud",
        "your text was kept" in inbox,
    )


# ----------------------------------------------------------------- item 74
def verify_item74() -> None:
    hub = read(CORE, "ai_hub.rs")
    check(
        "74: RoleAssignment/RoleMap compare equal (the snapshot guard)",
        "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]" in hub
        and "#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]" in hub,
    )
    ai_center = read(SHELL, "pages/ai_center.rs")
    check(
        "74: the page tracks the last persisted role map",
        "hub_roles_saved: spp_core::ai_hub::RoleMap," in ai_center,
    )
    check(
        "74: keystrokes only sync the live map (no save, no enqueue)",
        "model_edit != model_text" in ai_center
        and "self.hub_roles = roles.clone();" in ai_center,
    )
    check(
        "74: the persist + golden_eval ride a blur commit",
        "model_response.lost_focus() && roles != self.hub_roles_saved" in ai_center,
    )
    check(
        "74: the commit is one shared method",
        "fn commit_role_map(&mut self, db: &DbHandle, roles: &spp_core::ai_hub::RoleMap)" in ai_center,
    )
    check(
        "74: the commit updates the saved snapshot",
        "self.hub_roles_saved = roles.clone();" in ai_center,
    )
    # The per-keystroke path must NOT touch the database anymore.
    keystroke_block = ai_center[ai_center.index("if model_edit != model_text") :]
    keystroke_block = keystroke_block[: keystroke_block.index("if model_response.lost_focus()")]
    check(
        "74: the keystroke branch contains no write and no enqueue",
        "with_write" not in keystroke_block
        and "golden_eval" not in keystroke_block,
    )


# ----------------------------------------------------------------- item 75
def verify_item75() -> None:
    ai_center = read(SHELL, "pages/ai_center.rs")
    check(
        "75: the copilot submit requires the field's own focus loss",
        "let submit = response.lost_focus()" in ai_center,
    )
    check(
        "75: the global Enter idiom is gone",
        'let submit =\n                ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.copilot_input.is_empty();'
        not in ai_center,
    )
    check(
        "75: the button-click submit path survives",
        ".clicked()\n                && !busy)" in ai_center,
    )


# ----------------------------------------------------------------- item 76
def verify_item76() -> None:
    operations = read(SHELL, "pages/operations.rs")
    check(
        "76: the library model carries a stale flag",
        "stale: bool," in operations,
    )
    check(
        "76: invalidation marks stale instead of dropping the model",
        "if let Some(library) = &mut self.library {\n            library.stale = true;\n        }" in operations,
    )
    check(
        "76: a stale model re-reads its rows on render",
        "is_none_or(|l| l.stale)" in operations,
    )
    check(
        "76: the reload preserves the open edit form",
        "let reply_edit = previous.as_ref().and_then(|l| l.reply_edit.clone());" in operations,
    )
    check(
        "76: the reload preserves the picked mailbox (clamped)",
        "l.selected_mailbox.min(mailboxes.len().saturating_sub(1))" in operations,
    )
    check(
        "76: the reload preserves the typed workflow number",
        "let workflow_number = previous.map_or_else(String::new, |l| l.workflow_number);" in operations,
    )
    check(
        "76: the old drop-on-invalidate is gone",
        "self.library = None;" not in operations,
    )


def main() -> int:
    for verify in (
        verify_item67,
        verify_item68,
        verify_item69,
        verify_item70,
        verify_item71,
        verify_item72,
        verify_item73,
        verify_item74,
        verify_item75,
        verify_item76,
    ):
        verify()
    print()
    if FAILURES:
        print(f"{len(FAILURES)} check(s) FAILED:")
        for failure in FAILURES:
            print(f"  - {failure}")
        return 1
    print("All batch-26 checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
