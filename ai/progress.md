# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — Batch 31 (opened 2026-10-24)

117. [low|bug] refresh_replies can count the campaign's own message (clock skew) — outreach.rs:1146-1150
118. [low|bug] Duplicate graph edges for mutual links — support_graph.rs:1883-1884
119. [low|bug] Tag-filter case-folding asymmetry (Rust Unicode vs SQL ASCII LOWER) — saved_views.rs:1692-1698
120. [low|bug] Entity decode order double-decodes (&apos; missing) — knowledge_store.rs:1105-1114
121. [low|bug] Byte-unsafe slicing helpers (html helpers İ-misalignment, valid_iso_date, copilot truncate_result) — knowledge_store.rs:1136-1176, custom_objects.rs:460, copilot.rs:24
122. [low|bug] Numeric customer_property filters CAST text→0 — saved_views.rs:1843
123. [low|gap] freshness_low_usage_days setting dead — knowledge_store.rs:376,415
124. [low|bug] Pre-release vs pre-release compares as Same — version_check.rs:56-60
125. [low|bug] Sweep windows drop oldest of a burst (DESC+stamp) — notification_sweep.rs:360 (+4 sites)
126. [low|bug] parse_csv header/data index misalignment on empty headers — connectors.rs:904-916

Item 56 (report-builder wire-or-remove) stays parked in `plan.md` awaiting
the owner decision noted there, as do 150/151 (dead-feature decisions).
