# SupportOs-Plus — TODO

> Remaining work items, executed five at a time under `rule.md`.
> Status flow: `plan.md` → `progress.md` → implementation → verification → `completed.md`.
> Planning context and decision ledger: `decisions.md`.

## Items

Source: **v1.7.0 clean source audit** (owner-mandated: source code only, no test
suite, no descriptive docs; every item carries file:line evidence and a concrete
failure scenario in the audit report
`/home/z/my-project/download/SupportOS-Plus-v1.7.0-clean-audit.md` and the
per-domain AUD-A..AUD-M records in the engagement worklog, 2026-10-21/22).
The prior backlog (v2 blueprint, phases R/A/K/P) is complete — batches 11–15,
released as v1.4.0 → v1.7.0. Items are ordered by severity tier; within the
HIGH tier, small surgical fixes come first, architectural items later.
Merged root-causes share one item (A2+J2, A3+B2, E1+K8, E2+F4, G6+I2, …).
Batch 16 = items 1–5 (provider delegations, chunk_text UTF-8 panic,
not_equals inversion, dead 'customer_message' literal, no-simulation gate
blind spots) — verified and recorded in `completed.md`.
Batch 17 = items 6–10 (worker-restart guard flags, writer-lane-across-LLM,
duplicate-send idempotency, raw_json_hash writer, LM Studio probe API key) —
verified and recorded in `completed.md`.
Batch 18 = items 11–15 (backup restore verified+confirmed+fsync+WAL-safe,
QA rebuild preserves AI layers, SLA waiting from last customer message,
repeated_question_unsolved reads wired friction data, campaign sends use
the remote mailbox id) — verified and recorded in `completed.md`.
Batch 19 = items 16–20 (outreach attempts spent on every outcome, the
atomic 'sending' claim with mirror-probing crash recovery, the
dual-generation trigger parser, approved write actions reaching Help
Scout through the outbound push lane, activity events keyed by the local
conversation id) — verified and recorded in `completed.md`.
Batch 20 = items 21–25 (vector point-id namespacing per chunk family +
cross-family refusal + the one-time re-embed migration, rebuild_activity
enqueued after initial sync and repaired once for pre-fix mirrors, the
SyncTrigger field ending auto-sync toast spam, the composer-AI result
pinned to the job row, the phantom "waiting" status replaced by the
platform's own `open`) — verified and recorded in `completed.md`.

### HIGH — shipped features broken / silent permanent failure

26. [high|bug] Custom-objects form sends numbers as JSON strings — object creation always fails — pages/custom_objects.rs:286-292
27. [high|bug] Custom-objects type builder offers checkbox/url types core rejects — pages/custom_objects.rs:24-52

### MEDIUM — wrong behavior on realistic paths

28. [med|bug] Stale-thread deletion treats one unpaginated page as the full thread set — sync_engine.rs:2346-2348
29. [med|bug] `tags_changed` emitted with `prev=null` on every re-ingest — sync.rs:292-296
30. [med|bug] Retried saved-reply create re-POSTs (duplicates on the platform) — workers.rs:3236,1602
31. [med|gap] `api_concurrency` setting parsed/saved but never applied — api_queue.rs:94-97
32. [med|bug] Webhook secret capped to 40 chars at registration but verified with the full secret — webhook_health.rs:255,168
33. [med|bug] Team id written into the user-assignee slot; `assigned_team_id` unconsumed — helpscout_real.rs:1392-1394
34. [med|bug] Attribute extraction parses raw output without fence-stripping/json_mode — ai_attributes.rs:817-818
35. [med|issue] Attribute extraction bypasses the prompt-redaction boundary — ai_attributes.rs:776-787
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
56. [med|gap] Entire report-builder/dashboard surface computed but never rendered (32 endpoints) — reports.rs:1110+ — owner decision needed: wire or remove
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
67. [med|gap] translation_cache never pruned; retention policy omits it — translation.rs:44 + maintenance.rs:113-118
68. [med|gap] Audit trail never surfaced; dead reader drops non-JSON payloads — audit.rs:153
69. [med|bug] Toasts expire on wall-clock while hidden (errors silently lost) — toasts.rs:98,115
70. [med|gap] Confirm-dialog system never armed; destructive actions unconfirmed — app.rs:570-585
71. [med|gap] SPP_SMOKE returns before shell/renderer boot — renderer regressions pass CI — main.rs:139-144
72. [med|bug] Palette Enter on zero hits defocuses the input; keystrokes swallowed — palette.rs:248-261
73. [med|issue] Transform completion overwrites typing made in flight — pages/inbox.rs:1749-1753
74. [med|bug] Role-model field persists + enqueues golden_eval per keystroke — pages/ai_center.rs:1210-1242
75. [med|bug] Copilot submits on a global Enter with no focus check — pages/ai_center.rs:657-658
76. [med|issue] Sync invalidation destroys the open Library edit form — pages/operations.rs:648-652
77. [med|issue] Sync invalidation silently reverts unsaved Settings edits — pages/settings.rs:550-552
78. [med|gap] Inbox hard-capped at 50, no pagination — pages/inbox.rs:109-110
79. [med|issue] Row click silently wipes the typed reply draft — pages/inbox.rs:742-747
80. [med|bug] Approvals attributed to hardcoded user 5 — pages/automation.rs:323-330
81. [med|bug] Known-issue links accept dangling ids and count them — pages/issue_radar.rs:147-149
82. [med|gap] Customers page fetches page 1 only (50 cap) — pages/customers.rs:217-222

### LOW — edge cases, provable minor defects

83. [low|bug] LIKE input not escaped despite ESCAPE clause (customers.rs:162, inbox.rs:185, mirror_readouts.rs:207; trailing `\` errors)
84. [low|bug] jobs::fail called with literal job id 0 (2 sites) — workers.rs:2748,3067
85. [low|bug] Rating fetch errors swallowed; ratings permanently lost — workers.rs:1245-1246
86. [low|bug] Panic containment marks copilot jobs retryable (re-persists user message) — workers.rs:369-374
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
127. [low|bug] Digest chip 16.max slice (dead truncation + OOB risk) — pages/reports.rs:544
128. [low|bug] QA rows render `#Some(42)` — pages/reports.rs:885
129. [low|bug] Literal `\` mid-sentence in 3 hover texts — pages/issue_radar.rs:282,292,309
130. [low|bug] Notification toggle reads the wrong list for Mentions rows — pages/notifications.rs:111-116
131. [low|issue] Incidents: invalid ids silently dropped; filters without refetch — pages/incidents.rs:185-203,320-344
132. [low|issue] Diagnostics clear_log via reader lane, errors swallowed — pages/diagnostics.rs:101
133. [low|bug] Palette: same-frame Enter confirms stale hit; no outside-click close — palette.rs:258,215
134. [low|bug] Concave polygon to convex_polygon (area fill) — chart.rs:77-84
135. [low|bug] Stale inserted_draft provenance → phantom AI-draft acceptances — pages/inbox.rs:1435,1505
136. [low|gap] Mention popup highlight never moves — pages/inbox.rs:2277-2278
137. [low|gap] Filter/assign refs load once per session — pages/inbox.rs:455,882
138. [low|gap] log_level/display_timezone saved, never read — pages/settings.rs:232
139. [low|gap] qdrant_enabled hint references a nonexistent control; Integrations tab hint wrong — pages/settings.rs:1956, pages/ai_center.rs:949
140. [low|issue] Unconfirmed destructive deletes (knowledge docs, rules, types, objects, connectors, segments, clear-log) — route through the item-70 dialog
141. [low|issue] Business-hours `days` unvalidated, silently ignored — pages/settings.rs:2039 + sla.rs:252-257
142. [low|issue] Ops drill-down layers onto stale filters — pages/inbox.rs:96 + app.rs:228
143. [low|issue] `#778` number search silently dropped — pages/inbox.rs:1050
144. [low|gap] Support-graph invalidation clears results nothing refetches — pages/support_graph.rs:109-115
145. [low|gap] onboarding save_step dead; setting unread — pages/onboarding.rs:109-114
146. [low|gap] Recipient snapshot cap 5000, matched discarded — pages/outreach.rs:190-204
147. [low|issue] ensure_loaded-on-empty re-query pattern (7 pages, wasted work) — pages/outreach.rs:121 + 6 more

### GAPS — dead surfaces, drift traps, build/policy hygiene

148. [low|gap] Remove dead Runner/JobRegistry/bulk_*/EventEmitter machinery — runner.rs, workers.rs, eventing.rs
149. [low|gap] Dead prompt surfaces (report_narrative, duplicate attribute prompt, ai_features runtime incl. mismatched translation-v1) — ai_pipeline.rs:1780, ai_prompts.rs:454, ai_features.rs:159
150. [low|gap] intelligence_features ~70% dead (SLA-breach + knowledge-gap tracking never gain rows) — decide: wire or prune
151. [low|gap] ai_settings write API dead (provider_kind inert plumbing) — ai_center.rs:135-179
152. [low|gap] Dead copilot/embedding helper surface — copilot.rs:24-63, embeddings.rs:96-161, ai_lm_studio.rs:24-55
153. [low|gap] AppConfig helpscout/ai/sync/loopback fields constructed, never consumed — config.rs:18-61
154. [low|gap] Dead hybrid_search fusion stack (id-namespace mismatch latent) — hybrid_search.rs:162
155. [low|gap] Legacy FTS tables written every sync, never queried; chunk_version never read — search.rs:1356
156. [low|gap] Connector AI-search/auth-edit + data_tools runtime dead (legacy-schema drift) — connectors.rs:1024, data_tools.rs:63
157. [low|gap] AppDir staging never cleaned (stale files ship) — xtask/package.rs:157
158. [med|gap] appimagetool: unversioned URL, no checksum, executed in the release path — xtask/package.rs:61,551-574
159. [low|bug] AppImage arch hardcoded x86_64 vs aarch64 deb support — xtask/package.rs:162,173
160. [low|bug] Non-atomic tool-cache write + no download timeout — xtask/package.rs:551-571
161. [low|gap] deny.toml stale "becomes deny" promise; bans never evaluated in CI — deny.toml:49-51 + ci.yml
162. [low|gap] egui/egui_extras workspace deps unconsumed (egui_extras not in lock) — Cargo.toml:50,58
163. [low|gap] catalog all_metric_entries/all_dimension_entries dead — catalog/reporting.rs:328,401
164. [low|gap] verify-icons never invoked by any automation — xtask/icons.rs:186
165. [low|gap] No `--locked` on CI/release cargo invocations — ci.yml, release.yml

## Notes

- Severity is the audit's call; the owner may re-order freely before any batch.
- Items 56 (report-builder wire-or-remove) and 150/151 (dead-feature decisions) need an owner decision; they stay parked at their tier until then.
- Every item keeps its full evidence (excerpt + failure scenario) in the audit report and the worklog; this file is the execution queue only.
