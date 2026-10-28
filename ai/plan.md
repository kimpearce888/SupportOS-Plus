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

Batch 36 = items 56, 150, 151 — opened 2026-10-25 when the owner
resolved the final three parked decisions with explicit "wire it"
choices and the items moved to `progress.md` per Rule 2: the
report-builder surface rendered (Reports Builder tab: metric/dimension
form over the catalog, filters, comparison, saved-report CRUD) and the
dashboard surface rendered (Insights tab: health facts via
dashboard metrics, effectiveness, friction with a bounded recompute
action, graph nodes) plus the per-customer support-milestone timeline
with real writers; the intelligence SLA-breach / knowledge-gap / doc
freshness tracking wired to real writers (a workers intelligence sweep
persisting breaches from the live sla.rs alert evaluation, zero-hit
knowledge searches recorded as gaps, ingest stamps freshness and the
sweep marks stale docs); the ai_settings write API wired to an AI Center
provider panel over the pre-hub fallback layer the pipeline consumes —
verified and recorded in `completed.md`.

## Items — Design-System Reskin program (RS-01..RS-21, opened 2026-10-28)

Source: **owner-approved UX reskin** ("everything is working fine, we need to reskin
only according to the new design system"). Design source of truth: **Design System
v1.0** (commit `f6c88b3`: `mockups/design-lab/ds.css` + `ds.js` + `ds.html` +
`mismatch-register.md` — 17 canonical DS-C components, 6 DS-A archetypes, the DS.TONE
status map, and the DS-0 mismatch register: 8 Class-A + 3 Class-B findings, 2 Class-C
contractual keeps).

Scope contract (decisions.md **D6**): presentation only. No feature, data, workflow,
shortcut or page-set change. HTML/CSS/JS exists only in `mockups/design-lab/` as the
spec/preview artifact — it is never shipped; the program reskin is pure Rust/egui.
Every on-screen element must trace to the Design System or be a registered exception.

Batch mapping under `rule.md` (ten at a time):

- **Batch RS-1** = RS-01..RS-10 — Stage 1: design-lab conformance rebuild (lab files
  only, zero Rust). Ends at **Gate G1**: owner reviews the conformed lab; explicit
  approval freezes the design before any Rust change (rule.md STOP).
- **Batch RS-2** = RS-11..RS-20 — Stage 2: egui reskin R1–R3 (F0 contract, token port,
  widget sets, page waves). Gate evidence packs (G2 tokens, G3 Inbox pilot) are
  presented at the batch STOP; the owner may split this batch at either gate on request.
- **Batch RS-3** = RS-21 — Stage 2 close: R4 final acceptance + release (Gate G4).

### Stage 1 — design-lab conformance, DS-4 (touches only `mockups/design-lab/`)

**RS-01 — DS archetype render engine (foundation).** In `ds.js`, add the six archetype
page renderers (DS-A-01 list+detail workspace, DS-A-02 dashboard, DS-A-03 detail record,
DS-A-04 settings, DS-A-05 inbox workspace, DS-A-06 console) plus a page registry, so a
page = a PAGE_DATA object + one archetype call. Evidence: 17 bespoke per-page renderers
in `pages-reskin.js` (the a8492d0 build) are the composition drift the register logs
(mismatch-register.md Class A). Verify: `node --check`; every registry page resolves to
exactly one archetype; no page markup rendered outside `DS.*` calls.

**RS-02 — Inbox onto DS-A-05 (lab pilot).** Rebuild inbox as data + archetype: view
rail, list rows, DetailPanel reader (DS-C-17), role-typed message cards + day dividers +
quoted-history collapse + attachment chips, composer, typed copilot cards, sync
status-line (register A-8). Verify: golden-path interaction suite unchanged (open
conversation, reply/note send, saved-reply insert, AI insert, copilot actions); visual
equals the approved proposal; both themes.

**RS-03 — Dashboard onto DS-A-02.** Stat/metric tiles, chart, breakdowns through DS
renderers; counts as PageHeader counter pill (A-6); sync-note → status-line (A-8).
Verify: renderer-only composition; both themes; Reskin-notes drawer intact.

**RS-04 — Customers + Organizations onto DS-A-01/DS-A-03.** Customers abandons the
full-page profile replace (`pages-reskin.js:207-227`) for row → DetailPanel (A-1); one
toolbar + FilterBar for both (A-2); one detail record header (A-4); counts as header
pill (A-6). Verify: identical open/filter/act flow on both pages; blind walkthrough.

**RS-05 — Knowledge + Help Center.** One FilterBar (A-2: removes the stacked duplicate
search rows, `pages-reskin.js:265-266`); row-action rule (A-5: the reviewed/verify/
delete triple → ≤2 inline actions, destructive = danger + ConfirmDialog); DS-C-14 empty
states with why+next-step (A-7); article status via DS.TONE (A-3 site). Verify:
filter+apply parity; register rows A-5/A-7 closed for these pages.

**RS-06 — Incidents, Issue Radar, Reports (+ Report Builder).** Onto list/dashboard
archetypes; severity bands and tiles resolve through DS.TONE; approval/when-chip
composites per B-2. Verify: renderer-only; incident/report detail flows open DetailPanel.

**RS-07 — Operations + AI Center onto DS-A-06 (console).** engine-hero → DS-C-24 (B-2);
approval-line/when-chip → chip + status-line composition (B-2); gap-candidate and
endpoint blocks fold into Card + kv rows (B-2); receipt-cited quirks preserved (C-1:
endpoint name not index, approvals human-first with collapsible raw JSON). Verify:
console pages composed from DS parts only.

**RS-08 — Settings, Connectors, Notifications onto DS-A-04 (settings).** Tabs,
FormFields, isolated danger zones; notifications dot-as-state + relative time with
raw-UTC title kept as receipt (C-1); mark-all-read. Verify: settings tab parity;
danger-zone isolation visual check; both themes.

**RS-09 — All remaining NAV_GROUPS pages.** LiveChats, Search, SupportGraph, SyncHealth,
Onboarding, Shell, Outreach, Automation, CustomObjects, Diagnostics — final sweep onto
archetypes (page set = the 23 PageKeys, `crates/app-shell/src/pages/mod.rs:299-326`,
grouped per `app.rs:926-971`). Verify: every lab nav entry renders via the registry;
no stub left.

**RS-10 — Conformance burn-down + Gate G1.** Kill reskin.css one-off classes (the
235-class layer → DS layer only) and ~30 inline styles; off-scale 5/7/9/11px gaps →
4pt scale (B-3); five info-text patterns → five fixed roles (B-1). Tone audit: zero
status colors outside DS.TONE, both themes (A-3). Mismatch register: all A/B rows
closed; `ds.html` register section updated. Grep gate: pages ≥90% DS-composed, zero
one-off CSS. Goldens: headless screenshots of all 23 pages × both themes ×
1440/1024/800 — the reference set for R3. Blind walkthrough of same-type pages.
Release design-lab-v4 (commit + zip asset). **GATE G1 — owner reviews the conformed
lab and approves it as the final design; no Rust work before this (rule.md STOP).**

### Stage 2 — egui reskin R1–R4 (Rust; opens only after Gate G1 approval)

**RS-11 — F0: Design Contract v1.0 freeze.** Extract `docs/audit/design-contract-v1.md`
from the conformed lab: token sheet (--ds-* values, both themes), 17 DS-C component
sheets (anatomy/variants/do-don't + egui mapping note each), 6 archetype sheets,
DS.TONE table, exception register, change-control rule (DC bump before code). Verify:
DC committed; every register A/B row closed in DC; RS-10 goldens referenced as the
visual baseline.

**RS-12 — R1: token port + style lint.** Rebase on latest origin/main; re-scan stray
hardcodes (raw `Color32::from_rgb` sites). `crates/app-shell/src/design.rs` LIGHT/DARK
token tables → DC v1.0 tokens 1:1; spacing/type/radius scales; DS.TONE → one match arm.
Style lint (CI test): forbid raw Color32 outside design.rs and off-scale spacing.
Verify: cargo fmt/clippy/test green; SPP_SMOKE per-page paint gate green; page
structure unchanged (colors/typography now DS — this IS the G2 token sign-off);
fresh goldens lock the token state against drift; G2 evidence pack (token diff table)
presented at batch STOP.

**RS-13 — R2a: structure widget set.** Add egui_kittest (pinned to the egui 0.37 line)
as dev-dependency + harness. Widgets 1:1 with `DC: DS-C-xx` doc-comments: PageHeader
(counter pill), DataTable (virtualized rows), StatTile/MetricTile, FilterBar, Tabs
(underline), Badge/Chip/StatusDot via DS.TONE, button set. Verify: kittest snapshots
per widget/state, light+dark; fmt/clippy/tests green.

**RS-14 — R2b: feedback widget set.** EmptyState (+slim variant), Callout set (five
fixed roles, B-1), Toast, armed ConfirmDialog, FormField/inputs, DetailPanel,
Avatar/Assignee. Verify: same harness + snapshots + DC mapping.

**RS-15 — R2c: inbox identity widget set.** Role-typed Message Card
(agent/customer/note/AI), day divider, quoted-history collapse, attachment chips,
Composer (Reply|Note, saved-replies, AI insert, draft-saved flag), AI badge set, typed
copilot cards. Verify: harness snapshots; 5-second AI-identification pre-check on the
isolated card.

**RS-16 — R3 pilot: Inbox rebuild.** Rebuild the Inbox UI strictly from the widget set
(list, reader, composer, copilot panel; `crates/app-shell/src/pages/inbox.rs`); zero
bespoke styling. Functional regression checklist: every existing interaction identical
(pagination past the 50-row cap, draft retention on row click, sync trigger, triage
loads). No new per-frame DB re-queries introduced by the rebuild (existing data-path
issues stay out of scope — data track, decisions.md D6). Verify: checklist recorded;
kittest goldens at 1440/1024/800 vs the RS-10 lab goldens; G3 evidence pack
(before/after + checklist) presented at batch STOP.

**RS-17 — R3 wave A: Dashboard, Customers, Organizations, LiveChats.**

**RS-18 — R3 wave B: Knowledge, Incidents, IssueRadar, Reports (+ Report Builder).**

**RS-19 — R3 wave C: Operations, AiCenter, Automation, CustomObjects, Outreach.**

**RS-20 — R3 wave D: Settings, Notifications, Search, SupportGraph, Connectors,
Diagnostics, SyncHealth, Onboarding, Shell.**

Each wave: rebuild from the widget set only; per-page functional regression checklist;
kittest goldens vs lab goldens; any egui-cannot-reproduce gap becomes a registered
exception in the DC — never silently approximated. Verify per wave: checklist +
goldens + fmt/clippy/tests + SPP_SMOKE green.

**RS-21 — R4: final acceptance + release.** Traceability matrix (screen region → DS-C
id → DC row); blind test (5-second AI reply identification); SUS questionnaire, target
≥68; CI snapshot locks every rebuilt page against drift; v1.9.0 tag + release notes;
acceptance records folded into the printable report series (the deferred PDF task).
**GATE G4 — owner final acceptance.**

## Notes

- Severity is the audit's call; the owner may re-order freely before any batch.
- Items 56, 150 and 151 left the parked state on 2026-10-25 (owner decision:
  wire all three) and opened batch 36 in `progress.md`; `plan.md` holds no
  further items — the v1.7.0 clean-audit queue is fully consumed by that batch.
- Every item keeps its full evidence (excerpt + failure scenario) in the audit report and the worklog; this file is the execution queue only.
- 2026-10-28 — The v1.7.0 clean-audit queue (items 1–165, incl. 56/150/151) is fully
  consumed; the queue reopens with the **Design-System Reskin program RS-01..RS-21**
  (owner-approved, decisions.md D6). Numbering switches to the RS- prefix to keep the
  new program separate from the historical audit items. Batches: RS-1 = RS-01..10,
  RS-2 = RS-11..20, RS-3 = RS-21. Gate G1 (conformed-lab approval) blocks all Rust work.
