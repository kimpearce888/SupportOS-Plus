# Post-audit fix report — DEFECT-01/02/03 remediation

- **Date:** 2026-10-19 (same day as the audit)
- **Base:** audit target `19dfb35` (main, plan exhausted 46/46) — the
  audit branch was merged fast-forward into main, fixes landed on main,
  and the audit branch was deleted per maintainer instruction.
- **Scope:** remediation of the three defects recorded in `defects.md`,
  following the audit verdict ("fix promptly: DEFECT-01") and the
  maintainer's "fix everything and keep only the main branch" instruction.

## Changes

| Defect | Change | Files |
|---|---|---|
| DEFECT-01 (MAJOR) | Sidebar rows are no longer `selectable_label` widgets; `nav_row()` hand-allocates the full panel width (`allocate_space` + `interact(Sense::click())`), paints the label galley with `Style::interact_selectable` visuals, keeps the button role for assistive tech and the interact cursor. The sensed rect is exactly the painted row, so click/hover geometry cannot diverge for any label length. | `crates/app-shell/src/app.rs` |
| DEFECT-02 (MINOR) | None — misdiagnosis. egui 0.37 single-line `TextEdit` surrenders focus on Enter (`builder.rs:119` doc, `:1331` `surrender_focus`), `lost_focus()` is live-computed (`memory/mod.rs:872`) and `key_pressed` is not consumption-filtered (`input_state/mod.rs:767`), so the audited `lost_focus() && Enter` idiom fires as written. The audit record is corrected; the original code is kept. | `crates/app-shell/src/palette.rs` (tests only) |
| DEFECT-03 (MINOR) | `CommandPalette::open()` sets `wants_focus`; the first rendered pass calls `response.request_focus()` on the query field. Both open paths (⌘K button, Ctrl/Cmd+K) route through `open()`. | `crates/app-shell/src/palette.rs` |

## Verification

- **Headless egui E2E (production code paths, real event loop):**
  - `palette_takes_focus_on_open_and_keystrokes_reach_the_query` — after
    open the query field holds keyboard focus; typed keystrokes land in
    the query with no click and produce hits on the seeded fixture.
    This is exactly the behavior the audit's K1/K3 GUI probes found
    missing pre-fix (0-pixel change until the input was clicked).
  - `palette_enter_confirms_the_selected_hit` — type "Export", press
    Enter, the palette returns `NavRequest::Page(Inbox)` and closes.
    Passes against the ORIGINAL code (DEFECT-02 invalidation).
- **Workspace:** 306 passed / 0 failed / 4 env-gated ignored
  (140 shell incl. the two new tests; 164 core; 2 xtask);
  `cargo clippy --workspace --all-targets -- -D warnings` clean;
  `cargo fmt --all --check` clean.
- **Packaging:** release binary rebuilt; AppImage repacked via
  `cargo run --release -p supportos-plusplus-xtask -- package`.
- **GUI re-verification (same harness, thresholds and methodology as the
  audit):**
  - `F1-fix-verify` (AppImage): DEFECT-01 retest PASS. All four
    previously dead rows navigate at x=75 — Inbox 3.87%, Reports 2.46%,
    Search 1.62%, Shell 3.37% content diffs (audit nav threshold 0.9%) —
    and Inbox navigates from the panel's far right at x=130 (2.49%, a
    zone never clickable pre-fix). Control rows Organizations/Knowledge/
    Settings unaffected (1.71/1.91/2.26%). New row geometry is
    deterministic: centers at 65+25·(i−1), confirmed at 7 of 21 rows.
    Evidence: `evidence/artifacts/F1-fix-verify.json` +
    `F1-fix-verify-verdict.json`, `evidence/screenshots/F1-*.png`,
    `evidence/videos/F1-fix-verify.mp4`,
    `evidence/artifacts/harness/fix_verify.py`.
  - `F2..F5-palette-focus` (DEFECT-03 GUI leg): BLOCKED-ENV. The sandbox
    janitor severed each fresh session's X socket within ~8 s of boot
    (F2 log: "X connection to :93 broken" at +6 s; F3–F5 show frozen
    frames with 0.0000 pixel diffs across 8+ s of probes — no clock
    animation, no hover paint; the documented 5–180 s kill behavior, now
    much tighter). Four attempts; the headless E2E proof plus K1's
    verified winit key-delivery channel (Escape closed the palette
    pre-fix) carry the verification. Evidence:
    `evidence/artifacts/F2..F5-palette-focus-verdict.json`,
    `evidence/logs/F2..F5-palette-focus.log`,
    `evidence/artifacts/harness/fix_palette_probe.py`.

## Root-cause notes (egui 0.37.0, pinned registry source)

- egui 0.37 rebuilt its widgets on the new "atom" layout system
  (`src/atomics/`, `src/widgets/button.rs`). The audit measured that
  `selectable_label` (a `Button::selectable`) decoupled the
  click-acceptance rect from the painted/hover rect on the four
  shortest labels. Rather than fight the toolkit's internal hit
  geometry, `nav_row` allocates and senses its own full-width rect —
  immune to the anomaly by construction.
- DEFECT-02 was a misdiagnosis of older-egui behavior: 0.37's own docs
  and source show surrender-on-Enter restored, so
  `response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))`
  is the correct, working idiom (it is the pattern documented on
  `Response::lost_focus` itself, `response.rs:423`).

## Branch policy

Per maintainer instruction the audit branch was pushed, merged
fast-forward into `main`, and deleted locally and remotely; the
repository carries a single branch (`main`) again. CI runs on main
pushes (`.github/workflows/ci.yml`: fmt/clippy/test/deny +
sandbox-feature pass + AppImage build & SPP_SMOKE boot-chain smoke).
