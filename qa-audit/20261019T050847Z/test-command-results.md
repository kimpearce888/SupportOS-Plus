# Test Command Results — QA Audit 20261019T050847Z

Commands actually executed against the audit target (commit
`19dfb35a714822ed94b6b5144eabb222997cc2ca`), with outcomes. Declared checks
were recorded incrementally in commit `7a0a095`; GUI harness commands are
per-session scripts under `evidence/artifacts/harness/`.

## Build / static / test (declared checks)

| Command | Result |
|---|---|
| `cargo fmt --check` | pass (clean) |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass (zero warnings) |
| `cargo test --workspace` | **304 passed** (+2 bench and +1 LM Studio live test ignored-by-design), then the TODO-45 integration batch adds **169 passed** — 473 total, 0 failed |
| release build (`--release`) | pass |
| AppImage build (`cargo xtask appimage` path) | pass — `target/appimage/SupportOs-Plus-x86_64.AppImage` |
| `SPP_SMOKE=1` AppImage smoke | **exit 0** |
| CI (target commit) | run `39201357186` — both jobs success (AppImage build+smoke; fmt/clippy/test/deny) |

Blockers during the checks: one disk-exhaustion environment incident
(root fs at 97% from `target/debug` growth) resolved by pruning; documented
in commit `7a0a095`.

## GUI harness (audit sessions; foreground, ≤100 s each)

| Session | Script | Outcome |
|---|---|---|
| `A-first-use-attempt2` (nav sweep) | prior-session harness | completed; 17/21 rows navigate, 4 dead (DEFECT-01) |
| `B1-*` (6 sessions) | `s_b1_*.py` | hover/x-scan chunks completed (102 probes/28 s); longer sweeps killed by sandbox janitor — partial JSONs preserved |
| `D-*` (3 sessions) | `d_killer_probe.py` + inline variants | killed externally; environment evidence, not product defects |
| `harness-selftest` | `harness.py __main__` | boot + screenshot + teardown OK (`H0-harness-selftest.png`) |
| `win-enum` | `win_enum_debug.py` | window enumeration run; single app window 1180×760 |
| `K1-keyboard-focus` | `k1_keyboard_focus.py` | completed; Escape works with X focus set; click at (640,120) missed input |
| `K2-palette-input` | `k2_palette_input.py` | completed; Tab/dclick/typing inert without X focus; geometry forensics on screenshots |
| `K3-palette-real-input` | `k3_palette_real_input.py` | completed; input clickable at true rect, typing renders, Enter no-op (DEFECT-02), Escape closes; **dead rows navigate at x=10** |

Harness input channel: python-xlib XTEST (real X pointer/keyboard events).
Capture: PIL/XCB screenshots (PNG) + ffmpeg x11grab (MP4). All sessions boot
the AppImage via `APPIMAGE_EXTRACT_AND_RUN=1` against a fresh
`SPP_DATA_DIR` per session.

## Environment caveats affecting results

- `xdotool` unavailable (no root); XTEST via python-xlib substitutes.
- Bare Xvfb (no window manager): keyboard needs explicit `XSetInputFocus`
  (harness does this from S-K1 onward); no window decorations or focus
  policies to test.
- llvmpipe software Vulkan/GL: rendering correct, slow; not a performance
  measurement environment.
- Sandbox janitor kills long-lived processes at unpredictable ages
  (5–180 s observed); every scenario is a self-contained short session.

## Post-audit fix verification (2026-10-19, remediation batch)

| Command | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace` | 306 passed, 0 failed, 4 env-gated ignored (140 shell incl. 2 new headless palette E2E; 164 core; 2 xtask) |
| `cargo test -p supportos-plusplus-app-shell --lib palette` | 7/7 — incl. `palette_takes_focus_on_open_and_keystrokes_reach_the_query` and `palette_enter_confirms_the_selected_hit` |
| `cargo run --release -p supportos-plusplus-xtask -- package` | AppImage rebuilt from the fixed binary |
| `python3 fix_verify.py` — session `F1-fix-verify` (AppImage) | DEFECT-01 retest PASS: Inbox/Reports/Search/Shell navigate at x=75 (diffs 3.87/2.46/1.62/3.37%), Inbox at x=130 (2.49%), controls Organizations/Knowledge/Settings unaffected (1.71/1.91/2.26%) |
| `python3 fix_palette_probe.py F2..F5` | DEFECT-03 GUI leg BLOCKED-ENV: janitor severed each fresh session's X socket within ~8 s of boot (F2 log: "X connection to :93 broken" at +6 s; F3–F5 frozen frames, 0.0000 pixel diffs, no clock animation). Four attempts; headless E2E test + K1's winit key-delivery proof carry the verification |

Headless palette E2E tests drive the real egui event loop via
`Context::run_pass` (RawInput events: `Event::Text`, `Event::Key`) against
a seeded fixture DB — focus routing, text-edit event handling, search and
Enter-confirm are all the production code paths.
