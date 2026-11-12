# Refactor Progress — Live State

Last updated: session 1 (2026-11-12). Update rule: every session updates
this file before ending. Statuses: `pending` / `in_progress` / `done` /
`terminal` (done + closed in `complete.md`) / `dropped` (with reason in
`memory.md`).

## CURRENT POINTER

- Next task: **R1.0 — build `xtask strip-comments`** (see `plan.md` §7 R1).
- Pre-flight: confirm CI run on commit `<fill after session-1 push>` is green.
- Session counter: 1 completed.

## Task states

| Task | Status | Notes |
|---|---|---|
| R0 pipeline alignment | done (session 1) | traceability gate + 2 doc inputs archived; awaiting CI evidence in `complete.md` |
| R1.0 strip-comments tool | pending | spec in `plan.md` §5.1/§7 |
| R1.1 catalog+xtask strip | pending | pilot batch, 12 files |
| R1.2 core/ai_* strip | pending | ~13 files |
| R1.3 core/sync+workers strip | pending | |
| R1.4 core/helpscout strip | pending | |
| R1.5 core/storage+config strip | pending | includes audit.rs — see invariant 7 |
| R1.6 core/domain strip | pending | + core/tests |
| R1.7 app-shell/infra strip | pending | |
| R1.8 app-shell/pages-A strip | pending | 7 largest pages |
| R1.9 app-shell/pages-B strip | pending | + 26 kittest suites |
| R1.10 comment closeout | pending | lint-attr cleanup + zero-check |
| R2.1–R2.5 unnecessary code | pending | see plan §7 R2 |
| R3.1–R3.6 DRY | pending | see plan §7 R3 |
| R4.1–R4.5 clean code | pending | see plan §7 R4 |
| R5.1–R5.3 guardrails | pending (optional, stage-gated) | |
| R6.1–R6.2 polish + close | pending | |

## Review verdict tables (R2–R4 passes)

*(empty — first entries arrive with the first review batch; format:
`module → construct → verdict(keep/merge/delete/rename) → outcome`)*

## Session log

### Session 1 — 2026-11-12
- Research: full metrics snapshot (plan §6); read the no-simulation gate
  end-to-end (allowlist semantics: stale entries harmless; new marker lines
  fail); confirmed `missing_docs` is warn+allow (comment removal is
  clippy-safe); confirmed no local Vulkan (kittest is CI-only); confirmed
  rust-lang.org reachable (toolchain installable on demand).
- Environment: sandbox had been reset — re-cloned the repo, re-set git
  identity; toolchain NOT reinstalled this session (R0 needs no cargo;
  next session must provision per `handover.md` §2).
- Created this folder: plan/progress/complete/handover/memory.
- Executed R0 (pipeline alignment). Commits: `<fill>`.
- CI: `<fill: run id + conclusion>`.
