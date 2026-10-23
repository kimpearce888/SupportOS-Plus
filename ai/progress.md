# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — batch 25 (clean-audit items 57-66, MEDIUM tier)

Item 56 (report-builder wire-or-remove) stays parked in `plan.md` awaiting the
owner decision noted there; this batch is the next ten eligible items.

57. [med|bug] new_conversation + customer_reply both fire on every customer thread, no once-guard — automation.rs:1733 + workers.rs:1751-1766
58. [med|bug] Knowledge FTS lane never dedupes per document — search.rs:591-593
59. [med|bug] Fused RRF scores sorted against FTS-only 1.0 scores — search.rs:993-998
60. [med|bug] Orphaned vectors never cleaned; stale chunk text keeps matching — search.rs:908-911
61. [med|bug] Removing a type field bricks property updates of existing objects — custom_objects.rs:417-420,579-588
62. [med|bug] parse_mentions diverges from scan_for_mentions (ASCII-only, no boundary) — mentions.rs:223-226
63. [med|bug] rm_tags is a no-op in ammonia 4.x; title/textarea content leaks visible — richtext.rs:131-136
64. [med|bug] rebuild_indexes double-inserts every thread into fts_threads — search.rs:1286-1297
65. [med|bug] Notification sweep advances cursor past unprocessed events (>500 skipped forever) — notification_sweep.rs:124,46
66. [med|bug] textutil: bare `<` swallows the rest of the body; double entity decode — textutil.rs:37-48
