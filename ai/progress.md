# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — Batch 24 (clean-audit items 46-55, the first ten MEDIUM-tier findings)

46. [med|bug] Knowledge-chunk batch poisoned by one empty vector — embeddings.rs:895-914
47. [med|bug] Customer email patch deletes ALL stored emails, errors swallowed — people_store.rs:1245-1256
48. [med|bug] verify_bundle leaks decrypted plaintext temp on error paths, no sweep — encrypted_sync.rs:481-487
49. [med|gap] Workspace reset leaves downloaded attachment files on disk — data_reset.rs:110-118
50. [med|issue] CSS blocklist bypassable via escapes/comments — security.rs:106-110
51. [med|issue] Blocking DNS resolution inside async contexts — security.rs:482 + connectors.rs:633,641,757
52. [med|bug] shared_tag display query joins a conversation's tags to itself — quality.rs:1193-1195
53. [med|bug] waiting_stats.at_risk counts breached-vs-first-response for replied tickets — sla.rs:532
54. [med|issue] Radar trend precedence: "new" unreachable for ≥3 clusters — analytics.rs:375-380
55. [med|issue] backlog_jump compares against a false week-ago baseline — digest.rs:265-267

Batch 23 (items 36-45) is complete and verified; the record is in
`completed.md`. The remaining findings are queued in `plan.md` (items
56-165); the next ten start only on the owner's "continue".
