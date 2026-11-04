# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items

**Batch RS-4 (items RS-4-1..RS-4-10)** — active, moved from plan.md per Rule 2
(2026-10-30). Source: the owner's G4 rejection; root-cause reproduction and
disposition in the G4-verdict addendum (completed.md, 2026-10-30).
**Held 2026-11-04**: the owner directed Batch REL-2 (Build & Release, v1.9.3
from the final source) out-of-band; RS-4 items remain recorded here, not
started and not dropped (Rule 6), resuming on the owner's `continue` after
the release closes. Precedent: REL-1 ran as an owner-directed work order.

## Batch RS-4 — G4-rejection fix wave (owner-directed, 2026-10-30)


> Source: the owner's G4 rejection ("you need to fix the reskin…") + the
> root-cause reproduction recorded in the G4-verdict addendum (completed.md,
> 2026-10-30). Every item carries the evidence that opened it.

1. **RS-4-1 (core/richtext)** — add `pub fn strip_html(html:&str)->String`
   (block tags → spaces, entities decoded) and
   `pub fn break_long_tokens(s:&str,max:usize)->String` (U+200B into tokens
   longer than `max`, so egui can wrap unbreakable URLs); unit tests for
   both (empty, no-tags, nested tags, attribute-bearing tags, multi-break).
2. **RS-4-2 (core/ai_local)** — the copilot analyzer must consume
   `strip_html`'d thread text (summary, questions, sentiment/intent inputs);
   unit test: a `<br><br>`-laden customer body yields clean summary lines
   with zero markup tokens (the live-data leak the owner screenshot shows).
3. **RS-4-3 (app-shell/inbox)** — `render_rich_spans`: span text (incl. link
   labels) passes through `break_long_tokens`, so no thread-body token can
   exceed the reader viewport (root cause of the cards painting over the
   copilot panel; evidence: probe zz_probe_inbox_1625_copilot_open).
4. **RS-4-4 (app-shell/inbox)** — `copilot_pane_ui`: summary/question lines
   render wrapped + token-broken inside `card_titled`; the pane's ScrollArea
   clamps to the panel width so card content cannot overflow the card or the
   panel (owner screenshot: Summary text spilling past the card bounds).
5. **RS-4-5 (app-shell/inbox)** — composer pane: no fixed-width children may
   exceed the pane; Cc/Bcc inputs span the composer width (owner screenshot
   + r3 goldens show them ~400 px against a ~870 px composer).
6. **RS-4-6 (app-shell/inbox)** — move "Ctrl+Enter sends the composer" out of
   the views rail (inbox.rs:1252) into the composer footer by the send
   button where the shortcut acts; the rail keeps the counts only.
7. **RS-4-7 (app-shell/inbox)** — conversation list rows: a blank customer
   name falls back to the email local-part (Judy-row evidence: headerless
   row in the live list); reader head keeps its existing fallback.
8. **RS-4-8 (app-shell/tests)** — promote the temporary probe to the permanent
   `kittest_inbox_copilot.rs` suite: copilot-open 1625×828 light+dark +
   copilot-closed 1625 light, diff-locked (the state class no golden ever
   covered); delete `zz_g4_reskin_probe.rs` and its snapshots; regenerate the
   r3_inbox_page_* goldens the RS-4-5/6 changes legitimately alter, plus any
   widget goldens the fixes touch; register the suite in the traceability
   matrix and CI counts (24 suites / 175 tests / 171 goldens target).
9. **RS-4-9 (verify)** — full local verification: fmt, clippy -D warnings,
   core suite, app-shell lib suite, all kittest suites over lavapipe+Xvfb
   (a11y ON), SPP_SMOKE, gate_traceability, gate_no_simulation; the
   copilot-open probe must show zero overlap (reader cards inside their
   viewport, copilot panel clean) before anything is recorded.
10. **RS-4-10 (docs)** — sync every living count/state touched by the new
    suite and goldens (traceability matrix, CHANGELOG drift bullet, ci.yml
    comment, acceptance report regeneration), rebuild the G4 kit if its
    stimulus-source goldens changed, record the batch in completed.md, and
    re-present GATE G4 with the STOP.

## Batch RS-6 — release v1.9.1 (the accepted state) — opened 2026-10-30

Opened on the owner's instruction `release the new version with all the fix
applied`. Fact established: `v1.9.0` was tagged AND pushed (`464000d`, remote
confirmed via ls-remote) at the RS-3 STOP — before the RS-4 fix wave — so the
accepted state ships as patch **v1.9.1**. Items:

1. **RS-6-1 (workspace)** — root Cargo.toml version 1.9.0 → 1.9.1 (members
   inherit `version.workspace = true`); Cargo.lock synced — the Release
   workflow runs `--locked` everywhere, so the lock must be committed.
2. **RS-6-2 (docs)** — CHANGELOG `## [1.9.1] — 2026-10-30`: the fix wave
   (copilot suggested-reply EXTEND-mode wrap root cause, rich-span
   token-breaks, strip_html analyzer feed, Cc/Bcc composer width, Ctrl+Enter
   hint move, blank-name fallback, the new `kittest_inbox_copilot` lock,
   RS-4-11 clock seam + budget removal) in product terms; the Release
   workflow composes the GitHub release notes from this section.
3. **RS-6-3 (docs)** — acceptance report names v1.9.1 as the release carrying
   the accepted state (cover subtitle + status, §7 narrative, §7.1 prints the
   [1.9.1] section verbatim, §8 record); regenerated + probed.
4. **RS-6-4 (gates/records)** — gate_no_simulation + gate_traceability;
   completed.md record; progress → None.
5. **RS-6-5 (tag/push)** — annotated tag `v1.9.1` (tagger identity matching
   v1.9.0: kimpearce888) on the release commit; push main + tag when the
   credentials return — the tag push triggers build → verify → publish.

None — Batch RS-6 completed and moved to `completed.md` (2026-10-30): v1.9.1
staged locally (workspace + lock + CHANGELOG + report + tag), push pending the
credential re-supply.

## Batch RF-1 — recovery baseline (reconstructed program) — opened 2026-10-31

Opened on the owner's `continue and push the updates to git` after the sandbox
loss (decisions.md D7). Items RF-1-1..RF-1-10 moved from plan.md per Rule 2.
All ten are the recovery verification itself (toolchain + the full gate table
on the recovered v1.9.1 tree), executed before this record was written and
re-recorded here for the audit trail:

