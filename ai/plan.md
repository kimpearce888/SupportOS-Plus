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
  Opened 2026-10-29 on the owner's `continue` after the RS-2 STOP; the item
  moved to `progress.md` per Rule 2.

### Stage 1 — design-lab conformance, DS-4 (touches only `mockups/design-lab/`)

> **Batch RS-1 (RS-01..RS-10) COMPLETE 2026-10-28** — commit `af5bc2a` `design-lab-v4`,
> pushed; verification record in `completed.md`. **Gate G1 passed**: owner approved the
> conformed lab as the final design (receipt: owner `continue` after the RS-1 STOP).

### Stage 2 — egui reskin R1–R4 (Rust)

> **Batch RS-2 (RS-11..RS-20) COMPLETE and closed in `completed.md` (2026-10-29)**
> — R3 finished: every page renders through the DC widget set; G2/G3 evidence
> packs recorded. **Batch RS-3 (RS-21) opened 2026-10-29** on the owner's
> `continue`; the item moved to `progress.md` per Rule 2 — its full text lives
> there while the batch is active.

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

> **Batch RS-5 (RS-4-11) COMPLETE 2026-10-30** — the clock seam landed, the
> two drift budgets are gone, both suites lock byte-exact with zero
> tolerance; the verification record lives in `completed.md`. `plan.md`
> holds no further items for the reskin program (RS-01..RS-21 + RS-4-11 all
> consumed); the G4 verdict (blind test / SUS / accepted / waive) remains
> with the owner — GATE G4 is re-presented with the STOP.

> **GATE G4 CLOSED 2026-10-30 — owner verdict: accepted.** The owner
> administered the blind test + SUS via chat fallback (`blind test + SUS /
> accepted`) and the delivered answer lines were transcribed verbatim into the
> record tables. Blind test 5/10 with 2 false-AI — the pre-registered bar
> (≥ 9/10, 0 false-AI) NOT met, recorded as a disclosed, owner-accepted
> limitation; SUS 85.0 (n=1), bar ≥ 68 met. The reskin program is fully
> consumed: RS-01..RS-21, the RS-4 rejection wave (RS-4-1..10), RS-4-11 and
> the G4 closure. Nothing is open.

> **Batch RS-6 — release v1.9.1 (the accepted state) — opened 2026-10-30** on
> the owner's `release the new version with all the fix applied`. Fact: tag
> `v1.9.0` was cut AND pushed at the RS-3 STOP (`464000d`) BEFORE the RS-4
> fix wave existed — the fixes live only on main. The accepted state therefore
> ships as the **patch release v1.9.1**: workspace version bump + Cargo.lock,
> CHANGELOG `[1.9.1]` (the fix wave RS-4-1..11 in product terms — the section
> the Release workflow prints as the release notes), acceptance report synced
> to name v1.9.1, annotated tag `v1.9.1` on the release commit. The tag push
> triggers the Release workflow (build + no-sim/traceability gates + verify on
> ubuntu 22.04/24.04/debian 12 + publish with checksums). Main + tag push
> pending the credential re-supply (the sandbox rebuild lost
> `~/.git-credentials`; rs5 `0ad0bf1` and the G4 closure `157f0ef` are
> local-only too and go out in the same push).

## Items — Refactor program RF-1..RF-11 (reconstructed 2026-10-31)

Source: **owner-directed dead-code/refactor program** (explicitly separated from
the v1.7.0 defect-audit items above). History fact (recorded honestly, not
reconstructed): the program's first five batches (RF-1..RF-5) were opened,
executed and verified in the working sandbox on top of the v1.9.1 release
(`pre-refactor` anchor = `acd557a`), as local commits `b7a5fcb` (RF-3),
`4128f53` (RF-4), `9179e60` (RF-5, was HEAD) — never pushed. The sandbox was
reset between sessions (2026-10-31) and **all local state was lost**; only this
v1.9.1 tree survives (remote-confirmed: `main` = `acd557a` = tag `v1.9.1`).

This section is therefore the **reconstructed** program: same scope, same batch
order, same verification gates and same established patterns (the R100
`git mv` + re-export relocation pattern; the two unifications; kittest deferred
to CI from RF-4 onward), re-derived from the surviving codebase and the
session record. Re-derived items carry fresh file:line evidence gathered
2026-10-31 from the v1.9.1 tree. Nothing is silently skipped (rule.md Rule 6):
every lost batch is re-executed under its original number.

