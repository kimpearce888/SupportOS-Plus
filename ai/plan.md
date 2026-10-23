# SupportOs-Plus — TODO

> Remaining work items, executed ten at a time under `rule.md` (batch size raised from five by the owner, 2026-10-23).
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
Batch 21 = items 26-30 (custom-objects form payload typing, the type
builder's phantom checkbox/url field types, stale-thread deletion
pagination, tags_changed prev-state on re-ingest, retried saved-reply
create idempotency) — verified and recorded in `completed.md`.
Batch 22 = items 31-35 (api_concurrency applied at construction and on
live save, the webhook secret cap shared by registration and
verification, team ids kept out of the user-assignee slot with
assigned_team_id consumed, fenced/prose extraction parsing on the
json_mode lane, the extraction prompt crossing the redaction boundary)
— verified and recorded in `completed.md`.
Batch 23 = items 36-45 (customer_messages excludes soft-deleted threads
and takes the NEWEST 30, extraction runs leave `queued` and reach the
cost ledger, the cache-hit path pairs the hash-matched run's id with its
own analysis, the draft fallback stores the model's text rather than the
raw JSON envelope, the interaction-strategy prompt block feeds real
strategy data instead of a hard-coded None, get_customer_memory labels AI
rows as AI rather than human-written, the cluster prompt caps its
conversation list, copilot runs recorded in the cost ledger with
usage/endpoint, the tool-budget cutoff no longer strands dangling
tool_call ids, insert_message surfaces INSERT failures instead of echoing
a stale rowid) — verified and recorded in `completed.md`.
Batch 24 = items 46-55 (knowledge-chunk batch poisoned by one empty
vector, customer email patch deleting ALL stored emails with errors
swallowed, verify_bundle leaking decrypted plaintext temps on error
paths, workspace reset leaving downloaded attachment files on disk, the
CSS blocklist bypassable via escapes/comments, blocking DNS resolution
inside async contexts, the shared_tag display query joining a
conversation's tags to itself, waiting_stats.at_risk counting
breached-vs-first-response for replied tickets, the radar trend "new"
verdict unreachable for ≥3 clusters, backlog_jump comparing against a
false week-ago baseline) — verified and recorded in `completed.md`.

Batch 25 = items 57-66 (the trigger pair's once-guard, the knowledge FTS
per-document dedupe, the fused-vs-FTS-only score scale, the orphaned
vector sweep, the type-field removal that bricked patches, the mention
parser aligned with its scanner, the ammonia rm_tags no-op, the
fts_threads double-insert, the sweep cursor cap, and the textutil bare-'<'
and double-decode fixes) — verified and recorded in `completed.md`.
Item 56 (report-builder wire-or-remove) stays parked below pending the
owner's decision, as do 150/151 (dead-feature calls).

Batch 26 = items 67-76 (the translation-cache retention row, the audit
trail surfaced with its non-JSON-payload reader fixed, the toast lifetime
that burns only on rendered passes, the armed confirm-dialog system, the
SPP_SMOKE that renders the real shell per page, the palette dry-Enter
focus grab, the transform that keeps in-flight typing, the role-map
commit on blur, the copilot submit on its own field's focus loss, and
the Library invalidation that keeps the open edit form) — verified and
recorded in `completed.md`.
Item 56 (report-builder wire-or-remove) stays parked below pending
the owner's wire-or-remove decision, as do 150/151 (dead-feature calls).

Batch 27 = items 77-86 (the sync invalidation that keeps unsaved
Settings edits, the Inbox pagination past the 50-row cap, the row-click
that keeps the typed reply draft, the approvals attributed to the real
acting user, the known-issue links that reject dangling ids, the
Customers fetch past page 1, the LIKE escaping shared by its three
call sites, the jobs::fail calls carrying the real job id, the rating
fetch errors surfaced with re-enqueue, and the copilot panic containment
that no longer re-persists the user message) — verified and recorded in
`completed.md`.

Batch 28 = items 87-96 (the ratings upsert that never re-links its
conversation/customer on conflict, the cancelled initial sync recorded
LIVE with its never-re-run threads backfill, the threads pass hardcoding
failed:0 before its ok stamp, the post-initial-sync AI backfill capped
at 50 conversations, the platform-side deletions that never propagate,
the dead 404 mapping in get_rating, the dead OAuth TTL sweeper with its
never-expiring states, the webhook prune deleting unprocessed pending
events under backlog, the weekly_digest ledger row completed despite a
failed narrative, and the empty `{}` analysis accepted and cached
against its eval gate) — verified and recorded in `completed.md`.

Batch 29 = items 97-106 (the rewrite paths that bypass redaction, the
tool args as_i64 rejecting schema-valid floats, get_issue_clusters
returning 3x the documented limit, the knowledge-chunk embedding retry
with no attempts cap, the malformed LM Studio 200 landing as literal
"Unknown" content, the draft signature picking an arbitrary agent's
name, the org member_count capped by LIMIT 100, the last-activity and
resolution subqueries missing deleted_at, the org-name linking that
ignores NOCASE, and the CSV export without a formula-injection guard) —
verified and recorded in `completed.md`.

Batch 30 = items 107-116 (the redact_string byte-slicing panic, the digest
doc counts' space-format vs ISO-T lexical comparison, the new-conv windows
keyed on the local insert stamp with the false deleted-filter claim, the
%W year-boundary week split, the radar alert claims undercounted by silent
LIMIT caps, the concentration denominators counting NULL-key conversations,
open_closed_distribution dropping pending, the email_domain filter ignoring
its operator and matching unanchored, campaign_reconcile referencing the
nonexistent updated_at column without the self-heal, and the two bare
ORDER BY remote_created_at thread lanes) — verified and recorded in
`completed.md`.

### MEDIUM — wrong behavior on realistic paths

56. [med|gap] Entire report-builder/dashboard surface computed but never rendered (32 endpoints) — reports.rs:1110+ — owner decision needed: wire or remove (parked; not part of any batch until then)

### LOW — edge cases, provable minor defects

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
