# UX Audit Worklog — SupportOS+ (SupportOS-Plus)

## Session start

- Date: 2026-02-06 (Asia/Dhaka)
- Starting commit: `d306caa59e9e64a6ed3fcf13985dfc9849165442` (`d306caa` — "rel: Batch REL-3 closed — v1.9.4 published")
- `git status` at start: clean working tree, on `main`, up to date with `origin/main`.
- Repo cloned to `/home/z/SupportOS-Plus` (fresh clone; no uncommitted changes existed).

## Governing workflow rules

- `ai/rule.md` (read first, per owner directive): batches of exactly 10 items,
  `plan.md → progress.md → implementation → verification → completed.md`,
  STOP after each batch of ten and report; resume only on owner "continue".
- The plan queue (`ai/plan.md`) was **empty** at session start (last entry:
  REL-3 closed, "Rule 8 STOP — the queue is empty"). The owner's UX-audit
  directive is the new work item: Phase 0–2 discovery/audit will populate the
  plan; fixes proceed in ten-item batches per rule.md.

## Environment findings

- Sandbox: 2 CPUs, 4.1 GB RAM, 7.3 GB free disk, no sudo, no preinstalled Rust.
- Rust 1.99.1 (pinned toolchain) installed via rustup to `~/.cargo` — matches
  `rust-toolchain.toml`.
- Xvfb + xvfb-run available; Mesa GL (libGLX_mesa/llvmpipe) present → the
  app's default **glow** renderer works headless under Xvfb (the previous
  session's "no Vulkan ICD" blocker does not apply to glow; `SPP_RENDERER`
  can select wgpu if ever needed).
- ffmpeg available → screen capture via `ffmpeg -f x11grab` for screenshots.
- Port 3000 was occupied by an unrelated Next.js dev server (pid 1226) —
  needed for the OAuth callback journey (redirect URI registered as
  `http://localhost:3000/oauth/callback`); handled during Phase 1/3 when
  the OAuth journey runs.
- App stack: Rust workspace (edition 2024), eframe/egui 0.37 (glow), 4 crates:
  `app-shell` (GUI), `core` (services/DB/sync), `catalog`, `xtask`.
  889 `#[test]`s; kittest snapshot harness `crates/app-shell/tests/` behind
  `kittest-ui` feature; `cargo fmt`/`clippy` gates; `deny.toml` supply chain.
- SPP_SMOKE=1: headless boot render of every page (24 pages / 4222 shapes).
- First-run experience: app creates its SQLite DB in the data dir on first
  boot; onboarding page exists (`pages/onboarding.rs`).

## Actions log

1. Read `ai/rule.md`, `ai/plan.md`, `ai/progress.md`, `ai/completed.md`
   (queue empty, Rule 8 STOP state — new directive received from owner).
2. Cloned repo; recorded starting commit (above).
3. Installed Rust 1.99.1 (`rustup`, minimal profile + rustfmt + clippy).
4. Started `cargo build -p supportos-plus-app-shell --bin supportos-plus -j 2`
   (debug; background, log `/tmp/build-app.log`).
5. Created `ux-audit/` tree (before/after/diffs/reports/scripts);
   gitignored the binary evidence dirs (`ux-audit/before|after|diffs|scripts`)
   — `ux-audit/reports/` stays tracked so all textual audit deliverables
   (scope, journey map, issues, summary) are pushed to GitHub per owner
   instruction. Secrets live only in gitignored `ux-audit/scripts/app.env`.
6. Phase 0 completed: scope.md (frontend/backend boundary), baseline
   fmt/clippy/test/build; app driven under Xvfb :99 at 1920×1080 (glow
   renderer); first-run boot journey captured (before/00-boot/).
7. Phase 1 completed: journey-map.md — every page + shell/global journeys
   driven with synthetic input + VLM verification; OAuth + webhook relay
   verified live against the Help Scout test account (redirect URI is
   :3001 per the live app registration, not :3000; loopback listener
   verified with a live signed delivery).
