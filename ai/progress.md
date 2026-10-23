# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — Batch 29 (clean-audit items 97-106)

97. [low|issue] Rewrite paths bypass redaction — ai_pipeline.rs:1280-1285,1350-1358
98. [low|issue] Tool args: as_i64 rejects schema-valid floats — ai_tools.rs:418 (+7 sites)
99. [low|issue] get_issue_clusters returns 3× the documented limit with a query — ai_tools.rs:695
100. [low|issue] Knowledge chunks: no attempts cap → infinite retry — embeddings.rs:792,841-856
101. [low|issue] Malformed 200 → literal "Unknown" content — ai_lm_studio.rs:511-516
102. [low|issue] Draft signature picks an arbitrary agent's name — ai_local.rs:531-533
103. [low|bug] Org member_count capped by LIMIT 100 — people_store.rs:110
104. [low|bug] last-activity/resolution subqueries miss deleted_at — people_store.rs:753-754,985-989
105. [low|bug] Org name linking case-sensitive vs NOCASE elsewhere — people_store.rs:1145-1146
106. [low|issue] CSV export lacks formula-injection guard — backup_service.rs:351-353