1. RF-1-1 toolchain 1.99.1 (profile default, rust-analyzer incl.) re-provisioned; env contract set (PATH, CARGO_INCREMENTAL=0).
2. RF-1-2 `cargo fmt --all --check` clean.
3. RF-1-3 `cargo check --workspace --all-targets` clean (cold 2m59s).
4. RF-1-4 clippy `--workspace --all-targets -- -D warnings` = 0.
5. RF-1-5 gate_traceability OK (23 routes / 20 DC tags / 27 ids), gate_no_simulation clean.
6. RF-1-6 core lib 495 passed / 4 ignored / 0 failed (`--test-threads=1`, 455s — matches v1.9.1's 495/495 +4 ignored).
7. RF-1-7 app-shell lib 237 passed / 0 failed (matches v1.9.1).
8. RF-1-8 xtask 11/0; catalog lib 0/0; doc-tests 0/0; verify-icons clean.
9. RF-1-9 kittest 24 suites / 175 tests / 171 goldens re-confirmed CI-lane-only (sandbox 3 GB RAM / 7.8 GB disk; ~600 MB dev-dep chain) — standing disclosed gap.
10. RF-1-10 this record + plan.md RF section (reconstructed) + decisions.md D7 + completed.md recovery record.

None — Batch RF-1 completed and moved to `completed.md` (2026-10-31). Next
per plan.md (opens on the owner's `continue`): Batch RF-2.

## Batch RF-2 — core dead code, wave 1 — opened 2026-10-31

Opened per plan.md (RF program, reconstructed). Evidence: symbol census
2026-10-31 over all four crates (`/home/z/census/deadcensus.py`, identifier
frequency across every .rs file incl. string dispatch + doc mentions + tests;
163 zero-reference pub items in core, 0 in catalog). Spot-verified
definition-only: get_radar_snapshot, start_campaign, create_side_thread,
evaluate_rules. NO-TOUCH excluded from selection: response_state_sql.rs (3),
ai_center.rs (1). Ten items moved from plan.md per Rule 2 (one module group
each; deletion includes the now-orphaned private helpers clippy exposes):

1. RF-2-1 intelligence_features.rs [21 zero-ref pub fns] — dead radar/incident/issue API surface.
2. RF-2-2 outreach.rs [16] — dead segment/campaign API surface (live send path lives in workers).
3. RF-2-3 side_threads.rs [14] — dead side-thread API surface.
4. RF-2-4 automation.rs [12] — dead rule/approval API surface.
5. RF-2-5 conversation_ops.rs [9] — dead js/zod/eval helper surface.
6. RF-2-6 ticket_states.rs [8].
7. RF-2-7 saved_views.rs [7].
8. RF-2-8 embeddings.rs [5].
9. RF-2-9 search.rs [5] — escape_like stays (RF-4 canonical), delete only the flagged dead items.
10. RF-2-10 jobs.rs [5].

None — Batch RF-2 completed and moved to `completed.md` (2026-10-31): 3,761
lines of dead code removed from core (net -3,736 across 11 files, incl. the
orphaned private helpers clippy exposed). All ten items verified. Next per
plan.md (opens on the owner's `continue`): Batch RF-3.

## Batch RF-3 — core dead code wave 2 (second-order orphans + census tail) + app-shell — opened 2026-10-31

Evidence: census re-run on the post-RF-2 tree (`census_core2.txt`): RF-2's
root deletions cascaded — 70 zero-reference pub items remain in core (incl.
new orphans like automation::trigger_matches, whose only caller was the
deleted evaluate_rules), app-shell holds 1 (chart.rs bar_chart). NO-TOUCH
excluded: response_state_sql.rs [3], ai_center.rs [1], db.rs [1]. Ten items
moved from plan.md per Rule 2:

1. RF-3-1 custom_objects.rs [5]
2. RF-3-2 quality.rs [4]
3. RF-3-3 support_graph.rs [4]
4. RF-3-4 eventing.rs [4] — notify_* wrappers (zero subscribers verified)
5. RF-3-5 knowledge_store.rs [4]
6. RF-3-6 ai_attributes [3] + interaction_engine [3] + people_store [3]
7. RF-3-7 webhook [3] + segment [3] + ticket_states [3, new orphans]
8. RF-3-8 saved_views [2] + notification_prefs [2] + encrypted_sync [2]
9. RF-3-9 the singles wave (every [1] file, excl. NO-TOUCH: ai_pipeline,
   audit, automation, business_hours, coaching, config, copilot,
   customer_events, customers, embeddings, incident_workspace,
   intelligence_features, interaction_current, issue_impact, notifications,
   operations, outreach, richtext, translation, workload)
10. RF-3-10 app-shell chart.rs bar_chart [1] + catalog re-census (expected 0)
    + full re-census proving convergence to zero deletable items.

None — Batch RF-3 completed and moved to `completed.md` (2026-10-31): census
convergence reached (only the 5 NO-TOUCH items remain: response_state_sql ×3,
ai_center ×1, db ×1). Two swallowed regression-test modules (saved_views
batch31_tests, segment batch30_tests) were caught by the #[test] census and
restored byte-identical from acd557a — core test count back to the baseline
495/4ig/0. Next per plan.md (opens on the owner's `continue`): Batch RF-4.

## Batch RF-4 — unifications — opened 2026-10-31

Evidence: per-body hash + manual body diffs of the duplicate-helper census
(27 same-name families across core). Ten items per plan.md:

1. RF-4-1 `escape_like` 5→1 — DONE at open: all bodies byte-equivalent; canonical `search::escape_like` (pub); segment + support_graph now import it; saved_views re-exports it; the three external callers already used the canonical path.
2. RF-4-2 `add_column_if_missing` 9→2 — DONE at open: canonical `migrations::add_column_if_missing` (pub(crate), errors PROPAGATE — the majority semantic; quality/intelligence_features' old silent-ignore copies re-pointed, bootstrap now fails loudly on a failed ALTER instead of silently skipping a column); maintenance.rs copy retained (NO-TOUCH module, standing constraint); outreach's nested copy removed.
3. RF-4-3 `table_exists` 5→2 — canonical `sync_schema::table_exists` (already pub, sqlite_master COUNT); ai_attributes, mirror_parity re-point; db_breadth re-pointed (its silent unwrap_or(false) error path now propagates — same fail-loud rationale); search's table-or-view variant INTENTIONAL (it matches views too — different query).
4. RF-4-4 `now_iso` 7→4 — the two idioms producing byte-identical output (`format("%Y-%m-%dT%H:%M:%S%.3fZ")` ≡ `to_rfc3339_opts(Millis, true)`) collapse onto one canonical `business_hours::now_iso()`; activity, ai_pipeline, quality, backup_service, encrypted_sync re-pointed; outreach (SystemTime day-math) and sync_engine (Help Scout rejects sub-second/+00:00 — in-code reason) stay as documented semantic variants.
5. RF-4-5 `hex` 3→1 — canonical `crypto::hex` (the efficient fold); ai_pipeline + conversation_ops' naive format! copies deleted (identical output, lower perf).
6. RF-4-6 `is_closing_acknowledgment` 2→1 — canonical `quality::is_closing_acknowledgment` (OnceLock-cached regexes, the developed b-wave version); interaction_engine re-pointed.
7. RF-4-7 `is_valid_timezone` — saved_views' one-line wrapper becomes `pub use crate::business_hours::is_valid_timezone;`.
8. RF-4-8 `round2` 2→1 — canonical `interaction_current::round2`; workload re-pointed.
9. RF-4-9 INTENTIONAL VARIANTS — census-dispositioned, NOT unified (differing semantics, recorded): decode_entity (richtext: NBSP→U+00A0 + numeric/#x entities vs textutil: NBSP→space + '#39'), chunk_text (embeddings: char-window+overlap vs knowledge_store: whitespace-collapse+boundary), truncate_chars (plain take vs trim+ellipsis), is_word_char (ascii vs unicode alphanumeric), table_columns (typed pragma rows vs name list).
10. RF-4-10 — batch verification + census re-run + records.

None — Batch RF-4 completed and moved to `completed.md` (2026-10-31). One
incident recorded: the add_column_if_missing unification first propagated
ALTER failures unconditionally and broke 294 tests (m035's widen of
`incidents` legitimately targets a table created by a later boot step — the
old silent-ignore was load-bearing there); the canonical primitive now
skips missing TABLES (no-op — the creating migration ships the column) and
propagates real ALTER failures on existing tables; full suite re-green.
Next per plan.md (opens on the owner's `continue`): Batch RF-5.

## Batch RF-5 — migration module relocation (R100 pattern) — opened 2026-10-31

Census: exactly 8 modules are migration-only (every pub fn is a migration
fn): ai_analysis (49 ln), ai_features (69), data_tools (47), db_breadth
(1127), intelligence (48, NO-TOUCH module — relocates byte-identically per
the standing constraint, zero internal edits), mirror_parity (1226),
mirror_tables (319), side_threads (181). Feature modules that merely CONTAIN
migration fns (activity, outreach, reports, ...) stay — wrong home.
Ten items:

1. RF-5-1 selection census (this record).
2. RF-5-2 mkdir migrations/ + `git mv` wave 1 (ai_analysis, ai_features, data_tools, intelligence).
3. RF-5-3 `git mv` wave 2 (db_breadth, mirror_parity, mirror_tables, side_threads).
4. RF-5-4 migrations.rs gains the 8 `pub mod` declarations.
5. RF-5-5 lib.rs: the 8 `pub mod` lines replaced by one `pub use migrations::{…}` block — every `crate::X` and `spp_core::X` path keeps resolving, zero call-site edits.
6. RF-5-6 compile + clippy fix pass.
7. RF-5-7 zero-call-site-edit proof (grep counts of crate::X::apply_ / spp_core::bootstrap::apply_all before = after).
8. RF-5-8 it_sync regression: the install_master_key() tests still pass.
9. RF-5-9 full gate table.
10. RF-5-10 records + single commit.

None — Batch RF-5 completed and moved to `completed.md` (2026-10-31): the 8
migration-only modules relocated byte-identically under migrations/, re-export
wiring restored every path, zero call-site edits (154 crate::X::apply_ refs +
82 apply_all callers + 20 spp_core moved-module refs, all unchanged). Next
per plan.md (opens on the owner's `continue`): Batch RF-6 — god-file split,
part 1.

## Batch RF-6 — god-file split, part 1 — opened 2026-11-01

Opened per plan.md (RF program) on the owner's `continue`. Re-census on the
post-RF-5 tree (2026-11-01, `wc -l` + top-level/method structure scan): the
plan's evidence census was taken pre-RF-2..RF-5; on the current tree
saved_views.rs measures 1568 ln (RF-2/RF-3 removed its dead CRUD) and no
longer qualifies as a god-file — its split item dissolves with the size,
dispositioned here rather than silently dropped. The tenth slot goes to the
next qualifying file, interaction_engine.rs (2142 ln). app-shell pages
(inbox 4040, settings 3139, …) stay reserved for RF-7's app-shell wave per
plan.md. design.rs is gate-constrained: gate_traceability.py reads
`crates/app-shell/src/design.rs` by fixed path and validates every
`DC: DS-C-xx` doc-tag (recorded figure 20) — its split seam is chosen
tag-free so the DC-tag set never moves (gate table unchanged).

Pattern (per plan.md): byte-identical region extraction into sibling modules
under directory-converted parents (`git mv X.rs X/mod.rs` preserves rename
history) + `pub use` re-export at the old home; moved impl slices keep their
`impl` headers byte-identical (multi-impl blocks per type across child
modules of the defining crate); child files open with `use super::*;`
(descendants inherit the parent's items and import bindings); the only
permitted edits are the new import headers, `mod`/`pub use` wiring lines, and
`pub(super)` visibility bumps the compiler demands where the retained parent
code calls a moved private item. Zero call-site churn outside the split
module itself. lib.rs untouched. Tests stay in place (RF-10 owns test-path
work). NO-TOUCH modules unaffected (none of the ten is on the list).

Ten items moved from plan.md per Rule 2 (evidence = 2026-11-01 structure
scan; exact cut lines verified at implementation against boundary reads):

1. **RF-6-1 workers.rs (5276 ln)** — the job-execution engine moves to
   `workers/execute.rs`: impl slice ~1346–3976 (execute_job dispatcher,
   on_after_initial_sync, on_conversation_changed, run_engine, the 21
   run_*_job runners, run_attachment_downloads) + the `AiJob` enum slice
   (~3979–3984, family-internal). mod.rs keeps the struct, start/tick,
   execute_outbound, auto_sync, maintenance, sync-loop methods and all tests.
2. **RF-6-2 ai_pipeline.rs (3761 ln)** — the interaction-analysis family to
   `ai_pipeline/interaction.rs`: slice ~1929–2644 (build_interaction_strategy_block,
   extract_provided_facts, has_version_number, ProcessNewTicketResult [zero-ref
   pub struct found by the boundary scan — moved byte-identically, disclosed,
   NOT deleted in a split batch], AnalyzeInteractionOutcome, ObservationOutput,
   parse_observation, parse_recommendation, analyze_interaction). Re-export
   the pub surface at ai_pipeline.
3. **RF-6-3 helpscout_real.rs (3696 ln)** — the HTTP/auth core to
   `helpscout_real/http.rs`: impl slice ~829–1535 (docs_request_inner,
   docs_request_send, access_token, client_credentials_login_if_configured,
   request_inner). Provider struct, mappers and API methods stay.
4. **RF-6-4 sync_engine.rs (3115 ln)** — the resource-sync loops to
   `sync_engine/loops.rs`: impl slice ~1751–2896 (finish_resource, reconcile,
   sync_resource, sync_customers, sync_conversations, ingest_conversation).
   Engine struct, entries (initial/incremental/sync_docs_only), upsert/event
   fns and tests stay.
5. **RF-6-5 outreach.rs (2861 ln)** — the campaign send path to
   `outreach/send.rs`: slice ~1485–2030 (pick_batch, classify_error,
   record_outcome, datetime_now, ensure_recipient_updated_at, send_batch,
   BatchCounts, count_remaining_for_send, finalize_if_drained). send_batch
   re-exported at outreach (external callers unchanged).
6. **RF-6-6 design.rs (2814 ln)** — the card/controls family to
   `design/controls.rs`: slice 802–975 (chip_impl, card, card_titled,
   section_title, hint_text, pager) — the tag-free span between the DC tag
   clusters (563 → 1000); the 20 DC doc-tags stay byte-identical in
   design.rs and the gate figure is unchanged. chip_impl gets the minimal
   pub(super) bump for the retained chip wrappers.
7. **RF-6-7 segment.rs (2697 ln)** — the evaluator tree to `segment/eval.rs`:
   impl slice ~1155–2542 (tickets_for_customer_matching + the eval_* family
   through eval_customer_event; ends at renumber_placeholders whose callers
   are all inside the slice). Engine struct, parse/describe/preview/
   evaluate_nodes/build_rows/explain_for and tests stay.
8. **RF-6-8 reports.rs (2640 ln)** — the builder-config codec to
   `reports/config.rs`: two slices ~1248–1306 (config_to_json) and
   ~1358–1767 (parse_report_config, enum_error, metric_variants,
   dimension_variants, parse_report_config_inner, parse_report_filters); the
   saved-report CRUD (list/save/delete) stays between them. Re-export the
   pub codec surface at reports.
9. **RF-6-9 quality.rs (2536 ln)** — the AI-QA layer to `quality/qa_ai.rs`:
   slice 1517–1862 (coerce_ai_json, compute_qa_ai_layer, finish_qa_ai_layer,
   round1, DerivedOutcome, derive_outcome) — contiguous, ends at the tests
   mod. pub/pub(crate) surface re-exported (derive_outcome's segment
   consumer unchanged).
10. **RF-6-10 interaction_engine.rs (2142 ln)** — the baseline+profile family
    to `interaction_engine/baseline.rs`: slice ~1132–2077 (closed_conversation_ids,
    rebuild_baseline, comparison_baseline, compute_changes, ThreadRow,
    outcome_summary, heuristic_recommendation, detect_repeat_issue,
    ensure_history_backfill, build_timeline, build_profile, build_playbook).
    Schema/current-interaction/preferences fns and tests stay.

Batch verification (Rule 5, before any item completes): fmt clean; clippy
`--workspace --all-targets -D warnings` 0; gate_traceability OK (23 routes /
20 DC tags / 27 ids — the 20 must not move); gate_no_simulation clean;
verify-icons clean; core lib 495 passed / 4 ignored / 0 failed
(`--test-threads=1`); app-shell lib 237/0; xtask 11/0; doc-tests 0/0;
kittest 24 suites = standing CI-lane gap (disclosed). Zero-call-site-churn
proof: external callers of every moved pub surface grep-verified unchanged.

None — Batch RF-6 completed and moved to `completed.md` (2026-11-01): ten
cohesive regions (9,089 lines) extracted byte-identically into ten sibling
modules under directory-converted parents, re-export wiring at every old
home, zero call-site edits outside the split modules; full gate table green
(core 495/0/4ig, app-shell 237/0, clippy 0, gates 23/20/27). Next per
plan.md (opens on the owner's `continue`): Batch RF-7 — god-file split,
part 2 + pages/queries.rs.

CI hotfix (RF-6 follow-up, 2026-11-01): the RF-6 push (a60904c) failed the
CI run (41260762329) in BOTH the workspace-test job and the kittest job on
the same test — `style_lint::no_raw_colors_and_no_off_scale_spacing` —
flagging `src/design/controls.rs:91` and `src/design/mod.rs:136/468/656`
(raw `Color32::` constructions). Root cause: the lint's token-factory
exemption was keyed to the FILENAME (`const TOKEN_FACTORY = "design.rs"`,
`rel.ends_with(...)`) — RF-6's `git mv design.rs design/mod.rs` broke the
match, so the factory's own frozen primitives (the `rgb()` const fn, the
shadow helpers, the hover-tint BLACK darkening) stopped being exempt.
Process miss disclosed: RF-6's gate table ran verify-icons/xtask but
dropped `--test style_lint` from the app-shell lane, so the breakage never
ran locally. Notably the kittest job's real payload — all 24 snapshot
suites — PASSED in CI: the standing "CI-lane gap" is closed green.
Fix: verification-infra path-follow only, same class as RF-6's
gate_traceability.py DESIGN-constant follow — `TOKEN_FACTORY_PREFIX =
"src/design/"` prefix check (separator-normalized) replaces the filename
suffix check, doc comment updated; zero Rust source bytes changed (the
flagged lines are byte-identical pre-RF-6 design.rs lines 132/464/652/886).
Re-verified: style_lint 2 passed / 0 failed; fmt clean; clippy
`--workspace --all-targets -D warnings` 0; gate_traceability OK (23/20/27);
gate_no_simulation clean; verify-icons clean; xtask tests 0/0. Lib/core
suites unaffected (no lib or core bytes changed; CI already ran app-shell
lib 237/0 + all 24 kittest suites green on a60904c). Next per plan.md
(opens on the owner's `continue`): Batch RF-7.

## Batch RF-7 — god-file split, part 2 + pages/queries.rs (opened 2026-11-01)

Selection basis: fresh line census post-RF-6 (both crates, excluding pages/
—the RF-6 census precedent—excluding test-only modules it_helpscout.rs/it_sync.rs,
excluding the NO-TOUCH set; the next tier after RF-6's ten). Pattern unchanged
from RF-6: byte-identical contiguous-region extraction into a sibling child
module under the directory-converted parent (`git mv X.rs X/mod.rs`; pages/
is already a directory — item 10 only adds a child), `use super::*` at the
child top, re-exports at the old home, `pub(super)` bumps only where the
compiler demands them on moved privates called from retained parent code.
Zero call-site edits outside the split modules. `use super::*` + re-export
keeps every `crate::X::item` and `spp_core::X::item` path resolving.

1. **RF-7-1 ai_attributes.rs (2069 ln)** — the AI extraction engine to
   `ai_attributes/extraction.rs`: lines ~535–1464 (ATTRIBUTE_EXTRACTION_PROMPT_VERSION,
   ATTRIBUTE_EXTRACTION_SYSTEM, CustomerMessage + customer_messages,
   PreparedComputation + prepare_computation, ExtractionOutput +
   finish_computation, merge_extraction, is_ai_slot, parse_confidence,
   normalize_ai_value, build_extraction_user, ExtractionRunRow,
   store_extraction_run, lookup_cached_extraction, TECHNICAL_VOCAB +
   the four marker tables, find_evidence, excerpt, is_word_byte,
   has_marker, count_markers, DeterministicSignals, derive_signals,
   deterministic_records). Parent keeps the m033 migration block + the
   snapshot store (structs/insert/save/upsert/current/history/MatchOp/
   distributions/snapshot) + the three test mods.
2. **RF-7-2 helpscout.rs (1810 ln)** — the inert provider to
   `helpscout/inert.rs`: lines ~998–1751 (EmptyHelpScoutProvider + Default,
   InertWorld, `impl HelpScoutProvider for EmptyHelpScoutProvider`,
   encode_cursor, decode_cursor, no_account, not_found_remote). Parent
   keeps the DTO structs + the HelpScoutProvider trait + the input/output
   structs + tests.
3. **RF-7-3 app.rs (1700 ln)** — the shell chrome to `app/chrome.rs`:
   lines ~874–1195 (human_outbound_kind, sync_completed_toast, NAV_GROUPS,
   nav_row, palette_trigger, theme_row). Parent keeps the ShellApp struct
   + the three impl blocks + eframe::App + seed fixture + tests.
4. **RF-7-4 people_store.rs (1685 ln)** — the support-health report family
   to `people_store/health.rs`: lines ~176–702 (EVIDENCE_LIMIT, evidence_ids,
   health_metric, health_flag, build_support_health_report). Parent keeps
   the org queries + customer summary/detail/patch/create + tests.
5. **RF-7-5 support_graph.rs (1671 ln)** — the derived-edge engine to
   `support_graph/derived.rs`: lines ~261–1013 (struct DerivedEdge,
   derived_edges_touching). Parent keeps the edges schema + node refs +
   human edges + the GRAPH_MAX_* consts + graph_stats/graph_search/
   neighbors_json/subgraph_json + tests.
6. **RF-7-6 search.rs (1641 ln)** — the per-type lanes to `search/lanes.rs`:
   lines ~341–735 (search_conversations, recent_conversations,
   status_applies, has_filters, search_customers, search_knowledge_raw,
   search_knowledge, search_known_issues, search_saved_replies,
   search_ai_analyses). Parent keeps consts + the FTS migration + fts_query
   family + escape_like (RF-4 canonical) + search/search_with_semantics/
   universal_search + the indexing/rebuild family + tests.
7. **RF-7-7 ai_tools.rs (1624 ln)** — the tool-execution engine to
   `ai_tools/execute.rs`: execute (line 331) through collect_rows (1431)
   plus the execute-only helpers clamp_limit/arg_i64/red/red_chars/
   conv_by_number if the usage census confirms them execute-only (span
   finalized at implementation). Parent keeps ensure_tool_tables +
   definitions() + tests.
8. **RF-7-8 saved_views.rs (1568 ln)** — the compile engine to
   `saved_views/engine.rs`: lines ~603–1359 (`impl ViewEngine`). Parent
   keeps the view-model types (DateRange machinery, RESPONSE_STATE_SQL,
   VIEW_CONDITION_KINDS, ViewNode, ViewDefinition/SavedInboxView/requests,
   CompiledView, ViewEngine struct) + placeholders/text/integer/real +
   ensure_inbox_views_table + tests.
9. **RF-7-9 coaching.rs (1565 ln)** — the check family to
   `coaching/checks.rs`: lines ~403–1145 (check_unanswered_questions,
   check_duplicated_questions, check_timeframe, check_acknowledgment,
   check_wording, check_detail, check_internal_leakage, check_wrong_context,
   check_preference_mismatch). Parent keeps the consts/text helpers/row
   loaders/result/summarize/evidence + CoachingChatFn + the AI layer +
   get() + tests.
10. **RF-7-10 pages/queries.rs (the app-shell relocation)** — the
    invalidation map + page-model field helpers move out of pages/mod.rs
    (618 ln) into a new `pages/queries.rs` child: invalidated_pages with
    its doc block (~391–470, the port of the web `queries::keys_for_event`
    named by the pages/mod.rs doc comments) + str_field/i64_field/
    opt_str_field (~472–491). pages/mod.rs adds `mod queries;` + re-exports;
    the PageKey enum, its labels, all() and the tests stay — verified at
    open: the moved text contains no `PageKey::X => "label"` patterns, so
    gate_traceability's routes() source (PAGEKEY = pages/mod.rs) is
    unaffected; gate re-run proves 23 routes unchanged.

Batch verification (Rule 5, before any item completes): fmt clean; clippy
`--workspace --all-targets -D warnings` 0; gate_traceability OK (23 routes /
20 DC tags / 27 ids); gate_no_simulation clean; verify-icons clean;
style_lint 2/0 (added to the table per the RF-6 hotfix lesson); core lib
495 passed / 4 ignored / 0 failed (`--test-threads=1`); app-shell lib
237/0; xtask 11/0; doc-tests 0/0; kittest 24 suites = CI lane (green in CI
run 41284592762). Zero-call-site-churn proof: external callers of every
moved pub surface grep-verified unchanged (before = after).

Batch verification results (all green): fmt clean; clippy
`--workspace --all-targets -D warnings` 0; gate_traceability OK (23 routes /
20 DC tags / 27 ids); gate_no_simulation clean; verify-icons clean;
style_lint 2 passed / 0 failed; core lib 495 passed / 0 failed / 4 ignored
(--test-threads=1, 351.72s); app-shell lib 237/0; xtask 11/0; doc-tests
0/0. Churn proof: `git status` touches ONLY the ten split modules (9
git-mv renames + 10 new child files incl. pages/queries.rs) + this record
— zero call-site edits anywhere else in the tree.

None — Batch RF-7 completed and moved to `completed.md` (2026-11-01): the
second census wave (9 core/app-shell god-files) + the pages/queries.rs
relocation, all byte-identical region extractions under the RF-6 pattern.
Next per plan.md (opens on the owner's `continue`): Batch RF-8 — renames.

CI hotfix (RF-7 follow-up, 2026-11-01): run 41290521329 failed only the
Format-check step — `ai_tools/mod.rs` ended with a stray trailing blank
line left by the post-fmt relocation of the batch29_audit98_99_tests mod
into execute.rs (the trim appended `\n` to a file already ending `}\n`;
local fmt had run BEFORE that edit, so the seam was never re-checked —
process miss: fmt belongs AFTER the last content edit, not before).
Fix: strip the trailing blank; `cargo fmt --all --check` clean. Whitespace
only — no code, no tests touched. All other CI jobs (workspace tests,
kittest snapshot locks, AppImage/.deb smoke) green on this run.

---

**Batch RF-8 (RF-8-1..RF-8-10)** — active, moved from plan.md per Rule 2
(2026-11-01, on the owner's `continue` after the RF-7 close). Source:
plan.md §RF-8 — the rename wave (file- and item-level naming debts
surfaced by RF-2..RF-7 execution; census at open).

## Batch RF-8 — renames (census at open, 2026-11-01)

Census method: every module file in both crates size- and role-reviewed;
word-boundary reference census per candidate (`rg '\bname\b'` across all
four crates + tests + the two live gates + README/USER_MANUAL — zero doc
or gate references to any renamed name, so nothing outside the crate tree
follows). Excluded from the wave, with reasons: `helpscout_real`
(established real-vs-inert pairing with `helpscout/`), `webhook.rs` vs
`webhook_handler.rs` (distinct roles: event store+HMAC vs dispatch
pipeline), `vectorstore.rs` (holds the `VectorStore` trait — name is
right), `search::search` (the documented `/api/search` port; the stutter
lint is explicitly allowed off in lib.rs), the `it_*`/`mockserver`/`bench`
test modules (RF-10 territory), migration modules (`db_breadth` et al. —
migration wiring stays), audit-artifact test names (`batch29_audit98_99_tests`,
`auditNNN_*` — the audit-trail traceability convention, not a debt),
`ProviderHandle`/`ProviderKind` (moderate value, high UI churn — dropped),
`mirror_readouts.rs` (census FINDING, not a rename: zero references to any
of its five pub fns or the module path anywhere — disclosed at the STOP
for the owner's delete-or-keep decision, consistent with the RF-6
zero-ref disclosure precedent).

1. **RF-8-1 (file, core)** — `crates/core/src/eventing.rs` →
   `event_bus.rs`. The module is the EventBus home (broadcast bus +
   notify_* helpers); the `events` (type definitions) vs `eventing` (the
   bus) minimal pair is the confusable debt. lib.rs decl + all 53 path
   refs follow (`crate::eventing::` → `crate::event_bus::`,
   `spp_core::eventing::` → `spp_core::event_bus::`) across core src,
   app-shell src, it_sync/it_helpscout and the two kittest suites.
2. **RF-8-2 (file, core)** — `crates/core/src/hybrid_search.rs` →
   `rrf.rs`. Since batch 34 the module carries ONLY `RRF_K` + its
   characterization test; the name describes the deleted fusion stack
   (surfaced by RF-2/batch-34). lib.rs decl + 3 `crate::hybrid_search::RRF_K`
   refs in search/mod.rs + two comment mentions updated to stay truthful.
3. **RF-8-3 (file, core)** — `crates/core/src/sync.rs` → `mirror.rs`.
   The Hs* mirror-upsert layer (mailbox/user/team/tag/conversation/
   customer upserts into the local mirror); debt proven by the codebase's
   own alias `use crate::sync as mirror;` (sync_engine/mod.rs:12) — the
   alias drops, the 12 `mirror::` sites resolve directly, 5 `crate::sync::`
   sites re-point (bench×3, workers/execute×2). Word-boundary discipline:
   `sync_engine`/`sync_schema` untouched.
4. **RF-8-4 (file, core)** — `crates/core/src/interaction_current.rs` →
   `interaction_signals.rs`. "Interaction current" is a word-salad name
   sitting next to `interaction_engine/`; the module holds the
   interaction-signal infrastructure (client_current_signals +
   interaction_evidence tables, the signal vocabulary, needs_de_escalation,
   MessageStats). ~35 refs across 12 files follow. Disclosed path-follows:
   migrations/db_breadth.rs:360 (import path only — zero SQL semantics),
   the bench timeit label, one baseline.rs comment; intelligence.rs:46
   comment stays byte-identical (NO-TOUCH; the comment dies with all
   comments at RF-11).
5. **RF-8-5 (file, app-shell)** — `crates/app-shell/src/mention.rs` →
   `mention_popup.rs`. The module is the composer mention-autocomplete
   popup logic (MentionEntry, caret_query, the view model); "mention"
   alone collides conceptually with the core `mentions` parser it
   consumes. inbox.rs import + lib.rs decl follow; zero kittest refs.
6. **RF-8-6 (file, app-shell)** — `crates/app-shell/src/bridge.rs` →
   `task_bridge.rs`. The module holds `TaskBridge`; module name = type
   name; "bridge" alone is vague in a shell full of crossings.
   app/mod.rs import + lib.rs decl follow.
7. **RF-8-7 (item, core)** — `red` → `redact` in ai_tools/execute.rs.
   The fn applies the redaction gate (settings `redaction_enabled` +
   `security::redact_text`); "red" reads as a color. Private fn —
   file-local rename (definition + call sites).
8. **RF-8-8 (item, core)** — `red_chars` → `redact_chars` in
   ai_tools/execute.rs. Same debt; all call sites file-local (private).
9. **RF-8-9 (item, core)** — `conv_by_number` → `conversation_by_number`
   in ai_tools/execute.rs. Abbreviation debt surfaced by the RF-7 census
   that moved it; private, file-local.
10. **RF-8-10 (item, core)** — `upsert_customer_basic` →
    `upsert_customer_without_channels` (sync.rs→mirror.rs + its one
    caller sync_engine/loops.rs:762). "Basic" never says what it does —
    the doc comment does (audit C4: upsert WITHOUT touching the
    contact-channel tables); the name now carries the contract.

Batch verification (Rule 5, before any item completes): fmt clean (run
AFTER the last content edit — RF-7 hotfix lesson); clippy
`--workspace --all-targets -D warnings` 0; gate_traceability OK (23
routes / 20 DC tags / 27 ids); gate_no_simulation clean; verify-icons
clean; style_lint 2/0 (explicit in the table — RF-6 hotfix lesson); core
lib 495 passed / 4 ignored / 0 failed (`--test-threads=1`); app-shell lib
237/0; xtask 11/0; doc-tests 0/0; kittest 24 suites = CI lane. Renames
are zero-semantic-change by construction: compiler + full suite prove
behavior unchanged; `git status` review proves only the renamed files +
ref sites + ai records changed.

Batch verification results (all green): fmt clean (run after the last
content edit — the fmt re-sort of module decls/import order the renames
require was applied and re-checked); clippy `--workspace --all-targets -D
warnings` 0; gate_traceability OK (23 routes / 20 DC tags / 27 ids);
gate_no_simulation clean; verify-icons clean; style_lint 2 passed / 0
failed; core lib 495 passed / 0 failed / 4 ignored (--test-threads=1,
350.79s); app-shell lib 237/0; xtask 11/0; doc-tests 0/0. Churn proof:
`git status` = 6 git-mv renames (rename detection intact, 0-line deltas
on moved files) + 31 ref-site files (one-line path/name swaps, incl. the
disclosed db_breadth path-follow — exactly one line, zero SQL bytes) +
the two ai records. Diff: +233/−139 across 37 files.

None — Batch RF-8 completed and moved to `completed.md` (2026-11-01): the
rename wave (6 file renames + 4 item renames, all zero-semantic-change).
Next per plan.md (opens on the owner's `continue`): Batch RF-9 — docs sync.

## Batch RF-9 — docs sync (items RF-9-1..RF-9-10)

**Batch RF-9 (items RF-9-1..RF-9-10)** — active, moved from plan.md per Rule 2
(2026-11-01, owner's `continue`). Source: the plan.md RF-9 one-liner + the docs
census at open (2026-11-01): grep of the four live surfaces (README.md,
USER_MANUAL.md, .gitignore, ai/decisions.md + ai/rule.md) for every
pre-RF-5..RF-8 module basename/path, plus verification of every countable claim
in those surfaces against source ground truth (Cargo.toml members, xtask Cmd
enum, PageKey variants, .github/workflows/ci.yml lanes, .cargo/config.toml,
core env readers, xtask hygiene tests). Historical batch ledgers (plan.md
RF-sections, progress.md, completed.md) are excluded from the sync by design —
their old-path mentions ARE the audit trail.

1. **RF-9-1 (census)** — build the old→new module map of the whole refactor
   program (RF-5 migrations relocation; RF-6 splits workers/ai_pipeline/
   helpscout_real/sync_engine/outreach/design/segment/quality/
   interaction_engine; RF-7 splits ai_attributes/helpscout/app/people_store/
   support_graph/search/ai_tools/saved_views/coaching + pages/queries.rs;
   RF-8 renames eventing→event_bus, hybrid_search→rrf, sync→mirror,
   interaction_current→interaction_signals, mention→mention_popup,
   bridge→task_bridge) and grep all four live surfaces for each old
   basename/path; record the hit ledger (2 genuine doc hits + 3 dead
   .gitignore entries; everything else clean).
2. **RF-9-2 (README fix)** — the "Verification" section synced to
   `.github/workflows/ci.yml` ground truth (the batch's genuine README drift):
   the CI bullet gains (a) the two machine gates — no-simulation
   (`scripts/gate_no_simulation.py`, ci.yml:35) + traceability
   (`scripts/gate_traceability.py`, ci.yml:41), (b) the kittest snapshot-locks
   lane (job 3, ci.yml:181 — the reskin drift gate the RS-21 program added and
   the RF-6-era kittest CI gap closure confirmed), (c) `cargo-deny`'s `bans`
   check (ci.yml:95 `check advisories licenses bans`; README said
   advisories/licenses), (d) the sandbox-feature clippy half (ci.yml:79 clippy
   AND tests the sandbox feature; README said only tests). Every other claim
   in the section re-verified line-by-line against ci.yml.
3. **RF-9-3 (README verify)** — "What's inside" proven against source: crate
   table 4/4 = Cargo.toml workspace members (app-shell, catalog, core, xtask);
   xtask subcommand list 10/10 = `crates/xtask/src/bin/xtask.rs` Cmd enum
   (dev/lint/verify-icons/package/deb + migrate/backup/restore/healthcheck/
   vector-count); the 23-pages list 23/23 = PageKey variants incl. the
   display spellings (AI Center, Live Chats, Issue Radar, Support Graph, Sync
   Health, Custom Objects). No-op expected; any drift the check surfaces is
   fixed.
4. **RF-9-4 (README verify)** — "Building from source" proven: `.cargo/
   config.toml` alias = `run --locked -p supportos-plus-xtask --` (the README
   claim verbatim); rust-toolchain.toml present at the root; package output
   paths = the xtask defaults (target/appimage, target/deb); the
   bundled-libs claim vs ci.yml:122 (libxkbcommon0 libxkbcommon-x11-0
   libxcb-xkb1). No-op expected.
5. **RF-9-5 (.gitignore fix)** — the three dead crate-path entries removed:
   `/crates/ui/dist`, `/crates/app/src-tauri/target`, `/crates/app/src-tauri/
   gen` (crates/ui and crates/app ceased at the Phase 4 cutover — the
   workspace is 4 crates, none named ui/app). Every remaining entry kept with
   a one-line justification in the completed record; the spp_xtask hygiene
   test `gitignore_keeps_the_secrets_key_never_track_rule` (lib.rs:60, pins
   only the `secrets.key` line) re-run to prove the pinned rule survives.
6. **RF-9-6 (ai/decisions.md fix)** — D6's mapping line re-pointed: "every
   `--ds-*` token → one design.rs constant" → the design/ token factory
   (RF-6 converted design.rs → design/mod.rs + design/); the 1:1 mapping
   claim preserved truthfully. decisions.md otherwise checked end-to-end:
   line 21's `ui/src/pages` is the UPSTREAM original's tree (historical,
   correct as written); D7's git anchors are history.
7. **RF-9-7 (USER_MANUAL verify)** — grep-swept for every old basename +
   `src/` + `crates/` (0 hits expected — the manual is end-user writing) and
   §13's env-var table verified name-by-name against source readers:
   SPP_DATA_DIR (config.rs:35), SPP_LOOPBACK_PORT (app-shell main.rs),
   SPP_LOG_FORMAT (core logging.rs:41), SPP_RENDERER (app-shell lib.rs:211 +
   fatal.rs), HELPSCOUT_* (config.rs + loopback.rs:313), LMSTUDIO_*
   (config.rs:245..), QDRANT_* (config.rs), LOG_LEVEL (logging.rs:23),
   SYNC_INTERVAL_MINUTES / SYNC_API_CONCURRENCY (config.rs). §8 page
   walkthrough = 23/23 PageKey coverage. No-op expected; disclosed.
8. **RF-9-8 (frozen/dated artifacts disposition)** — recorded, NOT edited:
   docs/audit/design-contract-v1.md (frozen DC v1.0, a gate input — its two
   `design.rs` refs describe the v1.0 freeze-time state; the gate's own
   DESIGN constant already follows design/mod.rs since RF-6), mockups/
   design-lab/README.md + mismatch-register.md (dated reskin artifacts whose
   line refs predate the reskin they document), repo-audit-2026-10-24/,
   qa-audit/20261019T050847Z/, qa-redesign/, docs/audit/acceptance/ (dated
   audit artifacts). Zero live claims depend on their stale paths.
9. **RF-9-9 (CHANGELOG disposition)** — the three `design.rs` mentions
   (lines 66/837/842) are immutable release-history records describing v1.x
   states at release time; history is not rewritten; disclosed as deliberate
   so the post-batch grep expectations are explicit.
10. **RF-9-10 (closing gate)** — post-fix zero-stale-path grep re-run across
    the four live surfaces (clean required); full verification table (fmt
    AFTER the last content edit — RF-7 hotfix lesson; clippy
    `--workspace --all-targets -D warnings`; gate_traceability 23/20/27;
    gate_no_simulation; verify-icons; **style_lint explicit — RF-6 hotfix
    lesson**; core lib `--test-threads=1`; app-shell lib; xtask; doc-tests;
    kittest 24 suites = CI lane); ai records synced; single commit; push.

Batch verification (Rule 5, before any item completes): the table above is
the gate list; every row must be green with numbers recorded in this file
before the batch moves to completed.md.

Batch verification results (all green): fmt clean (run after the last
content edit — all three content edits landed first, docs-only, zero Rust
bytes touched so no fmt re-sort was possible); clippy `--workspace
--all-targets -D warnings` 0; gate_traceability OK (23 routes / 20 DC
tags / 27 ids, alias declared True); gate_no_simulation clean; verify-icons
clean; style_lint 2 passed / 0 failed (explicit in the table — RF-6 hotfix
lesson); core lib 495 passed / 0 failed / 4 ignored (--test-threads=1,
352.28s); app-shell lib 237/0 (213.39s); xtask 11/0 (lib) + 0 bin + 0
doc; doc-tests 0/0 (core + app-shell); kittest 24 suites = CI lane (runs
on this push). Hygiene proof for RF-9-5:
`gitignore_keeps_the_secrets_key_never_track_rule` +
`no_secret_key_artifacts_anywhere_in_the_repo` both pass after the prune.
Post-fix census re-run (RF-9-10): the 28-basename old-path grep over the
four live surfaces returns exactly one hit — the D6 historical framing
itself ("the frozen-in `design.rs` became `design/mod.rs` … in RF-6"),
which states the old name as PAST, not present; every other old basename
and every `crates/*/src` path returns zero hits (the only `crates/` path
now named is the new, correct `crates/app-shell/src/design/`).

None — Batch RF-9 completed and moved to `completed.md` (2026-11-01): the
docs-sync wave (2 README claims re-pointed to CI ground truth + 3 dead
.gitignore entries pruned + 1 decisions.md D6 mapping re-pointed to the
design/ token factory; 7 verify/disposition items disclosed). Next per
plan.md (opens on the owner's `continue`): Batch RF-10 — test paths follow
the new structure.

## Batch RF-10 — test paths follow the new structure (items RF-10-1..RF-10-10)

**Batch RF-10 (items RF-10-1..RF-10-10)** — active, moved from plan.md per
Rule 2 (2026-11-01, owner's `continue`). Source: the plan.md RF-10 one-liner +
the test-surface census at open (2026-11-01): every test vehicle inventoried
(core in-lib: it_sync/it_helpscout/it_loopback/it_lm_studio/bench.rs + ~40
`#[cfg(test)]` mods in the split parents; app-shell: 24 external suites +
in-src test mods), every RF-8 old name grepped (zero hits), every split
parent's child re-export lines extracted and cross-checked against its own
test-mod ranges. Finding: external vehicles and the overwhelming majority of
parent test mods already follow the new layout (RF-8 re-pointed the renames;
RF-6/7 re-exports carry the rest); exactly FIVE files carry parent-resident
test mods that resolve child-module items through `use super::*` → the
parent's compatibility re-exports instead of naming the RF-6/RF-7 child home
— nine concrete re-point edits (three ai_attributes mods, ai_tools, helpscout,
app, pages).

1. **RF-10-1 (census)** — the evidence ledger: (a) RF-8 old names
   (eventing/hybrid_search/interaction_current/crate::sync::/upsert_customer_
   basic/conv_by_number/red_chars) = zero hits across all test code; (b)
   kittest suites import `spp_shell::design` (design/ mod.rs = current home)
   + `pages::<page>` = current + `spp_core::bootstrap::apply_all` = the
   intended public API, 1-2 refs each; (c) core it_*/bench imports name only
   unaffected or current homes (db, helpscout [trait's parent home],
   helpscout_real, mockserver, sync_engine, settings, bootstrap, loopback,
   ai_lm_studio, ai_provider, error, event_bus); (d) child-homed item probes
   across all parents: search lanes / quality qa_ai / outreach send /
   design controls / interaction_engine baseline / segment eval / sync_engine
   loops / workers execute are used ONLY by parent production code or via
   inherent impls (no test-mod usage); the five targets below are the
   complete exception set.
2. **RF-10-2 (ai_attributes a2_wiring_tests)** — `mod a2_wiring_tests`
   (mod.rs:543) calls `compute_with_backend` (lines 584/615) through `use
   super::*` → the parent's `pub use extraction::{…}` (mod.rs:537-541).
   Re-point: explicit `use super::extraction::compute_with_backend;` in the
   mod (glob + explicit of the same item = clean shadow, import-path-only).
3. **RF-10-3 (ai_attributes batch22_audit_tests)** — `mod batch22_audit_tests`
   (mod.rs:647) uses prepare_computation (697), finish_computation (703),
   ExtractionOutput (706), run_extraction (841/882). Re-point: explicit `use
   super::extraction::{ExtractionOutput, finish_computation, prepare_
   computation, run_extraction};`.
4. **RF-10-4 (ai_attributes batch23_audit_tests)** — `mod batch23_audit_tests`
   (mod.rs:893) uses customer_messages (960), prepare_computation (1003/
   1110/1134), finish_computation (1006/1119), ExtractionOutput (1009/1122),
   ATTRIBUTE_EXTRACTION_PROMPT_VERSION (1102). Re-point: explicit `use
   super::extraction::{ATTRIBUTE_EXTRACTION_PROMPT_VERSION, ExtractionOutput,
   customer_messages, finish_computation, prepare_computation};`.
5. **RF-10-5 (ai_tools batch23_audit_tests)** — `mod batch23_audit_tests`
   (ai_tools/mod.rs:292) calls `execute` (line 341) through the parent's `pub
   use execute::execute;` (mod.rs:289). Re-point: explicit `use super::
   execute::execute;` (module + item share the name — valid path).
6. **RF-10-6 (helpscout batch20_audit25_tests)** — `mod batch20_audit25_tests`
   (helpscout/mod.rs:1001) builds EmptyHelpScoutProvider (1005/1006/1020) via
   the parent's `pub use inert::{EmptyHelpScoutProvider, InertWorld};`
   (mod.rs:998). Re-point: explicit `use super::inert::EmptyHelpScoutProvider;`
   (InertWorld is not referenced by the mod — verified).
7. **RF-10-7 (app-shell app/mod.rs tests)** — `mod tests` (app/mod.rs:1263)
   uses NAV_GROUPS (1298) + sync_completed_toast (1307/1320/1336/1360)
   through `use super::*` → the parent's production `use chrome::{…}`
   (mod.rs:874-877). Re-point: explicit `use super::chrome::{NAV_GROUPS,
   sync_completed_toast};` (the mod's other import — `crate::pages::
   invalidated_pages` — stays: `mod queries` is private to pages, so the
   parent re-export IS the load-bearing cross-module surface, disclosed).
8. **RF-10-8 (app-shell pages/mod.rs tests)** — `mod tests` (pages/mod.rs:397)
   uses invalidated_pages (402/437/457/460/462) + the three field helpers
   str_field/i64_field/opt_str_field (484-488) via `use super::*` → the
   parent's re-exports (393-394). Re-point: explicit `use super::queries::
   {i64_field, invalidated_pages, opt_str_field, str_field};`.
9. **RF-10-9 (external vehicles verify, no-op)** — it_sync / it_helpscout /
   it_loopback / it_lm_studio / bench.rs + the 24 app-shell external suites
   (23 kittest + style_lint) proven on the new structure: every import names
   an unaffected or current home; zero old names; style_lint's token-factory
   exemption already follows `src/design/` (RF-6 hotfix). Verify-only,
   disclosed.
10. **RF-10-10 (closing gate)** — post-edit re-census (the five files now
    name the child homes; nothing else moved), full verification table (fmt
    after the last content edit; clippy `--workspace --all-targets -D
    warnings`; gate_traceability 23/20/27; gate_no_simulation; verify-icons;
    **style_lint explicit — RF-6 hotfix lesson**; core lib
    `--test-threads=1`; app-shell lib; xtask; doc-tests; kittest 24 suites =
    CI lane), ai records synced, single commit, push.

Batch verification (Rule 5, before any item completes): the table above is
the gate list; every row must be green with numbers recorded in this file
before the batch moves to completed.md.

Batch verification results (all green): fmt clean (applied + re-checked
after the last content edit — the inserted use lines were already
rustfmt-conformant, zero reformatting needed); cargo check
--workspace --all-targets 0 errors (the glob + explicit same-item shadow
pattern compiles clean everywhere); clippy `--workspace --all-targets -D
warnings` 0; gate_traceability OK (23 routes / 20 DC tags / 27 ids, alias
declared True); gate_no_simulation clean; verify-icons clean; style_lint
2 passed / 0 failed (explicit — RF-6 hotfix lesson); core lib 495 passed
/ 0 failed / 4 ignored (--test-threads=1, 351.83s — the re-pointed
ai_attributes/ai_tools/helpscout mods all execute); app-shell lib 237/0
(206.24s — the re-pointed app/pages mods execute); xtask 11/0; doc-tests
0/0 (core + app-shell); kittest 24 suites = CI lane. Post-edit re-census
(RF-10-10): all nine test mods across the five files now name the
RF-6/RF-7 child homes explicitly (7 use statements, grep-verified); the
parent re-exports are untouched and remain load-bearing for cross-module
test imports (app/mod.rs:1266 `crate::pages::invalidated_pages` stays —
`mod queries` is private to pages) and production callers
(sync_engine/workers_host → helpscout::EmptyHelpScoutProvider). Churn:
+12 lines across the 5 source files (imports only) + the ai records;
zero semantic test change (same items, same tests, same assertions).

None — Batch RF-10 completed and moved to `completed.md` (2026-11-01):
test paths now follow the RF-5..RF-8 layout (5 files, 9 test mods
re-pointed to their child homes; external vehicles verified clean). Next
per plan.md (opens on the owner's `continue`): Batch RF-11 — comment
deletion (LAST), then FINISH.



## Batch RF-11 — comment deletion (items RF-11-1..RF-11-10)

**Batch RF-11 (items RF-11-1..RF-11-10)** — active, moved from plan.md per Rule 2
(2026-11-01). Source: plan.md RF-11 (LAST batch of the program order): "All
comments deleted in one final wave (module docs, item docs, inline); rustfmt +
full gates after." Census at open (RF-11-1) selected the ten deletion waves
below; every wave = one module group stripped + compiled + gated.

1. **RF-11-1 (census + protection contract)** — Rust-lexer census of all four
   crates: **11,929 comment lines** (11,927 line comments + 2 block comments —
   pages/ai_center.rs:742 and pages/knowledge.rs:1133 `/* DC sp-1 */`;
   zero `/**`/`/*!` doc blocks) ≈ 693 KB. Protection rules recorded at open:
   (a) the stripper is a real lexer (strings, raw strings, byte strings, char
   literals, lifetimes, nested block comments) — comment-like sequences inside
   string literals (SQL `--`, `https://`, security.rs's CSS `/*…*/` prose) are
   structurally unreachable; (b) **gate-locked keeps**: every DS-id-bearing
   comment line in `design/mod.rs` (58 lines) stays byte-identical so
   gate_traceability's "20 design.rs DC tags valid" output is unchanged;
   (c) `missing_docs` probe (rustc -D warnings on the core/catalog attr pair):
   the later `#![allow(..., missing_docs)]` overrides the earlier
   `#![warn(..., missing_docs)]` — doc deletion fires zero missing_docs
   warnings, **zero lib.rs attribute edits needed**; (d) `style-lint:allow`
   opt-outs: zero exist (nothing to preserve); (e) `#[doc(hidden)]`
   (pages/operations.rs:242) is an attribute, not a comment — stays.
2. **RF-11-2 (core root wave 1)** — 41 alphabetical files activity.rs→inbox.rs
   (~1,950 comment lines; heaviest: loopback 181, webhook_health 177,
   vectorstore_sqlitevec 158, it_helpscout 145, embeddings 136). Includes the
   NO-TOUCH members `ai_center.rs`, `db.rs` — **comment-only edits** (the
   RF-8 record: "intelligence.rs:46 comment … dies at RF-11" fixes the
   program's intent: NO-TOUCH = zero semantic/code edits, comments die with
   the batch; non-comment bytes stay byte-identical).
3. **RF-11-3 (core root wave 2)** — the remaining 42 root files
   (~1,945 lines; maintenance.rs, response_state_sql.rs — the other root
   NO-TOUCH members, comment-only).
4. **RF-11-4 (core subdirs wave 1)** — workers/ (556), helpscout_real/ (314),
   ai_pipeline/ (180), sync_engine/ (228) — the four heaviest subtrees.
5. **RF-11-5 (core subdirs wave 2)** — all remaining subdirs incl.
   `migrations/` (NO-TOUCH `migrations/intelligence.rs` comment-only; the
   MIGRATIONS SQL string contents — including `--` sequences inside the
   strings — byte-identical, migration-SQL red line untouched), outreach/,
   quality/, search/, reports/, people_store/, saved_views/, segment/,
   support_graph/, coaching/ and siblings.
6. **RF-11-6 (catalog + xtask)** — catalog/src (9 lines, 2 files) +
   xtask/src incl. bin/ (265 lines: package 229, lib 10, icons 4, bin 22).
7. **RF-11-7 (app-shell root + app/)** — 15 root files (732; palette 130,
   lib 113, toasts 62, chart 53, main 51) + app/ (198: mod 174, chrome 24).
8. **RF-11-8 (app-shell pages/)** — 21 page files (~2,962 lines; inbox 513,
   settings 360, ai_center 197 …) — the pages/ DC sp-1 block comments die
   here (gate_traceability does not read pages/).
9. **RF-11-9 (design/ + external test suites)** — design/mod.rs (389) with
   the 58 DS-id lines byte-identical (RF-11-1 rule b; everything else in the
   file deleted), design/controls.rs (13); then crates/app-shell/tests/
   (24 kittest suites + style_lint.rs, ~1,210 lines) — CI-lane suites,
   comment-only edits, compile-checked via `--all-targets`, goldens
   unaffected (comments never render).
10. **RF-11-10 (closing gate)** — `cargo fmt --all` (apply + re-check),
    `cargo check --workspace --all-targets`, clippy `--workspace
    --all-targets -D warnings`, gate_traceability (must print the identical
    "23 routes / 20 DC tags / 27 ids" line), gate_no_simulation,
    verify-icons, **style_lint explicit (RF-6 lesson)**, core lib
    `--test-threads=1` (495/0/4ig), app-shell lib (237/0), xtask (11/0),
    doc-tests (0/0), kittest 24 suites = CI lane; diff proof that zero
    string-literal bytes changed (SQL/prompt/URL lines); ai records synced;
    single commit; push.

Batch verification (Rule 5, before any item completes): the table above is
the gate list; every row must be green with numbers recorded in this file
before the batch moves to completed.md.

Batch verification results (all green): fmt clean (applied + re-checked
after the last content edit — the stripper's line removals and the four
collapse sites reflowed); cargo check --workspace --all-targets 0 errors;
clippy `--workspace --all-targets -D warnings` 0 (after 4 disclosed
collapse fixes, see below); gate_traceability OK — **identical output
line**: 23 routes / 20 DC tags / 27 ids, alias declared True (the 58
DS-id lines kept byte-identical); gate_no_simulation clean; verify-icons
clean; style_lint 2 passed / 0 failed (explicit — RF-6 lesson); core lib
495 passed / 0 failed / 4 ignored (--test-threads=1, 353.10s — the full
suite incl. the collapsed people_store path executes); app-shell lib
237/0 (204.41s); xtask 11/0; doc-tests 0/0 (all four crates); kittest 24
suites = CI lane. Structural proofs: string-literal multiset identity
verified for all 179 changed .rs files against HEAD (0 failures — SQL,
prompts, URLs, UI strings byte-identical); residual census = exactly 58
comment lines, all in design/mod.rs, all DS-id-bearing; NO-TOUCH five =
pure deletions only (0 insertions: ai_center −5, db −80, maintenance
−10, migrations/intelligence −6, response_state_sql 0 comments),
including the RF-8-recorded intelligence.rs:46 comment death.

Disclosed deviation (the batch's only non-comment code bytes): clippy's
`collapsible_if` is comment-aware — 11,929 deleted comments exposed four
latently-collapsible `if`s the intervening comments had been suppressing:
people_store/mod.rs:706 (collapsed by hand per the machine suggestion),
overlays.rs:119, pages/operations.rs:337, pages/settings.rs:810 (applied
via `cargo clippy --fix` machine suggestions, then fmt-reflowed). All
four are compiler-suggested, zero-behavior-change restructures, proven
by the full suite (495 + 237 + 11). Scope: .rs files only — harness
(scripts/*.py, ci.yml), Cargo.toml, and ai/*.md records keep their
comments; kittest/style_lint suites included (comments never render;
compile-checked via --all-targets, goldens unaffected).

Batch RF-11 completed and moved to `completed.md` (2026-11-01): the
comment-free tree — 11,871 comment lines deleted, 58 gate-locked DS-id
lines kept. Next per plan.md (opens on the owner's `continue`): FINISH —
rollback instructions + the known-bugs list carried verbatim.

## Batch KB-1 — known-bugs fix wave + kittest in-sandbox resolution (items KB-1-1..KB-1-6)

**Batch KB-1 (items KB-1-1..KB-1-6)** — active, opened 2026-11-01 on the
owner's explicit direction ("Fix this" the two known bugs, "Resolve this"
the kittest sandbox gap) recorded in plan.md; the FINISH program's
LIST-ONLY scope decision is superseded by the owner's instruction. Batch
verification (Rule 5): the KB-1-6 gate table; every row green with numbers
recorded here before the batch moves to completed.md.

Batch verification results (all green): fmt clean (applied + re-checked
after the last content edit); cargo check --workspace --all-targets
clean; clippy `--workspace --all-targets -D warnings` 0;
gate_traceability OK (23 routes / 20 DC tags / 27 ids, alias declared
True); gate_no_simulation clean; verify-icons clean; style_lint 2
passed / 0 failed (explicit — RF-6 lesson); core lib 495 passed / 0
failed / 4 ignored (--test-threads=1, 348.55s); app-shell lib 242/0
(235.32s — the 237 baseline plus exactly the 5 new regression tests);
xtask 11/0; doc-tests 0/0 (four crates).

**KB-1-5 — the kittest gap RESOLVED in-sandbox**: the RF-era constraint
that justified CI-lane-only no longer binds (7.7 GB free disk at open,
2.8 GB available RAM). CI's own recipe was replicated locally: Mesa's
lavapipe software Vulkan rasterizer installed WITHOUT root (apt-get
download + dpkg -x of mesa-vulkan-drivers + libllvm19 into
/home/z/mesa-lvp, VK_DRIVER_FILES/VK_ICD_FILENAMES pointed at the
extracted lvp_icd.json, LD_LIBRARY_PATH at the extracted libs,
WGPU_BACKEND=vulkan — the exact env block of ci.yml:198-220), then the
CI command `cargo test --locked -p supportos-plus-app-shell --features
kittest-ui` per suite. Result: **23 kittest suites — 173 tests passed /
0 failed — all 171 committed goldens diff-locked with ZERO drift**
(snapshots/ holds exactly 171 .png baselines — the census reconciles:
24 files = 23 suites + style_lint, 175 tests = 173 + 2, 171 goldens).
style_lint under the same feature build: 2/0. First-ever in-sandbox
kittest run in the program's history; the standing gap disclosed at
every STOP is now closed both at CI (RF-11/FINISH) and locally (KB-1).

Fix notes: the custom_objects `objects_loaded` latch is selection-scoped
(cleared on select_type, delete_type's selection-clear, and reload_types'
selection-vanished branch — a selection change still refetches); inbox's
inline ui() gates were extracted into `ensure_loaded` (the batch-34
shape; the transient `detail.is_none` gate is unchanged); knowledge got
one `loaded` latch over reload_all gating both tab UI fns; the Settings
fix mirrors General's `general_persisted` guard exactly
(`lmstudio_persisted` triple, reload assigns only when the form matches
the snapshot, save_lmstudio re-bases the snapshot on the saved trimmed
values). Regression tests: 3× `ensure_loaded_latches_and_invalidate_
refetches` (one per page, sentinel-error side-effect assertions prove
the no-refetch behavior) + the General pair mirrored for lmstudio
(`lmstudio_invalidation_refreshes_a_clean_form_from_the_db`,
`lmstudio_dirty_form_survives_invalidation`).

Environment note: fresh-sandbox toolchain re-provisioned (rustup
1.99.1); disk hit 100% once during the kittest feature build — the
kittest results were already recorded, so the feature-build artifacts
were reclaimed (incremental dir 634 MB) and the remaining battery
completed on the freed space. No gate was run under disk pressure
except verify-icons, which was re-run clean after the reclaim.

Batch KB-1 completed and moved to `completed.md` (2026-11-01): both
known bugs fixed with 5 pinned regression tests; the kittest CI-lane
gap resolved in-sandbox (175/175, 171/171 goldens). ai records synced;
single commit; push next.

## Batch AUDIT-1 — post-kb1 full-repo unwired/orphaned/dead-surface audit (completed 2026-11-02)

Scope: whole-repo read-only census @ `0f02e62` (kb1 = origin/main, tree
clean) for unwired sources, orphaned files, verified-dead pub APIs,
unused deps/features, placebo settings, dead events, stale doc paths,
and unreferenced artifacts. Method: 6 parallel scripted scans (entry-
point reachability walk, pub-item cross-module consumer check, Cargo
graph probe, settings read/write census, event-bus publish census,
basename/path resolution over live docs) + 3 manual spot-checks that
all confirmed the scan output. Read-only by necessity — the audit
sandbox hit 99% disk (148 MB free; `target/` = 5.1 GB), so zero cargo
runs backed this batch; every finding is grep/script/census evidence
plus manual confirmation, and the fix batch (kb2) re-proves each
deletion against the full gate battery. Environment: the sandbox reset
again after the kb1 push (repo, credentials, toolchain lost — third
reset of the program); this record and kb2 are written from a fresh
clone of origin/main @ `0f02e62` (RF-1 recovery precedent; tree is
byte-identical to the audited one, so all findings apply as stated).

Findings (11 groups):

**A1-1 — orphaned source files: 0.** 205/205 .rs files reachable from
the lib/bin entry points; no orphans.

**A1-2 — ~25 verified-dead pub API clusters** (lib-crate `pub` surface
is invisible to clippy's dead_code lint; deletion is kb2's primary
scope). Largest: the **saved-views engine cluster** — core/saved_views/
engine.rs + mod.rs:536-595 (`SavedInboxView`, `CreateViewRequest`,
`UpdateViewRequest`, `ViewIssue`, `VIEW_CONDITION_KINDS`, `SlaResolver`,
`CompiledView`, `resolve_date_range` and dependents). The rest:
`universal_search` (search/mod.rs:615), `rewrite_draft` (ai_pipeline
:1394), the outreach dead type group (mod.rs:62-164, 949), the
interaction_profile family (interaction_signals.rs:81, 951-1033),
`build_report`/`compute_friction` family (reports/mod.rs:106-423,
1341), `validate_ssrf_full` (security.rs:90, 439-518),
`InteractionCard`/`CardProvenance`, `AiEnrichment`, `LinkError`,
`scan_for_mentions`, `redacted_secret`, 5 dead constants, the `Toast`
struct, 11 dead design tokens (design/mod.rs:187-221), and 8 catalog
dead re-exports.

**A1-3 — ~80 EXPORTED-UNUSED de-pub candidates.** Internal callers
only, no cross-module consumers; disposition is de-pub (visibility
tightening), NOT deletion — deferred to a later visibility pass (kb3
candidate) to keep kb2's diff reviewable; kb2 takes the deletions and
deps.

**A1-4 — 2 `allow(dead_code)` stashes:** `ConversationInfo.number`
(interaction_engine/mod.rs:995), `BatchRow.email` (outreach/mod.rs
:1377) — fields written by nothing, suppressed by the attribute.

**A1-5 — unused deps/features:** `subtle` (core Cargo.toml:27),
xtask's `serde`+`serde_json`, workspace-root `anyhow` (root Cargo.toml
:26), `rusqlite` `blob`+`limits` features, `chrono/serde`. Lockfile
consequence for kb2 disclosed: pruning updates Cargo.lock only by
cargo's own resolution (entries removed, no version churn); no
hand-edit.

**A1-6 — 4 placebo settings switches** (UI renders + persisted, zero
readers): `automatic_reply_sending`, `fts_version`,
`demo_mode`/`demo_content_purged`, `display_timezone`. kb2 drops them
wire-or-drop; persisted-DB-schema is OFF-LIMITS, so the drop shape must
not alter migration semantics (keys die in code; schema untouched).

**A1-7 — 11 read-never-written knobs** (always-default, no UI):
`redaction_enabled`, `lmstudio_timeout_ms`, `lmstudio_concurrency`,
`ai_evaluation_mode`, `ops_waiting_threshold_minutes`,
`freshness_*_days`, `*_sweep_seconds`, `backup_interval_hours`,
`retention_days`, `notifications.{type}.enabled`. Several carry
behavior semantics (`retention_days=0`, `ai_evaluation_mode`) —
deleting the field is behavior-preserving only because the reader
always observes the default; each case re-verified in kb2 before
removal, else deferred.

**A1-8 — 1 dead event:** `CampaignUpdated` (no production publisher;
queries.rs:53 invalidation arm unreachable) + 3 dead helpers
(`ServerEvent::channel`, `ServerEvent::payload_json`,
`EventBus::subscriber_count`).

**A1-9 — 4 stale live-doc paths:** design-contract-v1.md:36,338;
blind-test-protocol.md:23; gate_traceability.py:20 docstring.

**A1-10 — orphaned artifacts:** manual/finalize_book.py +
manual/finalize_handover.py (2 orphaned scripts), verify_batch35.py
(archive gap), and ~48 MB of unreferenced history dirs (qa-audit/,
repo-audit-2026-10-24/, qa-redesign/) — dated artifacts preserved in
git history, so HEAD deletion is reversible. docs/audit and mockups are
live-referenced and stay.

**A1-11 — clean:** 0 TODO/FIXME, 0 asset mismatches, 0 schema drift,
23/23 routes reachable, CI paths all resolve.

Disposition recorded for kb2 (fix batch): A1-2/A1-4/A1-8 deletions;
A1-5 pruning; A1-6 wire-or-drop; A1-9 doc path fixes; A1-10 script/dir
disposition. **A1-2 exception disclosed:** the 3 dead fns in
response_state_sql.rs (:12, :25, :37) stay — the module is on the
NO-TOUCH list; owner confirmation required before it is ever touched.
A1-3 (de-pub sweep) and A1-7 (knob census) are recorded as open items,
not kb2 scope.

## Batch KB-2 — dead-surface deletion + dep pruning + placebo wire-or-drop (items KB-2-1..KB-2-9) — opened and completed 2026-11-02

Implements AUDIT-1's disposition at `0f02e62`. Compiler + full gate
battery re-proved every removal; two audit claims were corrected by
that re-proof (disclosed inline below). A1-3 (de-pub sweep) and A1-7
(read-never-written knob census) remain open for a later batch.

**KB-2-1 — saved-views engine cluster (A1-2's largest)**: engine.rs
(742 lines, pure `impl ViewEngine`) deleted; mod.rs dead surface
deleted — ViewCompileError (+Display/Error impls), DateRangeKind/
DateRange/DateRangeInput, resolve_date_range (+parse_time_of_day,
wall_parts, local_instant_ms, days_in_month, rolling, now_ms_epoch),
activity_field_column, age_metric_sql, VIEW_CONDITION_KINDS, ViewNode,
ViewDefinition, SavedInboxView, ViewIssue, CreateViewRequest,
UpdateViewRequest, SqlParams, Fragment, CompiledView, SlaResolver,
ViewEngine, mod engine, placeholders/text/integer/real helpers, both
re-exports (is_valid_timezone, escape_like), MAX_TREE_DEPTH — plus
batch31_tests (its 2 tests exercised only the dead engine). Kept: the
live trio `iso_utc`/`iso_utc_pub` (operations.rs:412),
`RESPONSE_STATE_SQL` (7 consumers), `ensure_inbox_views_table`
(bootstrap.rs + migrations/db_breadth.rs). mod.rs 800 → 51 lines.
Census note: MAX_TREE_DEPTH/SqlParams ext hits were same-name
collisions with segment's own items, not consumers.

**KB-2-2 — other verified-dead pub APIs**: `universal_search` +
cap_query + MAX_SEARCH_QUERY_LENGTH (search — cap_query and the const
became orphans of the deletion, deleted with it); `rewrite_draft`
(ai_pipeline; rewrite_instruction/get_draft stay — other callers);
`validate_ssrf_full` sync twin (security.rs — the async twin stays, 2
live consumers); `scan_for_mentions` + user/team_mention_regex + the
scanner/parser agreement test (mentions; is_preceded_by_word_char and
MAX_BODY_BYTES stay — live via parse_mentions); `redacted_secret` +
its assertion inside the secrets round-trip test (settings);
`InteractionCard`/`CardProvenance`/`AiEnrichment` (interaction_engine;
field types all live elsewhere); `LinkError` (support_graph); reports:
ReportRow/ReportResult/build_report/metric_info + the 2 tests that
were their only callers (week_dimension_is_iso…,
build_report_excludes_soft_deleted…) + `friction_findings` (re-verified
zero callers during kb2). **AUDIT CORRECTION disclosed**: the audit's
"build_report/compute_friction family" over-reached —
compute_friction/FrictionScore/FrictionFactors/HIGH_FRICTION_THRESHOLD
are LIVE (recompute_recent_friction at reports/mod.rs calls
compute_friction; app-shell pages/reports.rs:1245 calls
recompute_recent_friction) and were KEPT; the audit's family-span
reading (106-423, 1341) was too broad.

**KB-2-3 — outreach dead types + stashes**: SavedSegment, create_segment,
DEFAULT_MAX_CAMPAIGN_ATTEMPTS, Campaign, CampaignRecipient, CampaignStats,
campaign_reconcile (+ its batch30_tests module — only consumer),
count_remaining (orphaned by campaign_reconcile's deletion) deleted.
Live and kept: delete_segment (app-shell outreach.rs:307 — caught in the
first cut by the app-shell compile, restored byte-identical,
disclosed), add_to_dnc/remove_from_dnc/is_on_dnc/list_dnc, the whole
campaign_*_v2 surface. **AUDIT CORRECTION disclosed**: BatchRow is live
(constructed by outreach/send.rs::pick_batch — the audit's mod.rs-only
span missed the submodule); only its `email` field is a write-never-read
stash — field removed, SELECT and get() indices re-shifted. The other
stash: ConversationInfo.number (interaction_engine) removed the same way
(SELECT no longer selects number).

**KB-2-4 — interaction_profile family** (interaction_signals.rs): the
closed dead island 950-1121 (StoredCurrentSignals, ProfileDimensionValue,
ProfileDimension, ProfileResponsePreference, InteractionProfile,
InteractionOverrideRow, get_interaction_override, interaction_profile) +
INTERACTION_DIMENSIONS — zero external consumers, all references
intra-island. ClientInteractionProfile (interaction_engine, baseline.rs)
is a different, LIVE type — untouched.

**KB-2-5 — dead constants + design tokens + catalog re-exports**:
22 census-verified dead consts deleted: CAPACITY_WINDOW_DAYS;
COACHING_CHECK_KINDS / COACHING_MAX_DRAFT_BYTES / COACHING_MAX_DRAFT_CHARS;
FIELD_TYPES / LINK_TARGET_KINDS; FRICTION_KINDS;
HELPSCOUT_SIGNATURE_HEADER; HS_API_BASE / HS_DOCS_API_BASE;
MAIN_TRIGGERS / MAIN_ACTION_KINDS / MAIN_CONDITION_FIELDS /
MAIN_CONDITION_OPERATORS / MAIN_AI_ATTRIBUTE_KEYS;
MAX_RESULTS_PER_TYPE / SEARCH_SCOPES; OPS_INVALID_MESSAGE; and M003/
M004/M005/M006/M008/M010_SQL — never-executed historical duplicates of
the inline registry SQL; the MIGRATIONS registry is untouched, so
migration SQL semantics (OFF-LIMITS) are intact. NO-TOUCH exceptions
held: M009_SQL (ai_center.rs) and M014_SQL (migrations/intelligence.rs)
stay. 9 dead design tokens deleted (SP5, SP6, PAD_CARD_V, PAD_BODY_V,
PAD_READER_X, PAD_READER_V, radius SM/MD/LG — audit said 11, the kb2
census verified 9; DS-C-02/DS-A-05 remain cited by live tokens so the
traceability gate's id set is unchanged — 20 DC tags / 27 ids, same as
kb1). 9 dead catalog re-exports deleted (ActivityField, ConditionKind,
DateMode + the whole workspace block: ConnectorAuthMode, ConnectorKind,
CustomFieldType, IncidentSeverity, IncidentSource, IncidentStatus —
audit said 8; both counts approximate, the census is the record).
Disclosed census hole: AI_ATTRIBUTE_SCHEMA_VERSION and GraphHumanRelation
were initially cut too — the first pass missed braced single-line
`use crate::catalog::{…}` imports — both restored on the compile error
and their re-exports stand.

**KB-2-6 — dead event + helpers**: CampaignUpdatedEvent struct,
ServerEvent::CampaignUpdated variant + its wire_name arm, `channel()`,
`payload_json()`, campaign_updated() ctor, EventBus::subscriber_count —
all zero-consumer, deleted. App-shell consumers re-pointed: the
queries.rs CampaignUpdated arm died with the variant, and SyncCompleted
now also invalidates PageKey::Outreach — CampaignUpdated was Outreach's
ONLY invalidating event, so the D19 every-cacheable-page-covered
invariant fails without a truthful replacement; reply-state (replied_at,
hs_conversation_number) lands in outreach_recipients via sync, so
SyncCompleted is the correct carrier (the outreach send batch itself
emits no event — verified in workers/execute.rs). The D19 test caught
this on the first app-shell run (241/1: "page Outreach is subscribed to
no event at all"); after the re-wiring 242/0. Also: the pages/mod.rs
coverage probe list drops the campaign probe; the app/mod.rs test
campaign_events_invalidate_outreach_and_notifications_arrive renamed to
notification_events_invalidate_notifications_and_inbox minus its
campaign half.

**KB-2-7 — placebo switches + dep pruning**: automatic_reply_sending
dropped end-to-end (GeneralSettings field, Default, load, save, and the
"Automatic reply sending" UI checkbox row). The fts_version write-only
marker block dropped from the FTS rebuild path (runtime
application_settings write, NOT migrations SQL; zero readers).
demo_mode/demo_content_purged and display_timezone verified ALREADY
inert (no UI, no writer, no production reader — only audit.rs's legacy
key purge, which stays for old DBs, and data_reset test samples):
recorded, no change. Deps: root `subtle` + core's subtle.workspace line,
root `anyhow` (xtask keeps its own direct, used, anyhow), xtask
serde+serde_json, rusqlite `blob`+`limits` features (no rusqlite::Blob/
limits call sites; ValueRef::Blob is not feature-gated), chrono `serde`
feature (no serde-serialized chrono types). Cargo.lock diff = exactly
4 dep-reference removals, zero version churn (subtle remains as a
transitive of scrypt — only the direct declaration died).

**KB-2-8 — docs + artifacts**: 4 stale paths fixed —
design-contract-v1.md:36 `design.rs` → `design/mod.rs`; :338
`app.rs:926-971` → `app/chrome.rs` (NAV_GROUPS's current home);
blind-test-protocol.md:23 `design.rs` → `design/mod.rs`;
gate_traceability.py:20 docstring `design.rs` → `design/mod.rs` (the
gate's DESIGN constant was already correct — docstring-only change).
manual/finalize_book.py + manual/finalize_handover.py deleted (zero
references). qa-audit/ + repo-audit-2026-10-24/ + qa-redesign/ deleted
— 528 tracked files, so the git-history preservation the audit claimed
holds (they were tracked after all; deletion is reversible via
history). verify_batch35.py: never existed in git — archive gap
recorded only.

**KB-2-9 — verification (all green)**: fmt clean (applied + re-checked);
clippy --workspace --all-targets -D warnings 0; gate_traceability OK —
23 routes / 20 DC tags / 27 ids / alias True (identical to kb1);
gate_no_simulation clean; verify-icons clean; style_lint 2/0 (explicit);
core lib 489 passed / 0 failed / 4 ignored (--test-threads=1, 345.18s —
495 minus exactly the 6 deleted cluster tests, census reconciles);
app-shell lib 242/0; xtask 11/0; doc-tests 0/0; kittest 23 suites /
173 passed / 0 failed — 171 goldens total, census reconciled: exactly
the 6 settings goldens regenerated (UPDATE_SNAPSHOTS=1; the dropped
checkbox row changes the settings layout at all 3 widths × both
themes — the intended UI consequence of the wire-or-drop), the other
165 zero-drift, all .diff/.old artifacts cleaned. Churn: +139/−2011
across 50 tracked files (44 modified incl. 6 goldens + 3 deleted
sources/scripts) + 528 history-dir file deletions + the ai records.

Environment disclosure: 4th sandbox reset (repo, credentials,
toolchain, and the Mesa install all lost); everything re-provisioned
from scratch — fresh clone of origin/main @ `0f02e62` (tree
byte-identical to the audited one), rustup 1.99.1, Mesa lavapipe via
the kb1 dpkg -x recipe (lvp_icd.json library_path patched to the
extracted absolute path). Disk hit 100% once during the kittest feature
build (linker bus-errors, no code meaning) — reclaimed (the duplicate
`stable` toolchain 593 MB + re-downloadable browser caches) and the
feature build re-run with CARGO_PROFILE_DEV_DEBUG=0 + `-j 2` to bound
linking pressure; test semantics and rendering are unaffected by debug
info, disclosed for fidelity.

## Batch KB-3 — A1-3 de-pub sweep + A1-7 knob census + response_state_sql owner disposition + kb2 record correction — opened and completed 2026-11-02

Owner directives at open: (1) response_state_sql.rs's 3 dead fns —
"untouchable … take the best decision which is logical": disposition
recorded as **KEEP** (the module is NO-TOUCH; the 3 fns are dead but
removing them buys ~30 lines against a full gate re-proof on the
most sensitive SQL surface in the program — keeping is the logical
minimum-risk choice; the module was never touched, diff-verified).
(2) A1-3 de-pub sweep + A1-7 knob census — approved. Environment: 5th
sandbox reset — repo, credentials, toolchain, Mesa all re-provisioned
from scratch; fresh clone of origin/main @ `59f981d` (tree
byte-identical to the kb2-audited one); rustup 1.99.1 (minimal
profile + the pinned components), Mesa lavapipe re-extracted
(Debian trixie mesa-vulkan-drivers 25.0.7 + libllvm19, lvp_icd.json
library_path patched, VK_DRIVER_FILES/VK_ICD_FILENAMES +
LD_LIBRARY_PATH + WGPU_BACKEND=vulkan per the kb1 recipe).

**KB-3-1 — A1-3 census + execution.** A conservative name-based census
(collisions only suppress candidates) over catalog/core/app-shell lib
surfaces against the external consumer corpora (app-shell lib + bin +
integration tests, xtask) found **1,182 pub items with zero external
consumers**; caller-locality split: **44 same-file-only** (de-pub-able
in one step) + **1,134 cross-file crate-internal** (a 1,134-line
pub→pub(crate) sweep — recorded as open item **A1-3b**, its own
reviewable batch, NOT executed here). The audit's "~80" was a
pre-kb2 approximation of the same-file-only population; kb2's
deletions and stricter test-region tracking re-derived it as 44.

The 44 − 7 NO-TOUCH skips (response_state_sql.rs:12/25/37 —
owner-confirmed keep; db.rs:21 open_with_migrations; ai_center.rs:7
M009_SQL and ai_center.rs:172 copilot_tool_allowlist;
migrations/intelligence.rs:11 M014_SQL) − 1 (universal_search, see
KB-3-2) = 36 de-pubs, + 4 sibling cfg(test) lib mods (bench,
it_lm_studio, it_sync, mockserver — added for uniformity with the
flagged it_helpscout/it_loopback; all six are #[cfg(test)]-gated so
no external surface ever existed) = **40 visibility edits**.

**KB-3-2 — kb2 record correction (disclosed): universal_search was
claimed deleted in kb2 but never was.** The 59f981d diff for
search/mod.rs contains only the MAX_RESULTS_PER_TYPE/SEARCH_SCOPES +
fts_version deletions (18 lines); universal_search, cap_query and
MAX_SEARCH_QUERY_LENGTH survived at :603/:621/:7. kb3 completes the
recorded disposition: **universal_search + cap_query +
MAX_SEARCH_QUERY_LENGTH + SearchResult deleted** (39 lines) —
SearchResult (the fn's return struct) was orphaned with the cluster
and missed by the original audit too. Every other kb2 deletion claim
was re-verified present-and-deleted at HEAD (the census found none of
them).

**KB-3-3 — the compiler re-proof converted the de-pub sweep into a
deletion batch.** After the 40 edits, `cargo check` fired **40
dead_code warnings**: every de-pub'd item except l2_norm is FULLY dead
(the census's "same-file refs" were mentions inside other dead code)
plus 8 items inside mirror_readouts.rs. Compiler-authoritative
disposition (no allow() stamps — the A1-4 anti-pattern stays banned):
**34 dead items deleted** (32 de-pub'd + CapacityMetrics which the
deletion list initially missed + l2_norm whose only caller dot_product
died in the same sweep — the cascade the compiler caught) and the
**mirror_readouts module deleted whole** (291 lines + its mod decl) —
completing RF-8's parked owner decision with compiler proof; its
exactly-2 own tests (percent_and_underscore_match_literally,
plain_searches_still_match) exercised only the module's dead fns and
died with it, census-reconciling core lib 489 → 487. Alive-and-now-
private: l2_norm… (corrected: l2_norm was deleted with the cascade;
the alive set is exactly the 6 cfg(test) mods). Deleted: jobs
RUNNING/FAILED/AWAITING_APPROVAL; automation M007_SQL (an unregistered
never-executed duplicate — apply_m007's inline SQL untouched, zero
migration-semantics change; kb2's M003..M010 list missed it),
AutomationEvent, EvaluateResult, RuleInput, RuleUpdate;
ai_attributes AttributeMatch; conversation_ops ConvFull; embeddings
ConversationChunkStats/ConversationChunkWithEmbedding/
DocChunkWithEmbedding; copilot CopilotResult; migrations/side_threads
SideThread/SideThreadMessage/CreateSideThreadInput/ValidationIssues/
SideThreadMentionForUser/SideThreadSummary/SideThreadDetail (row
structs only — the executed SQL is byte-identical);
intelligence_features RadarSnapshot/SlaStatus/changed_columns (+
the now-empty impl IncidentPatch block); ticket_states
StateTransition/TicketStateDef; vectorstore dot_product + l2_norm;
notification_prefs from_bool; ai_local latest_ai_draft;
ai_lm_studio with_timeout_ms; app-shell inbox set_composer_mode;
mirror_readouts.rs entire.

**KB-3-4 — A1-7 knob census (deliverable: verified table +
dispositions; zero deletions).** 14 concrete keys across the 11 audit
groups; every key re-verified at HEAD: **zero production writers**
(the only writers found are tests: redaction_enabled ×2,
ai_evaluation_mode ×1) and **every key has a live reader** — these
knobs are running code, not dead code, and the settings table IS this
app's operator interface (USER_MANUAL:871 teaches it explicitly).
Dispositions: **KEEP documented** — lmstudio_timeout_ms (5 readers
incl. the AI-center display) + lmstudio_concurrency (README:109,
USER_MANUAL:920: "Request budget"; the audit's "no UI" was true but
they are shipped, documented settings-table configuration; removal
would contradict the manual) and ai_evaluation_mode (USER_MANUAL:871
documents the flag itself + the eval_mode_blocks_all_mutations test
covers the non-default path). **KEEP tested** — redaction_enabled (4
readers; two tests exercise disabled-redaction "honored verbatim").
**KEEP hidden-lever, undocumented** — ops_waiting_threshold_minutes
(reader sanitizes to 1..=20_160 — deliberately built to be set),
freshness_stale_days (2 readers), freshness_unreviewed_days,
freshness_low_usage_days, notification_sweep_seconds,
customer_event_sweep_seconds, intelligence_sweep_seconds,
backup_interval_hours, notifications.{type}.enabled (pref_for gates
every insert with per-type defaults). **DEFER** — retention_days
(reader feeds enforce_retention in NO-TOUCH maintenance.rs; the
knob+call pair stays byte-identical). A1-6 residue re-verified at
HEAD: automatic_reply_sending + fts_version zero refs (kb2 drop
complete); demo_mode/demo_content_purged + display_timezone exist
only in audit.rs's legacy-key purge + data_reset tests (old-DB
compat, intentional) — recorded, no change. Follow-up options for the
owner (recorded, not executed): document the 9 undocumented keys in
USER_MANUAL, or take a product decision to remove them.

**KB-3-5 — incident, disclosed.** The region-deleter's scanner
conflated () with {} in its depth counter, so all 6 fn deletions left
orphaned bodies (signature deleted, body remaining). The immediate
cargo check caught it (first syntax error at ai_lm_studio.rs:339);
all six repaired by hand (ai_lm_studio, app-shell inbox,
notification_prefs, vectorstore, intelligence_features — where the
now-empty impl IncidentPatch block was removed whole — and ai_local);
re-check green. Also investigated: terminal output showed
`#[must_use]` as `#ust_use]` across many files — proven a display-
pipeline artifact only (Read + git show show the correct bytes; the
files were never corrupted; no action).

**KB-3-6 — verified, all green:** fmt clean (applied + re-checked
after the last content edit); cargo clippy --workspace --all-targets
-D warnings 0; gate_traceability OK — identical line 23 routes / 20
DC tags / 27 ids; gate_no_simulation clean; verify-icons clean; core
lib **487 passed / 0 failed / 4 ignored** (--test-threads=1, 346.02s
— 489 minus exactly mirror_readouts' 2 own tests); app-shell lib 242
passed / 0 failed (228.45s); xtask 11 passed / 0 failed; doc-tests
0/0; **kittest 24 suites / 185 tests passed / 0 failed** in-sandbox
(batches: 13 suites/103 + 11 unique suites/82 + style_lint 2/0 —
live_chats double-run deduped) — **171 goldens, ZERO drift** (kb3
touched no UI; the settings page is byte-identical). Churn: 18 files,
+7/−670 (mirror_readouts.rs 291 + search/mod.rs 39 + the 32 dead-item
regions + 40 visibility edits' pub-keyword removals). Open items
recorded: **A1-3b** (1,134 cross-file crate-internal pub→pub(crate)
sweep), the 9 undocumented knobs (document-or-remove, owner's call),
response_state_sql.rs 3 fns (KEEP recorded, owner-delegated).

## Batch KB-4 — A1-3b visibility tranche (447 pub→pub(crate)) + A1-7 hidden-lever documentation (USER_MANUAL §13.1) — opened and completed 2026-11-02

Owner directive at open: "take the best course of action" on the two
recorded open items (the A1-3b 1,134-item sweep; the 9 undocumented
knobs). Course chosen: BOTH — the knobs documented (cheap, zero-risk,
completes A1-7's recorded follow-up) and the visibility sweep executed
as ONE reviewable tranche per kb3's "its own reviewable batch"
discipline, with the residual re-censused and recorded rather than
churned unreviewed. The tranche itself was found ALREADY APPLIED in the
working tree on session resume (the prior session was cut mid-batch
after applying it; no records, no compile proof, no commit) — this
session verified it line-by-line, completed it, proved it, and
recorded it. Nothing was trusted from the cut session: every edit
re-derived, every manual claim re-checked against the readers.

**KB-4-1 — A1-3b tranche: 447 pub→pub(crate) visibility edits across
99 source files (68 core + 31 app-shell), +517/−447 source lines;
ZERO deletions, zero signature changes, zero SQL/migration semantics
touched.** Diff purity proven line-by-line: filtering the whole diff
for non-visibility lines yields exactly the USER_MANUAL §13.1 block
plus rustfmt reflows of widened signatures (each a deleted one-line
`pub` signature replaced by a multi-line `pub(crate)` one — e.g.
chart.rs `line_chart`); nothing else. NO-TOUCH modules absent from the
diff (ai_center.rs, db.rs, maintenance.rs, response_state_sql.rs —
diff-verified; there is no separate intelligence.rs); migrations.rs
edits are visibility-only on non-SQL items. No allow() stamps.

**KB-4-2 — residual census re-derived, honestly recorded.** The lost
session applied 447 of kb3's 1,134-item census; the residual was
RE-DERIVED at the post-tranche tree with the kb3 methodology
(conservative name-based, collision-suppressing, external consumers =
app-shell src+tests / xtask / catalog, #[cfg(test)] mods blanked,
NO-TOUCH files counted apart): **651 remaining candidates (472 core +
170 app-shell + 9 catalog) + 41 plain-pub sites in NO-TOUCH files
(out of scope by owner directive)**. 447 + 651 = 1,098 ≈ kb3's 1,134
(±3% name-based noise on both ends — consistent). The 9 catalog
candidates are exactly the enum definitions whose kb2-deleted
re-exports orphaned them (ActivityField/DateMode/ConditionKind + 6
workspace enums) — recorded for a future batch, NOT executed here.

**KB-4-3 — A1-7 completed: USER_MANUAL §13.1 "Settings-table levers"
added** — all 9 KEEP-hidden-lever keys plus retention_days (the DEFER
resolved as "document instead") in an operator-facing table with
defaults, accepted ranges, read-timing, and the notification type
list. EVERY claim verified against the readers before shipping; the
verification caught two imprecisions, fixed in the same pass:
(1) ops_waiting_threshold_minutes is read by the Inbox filter bar
(inbox.rs:98 → tile_fragment) as well as the Ops page — row now reads
"Ops and Inbox pages"; (2) enforce_retention also prunes
NOTIFICATIONS (rows older than the window, or read longer ago than
the window) — the draft row omitted them; maintenance.rs:108-136 read
in full and the row corrected. All 15 notification type names match
NotificationType::as_str exactly (catalog/notifications.rs);
default_enabled() ≡ true; pref_for gates the insert path
(notifications.rs:229); sweep defaults/clamps verified at
workers/mod.rs:275/297/319 (15 s / 5–3600, 60 s / 15–3600,
900 s / 60–86400; each read once at worker start then looped —
"restart after changing" is literal); backup_interval_hours default
24, `0` = off (workers/mod.rs:985-987); freshness defaults
180/90/60 (knowledge_store.rs:336-340); ops threshold default 240,
clamp 1..=20_160 (operations.rs:11,148-158); the example SQL matches
the application_settings(key,value) schema; the §7 anchor and the
§13.1 numbering verified collision-free.

**KB-4-4 — verification, all green (incidents disclosed):** fmt clean
(applied + re-checked — the reflows were already fmt'd, zero churn);
cargo check --workspace --all-targets clean with ZERO warnings; cargo
clippy --workspace --all-targets -- -D warnings 0; gate_traceability
OK — identical 23 routes / 20 DC tags / 27 ids; gate_no_simulation
clean; verify-icons clean; core lib **487 passed / 0 failed / 4
ignored** (--test-threads=1, 345.74s — identical baseline); app-shell
lib **242/0** (215.41s); xtask **11/0**; doc-tests 0/0; **kittest:
batch A 13 suites/93 + batch B 12 targets/89 (style_lint 2 + the
live_chats double-run) = 24 unique targets / 175 unique tests /
0 failed — exactly the kb1/kb2 canonical baseline (23 kittest suites
/ 173 + style_lint 2) — 171 goldens ZERO drift** (UPDATE_SNAPSHOTS
never set; every suite is diff-locked against the committed baselines
and passed). **RECORD CORRECTION disclosed: kb3's "24 suites / 185
tests" was a batch-arithmetic artifact (103+82 double-counted the
live_chats overlap); the true canonical is 175 = 173 + 2, re-proven
here — kb3's tree had no UI changes and the golden set is
byte-identical.** Environment notes, disclosed: detached background
builds are reaped by this sandbox (setsid survives only briefly), so
long builds ran foreground-iteratively on cargo's cache; disk
pressure managed per the kb2 recipe (CARGO_PROFILE_DEV_DEBUG=0 +
CARGO_BUILD_JOBS=2; test-profile artifacts cannot reuse check/clippy
rmeta, so target/ was cleaned once mid-batch to reclaim 3.9G and the
proven lib-test binaries were deleted between phases). Churn: 100
files, +541/−447 (99 source files: 447 visibility edits + 70 reflow
lines; USER_MANUAL +24). Residual open items: **A1-3b remainder (651
candidates, re-censused)**, response_state_sql.rs 3 fns (KEEP
recorded, owner-delegated). ai records synced.

## Batch KB-5 — A1-3b completion: 649-item visibility tranche + compiler-re-proof deletions (~2,870 lines) — opened and completed 2026-11-02

Owner directive at open: "continue" — the recorded A1-3b remainder
(651 re-censused candidates from kb4) was the open work item. Course:
execute it to completion per the kb3-approved playbook (de-pub →
compiler re-proof → delete what the compiler proves dead → keep the
rest narrowed), in one reviewable batch.

**KB-5-1 — tranche + re-proof executed.** 649 visibility narrows
applied (632 from the kb4-methodology census at HEAD + 17 `pub async
fn` items the census regex missed — blind spot closed and disclosed).
The compiler re-proof then fired at scale: the kb4 finding that
"cross-file crate-internal" refs were largely mentions inside OTHER
dead code held across the whole census. Deleted as compiler-certified
dead: **the entire encrypted_sync bundle surface (1,047 lines — file
rewritten to its single live item `ensure_log_table`)**, the entire
security HTML-sanitizer surface (~225 lines — SsrfCheckResult/
redact_text live surface kept, its 5 audit50 tests died with it), 9
catalog dead enums + impls (orphaned by kb2's re-export deletions —
workspace.rs died whole), notification_prefs (whole file), 3
migrations/side_threads dead row structs, vectorstore dead trait
methods (search_sparse/count/snapshot/restore/delete + Filter builder
methods) + their 4 test-only-consumer tests, crypto dead scrypt
wrapper (derive_key/random_salt/SALT_LEN/SCRYPT_R/SCRYPT_P — its 4
tests REWRITTEN to fixed keys because they cover the LIVE
encrypt/decrypt), ai_provider trait contract trimmed, ~20 further dead
fns/consts/structs, and 7 write-never-read in-memory serde/struct
fields (RawToolCall.r#type is a serde-ignored wire field — wire
compatible). Net churn: 112 files, +542/−2,816. **Never-read fields
in DB-adjacent structs were NOT deleted** — those structs were
reverted to pub instead and the field list recorded as kb6's field-
surgery queue (SQL-mapping care per the kb2 precedent).

**KB-5-2 — three census blind spots found and closed, all
compiler-caught:** (1) `pub async fn` was outside the item regex (17
items); (2) mixed-name braced re-export lines narrow whole lines while
the census decides per name — 12 collateral names on 4 lines,
repaired by splitting the re-exports (search lanes, design controls,
helpscout inert, reports config); the `pub mod catalog {
pub(crate) use spp_catalog::*; }` inner GLOB re-narrowed twice by the
applier before being recognized as a live external surface (32
spp_core::catalog::X consumers) and restored; (3) types consumed only
as enum-variant payloads (events.rs OutboundJobFinishedEvent family —
zero name mentions, hard E0446-class errors) — reverted to pub. The
candidates TSV was destroyed after use so the applier can never
re-poison revert decisions again (it re-narrowed reverted lines
twice — process lesson recorded).

**KB-5-3 — NO-TOUCH violations self-inflicted and self-caught, all
restored byte-identical:** the applier re-narrowed `pub mod
response_state_sql` from its TSV row THREE separate times (after the
deleter had deleted one of its fns once — restored from HEAD); each
re-verified `git diff` = 0 lines vs HEAD. The module's 4 fns stay
KEEP per the owner's kb3 disposition; the incident is disclosed in
full. No other NO-TOUCH file was ever in the diff (db.rs,
maintenance.rs, ai_center.rs — verified at every check round).

**KB-5-4 — verification, all green:** fmt clean; cargo check
--workspace --all-targets 0 errors 0 warnings; clippy --workspace
--all-targets -D warnings 0; gate_traceability OK — identical 23
routes / 20 DC tags / 27 ids; gate_no_simulation clean; verify-icons
clean; core lib **467 passed / 0 failed / 3 ignored**
(--test-threads=1, 317.21s — census reconciles EXACTLY: 9
encrypted_sync + 5 audit50 + 4 vectorstore_sqlitevec + 3 it_lm_studio
tests died with their deleted surface = −20 passed −1 ignored against
kb4's 487/0/4); app-shell lib **241/0** (−1 = the deleted from_json
parser test; 212.14s); xtask 11/0; doc-tests 0/0; kittest batch A
13 suites/93 + batch B 12 targets/89 = **24 unique targets / 175
unique tests / 0 failed — 171 goldens ZERO drift** (dead surface
renders nothing). Test-count deltas census-reconciled to the item.

**KB-5-5 — residual recorded for kb6:** never-read fields in
DB-adjacent structs (RuleRecord name/priority/last_run_at/run_count,
RunRecord 6 fields, ConversationChunk email-adjacent fields,
AiRunLog run_id/model/prompt_version/latency_ms/created_at/sources,
jobs attempts/max_attempts, interaction_engine fields, webhook
event_id/processing_state, and their struct headers to re-narrow
after surgery); the kb6 batch = field-level deletions with
SQL-mapping care per the kb2 precedent, then re-narrow the reverted
struct headers. Environment notes: sandbox disk pressure recurred
(93% before batch B; kittest bins reclaimed between batches);
background builds still reaped (foreground-iterative pattern used
throughout). ai records synced.

## Batch KB-6 — never-read DB-adjacent field surgery + the A1-3b final re-narrows — opened and completed 2026-11-03

Owner directive at open: "continue" — the recorded KB-5-5 residual
(never-read fields in DB-adjacent structs + re-narrowing their held
headers) was the last open item in the engagement. Course: census
re-derived at HEAD with the compiler made authoritative, field-level
deletions with SQL-mapping care per the kb2 precedent, then the full
gate + test battery.

**KB-6-1 — census re-derived at HEAD; the loose labels reconciled.**
Of the KB-5-5 list: RuleRecord/RunRecord (automation.rs), ClaimedJob
(jobs.rs), WebhookEventRecord (webhook.rs) found exactly as recorded.
The labels "ConversationChunk email-adjacent fields" and "AiRunLog
run_id/model/prompt_version/latency_ms/created_at/sources" have NO
code counterpart at HEAD — no struct by either name exists in any of
the four crates, and -S archaeology shows the strings entered history
only through kb5's own ai/record diff (census display-name artifacts;
the corresponding embeddings-stats/ai-runs surfaces had already died
in kb2/kb3/kb5) — recorded as phantom labels and closed. The
"interaction_engine fields" label was resolved by the compiler rather
than by guessing. Method: the seven candidate struct headers
(RuleRecord, RunRecord, ClaimedJob, WebhookEventRecord,
StoredRecommendation, StoredCurrent, ValidationIssue) were narrowed
to pub(crate) — every one grep-verified zero external consumers
(app-shell/xtask/catalog) and zero serde paths; OverrideRow excluded
BECAUSE its parent ClientInteractionProfile is serde_json::to_value'd
LIVE at app-shell customers.rs:234 (kittest-rendered), SqliteVecStore
excluded (its single field conn is private and heavily read, and the
header must stay pub for the E0446 pub-interface constraint on
store_handle; kb5's widen was that constraint, not dead fields) —
then `cargo check` (lib-only, tests excluded) was made the census
authority: exactly 6 dead_code warnings fired, enumerating the dead
fields by name.

**KB-6-2 — the compiler-certified set, deleted with SQL-mapping
care.** RuleRecord: name/priority/last_run_at/run_count —
RULE_RECORD_SELECT re-shifted to 6 columns, RuleRow 10→6-tuple,
read_rule_row/parse_rule_record re-indexed; the SQL
`ORDER BY priority, id` stays (SQL-level ordering on the intact
schema column). RunRecord: ALL 6 fields → the struct died whole
(struct + impl read + RUN_SELECT + the per-rule readback block);
fire_trigger_for_conversation Result<Vec<RunRecord>>→Result<()> (both
production caller workers/execute.rs:638 and tests check is_ok only);
the automation_runs INSERT and the run_count/last_run_at UPDATE stay
byte-identical (DB-level behavior). ClaimedJob: attempts/max_attempts
— both claim SELECTs re-shifted to `SELECT id, type, payload,
attempt` (attempt kept: the claim UPDATE still writes attempt+1).
WebhookEventRecord: event_id/processing_state —
get_pending_webhook_events SELECT re-shifted to id/event_type/payload;
the WHERE processing_state/attempts filters stay (schema intact).
StoredRecommendation: prompt_version/model — the recommendation_json
parse now extracts only "recommendation". StoredCurrent:
analysis_version — SELECT re-shifted (recommendation_json re-indexed
6→5). Compiler silence certified the siblings live:
StoredCurrent.message_stats/sources/generated_at, ClaimedJob
id/kind/payload, WebhookEventRecord id/event_type/payload. Bonus
same-file deletions: ValidationIssue (zero uses anywhere incl.
tests — found during the census round) and the two EMPTY impl blocks
`impl ValidationIssue {}` / `impl RuleRecord {}` (zero content).
Total: 15 fields across 6 structs + RunRecord whole +
ValidationIssue whole + 2 empty impls; net +27/−144 across exactly 4
files (automation.rs, interaction_engine/mod.rs, jobs.rs,
webhook.rs). The six surviving headers are now pub(crate) — the
A1-3b direction completed for the last set kb5 had held back
precisely because of these fields.

**KB-6-3 — test coverage preserved.** Exactly one test touched:
batch19's ui_created_legacy_rules_parse_instead_of_poisoning_the_
engine read RuleRecord.name as a lookup handle (test-only read; the
standing census convention: test-only reads do not keep surface
alive — kb5's test-only-observer precedent). Rewritten to assert
records.len()==2 (both rule generations parse — a failed legacy parse
would skip the record and drop the count) plus
any(trigger=="tag_added") (the legacy M007 enum maps to its kind
name). The live coverage — parse survival + trigger mapping — is
fully preserved. Core lib census reconciles EXACTLY: 467/0/3 equals
kb5's baseline, zero tests died (no test exercised any deleted
field).

**KB-6-4 — verification, all green.** fmt clean (applied +
re-checked); cargo check --workspace --all-targets 0 errors 0
warnings; clippy --workspace --all-targets -- -D warnings 0;
gate_traceability OK — identical 23 routes / 20 DC tags / 27 ids;
gate_no_simulation clean; verify-icons clean; core lib 467 passed /
0 failed / 3 ignored (--test-threads=1, 318.82s — identical
baseline); app-shell lib 241/0 (229.36s — identical); xtask 11/0;
doc-tests 0/0; kittest batch A 13 suites/93 + batch B 11 targets/82
= 24 unique targets / 175 unique tests / 0 failed — 171 goldens ZERO
drift (UPDATE_SNAPSHOTS never set; this batch's split re-derived
alphabetically: 13+11 with NO overlap run — cleaner than kb3-5's
13/12-with-double-run accounting, same 175 unique). Migrations SQL
semantics untouched (no M-code, no schema, no executed migration in
any diff; only runtime SELECT column lists re-shifted per the kb2
precedent); NO-TOUCH modules (ai_center/db/maintenance/
response_state_sql) never in the diff; no allow() stamps.

**KB-6-5 — engagement state: no open items remain.** The KB-5-5
residual is closed; response_state_sql's 3 fns stay KEEP (owner
disposition, kb3 — nothing to execute); the phantom labels are
closed as census artifacts; plan.md's RF-1..RF-11 + FINISH + KB-1
were already closed; kb2..kb6 residuals are all consumed. The
ledger's Items sections hold nothing open. Environment: cold build
(target/ was cleaned at kb5 end; core check 73s warm-up then
steady-state); disk peaked 96% after the kittest feature build and
the recorded reclamation recipe applied post-run (test bins deleted,
back to 47%); foreground-iterative runs throughout (background
builds still reaped by the sandbox); CARGO_PROFILE_DEV_DEBUG=0 +
CARGO_BUILD_JOBS=2 per the kb2 recipe. ai records synced.

## KB-7 — independent closeout audit (fresh census at HEAD, compiler as authority)

**KB-7-1 — the census.** A fresh name-method census was derived at HEAD
(e5baba2) with two corpus corrections over kb3's methodology: external
consumers now include the integration tests (`crates/*/tests/` are
independent crates) and the bin target (`src/main.rs` consumes the lib pub
face — the bin sources are excluded from the internal corpus and counted as
consumers). Result: 229 candidates (candidates.tsv), of which 2 are
disposition-excluded by the standing rules (`response_state_sql` mod decl in
core lib.rs and M014_SQL in migrations/intelligence.rs — the kb3 NO-TOUCH
exclusions) → 227 applications recorded (applied.tsv).

**KB-7-2 — convergence.** The compiler was made authoritative: the narrow →
check → revert-signature-constraints → re-check loop ran until
`private_interfaces` warnings were zero (3 rounds: 91 names → 41 → 5; 138
items reverted to pub as signature-constrained and recorded in reverted.tsv —
the layered-constraint mechanism from kb5: a pub struct's pub fields and
enum-variant payloads constrain their field types back to pub). The dead_code
cascade then ran rounds 4–13, each round deleting compiler-enumerated dead
surface and re-checking until only two `unused_variables` (`id` bindings left
over from an issue_radar SELECT re-shift) remained; fixed; final check r14 =
0 errors, 0 warnings across the workspace, all targets, zero diagnostics.

**KB-7-3 — what shipped (43 files, +226/−1980).** 42 surviving pub→pub(crate)
narrows (mockserver 12, testsupport 5, ai_pipeline 4, interaction_signals 3,
issue_radar/ai_prompts/interaction_engine/webhook 2 each, and singles);
146 pub-decl lines removed as compiler-certified dead surface (~127 whole
items incl. 4 whole test mods): custom_objects patch_type/patch_object/
get_object/list_links/link_label, people_store NewCustomer/CustomerPatch/
create_customer/update_customer/patch_field_present, customers
CustomerConversation/get_customer/search_customers, jobs status module +
QueueJob/list_jobs/get_job/map_queue_job, security validate_config,
settings secret_or_error, support_graph HumanEdge/to_json/GRAPH_MAX_*.
/subgraph_json, reports count_graph_nodes_by_kind, reports_api
cached_report_keys, translation detect, connectors redacted_auth/redacted/
validate_config, ai_attributes MatchOp whole, automation SqlValue, copilot
ToolResult, mentions Mention, inbox ThreadType, intelligence_features
IncidentPatch, digest digest_by_id, helpscout revoke + has_stored_tokens,
conversation_ops ConvRef, webhook compute_signature+pending_event_count
retained as #[cfg(test)] helpers (their only consumers were tests — same
disposition as app-shell's build_sync_engine and the workers_host method:
test-only helpers gated, not deleted, preserving their coverage),
ai_pipeline dead-field surgery (kb6 pattern: CachedRun id/created_at,
LatestAnalysis run_id/model/prompt_version/latency_ms/created_at/sources
with the ai_sources readback query deleted, AnalyzeOutcome
sources/cached/run_id — SELECTs re-shifted to response_json only),
conversation_ops conv_by_local_id → conv_exists (SELECT 1; callers only
tested existence), AttachmentRow dead fields, interaction_engine
CustomerConversation.thread_count, outbound OutboundJob.max_attempts,
issue_radar ClusterRow id/category/conversation_ids + KnownIssueRow.id,
automation page ApprovalRow.status + delete_rule_confirmed, app-shell
reports sla_first_response_bars, connectors page health_color.

**KB-7-4 — test surgery, coverage preserved.** 16 tests died WITH their
compiler-certified dead subjects (core −15: custom_objects ×3,
people_store ×3, customers ×2, jobs list_jobs_binds…, provider_handle
setting_kind_round_trips (from_setting was a test-only observer of a dead
mapping; ProviderKind itself stays live and covered), security
audit51_connector_validation…, settings secret_or_error_reports…,
support_graph subgraph_renders…, vectorstore
delete_by_entity_removes_both_halves, it_helpscout
revoke_clears_the_token…; app-shell −1: health_badge_colors_map — every
one's subject verified dead, every live sibling (create_type/create_object,
ProviderKind, count_by_entity_type/search_dense, enqueue) retains its
remaining tests). Rewrites preserved live coverage where the readback
handle died: digest.rs's digest_by_id replaced by a #[cfg(test)]
digest_row helper in ai_pipeline (4 tests keep their assertions), jobs
batch20 get_job/list_jobs probes re-expressed as direct SQL, reports_api
cached_report_keys probe → COUNT(*), support_graph/people_store probes
likewise, helpscout r7 has_stored_tokens probes → COUNT(oauth_tokens),
the batch23 cache-hit test re-expresses run_id/sources provenance through
ai_runs/ai_sources SQL, and the two dropped `!outcome.cached` assertions
are redundant with the surviving audit96 empty-analysis tests (which
assert non-caching via run-status DB checks). The issue_radar tests lost
only their unused `id` bindings.

**KB-7-5 — verification, all green.** fmt clean; check --workspace
--all-targets 0 errors 0 warnings (r14, zero diagnostics); clippy
--workspace --all-targets -D warnings 0; gate_traceability OK — identical
23 routes / 20 DC tags / 27 ids; gate_no_simulation clean; verify-icons
clean; core lib 452 passed / 0 failed / 3 ignored (--test-threads=1 —
467−15 exactly the enumerated dead-subject tests, census reconciles);
app-shell lib 240/0 (241−1 health_badge_colors_map); xtask 11/0;
doc-tests 0/0; kittest 24 unique targets / 175 unique tests / 0 failed —
171 goldens ZERO drift (UPDATE_SNAPSHOTS never set; zero .new/.diff/.old
artifacts; alphabetical 13+11 split as kb6). NO-TOUCH modules
(ai_center/db/maintenance/response_state_sql/migrations-intelligence)
absent from the diff (git-verified); no allow() stamps; no CREATE/ALTER
TABLE or M-code in any diff (schema + migration semantics untouched);
the `#[must_use]` terminal display artifact re-confirmed as rendering-only
(od/rg verify true `#[must_use]` bytes in the diff; files never corrupted).

**KB-7-6 — environment incidents, disclosed.** (1) Two disk-full events
(100%): the first poisoned a core-lib test run — 35 failures, ALL
ENOSPC artifacts (tempdir SQLite writes), the immediate re-run at free
disk passed 452/0/3; the second broke the first kittest feature build
(incremental-dir creation + the kb2-known linker bus-error signature).
Reclamation per the kb2 recipe (incremental dir + stale bins deleted)
between phases. (2) This sandbox lacked the Mesa lavapipe env: the first
direct kittest runs all panicked "No adapter found" — resolved by
sourcing the provisioned /home/z/mesa-env.sh (VK_DRIVER_FILES/
VK_ICD_FILENAMES/LD_LIBRARY_PATH/WGPU_BACKEND), after which rendering
works. (3) Direct test-bin execution (forced by disk pressure) hit two
recipe gaps, both self-caught: egui_kittest resolves the snapshot dir
CWD-relative, so bins must run from crates/app-shell (the first
workspace-root runs wrote .new.png files into an UNTRACKED root-level
tests/ dir — pollution deleted, goldens untouched, re-runs green); and
CARGO_INCREMENTAL=0 changes local-crate metadata hashes (full recompile
of the workspace in a new hash space) — reverted to the default env and
the foreign hash-space artifacts removed. Net effect on the tree: none
(git status clean of untracked files apart from none; goldens
byte-identical).

**KB-7-7 — engagement state.** The A1-3b program is complete end-to-end:
kb3's census (1,134) → kb4's tranche (447) → kb5's completion (649 +
compiler-certified deletions) → kb6's field surgery → kb7's independent
re-derivation at HEAD finds zero surviving live candidates: every item
either narrowed (42), deleted as compiler-certified dead (~127), or
reverted as signature-constrained (138, structural). The remaining pub
face is, by compiler proof, exactly the externally-consumed surface.
ai records synced.

## Batch REL-1 — release v1.9.2 from the final source (items REL-1-1..REL-1-6) — opened 2026-11-03

Owner-directed Build & Release work order on the closed engagement (plan.md
carries the direction and the pipeline description; the ship-list is
release.yml's, already proven on v1.9.0/v1.9.1).

1. **REL-1-1 (release prep)** — workspace version 1.9.1 → 1.9.2 (Cargo.toml +
   Cargo.lock sync), CHANGELOG `[1.9.2]` section (product language: the two
   KB-1 fixes; internal hardening summarized; no feature/behavior change),
   ai records Rule-2 entries (this section + plan.md opener + decisions.md
   D8).
2. **REL-1-2 (gates on the release tree)** — the release.yml gate set run
   locally at the exact tagged content: fmt --all --check; check
   --workspace --all-targets (zero diagnostics); clippy --locked -D
   warnings (workspace + core/sandbox feature); test --locked --workspace
   (single-threaded in-sandbox per the 3 GB recipe); gate_no_simulation;
   gate_traceability; verify-icons.
3. **REL-1-3 (local packaging)** — cargo build --locked --release -p
   supportos-plus-app-shell --features supportos-plus-core/sandbox; xtask
   package --skip-build (AppImage; appimagetool 1.9.1 pinned + SHA-256);
   xtask deb --skip-build; dist staging (versioned names + SHA256SUMS.txt +
   BUILD-INFO.txt commit-pinned) exactly as release.yml's build job does.
4. **REL-1-4 (local verification, no-root scope — honest split)** —
   runnable without root, mirrored from release.yml's verify lanes:
   .deb — dpkg -I metadata lint (Depends/Section/Description), apt-get
   install -s dependency-resolution simulation, dpkg -x payload extraction,
   SPP_SMOKE=1 boot chain from the extracted tree, Xvfb+lavapipe GUI launch
   (rc==124 alive-at-timeout = window created, render loop running);
   AppImage — chmod +x, APPIMAGE_EXTRACT_AND_RUN=1 SPP_SMOKE=1, Xvfb GUI
   launch. The root-requiring lanes (real APT install with automatic
   dependency fetch, desktop-file-validate, reinstall, uninstall data
   retention, purge, the 0.2.x upgrade migration) run in CI's verify-deb
   matrix (ubuntu:22.04/24.04, debian:12) — the same lanes that verified
   v1.9.0/v1.9.1; locally blocked by the sudo password (disclosed, not
   skipped).
5. **REL-1-5 (publish)** — single commit (release prep + ledger) + annotated
   tag v1.9.2; push blocked pending credentials (~/.git-credentials lost in
   the environment reset, decisions.md D7) — every pre-push step completes;
   the push executes the moment the owner re-supplies the token; then CI
   (incl. the kittest lavapipe lane) + the Release workflow are watched to
   completion (scrape-verified: CI run conclusion + release assets + live
   link checksums, as the workflow's own final step also asserts).
6. **REL-1-6 (records + STOP)** — completed.md release record (evidence
   table: gates, package paths/sizes/SHA-256, local verify results, CI +
   release links); Rule 8 STOP.

**REL-1 execution evidence (2026-11-03, local).** REL-1-1 done: workspace
1.9.2 (Cargo.toml + Cargo.lock, all four members), CHANGELOG `[1.9.2]`,
plan/progress/decisions entries. REL-1-2 done — the release.yml gate set on
the exact release content: fmt clean; check --workspace --all-targets zero
diagnostics (2m55s); clippy --locked -D warnings workspace + core/sandbox
both clean; tests --locked: core 452/0/3 (--test-threads=1 — byte-identical
to the kb7 record), app-shell 240/0, xtask 11/0, core+sandbox 460/0/3,
doc-tests 0/0; gate_no_simulation clean; gate_traceability OK 23/20/27;
verify-icons clean. REL-1-3 done (rehearsal, disclosed): the release-profile
LTO link OOMs in this 4 GB no-swap sandbox (SIGKILL inside rustc LTO —
environmental, the RF-era reason release packaging is CI's job), so the
rehearsal built at CARGO_PROFILE_RELEASE_LTO=off and BOTH packages were
produced from the locked tree by the unmodified xtask: AppImage 15,292,920 B
(appimagetool 1.9.1, SHA-pinned, verified) and .deb 10,917,076 B; dist
staged with versioned names + SHA256SUMS.txt + BUILD-INFO.txt. Rehearsal
host-lib disclosure: libxkbcommon0/libxkbcommon-x11-0/libxcb-xkb1 (and
xauth/libxmuu1 for xvfb-run) were fetched via `apt-get download` + `dpkg -x`
(genuine distro packages, extracted uninstalled) with an `ldconfig -p` PATH
shim surfacing them — CI's build job installs these packages for real.
REL-1-4 done (every lane runnable without root, mirrored from release.yml):
dpkg -I metadata correct (Version 1.9.2, full Depends stack, Homepage,
Description); `apt-get install -s ./…deb` resolves the package and its
dependency set cleanly; payload = usr/bin + .desktop + 4 hicolor icons;
ldd fully resolved; SPP_SMOKE=1 EXIT=0 from the extracted tree — 53
migrations, boot services, background workers, automatic backup, loopback
listener (OS-assigned-port fallback exercised), first-run init (DB +
0600 secrets.key + instance.lock), all 24 pages rendered headlessly (4,220
shapes); GUI launch under Xvfb 1280×800 + lavapipe rc=124 (window created,
render loop alive at the 25 s TERM — the CI success criterion);
AppImage APPIMAGE_EXTRACT_AND_RUN=1 SPP_SMOKE=1 EXIT=0 (same 24 pages/4,220
shapes) and Xvfb+lavapipe GUI rc=124; sha256sum -c SHA256SUMS.txt OK for
both. The root-requiring lanes (real APT install with dependency fetch,
desktop-file-validate, reinstall, uninstall data-retention, purge, the
0.2.x upgrade migration) remain CI's verify-deb matrix — sudo needs a
password here; disclosed, not skipped. REL-1-5 done (2026-11-03): the
owner re-supplied the token; pushed main 88830e4→a53273d and the annotated
tag v1.9.2 (0b572c9); Release run 41731702400 completed success in 32.1
min — 6/6 jobs green (build+test+package on ubuntu:22.04; verify .deb on
ubuntu:22.04/24.04/debian:12; verify AppImage; publish); SupportOS+ 1.9.2
published (draft=false, prerelease=false, target a53273d) with 4 assets —
AppImage 12,777,976 B (sha256 a4dc96a1…67726a79), .deb 10,165,298 B (sha256
7ee1e689…bb08923d), SHA256SUMS.txt, BUILD-INFO.txt (commit a53273d = tag
target = main HEAD); all four downloaded live from the published URLs,
sha256sum -c OK; CI #212 on a53273d success. REL-1-6 done: Batch REL-1
recorded in completed.md with the full evidence table; no open items
remain anywhere in the ledger; Rule 8 STOP.

## Batch AU-1 — code-truth audit (owner-directed, 2026-11-03) — items AU-1-1..AU-1-10, moved from plan.md per Rule 2

Owner directive: program code only is the source of truth; tests and documentation are
not evidence; find every issue, bug, blocker, dead code, orphan, broken/unwired feature,
everything not done and due; repo-files only (pushed to GitHub); follow rule.md; no
answers from memory. Evidence rules + item definitions live in the plan.md batch opener
(binding for this batch). Items:

1. **AU-1-1 (baseline)** — HEAD pinned `49247d5`; corpus = app-shell + core + catalog
   non-test sources (+ program build config); xtask out of corpus (marker scan only).
2. **AU-1-2 (incompleteness markers)** — todo!/unimplemented!/panic!/TODO/FIXME/XXX/
   HACK/placeholder/cfg-out scan across the corpus, each hit classified.
3. **AU-1-3 (program-only dead-surface census)** — pub face vs non-test program
   consumers; test-only-referenced = dead-in-program.
4. **AU-1-4 (UI wiring audit)** — all 24 pages + palette + overlays + toasts: control →
   handler → core effect; find inert controls and never-persisted state.
5. **AU-1-5 (services wiring audit)** — workers/sync/backup/webhook/AI pipeline/
   crypto/sandbox/single-instance/version-check: started? consumed? honored?
6. **AU-1-6 (data-layer orphans)** — written-never-read / read-never-written columns,
   orphaned tables, one-way settings keys (program code only).
7. **AU-1-7 (logic bug hunt: core)** — SQL mismatch, inverted logic, off-by-one, units,
   swallowed Results, races.
8. **AU-1-8 (logic bug hunt: app-shell)** — latch invalidation, selection desync,
   routing, theme/zoom, shortcuts, stranded error paths.
9. **AU-1-9 (adjudication)** — re-verify every candidate at HEAD (file:line, program
   code only); discard false positives with reasons; confirmed findings → plan.md items.
10. **AU-1-10 (records + STOP)** — completed.md record + ledger sync + Rule 8 STOP.

**AU-1 execution evidence (2026-11-03, HEAD 49247d5).** AU-1-1 done: corpus = 166
non-test program files (core+app-shell+catalog); bench/it_*/mockserver/testsupport
confirmed #[cfg(test)]-gated in lib.rs; xtask out of corpus. AU-1-2 done: zero
todo!/unimplemented!/TODO/FIXME/XXX/HACK/"not implemented" markers in program code;
4 unreachable! all structurally guarded (2 invariants, 1 test-file hit, 1 redundant
defensive arm after an exhaustive outer match); no dead_code allow stamps; cfg features
ui-screenshot/kittest-ui are tooling gates. AU-1-3 done (mechanical census, strip-first
methodology): 1,874 pub items / 1,658 names; 21 dead-in-program items (0 non-test
references) + 1 name-collision catch (migrations::find inflated by Iterator::find);
4 import-only hits adjudicated as census artifacts (braced imports used by short name) —
discarded. AU-1-4/5/7/8 done: four independent deep-read passes (UI wiring, services
wiring, core logic, app-shell logic), every reported finding re-verified by me at
file:line before acceptance; two agent-vs-agent conflicts adjudicated (automation_
approvals INSERT sites are #[test]-gated → approval-loop severance CONFIRMED; my own
census-led translation_cache blocker claim OVERRULED — migrations/db_breadth.rs:327
creates the table at boot (m040) → downgraded to dead-redundant-creator, recorded in
completed.md). AU-1-9 done: 25 confirmed findings + 21 dead items + 16 orphaned tables +
6 low-confidence hygiene notes written into plan.md as Batch AU-2 items 1..15. AU-1-10
done: completed.md Batch AU-1 record; decisions.md D9 (audit evidence policy). Rule 8
STOP.

## Batch REL-2 — release v1.9.3 from the final source (items REL-2-1..REL-2-5) — opened 2026-11-04

Owner-directed Build & Release work order (2026-11-04, chat): "Build, test, and
publish install-ready Linux `.deb` and `.AppImage` packages from the final
repository source … Automate builds and releases through GitHub Actions where
possible … Verification: test both packages from clean environments … Release
integrity: publish only successfully built and tested artifacts … Do not claim
a package is complete or ready for release if it has not been built and
verified. Document any genuine environmental or permission blocker and
continue."

Final source = `origin/main` `4a8db7e` (CI run #224 success): it carries Batch
AU-2 wave 1 (`fa57955`) + wave 2 (`2833686`) + the relocked kittest baselines
(`4a8db7e`), none of which are in any published release — v1.9.2 shipped
`a53273d`, 11 commits earlier. The ship-list is release.yml's, proven on
v1.9.0/v1.9.1/v1.9.2 (Release runs 29/30/31, all success).

1. **REL-2-1 (release prep)** — workspace version 1.9.2 → 1.9.3 (Cargo.toml +
   Cargo.lock, all four members); CHANGELOG `[1.9.3]` section (product
   language: the AU-2 fixes, dead-surface removal, manual-only version check);
   decisions.md D12 (version policy); this ledger entry (Rule 2 move).
2. **REL-2-2 (gates on the release tree)** — the release.yml gate set run
   locally at the exact tagged content: fmt --all --check; check
   --workspace --all-targets (zero diagnostics); clippy --locked -D warnings
   (workspace + core/sandbox feature); test --locked --workspace + core
   sandbox (--test-threads=1, the 4 GB recipe); gate_no_simulation;
   gate_traceability; verify-icons.
3. **REL-2-3 (publish)** — single commit (release prep + ledger) + annotated
   tag `v1.9.3`; push main + tag. The unmodified release.yml then: builds in
   an ubuntu:22.04 container (glibc 2.35 baseline) with the full gate set,
   packages via `xtask package`/`xtask deb`, verifies the .deb on clean
   ubuntu:22.04/24.04 + debian:12 containers (APT dependency resolution,
   desktop entry + icons, SPP_SMOKE boot chain, 0.2.x upgrade migration,
   Xvfb GUI launch on declared deps only, reinstall, uninstall with user-data
   retention, purge), verifies the AppImage (smoke + Xvfb GUI launch), and
   publishes the GitHub Release only after every lane is green.
4. **REL-2-4 (post-publish verification)** — scrape-verify the Release run
   (6/6 jobs success) and the published release: 4 assets present,
   browser_download_url live-checked, sha256 re-verified from downloaded
   bytes, BUILD-INFO commit == the tag's commit, version 1.9.3, notes carry
   changelog + install + compatibility statements.
5. **REL-2-5 (records + STOP)** — completed.md release record (evidence
   table: gates, asset sizes/SHA-256, run + release links); progress.md sync;
   Rule 8 STOP.

Standing environmental disclosure (REL-1/D8, unchanged): this 4 GB no-swap
no-root sandbox cannot run the root-requiring verify lanes (real APT install,
desktop-file-validate, reinstall/uninstall/purge) or the release-profile LTO
link; those lanes run in CI's clean containers — the authoritative lane that
published v1.9.0–v1.9.2. No blocker to the release itself.