8. Phase 2 completed: before/pages/* screenshots (1920×1080, light + dark
   for Dashboard) + per-page VLM audits (reports/pages/*.md) + issues.md
   (UX-1..UX-35 incl. backend-blocked UX-26B/34/35 and false positives).
9. Rule-2 queueing: UX-audit program written to ai/plan.md; Batch UXA-1
   (UX-1..UX-9 + UX-25) opened in ai/progress.md.
10. Phase 3, Batch UXA-1 implementation: fixes applied to
    `crates/app-shell/src/` only (design/mod.rs, design/controls.rs,
    chart.rs, app/mod.rs, pages/{connectors,automation,onboarding,
    dashboard,reports,settings,inbox,customers,ai_center,issue_radar,
    notifications,operations}.rs). No backend/API/schema/CI/config files
    touched; Cargo.lock unchanged (`--locked` everywhere).
11. Verification: fmt clean; clippy `--workspace --all-targets -D warnings`
    clean; app-shell lib tests 239/239; xtask 11/11 (incl. secrets
    hygiene); SPP_SMOKE rc=0 (24 pages / 4222 shapes).
12. **kittest snapshot gate made locally runnable**: DENY-1's standing limit
    ("no Vulkan ICD, no sudo") worked around by downloading
    `mesa-vulkan-drivers` .deb and extracting lavapipe locally
    (`/tmp/lvp/extracted`, `VK_DRIVER_FILES` + `LD_LIBRARY_PATH`) — the
    24 kittest suites + style_lint now run diff-locked in this sandbox.
    89 snapshots drifted, every drift mapped to an intended UXA-1 fix
    (audit of the drift list: no unexpected pages); baselines regenerated
    with UPDATE_SNAPSHOTS=1 and the full suite re-ran diff-locked GREEN
    (the diff-locked-twice discipline).
13. After-evidence: app re-driven under Xvfb at 1920×1080 light theme
    (+ Dashboard dark) for all 10 items; header + luma verified per shot
    (a theme-toggle miss mid-run darkened 11 shots — detected via luma
    scan, all re-shot light).
14. Disk hygiene: incremental cache + registry cache + debug symbols
    stripped (9.9 GB rootfs had filled); no source/data loss.

## Next

- Finish the last two evidence shots (plain inbox list; Help Scout
  disclosure expanded while connected) — VLM rate-limit pauses only.
- Batch UXA-1 ledger sync (completed.md, plan.md, progress.md), commit
  series (one commit per page/design-system group, issue-tagged), push,
  Rule 8 STOP.

## Batch UXA-2 (2026-11-06, owner "continue" after the UXA-1 STOP)

15. Rule-2 queueing: UX-10..UX-19 moved from ai/plan.md to ai/progress.md.
16. Implementation (crates/app-shell/src/ only; no backend/CI/config):
    - design system: dark-token contrast pass (muted_foreground/border/
      sidebar), inverted selected-chip state, `empty_state_cta_with`,
      Title-Case composer/pager labels; shared `relative_time` helper
      moved to pages/mod.rs for reuse (UX-10/12/15).
    - notifications: (type,title,body) grouping with ×N counters, group
      read/unread, humanized type badges, monospace truncated error
      bodies, 3 new unit tests (UX-13).
    - operations: Failed jobs/Sync problems/Automation approvals tiles
      navigate to Diagnostics/Sync Health/Automation with named drill
      lines; truthful subtitle; relative "updated …" stamp (UX-14).
    - search: example-query chips replace the "+ Try 'export'" CTA;
      "Scope:" label (UX-15).
    - live chats / custom objects / outreach / support graph: empty-state
      CTAs wired to real destinations (Settings / New Type form / New
      Campaign form / Connectors), contradiction + pluralization fixes,
      de-jargonized subtitles (UX-16).
    - incidents: labeled Status/Severity filters, filter-aware empty
      states with Clear Filters + New Incident CTAs (UX-17).
    - knowledge: import disclosure auto-opens on first-run empty state,
      "Import Documents" CTA force-expands it, parenthesized empty
      titles rewritten (UX-18).
    - customers/organizations: "Never" / "No domains yet" fallbacks,
      Title-Cased headers (UX-19).
    - settings: user-language subtitle + Vector-tab hint, hover-note
      dev prefixes stripped (UX-11) + full Title-Case button sweep.
    - remaining pages (ai_center, automation, connectors, dashboard,
      diagnostics, inbox, issue_radar, reports, sync_health): Title-Case
      buttons, chips, tabs, and table headers (UX-10).
17. Verification: fmt clean; clippy `--workspace --all-targets -D warnings`
    clean; app-shell lib tests 242/242 (3 new UX-13 tests; token-freeze
    test updated for the intended UX-12 sidebar value); xtask 11/11;
    SPP_SMOKE rc=0 (24 pages / 5131 shapes).
18. kittest drift discipline: lavapipe re-extracted to a persistent home
    location (the earlier /tmp copy had been reaped mid-session, which
    first produced a bogus all-fail "drift" list — caught by auditing
    one failure's panic text before regenerating anything). The real
    drift: 157 snapshots, every one mapped to an intended UXA-2 change
    (three VLM-audited diff images: dashboard scope-chip = chip selected
    state; notifications = Mark Read/Unread/All Read renames + grouping;
    hero dark = token brightening). Baselines regenerated with
    UPDATE_SNAPSHOTS=1 (plus 5 label-dependent kittest assertions
    updated to the renamed buttons), and the full suite re-ran
    diff-locked GREEN end-to-end (exit 0, 24 suites).
19. After-evidence: deterministic per-page screenshots via the app's own
    `ui-screenshot` feature (SPP_SHOT_PAGE/SPP_SHOT_THEME) — 15 shots
    at 1920×1080 (dashboard light+dark, notifications, operations,
    search, incidents, customers, organizations, knowledge, custom
    objects, outreach, live chats, support graph, settings default +
    vector tab) — no navigation flakiness, no VLM dependency for
    capture. (Sidebar navcoord/palette driving was parked: VLM 429
    rate-limits mid-run.)
20. Disk hygiene again: diff artifacts + incremental cache removed,
    stale test binaries stripped, registry cache cleared (rootfs was at
    98%).
