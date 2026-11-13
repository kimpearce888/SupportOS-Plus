# Refactor Progress — Live State

Last updated: session 5 (2026-11-14). Update rule: every session updates
this file before ending. Statuses: `pending` / `in_progress` / `done` /
`terminal` (done + closed in `complete.md`) / `dropped` (with reason in
`memory.md`).

## CURRENT POINTER

- Next task: **R2.4-b — class-C post-condition expects** (17 guarded
  sites; eliminate where a natural Option-flow rewrite exists, keep the
  provably-unreachable restructures; policy D-14 class C). R2.4-a
  (mutex-poison elimination + class-A normalization) is DONE this
  session — see task table + session log. Then R2.5 (panic!/todo!/
  unimplemented!, 30 instances + the 3 unreachable! arms noted in the
  session-4 survey).
- Pre-flight: confirm CI green on the session-5 final head (run id in
  session log). If the session ended mid-CI, the run may still be in
  flight — next session must wait for it.
- Session counter: 5 completed (R1 terminal; R2.1-R2.3 terminal; R2.4
  batch a done, batch b pending).
- Environment: toolchain reinstalled this session (cold target/); G-05
  managed preemptively (incremental rm between lanes; peak 93%); ONE
  lane per tool call. Triage tooling: see G-08 — cross-line string
  state machine required, scanner at tool-results/r24_triage.py.

## Task states