Standing constraints for the whole program (owner decisions, unchanged):

- OFF-LIMITS: generated code, lockfiles, migration SQL semantics, entry points
  (`main.rs`/`lib.rs` module wiring beyond the documented relocation wiring),
  env handling, DB schema.
- NO-TOUCH modules (migration module internals stay as-is; they are relocated
  byte-identically only): `intelligence.rs`, `maintenance.rs`, `ai_center.rs`,
  `db.rs`, `response_state_sql.rs`.
- Known production bugs stay LIST-ONLY (4× empty-list per-frame refetch;
  Settings LM Studio dirty-clobber) — never fixed inside this program.
- Kittest 24 suites (175 tests / 171 goldens) run in CI / on the release
  machine, not in the sandbox (dev-dep chain ~600 MB; sandbox RAM 3 GB) —
  standing gap carried from the original program, disclosed at every STOP.
- Pushes to remote are owner-authorized (2026-10-31: "push the updates to
  git", credentials re-supplied and stored).

### RF-1 — recovery baseline (opened 2026-10-31)

1. **RF-1-1 (env)** — re-provision the pinned toolchain (rust-toolchain.toml
   → 1.99.1, profile default) and record the env contract: `PATH` +
   `~/.cargo/bin`, `CARGO_INCREMENTAL=0`, incremental dir cleaned at batch
   start.
2. **RF-1-2 (fmt gate)** — `cargo fmt --all --check` clean on the recovered
   tree.
3. **RF-1-3 (check gate)** — `cargo check --workspace --all-targets` clean
   (cold, 2m59s recorded).
4. **RF-1-4 (clippy gate)** — `cargo clippy --workspace --all-targets --
   -D warnings` = 0 warnings.
5. **RF-1-5 (python gates)** — `gate_traceability` OK (23 routes / 20 DC
   tags / 27 ids) and `gate_no_simulation` clean.
6. **RF-1-6 (core baseline)** — core lib 495 passed / 4 ignored / 0 failed
   (`--test-threads=1` for the 3 GB sandbox; matches the v1.9.1 verified
   record 495/495 (+1), 4 ignored).
7. **RF-1-7 (app-shell baseline)** — app-shell lib 237 passed / 0 failed
   (matches v1.9.1).
8. **RF-1-8 (aux baselines)** — xtask 11/0; catalog lib 0/0; doc-tests 0/0;
   `xtask verify-icons` clean.
9. **RF-1-9 (kittest disposition)** — 24 suites / 175 tests / 171 goldens
   re-confirmed as CI-lane-only for this sandbox (RAM/disk record); gap
   disclosed at every STOP and at FINISH.
10. **RF-1-10 (records)** — plan RF section (this text) + decisions.md D7
    (recovery) + completed.md recovery record; baseline numbers locked as
    the pre-refactor gate table.

### RF-2 — core dead code, wave 1 (re-derived from the v1.9.1 tree)

Evidence method: symbol-level reference census (each candidate's definition
grepped across all four crates + tests before deletion; deletion requires
zero non-test references). Ten items:

1. **RF-2-1** — `crates/core/src/hybrid_search.rs`: the legacy fusion stack
   survived batch 34's removal only where still `pub`; census and remove any
   remaining unreferenced surface (module or items).
2. **RF-2-2** — `crates/core/src/bench.rs` + `it_*` cfg(test) modules: keep
   (test-only by declaration); census ONLY the non-test modules for dead
   `pub(crate)` items invisible to the compiler (dead_code does not fire on
   `pub`).
3. **RF-2-3..RF-2-10** — the census wave: walk core's 108 modules by size
   class (workers/ai_pipeline/helpscout_real/outreach/segment/sync_engine/
   quality/reports/saved_views/automation …), collect unreferenced pub fns /
   consts / structs, delete with per-item evidence. Each item = one module
   group cleaned + compiled + tested. The census list is recorded in
   progress.md when the batch opens (rule.md Rule 2).

### RF-3 — catalog + app-shell dead code (re-derived)

1. **RF-3-1..RF-3-4** — catalog crate (9 modules): unreferenced pub surface.
2. **RF-3-5..RF-3-9** — app-shell: unreferenced pub items outside the pages/
   hot path (design.rs DC-tagged items are gate-locked — traceability gate
   must stay 20/20; any dead non-gated surface goes).
3. **RF-3-10** — cross-crate census re-run: the deletion wave's net effect
   recorded (lines removed per crate) in completed.md.

### RF-4 — unifications (re-derived; the two named consolidations + helpers)

1. **RF-4-1** — `escape_like` 5→1: definitions at segment.rs:9,
   saved_views.rs:2297 (pub), ai_attributes.rs:429, support_graph.rs:1502,
   search.rs:348 (pub) — one canonical `pub fn escape_like` stays in
   `search.rs` (the only externally-reachable consumer surface), the other
   four become call sites; callers: segment 22×, saved_views 11×,
   support_graph 2×, ai_attributes 2×, mirror_readouts/inbox/customers 1×.
2. **RF-4-2** — `add_column_if_missing` 9→1: definitions at mirror_tables,
   intelligence_features, search, quality, customer_events, connectors (+3
   more to census at open) — one canonical `pub(crate)` helper in
   `db.rs`-adjacent schema surface, eight modules re-pointed.
3. **RF-4-3..RF-4-10** — the duplicate-helper census (same shape as RF-2:
   identical-name or identical-body helpers across ≥2 core modules; each
   consolidated item = one helper family). Census recorded at batch open.

### RF-5 — migration module relocation (R100 pattern, re-derived)

Target: `migrations.rs` (currently the MIGRATIONS SQL array only, 205 lines)
becomes the migration home; the migration-bearing modules relocate under
`crates/core/src/migrations/` by byte-identical `git mv`, with a `pub use`
re-export block in lib.rs so every `crate::X::apply_mNNN` path keeps
resolving (bootstrap.rs BootStep table untouched, external
`spp_core::bootstrap::apply_all` callers untouched: xtask + kittest tests).
NO-TOUCH internals: intelligence.rs, maintenance.rs, ai_center.rs, db.rs,
response_state_sql.rs relocate as-is if they appear, without edits.

1. **RF-5-1..RF-5-8** — relocation waves by module group (apply_m003..m045
   owners: activity, ticket_states, notifications, side_threads, automation,
   embeddings, ai_analysis, ai_features, intelligence_features, reports,
   outreach, data_tools, inbox, sync_schema, conversation_ops,
   ai_attributes, customer_events, connectors, mirror_tables, mirror_parity,
   helpscout_real, db_breadth … census at open; each item = one group).
2. **RF-5-9** — re-export wiring + lib.rs `pub mod migrations;` block
   finalized; zero call-site edits verified by grep (`crate::X::apply_` and
   `spp_core::bootstrap::apply_all` counts unchanged).
3. **RF-5-10** — it_sync regression check: the 4 tests that needed
   `install_master_key()` in the original run re-verified; gates green.

### RF-6 — god-file split, part 1 (the batch this section re-opens)

Ten items selected at open from the size census (2026-10-31: workers.rs
5276; ai_pipeline.rs 3913; helpscout_real.rs 3696; outreach.rs 3190;
segment.rs 3171; sync_engine.rs 3115; quality.rs 3055; design.rs 2814;
reports.rs 2640; saved_views.rs 2618 …). Pattern: byte-identical `git mv` of
cohesive regions into sibling modules + `pub use` re-export at the old home
(zero call-site churn). Selection recorded in progress.md per Rule 2.

### RF-7 — god-file split, part 2 + pages/queries.rs

Second wave of the census list + the app-shell relocation: page-model query
surfaces move to `pages/queries.rs` (the invalidation-map port named in
pages/mod.rs doc comments).

### RF-8 — renames (OPENED 2026-11-01 on the owner's `continue`)

The rename wave recorded by the original program (file- and item-level
naming debts surfaced by RF-2..RF-7 execution; census at open). The ten
items selected by the census live in `progress.md` per Rule 2 while the
batch is active.

### RF-9 — docs sync (OPENED 2026-11-01 on the owner's `continue`)

README / USER_MANUAL / .gitignore / ai/* module maps synced to the post-split
tree; zero stale paths (grep-verified). The ten items selected by the docs
census at open live in `progress.md` per Rule 2 while the batch is active.

### RF-10 — test paths follow the new structure (OPENED 2026-11-01 on the owner's `continue`)

Test module paths/imports follow the RF-5..RF-7 layout (import-path-only
edits; no semantic test change). The ten items selected by the test-surface
census at open live in `progress.md` per Rule 2 while the batch is active.

### RF-11 — comment deletion (OPENED 2026-11-01 on the owner's `continue`, LAST per the original program order)

All comments deleted in one final wave (module docs, item docs, inline);
rustfmt + full gates after; the program's known-bugs list and rollback
instructions ship at FINISH. The ten deletion waves selected by the comment
census at open live in `progress.md` per Rule 2 while the batch is active.

### FINISH — EXECUTED 2026-11-01 (record in completed.md)

Final record: rollback = `git reset --hard pre-refactor` (tag created at
FINISH on acd557a = v1.9.1); known-bugs list carried verbatim (4× empty-list
per-frame refetch; Settings LM Studio dirty-clobber) with current-code
file:line evidence; kittest CI lane disposition (closed at CI — all three
check runs success on 672fef7). The program RF-1..RF-11 is complete:
plan.md has no open items.

## Items — post-FINISH owner-directed work order KB-1 (opened 2026-11-01)

Source: the owner's direction on the FINISH report (2026-11-01): "Fix this"
(the known-bugs list — the empty-list per-frame refetch and the Settings LM
Studio dirty-clobber) and "Resolve this" (the kittest CI-lane disposition —
run the suites in-sandbox). The FINISH program (RF-1..RF-11) is closed; this
is a new, owner-directed batch outside it. The LIST-ONLY protection on these
two bugs was a program scope decision, now superseded by the owner's explicit
instruction to fix them. Batch size: 6 items — the direction defines the
scope (rule.md's ten-at-a-time is the ceiling, not a pad; nothing skipped,
nothing invented).

### KB-1 — known-bugs fix wave + kittest in-sandbox resolution (COMPLETED 2026-11-01, record in completed.md)

1. **KB-1-1 (custom_objects latches)** — pages/custom_objects.rs: the
   batch-34 `loaded`-latch pattern (`reload` sets the latch, `invalidate`
   clears it, `ensure_loaded` fires once) applied to both gates:
   `types_loaded` (:182) and selection-scoped `objects_loaded` (:184 —
   cleared on `select_type`, `delete_type`'s selection-clear, and
   `reload_types`' selection-vanished branch so a selection change still
   refetches). Page-level regression test mirroring batch 34's
   `ensure_loaded_latches_and_invalidate_refetches`.
2. **KB-1-2 (inbox latches)** — pages/inbox.rs: `refs_loaded` (:847 +
   the filters-prerequisite gate inside `reload_list` at :402) and
   `list_loaded` (:850, set on both the normal and the page-clamp
   recursive path). `invalidate` clears both. The `detail` gate (:853,
   transient `is_none` shape) is not a bug and stays. Regression test.
3. **KB-1-3 (knowledge latch)** — pages/knowledge.rs: one `loaded` latch
   over `reload_all`, gating both `documents_ui` (:687) and `sources_ui`
   (:764); the empty-state rendering after the gate stays. Regression
   test.
4. **KB-1-4 (Settings LM Studio dirty guard)** — pages/settings.rs:
   replace the `lmstudio_base_url.is_empty()` gate (:538) with the
   General section's `general_persisted` dirty-guard shape (:507-511,
   the in-file counterexample): a `lmstudio_persisted` triple snapshot;
   a reload assigns to the form only when the form matches the snapshot;
   `save_lmstudio` re-bases the snapshot on the saved (trimmed) values.
   Two regression tests mirroring the General pair
   (`invalidation_refreshes_a_clean_form_from_the_db`,
   `saved_edits_clear_the_dirty_guard`).
5. **KB-1-5 (kittest in-sandbox resolution)** — the standing RF-1-9 gap,
   resolved on the owner's instruction: install Mesa's lavapipe software
   Vulkan rasterizer (CI's own recipe, ci.yml:198-220), then run the
   full CI command in-sandbox: `cargo test --locked -p
   supportos-plus-app-shell --features kittest-ui` — 24 suites / 175
   tests / 171 goldens, diff-locked against the committed baselines
   (UPDATE_SNAPSHOTS deliberately unset, same as CI). Sandbox capacity
   re-verified at open: 7.7 GB free disk, 2.8 GB available RAM (the
   ~600 MB / 3 GB constraint that justified CI-lane-only no longer
   binds). Result recorded either way (suite failures = honest finding).
6. **KB-1-6 (closing gate)** — the full battery: fmt (apply + re-check),
   check `--workspace --all-targets`, clippy `-D warnings`,
   gate_traceability / gate_no_simulation / verify-icons / style_lint,
   core lib `--test-threads=1`, app-shell lib, xtask, doc-tests, the
   kittest run from KB-1-5; ai records synced; single commit; push; CI
   watched to completion on the push.
