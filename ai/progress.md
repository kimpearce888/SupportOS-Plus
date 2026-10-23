# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — Batch 28 (clean-audit items 87-96)

87. [low|bug] Ratings upsert never re-links conversation/customer on conflict — workers.rs:3771-3774
88. [low|bug] Cancelled initial sync recorded LIVE; threads backfill never re-runs — sync_engine.rs:1309-1316
89. [low|bug] Threads pass hardcodes failed:0 then gets stamped ok — sync_engine.rs:1167-1172,1246-1249
90. [low|gap] Post-initial-sync AI backfill covers only first 50 conversations — workers.rs:1638
91. [low|gap] Platform-side deletions never propagate (saved replies/orgs/workflows) — sync_engine.rs:1962-2015
92. [low|bug] Dead "-> 404" mapping in get_rating (use is_not_found) — helpscout_real.rs:1737
93. [low|gap] OAuth TTL sweeper + redirect-URI verifier dead; states never expire — oauth_state.rs:49-66
94. [low|bug] Webhook prune deletes unprocessed pending events under backlog — webhook.rs:131-135
95. [low|bug] weekly_digest completes the ledger row despite failed narrative — ai_pipeline.rs:2858-2868
96. [low|bug] Empty `{}` analysis accepted+cached; eval gate disagrees — ai_pipeline.rs:954-955
