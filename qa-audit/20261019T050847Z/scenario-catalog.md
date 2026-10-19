# Scenario Catalog — QA Audit 20261019T050847Z

Every scenario the audit executed, with preconditions, steps, expected and
actual results, and evidence. Sessions were run as self-contained foreground
harness boots (Xvfb + AppImage + XTEST input + ffmpeg capture); see
`evidence/artifacts/harness/harness.py`.

---

## S-A — First use, navigation sweep (session `A-first-use-attempt2`)

- **Preconditions**: fresh data dir, empty DB, no first-use dialog blocking.
- **Steps**: boot → screenshot → click each of the 21 sidebar rows at its
  visual center → screenshot after each click → open palette via sidebar
  button (A22), via keyboard (A23), type `inbox` (A24), press Enter (A25).
- **Expected**: each click lands the labeled page; palette accepts the query.
- **Actual**: 17/21 navigated (pixel-verified page changes); **Inbox,
  Reports, Search, Shell did not** (screenshots identical to the previous
  page — DEFECT-01). Palette opened via button; typed text and Enter
  produced **zero** pixel change (input not focused — DEFECT-03; keyboard
  focus missing in a no-WM environment).
- **Evidence**: `artifacts/nav-sweep-A.json`, `screenshots/A00…A25`,
  `videos/B1-sidebar-scan.mp4` (companion scan), log `logs/B1-sidebar-scan.log`.

## S-B1 — Sidebar deep scan (sessions `B1-sidebar-scan`, `B1-hover-chunk0/1/2`, `B1-xscan-chunk0/1`, `B1-click-probes`, `B1-diag-hover`)

- **Preconditions**: fresh DB; pointer parked at a neutral screen point
  between probes.
- **Steps**: hover-sweep the sidebar (y 28–620 step 2) recording highlight
  bands; x-scan rows Dashboard/Inbox/Reports (x 10–142) recording hover
  paints; 139-position click scan; press-visual diagnostics at row edges.
- **Expected**: hover bands ≈ visual rows; clicks at row centers navigate.
- **Actual**: hover paints live at x=10–58 on **all** rows including the
  dead four (paint/hover rect ≠ click rect); no y position selects the
  dead four at x=40; several sessions were killed by the sandbox janitor
  mid-run (empty step journals — see S-D).
- **Evidence**: `artifacts/hitbox-scan.json`, `B1-xscan-chunk*.json`,
  `B1-hover-chunk*.json`, screenshots `B1-*`, videos `B1-click-probes.mp4`.

## S-D — Session-killer diagnostics (sessions `D-killer-probe`, `D-rec-rename-test`, `D-xa-rename-test`)

- **Preconditions**: suspected ~49 s kill of Xvfb during sustained input.
- **Steps**: heartbeat grab + child-health polling every 2 s; recorder and
  Xvfb-helper binary rename experiments to isolate the trigger.
- **Actual**: kills are external to the harness (whole tool-call process
  trees die; partial JSONs show children alive until the abrupt end);
  neither the recorder nor the Xvfb helper was the trigger. Session age at
  kill varied 5 s…180 s across the audit. Environment limitation, recorded.
- **Evidence**: `logs/D-*.log`, `D-*.xvfb.log`, `videos/D-killer-probe.mp4`,
  `D-rec-rename-test.mp4`, empty-step `artifacts/D-*.json`.

## S-K1 — Keyboard channel discrimination (session `K1-keyboard-focus`)

- **Steps**: explicit `XSetInputFocus` on the app window → open palette via
  button (30,558) → click (640,120) → type `inbox` → Escape.
- **Actual**: palette opened (3.6% diff); typed text 0-pixel change (click
  missed the input box — true rect x310–597, see S-K3); **Escape closed the
  palette (3.55%)** — keyboard events reach the app once X focus is set.
- **Evidence**: `artifacts/K1-keyboard-focus.json`, screenshots `K1-*`.

## S-K2 — Input-focus paths (session `K2-palette-input`)

- **Steps**: palette open → single-click / Tab / double-click at (640,130),
  typing one character after each.
- **Actual**: all three paths 0-pixel effect (x=640 is outside the input
  box); no X focus set in this session → Escape also inert. Geometry
  forensics on the screenshots then located the true input rect.
- **Evidence**: `artifacts/K2-palette-input.json`, screenshots `K2-*`.

## S-K3 — Palette end-to-end at true geometry + dead-row x-sweep (session `K3-palette-real-input`)

- **Preconditions**: X focus set; input rect known (x310–597, y115–146).
- **Steps**: click (450,130) → type `inbox` → Enter → Escape → for each dead
  row (Inbox 87, Reports 201, Search 386, Shell 523) click at x=10 and x=45.
- **Expected**: caret renders; typed glyphs render; query runs; Escape
  closes; x=10 selects the target row if the hitbox is left-compressed.
- **Actual**: caret appeared (2165 px), glyphs rendered (1722 px), query ran
  (hint changed), Enter produced no navigation (612 px = caret blink —
  DEFECT-02 corroboration), Escape closed (4.5%). **All four dead rows
  navigated at x=10** (1.3–3.6% content diff, sidebar selection moved to
  the target row); x=45 inert. DEFECT-01 fully characterized; all 21 pages
  reachable.
- **Evidence**: `artifacts/K3-palette-real-input.json`, screenshots
  `K3-00…K3-60-dead-Shell-45`, `videos/K3-palette-real-input.mp4`.

## S-CHK — Declared checks (static/build/test; no GUI)

- **Steps**: `cargo fmt --check`, `cargo clippy --workspace --all-targets
  -D warnings`, `cargo test --workspace` (304 passed + 169 integration),
  release build, AppImage build, `SPP_SMOKE=1` AppImage smoke.
- **Actual**: all green; smoke exit 0. CI run 39201357186 on the audit
  target commit 19dfb35: both jobs success.
- **Evidence**: commit `7a0a095` (declared-check record), CI API check
  recorded in `audit-metadata.json`.

## S-RT — Runtime observation (passive, from session logs)

- **Observed on every GUI boot**: 49 migrations applied in sequence
  (`logs/*.log`), background workers started (job, webhook, embedding,
  maintenance; auto-sync timer 5 min), loopback listener armed
  (`POST /webhook`, `GET /oauth/callback`), automatic backup created,
  ratings refresh worker pushed 7 ratings, window created at 1180×760
  (matching `with_inner_size([1180.0, 760.0])`), llvmpipe software GL
  warning from the environment.
- **Result**: boot pipeline, workers, and listener behave as declared on
  every session; no boot failures across ~20 boots.
