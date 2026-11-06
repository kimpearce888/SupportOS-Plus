# SupportOs-Plus — TODO

> Live TODO queue for remaining work items, executed ten at a time under
> `rule.md` (batch size raised from five by the owner, 2026-10-23).
> Status flow: `plan.md` → `progress.md` → implementation → verification →
> `completed.md`. Planning context and decision ledger: `decisions.md`.

## Queue state

**UX-audit program queued 2026-11-05/06 — 23 items open after Batch UXA-1
(owner directive: full UX/UI audit + fix, evidence-gated; UXA-1 consumed
UX-1..UX-9 + UX-25 on 2026-11-06, record in `completed.md`).**

Every prior program is consumed with completion records in `completed.md`
(v2-blueprint 1–15; clean-audit 1–165; RS-01..21 + RS-6; RF-1..11 + FINISH;
KB/AUDIT batches; REL-1/2/3; AU-1/AU-2; GA-1/GA-2; RW-1; DENY-1). The new
program below opens on the owner's UX-audit directive (session 2026-11-05):
discover every journey, audit every page/state at 1920×1080 (+dark theme),
fix frontend-only defects in ten-item batches per rule.md, evidence per
`ux-audit/reports/` (scope contract: `ux-audit/reports/scope.md`; issue
detail: `ux-audit/reports/issues.md`). Backend-blocked findings are logged
there, not queued here.

### UX-audit queue (Batch UXA-1 CONSUMED 2026-11-06 — items UX-1..UX-9 + UX-25 completed and verified, record in `completed.md`; next batch UXA-2 starts below at UX-10)

1. UX-10 (minor) — Global button/label casing inconsistency (lowercase
    "save credentials"/"send"/"edit" vs Title Case elsewhere); normalize.
