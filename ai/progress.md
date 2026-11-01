# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items

**Batch RS-4 (items RS-4-1..RS-4-10)** — active, moved from plan.md per Rule 2
(2026-10-30). Source: the owner's G4 rejection; root-cause reproduction and
disposition in the G4-verdict addendum (completed.md, 2026-10-30).

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

