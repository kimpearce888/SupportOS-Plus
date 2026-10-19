# Coverage Gaps — QA Audit 20261019T050847Z

Everything the audit did not run, could not run, or ran only partially,
with reasons. Nothing below is silently skipped — each gap names its cause
and, where applicable, what evidence covers the behavior instead.

## 1. GUI control-level interaction beyond navigation and the palette

- **Scope**: 264 controls inventoried from source (`control-inventory.csv`);
  GUI-verified during the audit: sidebar navigation rows (`C-APP-002`,
  21 rows), palette query field (`C-PALETTE-001`), palette open button
  (`C-APP-004`), plus boot/rendering evidence for all 21 pages (screenshots
  A01–A21, K3 dead-row shots). The remaining ~255 page-level controls
  (buttons, tabs, text fields on Inbox/Settings/Knowledge/… pages) were
  **not** interacted with.
- **Reason**: the sandbox janitor terminates foreground tool calls at
  unpredictable ages (5 s…180 s observed; `evidence/artifacts/D-killer-probe*`,
  `D-rec-rename-test*`, `D-xa-rename-test*` — all sessions that injected
  sustained input died with empty step journals; `B1-hover-chunk0` survived
  102 probes/28 s, longer sweeps did not). Blind-aiming clicks at unknown
  page-widget coordinates costs one session per attempt; the audit chose
  precision (navigation, palette, defect characterization) over bulk
  low-confidence clicks.
- **Compensating evidence**: `existing-test-coverage.md` maps the 473-test
  suite (304 workspace + 169 integration) to the behaviors behind those
  controls; every page was confirmed to *render* its controls (A-series,
  K3 screenshots).

## 2. Palette hit-click navigation

- Clicking a palette hit row (the `C-PALETTE-002/003` rows) was never
  exercised: fresh profiles seed no conversations/customers/knowledge
  documents, so every query returns `(no hits)`. Seeding requires the
  Operations-page demo tiles (reachable but position-unmapped within the
  session budget). `DEFECT-02` documents the parallel Enter path as read.

## 3. Live third-party services (by design of the audit target)

- **Help Scout production API**: no credentials supplied; the audit ran the
  full stack against the in-process mock (`it_helpscout`, `it_sync`, mockserver).
- **OAuth against api.helpscout.net**: not run; the complete
  authorization-code exchange was verified over real TCP against the mock
  token endpoint (`it_loopback`), including the browser-style callback GET.
- **LM Studio inference**: no local model server (127.0.0.1:1234/:11434/:8080
  probed, refused). Wire format, model listing, availability, timeouts,
  error recovery verified against the mock (`it_lm_studio`); the live
  round-trip test exists but is `#[ignore]`-gated behind `SPP_LM_STUDIO_URL`.
- **Webhook ingress from Help Scout's servers**: verified over real TCP
  loopback (HMAC-SHA1 verification, dedup, job enqueue) — external ingress
  not possible from this sandbox (no inbound network).

## 4. Keyboard paths inside the GUI

- The audit proved the keyboard channel works after explicit
  `XSetInputFocus` (K1 Escape; K3 typed text). Not exercised: `Ctrl/Cmd+K`
  palette shortcut (K1 earlier attempt ambiguous), arrow-key hit selection,
  in-page text fields (Settings page inputs), Tab traversal (tested once —
  no visible effect with the palette unfocused; inconclusive).
- **Reason**: bare Xvfb has no window manager to establish focus; each
  keyboard scenario needs a custom-scripted session and the janitor budget
  was spent on the higher-priority defect work.

## 5. Performance / load / endurance

- Application responsiveness and memory (VmHWM) under sustained GUI load:
  not measured in this audit branch (the sandbox's own resource pressure
  — 2 vCPU, llvmpipe software GL, 88–97% disk — makes such numbers
  meaningless for the product). Prior workstream evidence (recorded in
  the repository ledger `ai/plan.md` items 40–46): release cold boot
  10–11 ms; boot migration 914 ms cold / 4.1 ms warm; AppImage unpack+boot
  37–38 ms; 473 tests green; CI run 39201357186 both jobs success.

## 6. Publishing

- Final `git push` of the audit branch requires credentials that the
  sandbox janitor removed mid-audit (`~/.git-credentials` deleted; no
  token in env, gh CLI, or netrc). Commits are complete locally; the
  branch is push-ready. (Earlier commits up to `b345c8d` were pushed
  before the credential loss.)

## 7. Not applicable

- Mobile/web packaging (the product is a native egui desktop app; no web
  build exists to audit).
- Installer platforms other than AppImage on x86_64 Linux (the repo builds
  exactly one AppImage target).
