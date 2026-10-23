# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — Batch 30 (opened 2026-10-24)

107. [low|bug] redact_string byte-slicing panic (dead caller) — config.rs:304-306
108. [low|bug] Digest doc counts: space-format vs ISO-T lexical comparison — digest.rs:471
109. [low|bug] New-conv windows key on local_created_at; deleted-filter claim false — reports.rs:133,251-256
110. [low|issue] %W year-boundary week split — reports.rs:236
111. [low|issue] LIMIT caps silently undercount alert claims — analytics.rs:219,241-246
112. [low|issue] Concentration denominator counts NULL-customer conversations — analytics.rs:483,512
113. [low|issue] open_closed_distribution drops pending — issue_impact.rs:85
114. [low|issue] email_domain filter unanchored, op ignored — segment.rs:1575
115. [low|bug] campaign_reconcile references nonexistent updated_at column — outreach.rs:1047-1054
116. [low|bug] ORDER BY remote_created_at never populated by sync (2 sites) — interaction_current.rs:808, interaction_engine.rs:1448

Item 56 (report-builder wire-or-remove) stays parked in `plan.md` awaiting
the owner decision noted there, as do 150/151 (dead-feature decisions).
