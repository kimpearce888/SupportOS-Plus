# SupportOs-Plus — TODO

> Live TODO queue for remaining work items, executed ten at a time under
> `rule.md` (batch size raised from five by the owner, 2026-10-23).
> Status flow: `plan.md` → `progress.md` → implementation → verification →
> `completed.md`. Planning context and decision ledger: `decisions.md`.

## Queue state

**EMPTY — no open items (authoritative, 2026-11-04, post-Batch GA-1).**

Every program recorded in the engagement is consumed and has its completion
record in `completed.md`: the v2-blueprint batches 1–15 (v1.4.0→v1.7.0); the
v1.7.0 clean-audit items 1–165 (batches 16–36); the Design-System Reskin
RS-01..RS-21 + the RS-4 rejection wave + RS-4-11 + Gate G4 (accepted) + RS-6
(v1.9.1); the refactor program RF-1..RF-11 + FINISH; KB-1, AUDIT-1,
KB-2..KB-7; REL-1 (v1.9.2); AU-1 (10/10); AU-2 (15/15); REL-2 (v1.9.3,
published and live-verified). Nothing is pending, parked, or held; nothing
resumes on `continue` until new work is queued here by the owner.

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
