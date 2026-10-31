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
