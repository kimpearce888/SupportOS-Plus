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

Batch 31 = items 117-126 (the refresh_replies clock-skew self-count, the
duplicate graph edges for mutual links, the tag-filter case-folding
asymmetry, the entity decode double-decode with the missing &apos;, the
byte-unsafe slicing helpers, the numeric customer_property CAST-to-0, the
dead freshness_low_usage_days setting, the pre-release Same comparison, the
sweep windows dropping the oldest of a burst, and the parse_csv empty-header
misalignment) — verified and recorded in `completed.md`.

### MEDIUM — wrong behavior on realistic paths

56. [med|gap] Entire report-builder/dashboard surface computed but never rendered (32 endpoints) — reports.rs:1110+ — owner decision needed: wire or remove (parked; not part of any batch until then)

### LOW — edge cases, provable minor defects

Batch 32 = items 127-136 (the digest chip 16.max slice, the `#Some(42)` QA
rows, the mid-sentence backslashes in the radar hovers, the Mentions
notification toggle reading the wrong list, the incidents invalid-id drop +
stale-filter refetch, the diagnostics clear_log on the reader lane with
swallowed errors, the palette same-frame stale-Enter confirm + no
outside-click close, the concave polygon area fill, the stale
inserted_draft provenance phantom acceptances, and the mention-popup
highlight that never moves) — verified and recorded in `completed.md`.

Batch 33 = items 137-146 (the filter/assign refs that load once per
session, the log_level/display_timezone settings saved but never read,
the qdrant_enabled hint pointing at a nonexistent control with the
Integrations hint wrong alongside, the unconfirmed destructive deletes
routed through the item-70 dialog, the business-hours `days` list
validated and honored, the ops drill-down layering onto stale filters,
the `#778` number search silently dropped, the support-graph
invalidation clearing results nothing refetches, the onboarding
save_step dead with its unread setting, and the outreach recipient
snapshot cap discarding matches) — opened 2026-10-25, verified and recorded in `completed.md`.

Batch 34 = items 147-149, 152-158 (the ensure_loaded-on-empty re-query
pattern across seven pages, the dead Runner/JobRegistry/bulk_*/
EventEmitter machinery removed, the dead prompt surfaces removed, the
dead copilot/embedding helper surface removed, the unconsumed AppConfig
helpscout/ai/sync/loopback fields removed, the dead hybrid_search fusion
stack removed, the legacy FTS tables and chunk_version column retired,
the connector AI-search/auth-edit + data_tools runtime retired, the
AppDir staging cleaned before packaging, and appimagetool pinned and
checksummed in the release path) — opened 2026-10-25, executed
concurrently with batch 34's rebase onto this batch's commits, verified
and recorded in `completed.md`. Items 150/151 stay parked pending the
owner's wire-or-prune decisions.

Batch 35 = items 159-165 (the AppImage arch following the build host
instead of a hardcoded x86_64, the atomic + timeouted tool-cache
download, the deny.toml bans promise made enforceable and evaluated in
CI, the unconsumed egui/egui_extras workspace deps, the dead catalog
all-entries surfaces, verify-icons wired into packaging and CI, and
`--locked` on the CI/release cargo invocations) — opened 2026-10-24 and
executed concurrently with batch 34 by a second agent at the owner's
direction; verified and recorded in `completed.md`.

## Notes

- Severity is the audit's call; the owner may re-order freely before any batch.
- Items 56 (report-builder wire-or-remove) and 150/151 (dead-feature decisions) need an owner decision; they stay parked at their tier until then.
- Every item keeps its full evidence (excerpt + failure scenario) in the audit report and the worklog; this file is the execution queue only.
