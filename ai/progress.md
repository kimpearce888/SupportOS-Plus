# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — Batch 23 (items 36-45, started 2026-10-23)

36. [med|bug] customer_messages includes soft-deleted threads, takes OLDEST 30 — ai_attributes.rs:690-692
37. [med|issue] Extraction runs stuck `queued`, invisible to the cost ledger — ai_attributes.rs:966-968
38. [med|bug] Cache-hit path mixes hash-matched run id with newest run's analysis — ai_pipeline.rs:872-877
39. [med|bug] Draft fallback stores the raw JSON envelope as the draft — ai_pipeline.rs:1071
40. [med|gap] Interaction-strategy prompt block hard-coded None; section unreachable — ai_pipeline.rs:1898-1900
41. [med|issue] get_customer_memory relays AI rows as "human-written" — ai_tools.rs:1378
42. [med|issue] Cluster prompt has no conversation cap → context blowup at volume — ai_pipeline.rs:1518-1520
43. [med|bug] Copilot runs invisible to the cost ledger (no usage/endpoint) — copilot.rs:216-219,354-375
44. [med|bug] Tool-budget cutoff leaves dangling tool_call ids (400 on strict servers) — copilot.rs:254-292
45. [med|bug] insert_message swallows INSERT failure, echoes stale rowid — copilot.rs:618-635

Per Rule 2 these ten items were removed from `plan.md` before implementation
began. Implementation is in progress; items leave this file for `completed.md`
only after Rule 5 verification.