2. UX-11 (minor) — Dev jargon in user copy (Settings subtitle "persisted
    in the settings table", Vector tab "decision D4 …", raw `qdrant_enabled`).
3. UX-12 (minor) — Dark-theme contrast: dim section labels/subtitles;
    near-identical surface grays; too-subtle active filter chips.
4. UX-13 (major) — Notifications: 18 identical job-failure rows, no
    grouping/action; raw error text with localhost URL, no truncation.
5. UX-14 (major) — Operations: highest-count tiles not clickable; subtitle
    contradicts which tiles filter the inbox.
6. UX-15 (major) — Search: nonsensical "+ Try 'export'" empty-state CTA;
    scope selector lacks a visible selected state.
7. UX-16 (major) — Empty-state dead ends on Live chats, Custom Objects,
    Outreach, Support Graph (copy references actions with no CTA).
8. UX-17 (major) — Incidents: empty state blames filters when none are
    applied; two unlabeled "Any" dropdowns.
9. UX-18 (major) — Knowledge: empty-state primary action hidden in a
    disclosure; CTA is plain text.
10. UX-19 (major) — Customers/Organizations: fully-empty columns ("—")
    with no explanation or fallback content.
11. UX-20 (major) — Sync Health: "last success" header/value misalignment;
    recent-runs table clipped without scroll affordance.
12. UX-21 (major) — Diagnostics: error/warning counts not linked to the
    log; sync-reports trailing column detached.
13. UX-22 (major) — Backups: destructive "restore" without confirmation
    affordance or danger styling; raw bytes/ISO timestamps.
14. UX-23 (major) — LM Studio tab: dead-end "Test the connection from the
    AI Center page" (no link); missing validation/labels.
15. UX-24 (major) — Vector tab: static "✓ enabled" text, no toggle or
    rebuild/copy affordances.
16. UX-26 (major, frontend copy) — Webhook-secret copy claims "the platform
    caps webhook secrets at 40 characters" — verified false against the live
    platform (stored 48). Correct the copy; the cap itself is backend
    (logged UX-26B, blocked).
17. UX-27 (minor) — Inbox polish: pill style inconsistency; Cc/Bcc
    clipping; row crowding. (Partial: the truncated search placeholder was
    fixed with the UX-8 panel work in UXA-1.)
18. UX-28 (minor) — Dashboard polish: filter-card width/dead space; clipped
    "By tag" row; ambiguous chart caption.
19. UX-29 (minor) — Hours tab: no explanation; "not configured" badge tone;
    lowercase "edit".
20. UX-30 (minor) — Sync&AI tab: inconsistent checkbox styling; cramped
    numeric inputs; no dirty-state/per-card save feedback.
21. UX-31 (minor) — Palette: selected-row subtitle contrast; possible text
    bleed-through on the selected row.
22. UX-32 (polish) — Backups: export buttons look disabled; duplicate
    header; no danger-zone separation.
23. UX-33 (polish) — Bell icon tooltip; "synced Ns ago" context.

Prior programs (v2-blueprint 1–15; clean-audit 1–165; RS-01..21 + RS-6;
RF-1..11 + FINISH; KB/AUDIT; REL-1/2/3; AU-1/AU-2; GA-1/GA-2; RW-1; DENY-1)
are fully consumed with completion records in `completed.md`.

## Notes

- Historical batch narratives and the original program openers (the v1.7.0
  audit queue, the RS/RF/KB/REL/AU openers, scope contracts) live in
  `ai/archive-plan.md` (moved 2026-11-04, HYG-1); completed-batch item
  transcripts live in `ai/archive-progress.md`. Completion records + full
  verification evidence: `completed.md`. Decisions: `decisions.md` (D1–D13).
- New work opens only on an owner directive: it is written into this file as
  numbered items, moved ten at a time to `progress.md` (Rule 2), and leaves
  for `completed.md` only after verification (Rules 4–5).
- Batch GA-1 (item GA-1-1: decouple `api_queue` from `helpscout_real`) was
  opened here 2026-11-04 on the owner's "yes, continue" after the graphify
  knowledge-graph audit surfaced the `api_queue.rs <-> helpscout_real/mod.rs`
  module cycle, moved to `progress.md` per Rule 2, completed and verified the
  same session, and left for `completed.md` at the Rule 4 sync. The queue is
  empty again; the graphify outputs live outside the repo (`/tmp/spp-graph/`).
- Batch GA-2 (item GA-2-1: give the shared JSON field helpers a neutral home)
  opened here 2026-11-04 on the owner's "continue" — the graphify audit's
  remaining candidate: `str_field`/`i64_field`/`opt_str_field` were defined
  inside the `pages/queries.rs` page module while 9+ sibling pages consume
  them through the `pages` re-export (mod.rs:352). Moved to
  `progress.md` per Rule 2, completed and verified the same session, left for
  `completed.md` at the Rule 4 sync; queue empty again.
- Batch RW-1 (item RW-1-1: connectors must not block the async runtime)
  opened here 2026-11-05 on the owner's "continue" after the repowise
  (repowise-dev/repowise v0.60.1) code-health audit of 3845e10 surfaced the
  one real defect: `fetch_rows` (async, awaited from the background worker
  loop at workers/mod.rs:1130) made blocking `std::fs` calls (local_json +
  csv branches) and synchronous rusqlite calls (sqlite branch) on the shared
  tokio runtime. Moved to `progress.md` per Rule 2, completed and verified
  the same session, left for `completed.md` at the Rule 4 sync; queue empty
  again. The repowise index stays outside the repo (`<worktree>/.repowise/`,
  untracked).
- Batch DENY-1 (items DENY-1-1..3: make the deny lane warning-free) opened
  here 2026-11-05 on the owner's "continue" after the reviewdog verification
  battery (reviewdog v0.21.2 check of 1c50e4f — tauri-driver inapplicable
  since the Phase 4 cutover, so the battery was a full local CI-lane re-run
  plus SPP_SMOKE) surfaced three cargo-deny warnings: yanked
  async-executor 1.14.0, a dead `bitflags` bans skip, and an unmatched
  `CDLA-Permissive-2.0` license allowance. Moved to `progress.md` per
  Rule 2, completed and verified the same session, left for `completed.md`
  at the Rule 4 sync; queue empty again.
- Batch REL-3 (release v1.9.4 from the final source) opened here 2026-11-05
  on the owner's Build-and-Release directive: workspace 1.9.3 → 1.9.4
  (Cargo.toml + Cargo.lock, the four member entries; the recycled sandbox
  has no Rust toolchain and ~1.7 GB free disk, so the bump was applied and
  verified mechanically — tomllib re-parse: exactly the four member package
  versions changed, 620 packages, zero stray 1.9.3 — and the Release
  workflow's `--locked` runs revalidate the lock against the manifest on
  the tagged content before packaging), CHANGELOG [1.9.4] in product
  language (RW-1: local connector reads moved off the async runtime;
  DENY-1: supply-chain gate warning-free), the release.yml extraction
  commands simulated locally (version awk → 1.9.4; changelog section awk →
  the full [1.9.4] section). Moved to `progress.md` per Rule 2; the
  annotated tag v1.9.4 triggers release.yml (gates + package + 3-distro
  .deb verification incl. APT dependency resolution / upgrade / uninstall
  + AppImage verification + publish-after-green with per-asset link and
  checksum re-verification); completed and verified the same session,
  record left for `completed.md` at the Rule 4 sync.
- Batch HYG-2 (item HYG-2-1: CHANGELOG link-reference completeness) opened
  here 2026-11-05 on the owner's "continue" (queue empty post-REL-3, so the
  continue ran an honest post-release consistency sweep, read-only): the
  sweep found the CHANGELOG header/tag/ref triangle intact everywhere else
  (zero stale 1.9.3 strings outside ledger history; README install section
  version-parameterized; 0 open issues; last 8 workflow runs green; all 23
  version headers have matching origin tags), but only 3 of 23 version
  headers carry a Keep-a-Changelog link reference ([1.1.0], [1.0.0],
  [0.2.0]) — 20 refs missing, every one backed by an existing tag, zero
  404 risk. Fix: add the 20 refs in descending order above [1.1.0], then
  mechanical verification (23/23 header→ref parity, every URL live-checked
  HTTP 200). Moved to `progress.md` per Rule 2, completed and verified the
  same session, record left for `completed.md` at the Rule 4 sync.
