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

## Batch UXA-3 (2026-11-06, owner "continue" after the UXA-2 STOP)

21. Rule-2 queueing: UX-20..UX-30 moved from ai/plan.md to
    ai/progress.md (UX-25 already fixed in UXA-1; UX-26's backend half
    stays blocked as UX-26B).
22. Implementation (crates/app-shell/src/ only; no backend/CI/config;
    the sandbox was recycled between sessions so lavapipe + the GL/X11
    userland had to be re-extracted first — see item 26):
    - sync_health: hero subtitle in user language + relative time
      ("Last success: <iso> · 3h ago", dev note "forensic value" gone),
      both tables `table_stretched` + bounded ScrollAreas, Title-Case
      headers, hover-anchored last-success stamps (UX-20).
    - diagnostics: EventCounts (one unfiltered fetch feeds stable
      per-level chip counts — "Errors 36" IS the path to the errors),
      proper pluralization, runs + failed-outbound tables stretched and
      scroll-bounded; new unit test
      level_counts_cover_the_full_log_while_the_view_filters (UX-21).
    - settings/backups: Destructive restore + file-naming confirmation,
      human_bytes + relative_time metadata with exact values on hover
      (UX-22); lmstudio: "Open AI Center" NavRequest button (the tab can
      now navigate — Settings returns Option<NavRequest> to the shell),
      purpose subtitle, field hints/hovers, base-URL validation on save
      (UX-23); vector: labelled toggle wired on change, truncating +
      copyable store path (UX-24); helpscout: truthful 40-char copy
      before AND after save, no "the platform" blame (UX-26); hours:
      purpose subtitle, Warning badge, spelled-out abbreviations,
      self-explaining empty state (UX-29); syncai/general: uniform
      checkbox list, 110×26 DragValues with hovers, dirty-state save
      row ("Save Changes" + note / "All changes saved" badge) backed by
      general_persisted (UX-30).
    - inbox: composer pane scrolls in place; Cc/Bcc above the body
      (standard mail layout); SP1 breathing between the three list-row
      lines (UX-27).
    - dashboard: filter card spans content width with one wrapped chip
      row; breakdown cards cap at 8 bars + "and N more…"; chart caption
      inside the frame's top-right corner, named ("First X → Latest Y")
      via the shared line_chart_axes — Reports inherit it (UX-28).
    - design system: assignee(None) is plain italic "Unassigned" text
      (the dashed pill read as a second chip language) (UX-27).
    - shared helpers: human_bytes in pages/mod.rs + unit test.
23. Evidence harness extension (dev-only, feature-gated):
    SPP_SHOT_TAB env var in the ui-screenshot build selects the
    Settings tab by label (set_tab_by_label) — the audit evidence needs
    Backups/LM Studio/Vector/Help Scout/Hours/Sync&AI tab shots and no
    click automation exists in-sandbox (no xdotool; xvfb-run needs
    xauth). Off in every normal/CI build like the rest of the harness.
24. Verification: fmt clean; clippy --locked --workspace --all-targets
    -D warnings clean (re-verified after the two in-session code
    changes below); app-shell lib 244/244 (2 new tests: human_bytes,
    event-count coverage); xtask 11/11; SPP_SMOKE rc=0 (24 pages /
    4259 shapes — the count differs from UXA-2's 5131 because the
    recycled sandbox's data dir is a fresh DB, not the audit-era one;
    the criterion is per-page non-empty render, unchanged).
25. kittest drift discipline: first diff-locked run --no-fail-fast
    produced 46 snapshot drifts across 8 suites + 1 style-lint failure.
    The lint failure was REAL and fixed in code: UX-27's row breathing
    used off-scale add_space(1.0) — DC §2.4 — replaced with
    design::scale::space::SP1, style_lint 2/2 after. All 46 drifts
    audited one-by-one, every one mapping to an intended UXA-3 change:
    dashboard ×7 (UX-28), diagnostics ×7 (UX-21), sync_health ×7
    (UX-20), settings ×6 (UX-22/23/24/29/30; helpscout-tab golden
    byte-identical — UX-26 is hover/save-note copy, invisible in a
    static shot), reports ×6 (shared chart caption, UX-28),
    inbox_page ×6 + inbox_copilot ×3 (UX-27), kittest_widgets ×4
    (assignee). Zero unexpected suites. Baselines regenerated with
    UPDATE_SNAPSHOTS=1, full suite re-ran diff-locked GREEN (exit 0,
    26 binaries incl. lib + style_lint); exactly 46 goldens changed in
    git, one-for-one with the audited drift list.
26. Environment rebuild for the recycled sandbox: mesa-vulkan-drivers
    .deb re-downloaded + extracted to /home/z/mesa-lvp/extracted
    (VK_DRIVER_FILES/VK_ICD_FILENAMES/LD_LIBRARY_PATH); xvfb-run
    unusable (no xauth) → Xvfb :N -ac driven per-shot from a script
    (the sandbox reaps background processes between commands — one
    Xvfb lifetime per shot); libxkbcommon-x11 + libxcb-xkb .debs
    extracted into the same prefix (egui-winit dlopens them).
27. After-evidence: 11 shots at 1920×1080 via ui-screenshot + the new
    SPP_SHOT_TAB (sync_health, diagnostics, settings backups/lmstudio/
    vector/helpscout/hours/syncai, inbox, dashboard light+dark), all
    rc=0, all luma-verified (10 light 244-250, 1 dark 26.6). Backups
    shot seeded with two real SQLite backup files (random bytes fail
    the integrity check the restore flow runs). VLM read-back spot
    checks were 429-rate-limited all session (three attempts, same as
    the UXA-1/UXA-2 sessions hit) — page identity is instead
    structurally verified: same binary + SPP_SHOT_PAGE label match, per
    shot rc=0, luma theme scan, distinct byte sizes, and the
    diff-locked kittest goldens that encode the exact fixed states.
28. Disk hygiene: incremental cache (2.4 GB) + registry cache +
    kittest diff artifacts removed; rootfs back from 99% to a working
    level mid-batch.
