# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — none

Batch 25 (clean-audit items 57-66, the second ten MEDIUM-tier findings:
the automation trigger pair firing both on every customer thread, the
knowledge FTS lane a single multi-chunk document could flood, the fused
ranking that buried its own semantic hits under FTS-only 1.0 scores, the
orphaned vectors outliving their chunks with stale text, the type-field
removal bricking its objects' patches, the mention parser disagreeing
with its scanner, the sanitizer leaking the text of the tags it claimed
to drop, the rebuild writing every thread into fts_threads twice, the
sweep cursor sailing past its >500 backlog, and the text extractor
swallowing bodies at a bare '<' while double-decoding entities) is
complete and verified; the record is in `completed.md`. Item 56 remains
parked pending the owner's wire-or-remove decision. The remaining 99
audit findings are queued in `plan.md` (items 67-165) and the next ten
start only on the owner's "continue".