| Task | Status | Notes |
|---|---|---|
| R0 pipeline alignment | terminal (session 1) | CI run#301 green — see `complete.md` |
| R1.0 strip-comments tool | done (session 2) | lexer + 38 edge tests + --check mode; full battery green; commit 88e1770 |
| R1.1 catalog+xtask strip | done (session 2) | 59 comment lines removed (catalog was already comment-free: 0 of 8 files); subset --check clean; CI run#303 green |
| R1.2 core/ai_* strip | done (session 2) | 90 comment lines removed across 7 of 15 files; cumulative subset check clean (28/28); battery green; CI run#304 green |
| R1.3 core/sync+workers strip | done (session 2) | 162 whole lines + 1 trailing comment removed from 8 of 13 files; cumulative subset check clean (41/41); battery green (per-crate lanes); CI run#305 green |
| R1.4 core/helpscout strip | done (session 2) | 26 lines removed from 3 of 9 files; cumulative subset check clean (50/50); battery green (all 4 test lanes); CI run#306 green |
| R1.5 core/storage+config strip | done (session 2) | 54 lines removed from 6 of 23 files; audit.rs invariant-7 duty verified (allowlist lines untouched, gate clean); cumulative subset check clean (73/73); battery green; CI run#307 green |
| R1.6 core/domain strip | done (session 3) | 479 whole lines + 1 inline comment removed from 26 of 122 files — core crate now 100% comment-free (135/135 cumulative); `core/tests/` in the plan is vacuous (tests are inline `#[cfg(test)]`); battery green (app-shell lane needed one G-05 disk retry); CI run#308 green |
| R1.7 app-shell/infra strip | done (session 3) | 429 whole lines + 2 inline comments removed from 11 of 19 files; G-07 hit twice (comments had masked let_and_return in design/mod.rs + collapsible_if in workers_host.rs — fixed by behavior-identical restructure); cumulative subset check clean (154/154); battery green; CI run#309 green |
| R1.8 app-shell/pages-A strip | done (session 3) | 864 whole lines + 1 inline comment removed from 7 of 7 pages (inbox 226, knowledge 150, outreach 157, custom_objects 132, settings 114, ai_center 55, reports 51); G-07 hit twice more (collapsible_if in custom_objects.rs + outreach.rs empty-state CTAs — same restructure fix); cumulative check 161/161 clean over stripped scopes; battery green; CI run#310 green |
| R1.9 app-shell/pages-B strip | done (session 3) | 1011 whole lines + 1 inline comment removed from 44 of 51 scanned (16 pages + mod.rs + queries.rs + json_fields.rs + all 26 tests files); NO G-07 hits; whole-tree check crates 205/205 clean — the entire Rust source tree is comment-free; battery green (app-shell lane re-linked all 26 stripped kittest binaries); CI run#311 green |
| R1.10 comment closeout | terminal (session 4) | redundant warn+allow missing_docs pairs deleted from catalog/core lib.rs (12 attr lines, 4 insertions after fmt); crate lint headers now match app-shell's clean form; whole-tree check crates 205/205 green; battery green; CI run#313 green — see complete.md |
| R2.1 dead-code sweep | terminal (session 4) | ZERO active warnings (clippy -D warnings already green); 2 vestigial suppressions deleted (testsupport.rs allow(dead_code) — module already cfg(test)-gated, all helpers used; issue_radar.rs allow(unused_mut) — nav mutated at 8 sites); battery green; CI run#315 green — see complete.md |
| R2.2 #[allow] audit | terminal (session 4) | 37 instances audited (28 in-src + 9 in-test): verdict table in progress.md, survivor policy memory D-13; 1 vestigial deleted (search/lanes.rs ref_option — signature already Option<&str>); 34 kept with written justifications (FFI islands, constraint-3-unsatisfiable style pairs, R3.4/R4.5-owned clusters, D-10 test regions); battery green; CI run#317 green — see complete.md |
| R2.3 dependency hygiene | terminal (session 4) | machete v0.9.2: 4 candidates = 1 real unused (scrypt — zero source occurrences; removed from workspace+core manifests; lock dropped exactly scrypt+salsa20+pbkdf2, 747 bytes) + 3 false positives (custom lib names spp_core/spp_catalog defeat machete's string matcher); cargo-deny v0.20.2: advisories/bans/licenses/sources all ok; battery green; CI run#319 green — see complete.md |
| R2.4 unwrap/expect policy | in_progress (session 5) | batch a DONE: D-14 policy written (G-08 census 86 prod = A45/B21/C17/D3); B-class 21 mutex-poison ELIMINATED (into_inner recovery; 5 inert sites collapsed into lock_world()); A-class 20 unwraps normalized to expect("static regex"); prod 86→65, unwrap 24→4; battery green; CI run#322 green (id 43391137472) — batch b (class C, 17 sites) next |
| R2.5 panic!/todo!/unimplemented! | pending | 30 instances + 3 unreachable! arms (session-4 survey); same D-14-shaped policy expected |
| R3.1–R3.6 DRY | pending | see plan §7 R3 |
| R4.1–R4.5 clean code | pending | see plan §7 R4 |
| R5.1–R5.3 guardrails | pending (optional, stage-gated) | |
| R6.1–R6.2 polish + close | pending | |

## Review verdict tables (R2–R4 passes)

*(format: `module → construct → verdict(keep/merge/delete/rename) → outcome`)*

### R2.2 — `#[allow]` audit (session 4; 25 in-src + 9 in-test instances after R2.1's deletions)

| module | construct | verdict | outcome |
|---|---|---|---|
| core/search/lanes.rs | `#[allow(clippy::ref_option)]` on search_knowledge_raw | delete | VESTIGIAL — signature already `Option<&str>` (the recommended form); allow outlived an earlier signature; deleted, clippy green without it |
| core/vectorstore_sqlitevec.rs | `#![allow(unsafe_code)]` (crate-level for this module) | keep | sqlite-vec C-API FFI — the one confined unsafe island in an otherwise `deny(unsafe_code)` crate; D-13 |
| core/sandbox.rs:153 | `#![allow(unsafe_code)]` inside `#[cfg(test)] mod tests` | keep | test-region FFI (sandbox feature); D-10 exempts test regions; D-13 |
| app-shell/src/lib.rs:14 | `#[allow(unsafe_code)]` gating `pub mod single_instance` | keep | OS single-instance lock FFI; module-scoped allow over crate-level deny; D-13 |
| app-shell/catalog/core lib.rs (x3) | `#![allow(module_name_repetitions, missing_errors_doc)]` crate-level pairs | keep | missing_errors_doc unsatisfiable under constraint 3 (remedy is doc comments — deleted); module_name_repetitions = idiomatic self-named modules; D-13 |
| core (event_bus, ai_pipeline, connectors x2, copilot, events, interaction_engine x4, jobs x2, outreach, quality) + app-shell/testsupport | 15 `#[allow(clippy::too_many_arguments)]` | keep | parameter-object consolidation is R3.4's DRY decision (ad-hoc structs now would be re-merged by R3); testsupport's seed_conversation rides the test-support exemption; D-13 |
| app-shell/pages/inbox.rs x3 + app-shell/design/mod.rs | 4 `#[allow(clippy::too_many_lines)]` | keep | F-04/R4.5 split candidates; the split is R4.5's deliberate batch (needs CI golden verification; kittest is CI-only F-06); D-13 |
| app-shell/tests (inbox_copilot x2, list_pages x2, inbox_page x2, operations, reports, dashboard) | 9 `#[allow(clippy::too_many_arguments)]` on kittest fixture/seed constructors | keep | test region (D-10); fixture constructors read better with explicit seeds than nested struct literals; D-13 |

**R2.2 verdict summary (whole audit incl. R2.1's findings):** 37 instances received verdicts (28 in-src + 9 in-test at audit start) — 3 DELETED (all vestigial: dead_code, unused_mut in R2.1; ref_option in R2.2), 34 KEPT, every keep carrying an active written justification in memory D-13. No keep is unjustified; no delete was load-bearing.

### R2.3 — dependency hygiene (session 4)

| module | construct | verdict | outcome |
|---|---|---|---|
| core/Cargo.toml + workspace root | `scrypt = "0.12"` | delete | UNUSED — zero `scrypt`/`Scrypt` occurrences across all .rs files (the KDF stack is aes-gcm + sha2); removed from both manifests; Cargo.lock dropped exactly scrypt + salsa20 + pbkdf2 (its exclusive transitive deps; 747 bytes, no other change) |
| xtask/app-shell → supportos-plus-core; core → supportos-plus-catalog | path dependencies | keep (machete false positives) | machete is string-based; the custom lib names (spp_core, spp_catalog) defeat its matcher — xtask uses `spp_core::` 14×, app-shell 56+×, catalog is re-exported via core's `pub mod catalog { pub use spp_catalog::*; }` |

Checks: cargo machete (v0.9.2, installed ad hoc) now reports only the 3 documented false positives; cargo deny (v0.20.2, installed ad hoc) — advisories ok, bans ok, licenses ok, sources ok.

### R2.4 — `unwrap`/`expect` policy (session 5; precise census 86 production occurrences)

| class | sites | verdict | outcome |
|---|---|---|---|
| B — mutex/rwlock poison (vectorstore_sqlitevec 12, helpscout/inert 5, settings 4) | 21 | eliminate | `.lock().expect(...)` → `.lock().unwrap_or_else(\|p\| p.into_inner())` (std poison recovery; locks guard plain data, no poison-corruptible invariants); inert's 5 sites were inline duplicates of the existing `lock_world()` recovery helper — collapsed into it; happy path behavior-identical, the panic class is gone |
| A — constant-input constructors (static Regex::new 40, seccomp const-args 4, chrono midnight 1) | 45 | keep as expect | input is compile-time constant — failure unrepresentable; Result propagation = ~45 signature churn colliding with R3 (session-4 note); the 20 message-less unwraps normalized to `.expect("static regex")` / `.expect("midnight is always a valid time")` (error-path-only; constraint 3 leaves the expect message as the only in-code doc a survivor may carry) |
| C — guarded post-conditions (live_chats 2, embeddings 3, ai_pipeline 3, notification_sweep 1, support_graph 1, workers/execute 1, coaching 2, quality/mod 1, inert 1, webhook 1, settings 1) | 17 | R2.4-b | each guarded by an explicit same-function check; eliminate where a natural Option-flow rewrite exists, keep where elimination restructures control flow for a provably-unreachable branch (R2.5's unreachable-arm family) |
| D — runtime/bootstrap infra (workers/mod tokio runtime, task_bridge thread spawn, design font family) | 3 | keep | one-time infra construction, no recovery path; bootstrap-equivalent (plan exempts main.rs bootstrap — same nature outside main.rs) |
| xtask + bench machinery | 43 + bench.rs | exempt | dev-invoked build machinery; bench.rs is cfg(test)-gated at declaration — D-10 exemption |

**R2.4-a verdict summary:** 86 production occurrences triaged by the precise (G-08) scanner — 21 ELIMINATED (class B), 20 normalized unwrap→expect (class A), 45 kept-under-D-14 (A + D), 17 deferred to R2.4-b (class C). Net tree after batch a: 65 production expects + 4 guarded unwraps; zero mutex-poison panics. Q-03 records the plan-letter deviation for owner override.

## Session log

### Session 5 — 2026-11-14
- Pre-flight: sandbox reset AGAIN (clone + toolchain gone) — re-provisioned
  per handover §2. Repo state was AHEAD of the session-4 summary (the
  previous context ended mid-summary): R2.2 terminal (f06e1ba, run#317),
  R2.3 terminal (981f2a0, run#319), and an R2.4 triage survey committed
  on head adbb390 (run#321 green — verified via API before work began;
  run#320 on the intermediate ledger head was cancelled-superseded, its
  content covered by #321's descendant).
- Precise triage executed (session-4 survey item 1): wrote a
  cfg(test)-aware scanner — cross-line string state machine + external
  `#[cfg(test)] mod x;` declaration resolution + brace-balanced region
  tracking. THREE parser bugs found and fixed en route (line-based
  string stripping vs multi-line SQL literals; newline-swallowing line
  misalignment; first-pass prod/test double-count for external test
  modules) — recorded as G-08. Final census, machine-verified against
  raw lines: 86 production (24 unwrap + 62 expect), 2,529 test-region,
  43 xtask; 7 whole-file test modules correctly classified (bench.rs,
  it_*.rs x4, mockserver.rs, testsupport.rs — the crude tracker had
  bench.rs and mockserver in production).
- D-14 written (survey item 2): per-class policy A/B/C/D + xtask/bench
  exemption + Q-03 owner-override record. The survey's "do not
  bulk-convert static regexes" caution is codified as class A's keep
  verdict with the R3-collision rationale.
- R2.4-a executed (survey item 3, first of the 2-3 planned batches):
  class B eliminated (21 sites; into_inner recovery; inert collapsed
  into lock_world()), class A normalized (20 unwrap→expect). 8 files,
  +64/−64 after fmt. Post-batch scanner verification: production
  86→65, unwraps 24→4 (all four are class-C guarded), mutex-poison
  sites 0.
- Battery: gate clean, fmt --check clean, clippy workspace (3m07s) +
  sandbox (39.5s) clean, core 520 (464s), core-sandbox 528 (489s),
  app-shell 293+2 (298s), catalog+xtask 56, verify-icons verified,
  strip-check 205/205 — all green. G-05 managed preemptively (rm
  incremental between lanes; disk peaked 93% during app-shell link).
- Committed as `refactor(R2.4a): unwrap/expect policy batch a — D-14,
  poison recovery, static-regex normalization` (code + ledger in one
  commit per D-08). CI rides the push; run id recorded below after
  the first poll.
- CI closure: run#322 (id 43391137472) GREEN on `d8cbb9c` — all four
  jobs success (fmt/clippy/test/deny; AppImage+.deb x2; kittest
  snapshot locks drift gate). The diff touched only core (8 files,
  app-shell untouched — goldens unaffected, as predicted).
- R2.4-b prep notes (read-only analysis done while CI ran; the 17
  class-C sites triaged for the next session):
  - ELIMINATE, clean rewrites (6): live_chats.rs:349 (`is_some_and`+
    expect → `if let Some(cache) = …filter(…)`); notification_sweep.rs:
    164 (`Some(last().map(…).expect(…))` inside `if hit_cap` → plain
    `events.last().map(…)` — identical semantics minus the expect);
    coaching/mod.rs:461+485 (`unwrap_or("").is_empty() {} else {
    as_str().unwrap() }` → `match as_str() { Some(s) if !s.is_empty()
    => …, _ => "(see reasoning)" }`); inert.rs:644 (`is_none()` early
    return + `wf.unwrap()` → `let Some(wf) = wf else { return Err }`);
    quality/mod.rs:629 (`json!` literal + `as_object_mut().unwrap()`
    → build a `serde_json::Map` directly, wrap once at the end).
  - ELIMINATE-IF-NATURAL, needs flow study (3): embeddings.rs:690+744
    (qdrant `Option` checked far above — rebind the checked value or
    `if let` through the point-building loop); settings.rs:15 (match
    arm `bytes.len() == KEY_LEN` + `try_into().expect("length
    checked")` → nested match on `try_from` with the wrong-length
    error; borderline — the restructure unifies two error paths).
  - KEEP, documented post-conditions (8): live_chats.rs:111 (page-model
    contract; ui-closure restructure risks pixel drift, D-05);
    ai_pipeline 1116/1747/1751 (rows/JSON written by this same code;
    1747's `?` propagation needs a rusqlite conversion-failure mapping
    for provably-valid data); support_graph:294 (static 12-kinds
    table); workers/execute:675 (decision-type guarantee — note for
    R3.4); webhook.rs:29 (HMAC library contract); embeddings.rs:813
    (`json!` macro literal).
  Batch b = the 6 clean eliminations (+2-3 if-natural after study),
  full battery, one commit. Expected end state: ~10-11 kept class-C
  expects, each with a written D-14 justification; R2.4 then terminal.

### Session 4 — 2026-11-13
- Pre-flight: sandbox reset AGAIN (clone + toolchain gone) — re-provisioned
  per handover §2 (re-clone ~1 min + toolchain 1.99.1 ~4 min). CI run#312
  green on `bb29f10` confirmed before work began.
- R1.10 executed: deleted the redundant `#![warn(clippy::all,
  missing_docs)]` → `#![warn(clippy::all)]` and trimmed `missing_docs`
  from the allow lists in `crates/catalog/src/lib.rs` +
  `crates/core/src/lib.rs` (D-04 resolved; net lint config unchanged —
  the allow was winning). app-shell already had the clean form; xtask has
  no lint attrs; the three crate headers are now uniform. 12 attr lines
  removed, 4 insertions after fmt (list reflow).
- Whole-tree zero-check: `strip-comments --check crates` green
  (205/205) — this was already green from session 3; R1.10's own diff
  touched only attribute lines.
- Battery: gate clean, fmt --check clean, clippy workspace (2m55s) +
  sandbox lane (38s) clean, catalog+xtask 56, core 520 (460s),
  core-sandbox 528 (455s), app-shell 293+2 (290s after one G-05 retry —
  disk hit 100% full at the link step; `rm -rf
  target/debug/incremental` freed 2.5G), verify-icons verified.
- R1 STAGE TERMINAL (R1.0–R1.10). Exact stripper-verified counts for the
  record: 3,174 whole comment lines + 6 inline comments removed across
  R1.1–R1.9 (catalog contributed 0 — already clean); R1.10 removed 12
  redundant lint-attr lines. Terminal entries written to `complete.md`
  (R1.10's own CI row rides this push — final confirmation next poll).
- Note for R2.2: the `#[allow]` census in the plan says 31 instances —
  re-count at execution time (R1.x restructuring touched two sites:
  design/mod.rs and workers_host.rs G-07 fixes, plus the audit allowlist
  lines in core/src/audit.rs which are gate-anchored, invariant 7).
- R2.1 executed (same session, after run#313 went green on the R1.10
  head): the sweep proper found ZERO active dead-code/unused warnings
  (clippy lanes already run `-D warnings` and pass — R1's lint hygiene
  had kept the tree warning-clean). The audit's real finds were two
  VESTIGIAL suppressions of exactly R2.1's lint classes: (a)
  `testsupport.rs` `#![allow(dead_code)]` — the module is already
  `#[cfg(test)]`-gated in lib.rs and every helper is used in the test
  profile (seed_users/mailboxes/customers feed fixture(); empty_db,
  install_master_key, ConvSeed, fixture, seed_conversation, seed_thread,
  seed_rating, seed_tag, seed_doc_article all have call sites — 56
  verified) — deleted; (b) `issue_radar.rs` `#[allow(unused_mut)]` —
  nav is mutated at 8 sites; the stripped AUD-P2 stub comment had
  obscured that the writers landed — deleted. Both deletion-only,
  behavior-identical.
- Battery after R2.1: gate clean, fmt --check clean, clippy workspace
  (6.8s) + sandbox lane clean, app-shell 293+2 (272s), core 520 (478s),
  core-sandbox 528 (444s), catalog+xtask 56, verify-icons verified,
  whole-tree strip check 205/205.
- R2.2 census recorded for the next session: in-src 26 = 15
  too_many_arguments + 4 too_many_lines + 3 unsafe_code (vectorstore_
  sqlitevec.rs FFI, sandbox.rs INSIDE cfg(test) mod tests, app-shell
  lib.rs gating single_instance FFI) + 3 crate-level style pairs
  (module_name_repetitions + missing_errors_doc) + 1 ref_option
  (search/lanes.rs); in-tests 9 too_many_arguments. The plan's "31"
  was the all-crates count at session 1.
- R2.2 executed (same session, after run#315 went green on the R2.1
  head): full audit of all 37 instances (28 in-src + 9 in-test) —
  verdict table written above; survivor justifications written to
  memory D-13 (unsafe FFI islands, unsatisfiable-under-constraint-3
  style pairs, R3.4-owned argument clusters, R4.5-owned file splits,
  D-10 test regions). ONE additional deletion found: search/lanes.rs
  `#[allow(clippy::ref_option)]` was vestigial — the signature is
  already `Option<&str>` (the lint's recommended form); empirical
  delete + clippy -D warnings green confirmed. Net tree: 34 allows,
  all justified (R2.2's bar: every allow either gone or carrying an
  active written justification).
- Battery (R2.2): gate, fmt --check, clippy workspace (26s) + sandbox
  (22s), app-shell 293+2 (282s), core 520 (477s), core-sandbox 528
  (474s), catalog+xtask 56, verify-icons, whole-tree strip check
  205/205 — all green. (Battery was split across a mid-run
  communication interruption — disk freed 2.1G incremental between
  segments; no test was run twice-unclean.)
- CI note: run#316 (docs-only head `5c8dc5c`) was still in flight at
  R2.2 battery end; the R2.2 push stacks on it.
- CI closures mid-session: run#316 green on `5c8dc5c` (docs head),
  run#317 green on `f06e1ba` (R2.2 head), run#318 green on `5e47b50`
  (R2.2 ledger addendum) — each verified before the next batch started.

- R2.4 survey (prep for the next session; crude cfg(test) tracker —
  numbers to be re-verified with a precise pass): 126 production-path
  candidates vs 2,588 test-region occurrences across 131 files.
  Classification of the 126: (a) ~45 static regex compiles
  (Regex::new(CONST).unwrap/expect — security.rs 7, quality/* ~18,
  coaching/* ~10, interaction_engine/* ~6, qa_ai 3, signals macro 1);
  (b) ~19 mutex-poison expects (vectorstore_sqlitevec 11, helpscout/
  inert 5, settings MASTER_KEY 3 — standard poison idiom; the
  into_inner() recovery pattern is the likely elimination);
  (c) ~15 post-condition expects ("checked above"/"loaded above"/
  "hit_cap implies"/"12 kinds"/"static" — control-flow restructure);
  (d) bench.rs ~25 (dev-only bench harness — classification verdict
  needed: is xtask/bench-style build machinery 'production' under
  D-10?); (e) xtask 4 (same verdict needed); (f) unreachable! arms
  3 (app/mod PageKey, oauth_state hex_digit, workers/execute — R2.5
  territory); (g) bootstrap-ish 3 (task_bridge spawn, design font
  family, mockserver panic — mockserver is test-support in a prod
  tree); (h) scanner false-positives possible (crude tracker; e.g.
  inert.rs:659 sits near test-support code). NEXT SESSION must:
  (1) re-run the triage with a precise parser, (2) write the per-class
  policy as D-14 (static-regex + poison-recovery + xtask/bench
  classification — possibly an owner question), (3) execute in 2-3
  batches. NOTE: the plan's R2.4 policy is strict (bootstrap-only
  exemption) — do not bulk-convert static regexes to error
  propagation without the D-14 decision; that signature churn would
  collide with R3's DRY pass.
- R2.3 executed (after run#318 confirmed the green base): installed
  cargo-machete v0.9.2 + cargo-deny v0.20.2 ad hoc (both to ~/.cargo,
  ~5 min total). Machete's 4 candidates triaged: 1 REAL (scrypt —
  zero source occurrences anywhere; the KDF stack is aes-gcm + sha2
  only) + 3 FALSE POSITIVES (machete's string matcher can't see the
  custom lib names: xtask's spp_core:: x14, app-shell's spp_core::
  x56+, core's catalog re-export shim `pub mod catalog { pub use
  spp_catalog::*; }`). Removed scrypt from the workspace
  [workspace.dependencies] + core's manifest; Cargo.lock pruned exactly
  scrypt + salsa20 + pbkdf2 (scrypt's exclusive transitives; 747 bytes
  — verified by diff, nothing else moved). cargo deny check: advisories
  / bans / licenses / sources ALL ok. Full battery green: gate, fmt,
  clippy x2 (31s/22s — dep-graph change rebuilt core+dependents), core
  520 (453s), core-sandbox 528 (483s), app-shell 293+2 (294s),
  catalog+xtask 56, verify-icons, strip check 205/205. G-05 hit twice
  (96%→97% at test-binary links) — incremental rm + retry green both
  times.


### Session 3 — 2026-11-12
- Pre-flight: sandbox reset AGAIN (clone + toolchain gone) — re-provisioned
  per handover §2 (re-clone ~1 min + toolchain 1.99.1 ~4 min).
- CI on the session-2 final head: run#307 (id 43293054746) — completed /
  success on `7a7b264` (R1.5 CI-verified; the session-2 "CI pending" note
  resolved). Green base confirmed before starting the batch.
- R1.6 executed: the stripper's --check pass over all of `crates/core`
  listed exactly the 26 files still carrying comments (the plan's
  `core/tests/` path does not exist — all tests are inline
  `#[cfg(test)]` modules, so the batch is simply "the rest of core/src").
  Strip removed 479 whole comment lines + 1 inline comment; `cargo fmt
  --all` produced 2 reflow insertions (automation.rs: trailing comment on
  `enabled: true,`; embeddings.rs: comment-only match arm collapsed to
  `{}`). Biggest files: automation 87, knowledge_store 50, embeddings 34,
  intelligence_features 35, custom_objects 33, business_hours 26,
  ticket_states 25, webhook 23, notifications 21, inbox 17, webhook_handler
  17, webhook_health 18, chats 16, notification_sweep 14.
- Battery: gate clean, fmt --check clean, clippy workspace (2m22s) +
  sandbox lane (41s) clean, core 520 passed (472s), core-sandbox 528
  passed (466s), catalog+xtask 56 passed, app-shell 293+2 passed (290s,
  after one G-05 retry), verify-icons verified, cumulative subset --check
  clean (135/135 across core+catalog+xtask). audit.rs untouched this
  batch (invariant 7 moot).
- G-05 recurred exactly as documented: disk hit 94% (657M free) during
  the app-shell test-binary link → `ld terminated with signal 7 [Bus
  error]` on kittest_widgets/kittest_onboarding_page → `rm -rf
  target/debug/incremental` freed 2.6G → retry green. Session-2's memory
  entry held; nothing new to add to G-05.
- Note for R1.9: the 26 kittest test binaries DO build+link in the default
  app-shell lane (they carry 0 test fns locally — the fns are
  kittest-ui-gated), so an R1.9 strip touching `crates/app-shell/tests/`
  is exercised by the default lane's link step. Disk headroom ≥2G required
  for that lane (G-05 budget).
- R1.7 executed (same session, after run#308 went green on the R1.6 head):
  --check over the plan's 17 infra paths listed 11 dirty of 19 scanned
  (app/chrome 5, app/mod 82, chart 25, design/controls 39, design/mod 225,
  lib 3, main 2, mention_popup 7, palette 33, task_bridge 1, workers_host
  14-line region). Strip removed 429 whole lines + 2 inline comments.
- G-07 discovered and hit twice: comments had been masking clippy lints.
  design/mod.rs badge() — the UXF-1-1 comment sat between `let response =
  ui.interact(..)` and the `response` tail → let_and_return once stripped
  → fixed by returning `ui.interact(..)` directly. workers_host.rs
  restart() — the AU2-13g comment sat between outer and inner if →
  collapsible_if once stripped → fixed by folding `&& let Err(join_err)
  = block_on(prev)` into the chain (same short-circuit semantics).
  Recorded as G-07 in memory.md; clippy lane now runs immediately after
  fmt, before the test lanes, to catch this class early in R1.8/R1.9.
- Battery: gate clean, fmt --check clean, clippy workspace (6s after fix)
  + sandbox lane clean, app-shell 293+2 (297s), core 520 (452s),
  core-sandbox 528 (473s), catalog+xtask 56, verify-icons verified,
  cumulative subset --check clean (154/154 across core+catalog+xtask+
  app-shell-infra).
- G-05 hit a second time mid-battery at the same spot (app-shell test
  link, 77% full): `rm -rf target/debug/incremental` + retry green.
- R1.8 executed (same session, after run#309 went green on the R1.7 head):
  the seven 2K+ pages (inbox 4715, settings 3316, knowledge 2647,
  reports 2374, ai_center 2299, custom_objects 2092, outreach 1844) —
  all 7 carried comments. Strip removed 864 whole lines + 1 inline
  (inbox 226, knowledge 150, outreach 157, custom_objects 132,
  settings 114, ai_center 55, reports 51; 8 fmt reflow insertions: the
  knowledge.rs test trailing comment + a custom_objects.rs method-chain
  reflow).
- G-07 hit exactly as predicted (twice more): custom_objects.rs +
  outreach.rs empty-state CTA blocks — the guard comment sat between
  `if types.is_empty()` / `if campaigns.is_empty()` and the nested
  `if design::empty_state_cta(..)`. Fixed by the same collapse
  (`is_empty() && empty_state_cta(..)`) — the CTA render is still
  gated on the empty list, pixel-neutral.
- Battery: gate clean, fmt --check clean, clippy workspace (5.8s after
  fix) + sandbox lane clean, app-shell 293+2 (289s), core 520 (459s),
  core-sandbox 528 (483s), catalog+xtask 56, verify-icons verified,
  cumulative subset --check clean (161/161 across core+catalog+xtask+
  app-shell-infra+pages-A).
- Process slip caught by the tool window: chaining core + sandbox lanes
  in one command exceeded 10 min and the call was killed mid-second-lane
  (the core lane itself had already passed — log confirmed 520/520;
  clean rerun also green). Lesson recorded in the pointer: ONE test lane
  per tool call.
- R1.9 executed (same session, after run#310 went green on the R1.8 head):
  scope = pages B (16 pages + pages/mod.rs + queries.rs) + the whole
  `crates/app-shell/tests/` tree (25 kittest suites + style_lint.rs).
  Strip removed 1011 whole comment lines + 1 inline from 44 of 51
  scanned (1 fmt reflow: issue_radar's `#[allow(unused_mut)]` trailing
  comment trimmed). NO G-07 hits — the tests-tree comments masked no
  lints. Kittest suite file names + test fn names untouched (D-06
  intact); the app-shell lane re-linked every stripped kittest binary.
- MILESTONE: `strip-comments --check crates` — the WHOLE-TREE zero
  check that R1.10 was to enforce — is already green (205/205 scanned).
  The entire Rust source tree (4 crates, 205 .rs files) is comment-free.
- Battery: gate clean, fmt --check clean, clippy workspace (5.9s) +
  sandbox lane clean, app-shell 293+2 (293s), core 520 (501s),
  core-sandbox 528 (464s), catalog+xtask 56, verify-icons verified.
- Session-3 tally: four batches (R1.6-R1.9), 2,783 whole comment lines
  + 5 inline comments removed, 5 G-07 masked-lint fixes (2 x
  let_and_return/collapsible_if in infra, 2 x collapsible_if empty-state
  CTAs in pages A, 0 in pages B), 3 G-05 disk retries, all four CI runs
  green so far (308, 309, 310, 311 — the R1.9 run concluded green at
  session close; all four heads CI-verified).

### Session 2 — 2026-11-12
- Pre-flight: sandbox reset AGAIN (clone + toolchain gone) — re-provisioned
  per handover §2 (~15 min incl. toolchain download).
- R1.0 built: `crates/xtask/src/strip.rs` (lexer, ~280 lines + 38 unit
  tests), wired as `xtask strip-comments [--check] [paths...]`; default root
  `crates`; skips target/.git dirs; report: files scanned/rewritten, whole
  lines + inline comments removed. One test-expectation fix during
  development (inline block comment keeps the space AFTER it; rustfmt
  canonicalizes).
- Local battery green: gate, fmt, clippy x2, tests x2 (520 + 528),
  verify-icons, pilot --check.
- R1.1 executed: catalog is ALREADY comment-free (0 lines across 8 files —
  plan batch was sized from the session-1 aggregate numbers); xtask had 59
  lines in package.rs (56) + bin/xtask.rs (3). Subset --check green (13/13).
- Commits: `88e1770` (R1.0), `6362842` (R1.1).
- CI: run#303 (id 43269465470) — completed / success on `6362842`
  (R1.0 + R1.1 CI-verified).
- R1.2 executed (same session, time allowed): 90 comment lines removed from
  7 of 15 core/ai_* files (ai_center 17, ai_lm_studio 33, ai_evidence 13,
  ai_hub 10, ai_local 8, ai_pipeline/mod 6, ai_prompts 3; ai_attributes,
  ai_tools, ai_eval, ai_memory_guard, ai_provider were already clean).
  Full battery green: gate, fmt, clippy x2, tests x2 (293+2+520+56,
  528 sandbox), verify-icons, cumulative subset --check 28/28.
  Disk discipline applied (G-05: freed incremental before sandbox link).
- R1.3 executed (same session): 162 whole comment lines + 1 trailing comment
  (jobs.rs tick line) removed from 8 of 13 files (execute.rs 62,
  workers/mod 48, connectors 20, loopback 8, api_queue 11, events 5, jobs 6,
  sync_engine/mod 2). Cumulative subset --check 41/41. CI run#304 green
  on the R1.2 head mid-batch. Battery executed as per-crate lanes (the
  10-min tool window kills a full `--workspace` run after cached compile;
  per-crate == same coverage): catalog+xtask 56, app-shell 293+2,
  core 520, core-sandbox 528 — all green; clippy x2 and fmt green before
  the lanes.
- Session-2 disk incidents (for G-05/G-07 refinement): sandbox link
  bus-error fixed by rm incremental; later ENOSPC fixed by
  `cargo clean -p supportos-plus-core` (2.6G freed) + rebuild.
- R1.4 executed (same session): 26 comment lines removed from 3 of 9 files
  (it_lm_studio 16, it_helpscout 6, helpscout_real/mod 4; helpscout/,
  oauth_state, mirror, mockserver already clean). CI run#305 green on the
  R1.3 head mid-batch. Full per-crate battery: clippy x2, core 520,
  core-sandbox 528 (after clean -p core), catalog+xtask 56, app-shell
  293; verify-icons; cumulative subset --check 50/50.
- R1.5 executed (same session): 54 comment lines removed from 6 of 23 files
  (maintenance 21, people_store/mod 13, migrations/cleanup 14, db 2,
  audit 3, backup_service 1). Invariant 7 held: audit.rs's two gate-allowlist
  lines are pure code lines (no trailing comments) — gate re-ran clean
  with zero allowlist changes. Cumulative subset --check 73/73. Battery:
  clippy x2, core 520, core-sandbox 528, catalog+xtask 56, app-shell 293,
  verify-icons, gate — all green. CI run#306 green on the R1.4 head
  mid-batch.

### Session 1 — 2026-11-12
- Research: full metrics snapshot (plan §6); read the no-simulation gate
  end-to-end (allowlist semantics: stale entries harmless; new marker lines
  fail); confirmed `missing_docs` is warn+allow (comment removal is
  clippy-safe); confirmed no local Vulkan (kittest is CI-only); confirmed
  rust-lang.org reachable (toolchain installable on demand).
- Environment: sandbox had been reset — re-cloned the repo, re-set git
  identity; toolchain NOT reinstalled this session (R0 needs no cargo;
  next session must provision per `handover.md` §2).
- Created this folder: plan/progress/complete/handover/memory.
- Executed R0 (pipeline alignment): traceability-gate step removed from
  ci.yml and release.yml; `scripts/gate_traceability.py` →
  `_archive/scripts/`; `docs/audit/design-contract-v1.md` +
  `docs/audit/acceptance/traceability-matrix.md` → `_archive/docs/audit/`;
  root `docs/` removed entirely (root = code + code-only build machinery).
  Verified: zero references to `gate_traceability|docs/audit` outside
  `_archive`; no-simulation gate clean.
- Commits: `38f480b` (framework), `c6ab15d` (R0).
- CI: run#301 (id 43227385165) — completed / success on all four lanes
  (check, packages x86_64, packages arm64, kittest snapshot locks).
  R0 terminal; evidence in `complete.md`.
