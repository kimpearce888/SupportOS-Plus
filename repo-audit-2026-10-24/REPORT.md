# Complete Repository Audit

**Status: IN PROGRESS — preliminary; not yet fully reconciled**
Repository: `https://github.com/kimpearce888/SupportOS-Plus`
Application revision audited: `main @ 3affe7e` (short; full sha in HANDOVER.md)
Audit branch/commit: audit checkpoint commits on `main` (see HANDOVER.md)
Audit date/timezone: 2026-10-24 (UTC)

## 1. Scope and methodology
Full instruction-driven audit per `repo-audit-2026-10-24/instructions/` (gate SATISFIED, see INSTRUCTION-LOADING-RECORD.md). Source of truth: actual build/runtime-relevant implementation + observed behavior. This section will describe methodology in detail as phases complete.

## 2. Environment and reproducibility
See BUILD-AND-INSTALL.md (live). Summary so far: Debian sandbox, 2 vCPU/4 GB, Rust 1.99.1 (pinned match) freshly installed, Xvfb + ffmpeg available.

## 3. Key findings (preliminary until reconciled)
- FIND-001 (Medium, confirmed): default :3000 webhook/OAuth loopback port collides with common dev servers; ineffective SPP_LOOPBACK_PORT override in default config; degradation log-only (FINDINGS.md)
- FIND-002 (Low, confirmed observation): second-instance flock guard works; console-only message — UI dialog presence TBD
- FIND-003 (Low, confirmed, pre-existing): prior audit committed app-local secret-envelope key material as repo artifacts (observation for the maintainer; not touched)
- FIND-004 (Low, confirmed): pinned theme row overlaps the bottom sidebar navigation rows; the Shell row is only clickable through a ~10 px sliver at the default 1180x760 window
- FIND-005 (Low, source-verified): destructive actions without confirmation on several pages while equivalent deletes elsewhere are shell-confirm-gated (inconsistent destructive-action UX)
- FIND-006 (Low, runtime + source): Sync Health "Cancel running sync" on an idle engine toasts "(started in background)" — misleading feedback; no state change (DB-verified)
- FIND-007 (Low, source-verified): zero-hit help-center searches silently persist knowledge-gap records (hidden write from a read-looking control)
- FIND-008 (Low, source-verified): Issue Radar "Detect gaps now"/"Cluster now" discard enqueue results; no completion signal
- FIND-009 (Low, source-verified): silent input coercion (Connectors refresh-seconds → default 3600; Operations capacity clamp 1..=500; Builder dates length-10 only)
- FIND-010 (Low, source-verified): Customers "← back to search" clears results without re-running the applied query
- FIND-011 (Low, runtime-confirmed): command-palette click-outside backdrop unreliable (5/6 outside clicks failed, two fell through to the UI beneath); mouse-only dismissal unreliable; triggers are open()-only
- Environment note: sandbox lacked libxkbcommon-x11/libxcb-xkb (remediated environment-only; not an app defect); XTEST keyboard injection does not reach egui TextEdits in this sandbox (test tooling limitation, recorded)

## 4. Program purpose and problem coverage
(pending)

## 5. Architecture and construction
(pending — ARCHITECTURE.md will be created during discovery)

## 6. Complete feature inventory (reference FEATURE-INVENTORY.md)
Pass 1 COMPLETE (2026-10-24): 23/23 sidebar pages opened in the running app by XTEST-injected, machine-verified clicks with per-page screenshots (E-006..E-031), plus 3 shell-level features (F-024..F-026).
Pass 2 COMPLETE (2026-10-24): 535-row source-first control/option ledger (C-<PAGE>-<nn>) with per-row conditions, validation/constraints, effect paths, dependencies and non-obvious notes; reconciled by 46 real-input runtime control exercises over 21 pages (control-verify.py v3; E-032..E-080). Recorded notes: XTEST keyboard injection does not reach egui TextEdits in this sandbox (Customers/Support Graph text entry source-verified only); Onboarding/Shell sync-controls deliberately deferred to W-002/W-007 to preserve the never-synced precondition; Diagnostics clear-log executed end-to-end through its confirmation dialog.
Workflows: W-008 executed PARTIAL (2026-10-24, E-081..E-086): palette opens via header button with correct empty state; Ctrl+K chord is a no-op (keyboard limitation); backdrop dismissal unreliable — FIND-011.

## 7. Distinctive implementation behavior
Observed so far: fresh launches always open Dashboard with nav scroll 0 (no last-page/scroll restore); theme persists via the settings table; single-instance flock (exit 1, log-only message); loopback listener degradation is log-only in the default config (FIND-001); the Shell page is sidebar-only reachable (no CLI arg, no cross-links; palette searches records, not pages).

## 8. Partial/incomplete/unreachable implementations
Control-level assessment (source-first, pass 2): no no-op buttons found in the main flows — every enumerated control has a wired handler; notable gaps are behavioral, not missing handlers: FIND-005 (destructive UX inconsistency), FIND-006 (misleading idle-cancel toast), FIND-007 (hidden writes), FIND-008 (fire-and-forget enqueues), FIND-009 (silent input coercion), FIND-010 (query loss on back), FIND-011 (palette dismissal unreliable at runtime despite the source-claimed full-screen catcher). Known candidate areas queued for workflow testing: Help Scout OAuth round-trip (external dependency), conversation lifecycle (needs data).

## 9. Workflow map and branch coverage
(pending)

## 10. Manual testing results
(pending)

## 11. Defect and gap register (reference FINDINGS.md)
(pending)

## 12. Recommended fixes, reasoning, acceptance criteria, regression tests
(pending)

## 13. Build/install/runtime failures
None: build 0 warnings; launch verified (E-001); runtime WARN only for the sandbox's :3000 collision (FIND-001 — recorded as a product robustness finding, not an environment failure).

## 14. Existing test-suite results (separate from manual QA)
`cargo test --workspace` @ 3affe7e, 2026-10-24: spp_shell 228/0/0-ign; spp_core 466/0/4-ign; spp_xtask 9/0; spp_catalog 0; **703 passed / 0 failed / 4 ignored** (details + run notes in BUILD-AND-INSTALL.md). Supplementary only — not proof of product correctness.

## 15. Evidence index and validation results
See evidence/manifest.csv: E-001..E-086 — all validated (PIL open + visual read for screenshots; full read for logs; machine events cross-checked). 24 validated screenshots in evidence/screenshots/FEATURE/, 71 in evidence/screenshots/CONTROL/, 1 in evidence/screenshots/BUILD/.

## 16. Evidence exceptions and unresolved gaps
None accepted, none unresolved (evidence/exceptions.csv not yet needed).

## 17. Coverage accounting and definitions
(pending)

## 18. Continuity and publication status
Live state: HANDOVER.md (authoritative), PROGRESS.md, NEXT-ACTIONS.md, SESSION-LOG.md. Audit folder committed incrementally to the audit branch `repo-audit/2026-10-24` (audit artifacts only).

Do not remove the IN PROGRESS status until inventory, test, evidence, and handover records are reconciled.
