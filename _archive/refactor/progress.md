# Refactor Progress — Live State

Last updated: session 24 close-out (2026-11-16). Update rule: every
session updates this file before ending. Statuses: `pending` /
`in_progress` / `done` / `terminal` (done + closed in `complete.md`) /
`dropped` (with reason in `memory.md`).

## CURRENT POINTER

- **PROGRAM 3 CLOSED (session 25, 2026-11-16) — Rule 8 STOP.**
  All five R-letters executed and terminal (records in
  `complete.md`'s Program 3 closing report): R8.1 (475e055,
  run#386), R8.2 (d3cdb36, run#387), R8.3/R8.4 (the D-39 rulings,
  riding d3cdb36's ledger block), R8.5 (d4a19f5, CI run#388 + tag
  run#389 + Release run#39 — **v1.10.1 PUBLISHED** at tag
  `v1.10.1`, target `d4a19f5`, post-publish live-verified:
  BUILD-INFO version/commit match, both binary assets HTTP 200,
  SHA256SUMS present). The residue is fully resolved — nothing
  recorded as deferred/if-ordered remains open anywhere in the
  ledger. The queue is empty — new work opens only on an owner
  directive.

- **PROGRAM 2 CLOSED (session 24, 2026-11-16) — Rule 8 STOP.** All
  six Q-letters executed and terminal (records in `complete.md`'s
  Program 2 closing report): R7.1 (f9c568c), R7.2 (e62bf08), R7.3
  (10c3cde, run#361), R7.4-a/1..6 + R7.4-b + R7.4-c (runs
  #362–#378), R7.5 (84cc569, run#380), R7.6 (2f8a192, CI run#382 +
  tag run#383 + Release run#38 — **v1.10.0 PUBLISHED** at tag
  `v1.10.0`, target `2f8a192`, post-publish live-verified: BUILD-INFO
  version/commit match, both binary assets HTTP 200 through the
  public URLs, SHA256SUMS present). The queue is empty — new work
  opens only on an owner directive.

- **PROGRAM 2 OPEN** (session 21, 2026-11-15) — owner override
  "complete anything ignored and skipped"; operative task list is
  plan §11 (Q-01..Q-06 execution). Program 1 stays closed; every
  protocol carries over. **CLOSED session 24** (see the pointer
  above; the original letter preserved for the record). Done:
  R7.1 (f9c568c), R7.2 (e62bf08), R7.3 (10c3cde,
  CI run#361), R7.4-a/1 sandbox (05f7006, run#362), R7.4-a/2
  security cluster (6c9538a, run#364), R7.4-a/3 coaching cluster
  (dd9c3da, run#366), R7.4-a/4 interaction_engine (c391955,
  run#368), R7.4-a/5 quality cluster (e721e39 + fmt fix
  8c6dab2, runs #370-fail/#371-green), R7.4-a/6 ai_evidence
  midnight (c540871, run#373) — CLASS A COMPLETE — R7.4-b
  class-C post-conditions (4d91080, run#375; prod_expect 12 -> 6
  r25-series / 5 truncation: the census's 8 included 2
  cfg(test)-region digest sites — 6 production sites converted;
  remaining 5 truncation = class D 3 + D-14 keeps 2) — R7.4-c
  class-D bootstrap sites (4fd37af, run#378: block_ai_pipeline
  Result<F::Output> + 13 '?', TaskBridge/ShellApp io::Result up to
  main's fatal-exit channel, design font if-let gate; prod unwraps 0;
  prod_expect r25-series 6 -> 3 / truncation 5 -> 2 — CLASS D
  COMPLETE, the Q-03 strict-letter conversion is tree-wide complete;
  remaining = interaction_signals D-14 keep + 2 cfg(test)-region
  digest sites + xtask machinery) — R7.5 Q-01 guardrails (session
  24: R5.1 gate port to xtask Cmd::Gate + release-container python3
  drop, G-03 resolved, scripts/ deleted; R5.2 unwrap-policy
  subcommand — string-aware scanner, 5-entry justified ALLOWED
  table, prod unwrap 0 / panic-family 0 / expect exact-allowlist,
  CI step added; R5.3 20 zero-fire pedantic lints via
  [workspace.lints.clippy] + clippy.toml msrv, D-36 correction).
  Then R7.6 (Q-02: 1.10.0 + tagged
  release) -> program close-out — all closed; see the CURRENT
  POINTER above and `complete.md`.
- Last program-1 head `6dd8c8b` CI-green (run#360, all four lanes —
  verified this session before Program 2 opened). Session counter
  resets for Program 2: session 1 of Program 2 = session 21 overall.
  Program-1 final metrics + closing report stay in `complete.md`.

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
| R2.4 unwrap/expect policy | terminal (session 6) | batch a: D-14 policy + B-class poison elimination + A-class normalization (CI run#322 green); batch b: class C resolved — 9 eliminated (incl. ALL 4 production unwraps), 8 kept with written D-14 justifications; scanner census 56 prod expects + 0 unwraps; battery green; CI row rides this push — see complete.md |
| R2.5 panic!/todo!/unimplemented! | terminal (session 7) | precise census: 4 production occurrences, not the plan's 30 (27 were test-region overcounts; todo!/unimplemented! zero tree-wide); policy D-15: 3 exhaustiveness-forced unreachable! kept with written local proofs, 1 arm-level cfg(test) panic reclassified D-10-exempt; one code increment (execute.rs:1833 message normalization); battery green; CI row rides this push — see complete.md |
| R3.1 error types | terminal (session 8) | census: FIVE types tree-wide, ZERO duplicate enums (root+boxed-subtype architecture already consolidated); 3 hand-rolled Display/Error impls → thiserror derives (byte-identical messages; AcquireError #[source] reproduces the hand-rolled source()); TranslateError enum→newtype (class dimension unread by its single consumer); app-shell +thiserror dep (lock: one edge line); D-16; CI run#332 green on `5ca31af` — see complete.md |
| R3.2 SQL plumbing | terminal (session 9) | lenient row-collect tail consolidated into db::query_rows + db::rows_ok; 80 sites (73 census + 7 turbofish blind-spot); +657/−823 across 32 files; battery green (520/528/293+2/56); CI run#335 green on `5cc08fc` — see complete.md |
| R3.3 HTTP/serde plumbing | terminal (session 10) | 4 LM-Studio send tails → `send_json` + `authorized` (byte-identical messages); 2 docs tails → `docs_request_json` (byte-identical friendly strings, verified by continuation-resolution); 122 flat key-indexed extraction sites → `json_fields::json_str`/`json_string` (new module, 9 LOC); 13 send-tails/6 builders/139 idioms censused, keeps recorded in the R3.3 verdict table; battery green (520/528/293+2/56); strip-check 206/206; CI run#337 green on `518ba64` — see complete.md |
| R3.4 page scaffolding | terminal (session 11) | census: hosting layer ALREADY shared (design:: hosts/page_header; 21/23 pages); the real repeat = the "Sync now" toolbar fragment byte-identical ×4 (dashboard/customers/reports/organizations) → `sync_health::sync_now_button` (button+hover+match+3 note strings in ONE place; re-read flag returns for the caller-side page-specific invalidate()); inbox variant, ensure_loaded latches, RANGES chip loops, LIST_CAP hint censused as keeps (R3.4 verdict table); +41/−97 across 5 files; battery green (520/528/293+2/56); strip-check 206/206; CI run#339 green on `9edd9b9` — see complete.md |
| R3.5 PageKey tables | terminal (session 12) | census: ZERO folds land — the attribute layer is already single-sourced (label() str site, Icon::for_page Icon site, apply_invalidations field-dispatch, render-dispatch heterogeneous payloads — four disjoint payload shapes, no ≥2 same-shape pair); titles/nav-labels are decoupled roles (2 intentional divergences); NAV_GROUPS + all() already tables; drill-slug pair queued to R3.6 — see complete.md |
| R3.6 cross-crate vocabulary | terminal (session 12) | census: ZERO core↔catalog const duplication (plan letter overcounted — 6th consecutive correction); real debt = 8 inline-parse twins of existing catalog parse fns (GraphNodeKind ×7 + AiAttributeKey ×1) + the drill-slug wire pair → `catalog::routes` (5 consts, new module); +24/−44 +6 (net −20); battery green (520/528/293+2/56, strip-check 207/207) — see complete.md |
| R4.1 naming | terminal (session 13) | census (r41_census.py): 341 bool decls (272 prod) + 118 bool-returning fns + 478 short-id kinds + 232 suspicious-word hits; SEVENTH consecutive census correction — the tree's naming was overwhelmingly honest; 10 surgical renames + 1 deletion (aa_get, D-18 rule): temp→temperature, already→already_exists/already_queued, has→has_column, known/conversation→*_exists, palette_take→query_committed, right→align_right ×3, drill→drillable, assistant→from_assistant, ins→stmt ×2; keep rulings in the R4.1 verdict table; dt_cell ×3 fold candidate recorded, not acted; battery green (520/528/293+2/56, clippy x2, gate, icons, strip-check 207/207); CI run#344 green on `1b653d4` — see complete.md |
| R4.2 decomposition (batch a) | done (session 14) | census (r42_census.py, cfg-aware fn-span scanner): 3,277 prod fns / 991 test fns; top-20 censused; 6 CORE mega-fns split (execute 1051→79 dispatch + 22 tool fns; derived_edges_touching 686→26 + 11 Kind fns + 4 module helpers + 2 epilogue fns; apply_m046 405→15 manifest + ensure_fk + 10 data consts; execute_job 3 fat arms→3 methods incl. embed×3 fold; build_support_health_report 465→34 manifest + 9 domain fns; analyze_friction 410→manifest + 6 finding fns + finding helper); 6 keeps + 11 queued to R4.2-b (verdict table below); battery green (520/528/293+2/56, clippy ×2, gate, icons, strip 207/207); CI run#346 green on `fa03cad` (all 4 lanes incl. kittest drift gate); D-23 |
| R4.2 decomposition (batch b) | done (session 15) | all 11 queued fns split: execute_outbound 390→~86-line dispatcher + 6 send_* arm methods + payload_num/payload_str_or module fns (closures promoted per the batch-a recipe); sync_resource 331→19-arm dispatcher table + 16 sync_* methods (conversations/threads delegations + the `customers` since-seam + the unknown-resource fallback stay inline); incidents ui 527→~180 dispatcher + filter_controls/empty_list_ui/lifecycle_card_ui/linked_column_ui/info_column_ui methods + incident_row/incident_detail_header module fns; ai_center hub_ui 481→35-line dispatcher + 8 methods (endpoints card/row/key-edit/price/add-form/roles/features/ledger); organizations ui 393→~170 dispatcher + org_form_card_ui method + org_row/org_detail_header_ui/members_card_ui module fns; inbox list_ui→search_filter_ui/list_toolbar_ui/conversation_rows_ui methods, copilot_pane_ui→copilot_cards_ui/copilot_lmstudio_ui/assistant_ui methods, composer_pane_ui→enqueue_composer_tool/apply_composer_act/mention_popup_ui methods + composer_tool_row module fn + ToolAction enum hoisted to module scope; settings helpscout_ui→4 card methods; diagnostics ui→event_log_card_ui method + audit_trail/sync_reports/outbound_deliveries_card_ui module fns; design composer→composer_header_row/composer_footer_row module fns; ALL 4 too_many_lines allows deleted (every post-split fn under the 100-line threshold — clippy -D warnings green); battery green at EXACT baselines (520/528/293+2/56, clippy ×2, gate, icons, strip 207/207); pixel-neutrality verified locally by the widget-call multiset comparison (design::/ui. call multisets byte-identical old-vs-new on all 9 files after fixes — D-24); the CI kittest drift gate rides this push; D-23/D-24 |
| R4.3 visibility | terminal (session 16) | CI run#350 green — see `complete.md` |
| R4.4 magic values | terminal (session 17) | CI run#352 green — see `complete.md` |
| R4.5 file splits (batch a — cfg(test) mod extraction) | done (session 18) | CI run#354 green on `c503d02` (all 4 lanes incl. kittest drift gate — 203 goldens byte-identical); census: 12 files >2K re-measured; 11 carried 37 inline test mods (6,843 lines) extracted to module files (string-aware extractor, per-mod conservation asserts); module tree byte-identical; 6 files exit the >2K set |
| R4.5 file splits (batch b — prod-region splits) | terminal (session 19) | CI run#356 green on `9139e8d` (all 4 lanes incl. kittest drift gate — 203 goldens byte-identical) — see `complete.md`; local battery green at exact baselines (520/528/293+2/56, clippy ×2, gate, fmt, icons, strip 259/259); six hosts split along R4.2 seams into 14 child files, 6,198 lines moved; D-24 multiset 4/4 identical; post-split largest file = ai_center 2003 (the D-27 at-threshold KEEP) — the >2K set is empty |
| R5.1–R5.3 guardrails | dropped (session 20) | owner-gated option never exercised across the program — Q-01 records the standing option (gate port to xtask / unwrap-policy lint / pedantic subset) with the sub-batch letters; dropped-with-reason at close per the DoD |
| R6.1 build-file comment strip | terminal (session 20) | CI run#358 green on `168e71a` — see `complete.md`; 279 comment lines / 12 build files; functional zero-diff proven four ways (tomllib, PyYAML normalized, AST, multiset audit); battery at exact baselines + local cargo-deny four-way ok (G-09); D-29 + G-20 |
| R6.2 final audit — PROGRAM CLOSED | terminal (session 20) | closing docs-only head `9522cde` CI-VERIFIED: run#359 (id 43696107214) green, all 4 lanes (R3.5 docs-only precedent; battery = session-20 run + CI run#358 on the identical code tree); DoD checklist closed in `complete.md`; final metrics vs §6 recorded (259 files / 138,593 LOC / 0 comments tree-wide incl. build files / 0 production unwraps / 56 justified expects / 4 classified panic-family / 30 justified allows / pub −90 / >2K set empty / 203 goldens byte-identical); per-module-cluster verification statements below; standing owner options Q-01..Q-06 |

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

### R2.4 — `unwrap`/`expect` policy (sessions 5-6; precise census 86 production occurrences)

| class | sites | verdict | outcome |
|---|---|---|---|
| B — mutex/rwlock poison (vectorstore_sqlitevec 12, helpscout/inert 5, settings 4) | 21 | eliminate | `.lock().expect(...)` → `.lock().unwrap_or_else(\|p\| p.into_inner())` (std poison recovery; locks guard plain data, no poison-corruptible invariants); inert's 5 sites were inline duplicates of the existing `lock_world()` recovery helper — collapsed into it; happy path behavior-identical, the panic class is gone |
| A — constant-input constructors (static Regex::new 40, seccomp const-args 4, chrono midnight 1) | 45 | keep as expect | input is compile-time constant — failure unrepresentable; Result propagation = ~45 signature churn colliding with R3 (session-4 note); the 20 message-less unwraps normalized to `.expect("static regex")` / `.expect("midnight is always a valid time")` (error-path-only; constraint 3 leaves the expect message as the only in-code doc a survivor may carry) |
| C — guarded post-conditions (live_chats 2, embeddings 3, ai_pipeline 3, notification_sweep 1, support_graph 1, workers/execute 1, coaching 2, quality/mod 1, inert 1, webhook 1, settings 1) | 17 | eliminate-if-natural (R2.4-b) | RESOLVED: 9 eliminated (live_chats 349 if-let/filter; notification_sweep 164 last().map; coaching 461+485 match as_str; inert 644 let-else; quality 629 Map build; embeddings 690+744 qdrant.filter rebind + if-let gates; settings 19 nested try_from match) incl. ALL 4 production unwraps; 8 kept with written D-14 justifications (live_chats 111, ai_pipeline 1116/1747/1751, support_graph 294, workers/execute 675→R3.4 note, webhook 29, embeddings 813) |
| D — runtime/bootstrap infra (workers/mod tokio runtime, task_bridge thread spawn, design font family) | 3 | keep | one-time infra construction, no recovery path; bootstrap-equivalent (plan exempts main.rs bootstrap — same nature outside main.rs) |
| xtask + bench machinery | 43 + bench.rs | exempt | dev-invoked build machinery; bench.rs is cfg(test)-gated at declaration — D-10 exemption |

**R2.4-a verdict summary:** 86 production occurrences triaged by the precise (G-08) scanner — 21 ELIMINATED (class B), 20 normalized unwrap→expect (class A), 45 kept-under-D-14 (A + D), 17 deferred to R2.4-b (class C). Net tree after batch a: 65 production expects + 4 guarded unwraps; zero mutex-poison panics. Q-03 records the plan-letter deviation for owner override.

**R2.4-b verdict summary (session 6):** the 17 class-C sites resolved — 9 ELIMINATED (the 6 prep-note clean rewrites + all 3 if-natural studies resolved to eliminate: embeddings' qdrant rebind, settings' nested try_from), 8 KEPT, every keep carrying an active written justification in memory D-14 class C. Net tree after batch b: 56 production occurrences, ALL expects, ZERO unwraps; 8 file diff, +32/−33 after fmt. The 3 settings/embeddings deviations from the session-5 prep note's borderline leanings were resolved in favor of elimination after flow study (rationale in D-14); no keep was left unjustified.

### R2.5 — panic family (session 7; precise census FOUR production occurrences)

| site | construct | verdict | outcome |
|---|---|---|---|
| app-shell/src/app/mod.rs:891 | `unreachable!("inbox handled above")` | keep | exhaustiveness-forced arm; proof is cross-branch (match runs only in the else of `if self.page == PageKey::Inbox`, verified by read); removal needs a wildcard-None silent no-op (dishonest) or a UI-dispatch restructure (pixel drift, D-05); D-15 |
| core/src/oauth_state.rs:95 | `unreachable!("hex_digit got {n} …")` | keep | `_` on u8 forced by the type; proof is the callers' bit-masks (b>>4, b&0x0f); range-newtype = R3-territory overkill; D-15 |
| core/src/workers/execute.rs:1833 | `_ => unreachable!()` (bare) | keep + normalize | exhaustiveness-forced for `Result<Option<T>,E>` narrowed by the outer match's `Ok(Some(article))` arm; message added: `unreachable!("Ok(Some) is handled by the outer match arm")` — the batch's one code increment (D-14 class-A precedent: the message is the only in-code doc a survivor may carry); D-15 |
| core/src/workers/execute.rs:468 | `panic!("intentional test panic …")` | exempt (D-10) | inside `#[cfg(test)] "__test_panic" =>` match arm — arm-level cfg(test), compiles only in test builds; the G-08 tracker models cfg(test) at mod level so this escaped it (refinement recorded in memory G-08); semantically test machinery, not production |

**R2.5 verdict summary:** the panic-family production census is FOUR (1 panic! + 3 unreachable!), not the plan's 30 — 27 were test-region overcounts (the third census correction of the program after R2.2's allows and R2.4's unwraps; lesson for R3/R4: ALWAYS run the precise scanner before sizing a plan batch). todo!/unimplemented!: ZERO tree-wide in production. xtask: zero. One code increment (1833 message). STAGE R2 TERMINAL: R2.1–R2.5 all closed; every kept panic-family member carries a written local proof (D-15 + Q-04).

### R3.1 — error types (session 8; census: FIVE types tree-wide, ZERO duplicates)

| construct | verdict | outcome |
|---|---|---|
| core/src/error.rs `Error` (thiserror, 11 variants) + `Result` alias | keep | the per-domain root; `Other(Box<dyn Error>)` is the sanctioned bridge for domain subtypes — the plan's "per-domain consolidation" prong found the architecture ALREADY in place |
| ai_pipeline/mod.rs `LmStudioError` + hand-rolled Display + hand-rolled `impl std::error::Error` | merge (derive) | `#[derive(Debug, Clone, PartialEq, thiserror::Error)] #[error("{message}")]` — Display byte-identical (verbatim message); `new()` ctor, `lm_error()` downcast, Other-box flow unchanged |
| helpscout_real/mod.rs `HsApiError` + hand-rolled Display + hand-rolled `impl std::error::Error` | merge (derive) | `#[derive(Debug, Clone, thiserror::Error)] #[error("{friendly}")]` — Display byte-identical (verbatim friendly); the hand-written `From<HsApiError> for Error` bridge KEPT (domain bridge, not duplication) |
| app-shell/single_instance.rs `AcquireError` + hand-rolled Display + hand-rolled source() | merge (derive) | `#[derive(Debug, thiserror::Error)]`, per-variant `#[error]`, `Io(#[source] std::io::Error)` — Display + source() byte-identical (main.rs renders both user-visibly via tracing+fatal); app-shell gains `thiserror.workspace = true` (lock diff: exactly one edge line, crate already present via core) |
| translation.rs `TranslateError` enum{Client,Service,BadGateway}(String) | merge (collapse) | the class dimension was UNREAD structure: the single caller (execute.rs translate_composer) destructured all three variants identically to the message string; newtype `TranslateError(pub String)`; 8 ctor sites + the execute.rs triple-arm match collapsed to `e.0`; deliberately Display-less — no consumer ever displays it, so a thiserror Display would mint unused API surface (§5.3) |
| ai_eval.rs `CaseErr` = (String, Option<u64>) alias | keep | golden-harness data tuple (message + latency), not a domain error type |
| xtask anyhow usage | keep | dev-invoked build machinery (D-14 xtask precedent); anyhow is the idiomatic tooling error type, not a duplicate domain enum |

**R3.1 verdict summary:** census-first again — the plan's "no duplicate error enums" prong found ZERO duplicates (fourth consecutive batch where the precise census beat the plan's sizing assumptions, though this time the correction is favorable: less to do, not more). Actionable set: exactly the 3 hand-rolled Display/Error impls (→ thiserror derives, byte-identical messages) + 1 class-dead enum (→ newtype). End state: every named production error type either derives thiserror or is Display-less by design (D-16). Net source diff +19/−62 across 6 files + 1 manifest line + 1 lock edge.

### R3.2 — SQL plumbing (session 9; census 73 lenient tails + 7 turbofish blind-spot = 80)

| class | sites | verdict | outcome |
|---|---|---|---|
| full-fold → `crate::db::query_rows` | 75 | consolidate | prepare+query_map+lenient-collect collapse to one call; the site's own outer fallback (`?` / `.unwrap_or_default()` / `.ok()` chains / same-fallback matches / let-else-default / is_empty-guarded blocks) kept verbatim — error-path behavior byte-equivalent; non-Vec targets (HashSet/HashMap: analytics bad_ids, outreach dnc, segment def_names) add `.into_iter().collect()` |
| tail-only → `crate::db::rows_ok` | 5 | consolidate tail, keep prepare structure | sites whose prepare-error and query-error fallbacks DIFFER: embeddings ~613 (prepare-Err aborts the vacuum fn), workers/execute 537+569 (prepare-Err early-return vs query-Err empty-continue), workload 495 (prepare-Err skips the team via `continue`), segment/mod 731 (distinct prepare/query warn messages preserved) |
| local helper absorption | 1 fn + 2 calls | delete | ai_tools/execute.rs `collect_rows` (Value-only module-private pre-existing one-off) deleted; call sites ride `query_rows`; standing rule D-17: ONE lenient-collect definition tree-wide |
| turbofish census blind-spot | 7 | convert (same rules) | `collect::<Vec<_>>()` / `collect::<Vec<i64>>()` / `collect::<HashMap<_,_>>()` spellings the census pattern missed: segment/eval ×2, segment/mod ×2, ai_evidence ×1, copilot ×2 — fifth consecutive census correction; lesson recorded in D-17 |
| strict collects `collect::<rusqlite::Result<Vec<_>>>()` | 60 | keep | OPPOSITE semantics (row errors propagate); the canonical one-expression idiom — converting them would be a behavior change smuggled into a DRY batch |
| scalar `\|r\| r.get(0)` mappers | 362 | keep | canonical rusqlite idiom; nothing to consolidate without obscuring |
| JSON column defaults | ~15–20 | keep | four semantic shapes (json!({}) / Value::Null / Default / "{}" string); a helper per shape = 4 micro-helpers, net complexity UP |
| per-module named row mappers | digest_from_row et al. | keep | the established GOOD pattern — each maps its own table's columns; zero cross-module duplication found |
| incidental SQL literal repeats | 3–5 | keep | short test/seed textual coincidences; coupling unrelated code would be anti-DRY |
| `.optional()` | 9 | keep | idiomatic rusqlite extension |

**R3.2 verdict summary:** the plan's "row-mapping/statement boilerplate" letter resolved to ONE real duplication class (the lenient row-collect tail) + a wall of canonical-idiom keeps. 80 tails eliminated (75 full-folds → 77 `query_rows` calls — support_graph search and ai_tools clusters each carry a 2-branch if/else — plus 5 `rows_ok` tails); 1 pre-existing local helper absorbed; 32 files changed, +657/−823 (net −166); test-region sites included (8 census + turbofish — DRY is not production-only); battery green with EXACT baseline counts (520/528/293+2/56) — the lenient-vs-strict semantic boundary held everywhere (D-17 contract).

### R3.3 — HTTP/serde plumbing (session 10; census: 13 send-tails, 6 client builders, 139 string-extraction idioms; scanner r33_triage.py in tool-results, G-08 discipline, line_starts element/char bug found and fixed en route)

| class | sites | verdict | outcome |
|---|---|---|---|
| OpenAI-compatible send tail → `send_json` helper in ai_lm_studio.rs | 4 (list_models 112, chat 140, post_chat_body 210, embed 306) | consolidate | the EXACT 4-stage tail (send → status+text → 300-char-snippet non-2xx error → from_str) with byte-identical `{op} failed/read failed/request failed (HTTP …)/parse failed` messages; op ∈ {list_models, chat, embed}; bearer-auth attach (`authorized`, 5 sites incl. is_available) folded as a client method |
| docs_request tail → `docs_request_json` helper in helpscout_real/http.rs | 2 (docs_request_inner, docs_request_send) | consolidate | same-file DRY-ladder-1: shared skeleton (network HsApiError → status+text(unwrap_or_default) → 2xx empty→Null/from_str → non-2xx HsApiError with friendly_error); the 3 differing strings (network friendly, parse friendly) + method_label/path parameterized; all other messages byte-identical |
| flat key-indexed string extraction → `json_fields::json_str`/`json_string` | 122 (93 `.map(\|s\| s.to_string())` + 4 `.map(String::from)` + 23 `.unwrap_or_default().to_string()` + 2 test-region `String::from` in copilot) | consolidate | new `core/src/json_fields.rs` (pub(crate), 2 fns, json_string delegates to json_str().unwrap_or_default()); files: helpscout_real/mod.rs 75, helpscout_real/http.rs 44, analytics 1, copilot 2 (test region rides D-10 exemption but DRY is not production-only — R3.2 precedent) |
| nested depth-2 extraction `v["a"]["b"].as_str().map(…)` | 8 (all helpscout_real/mod.rs) | keep | a two-arg helper call with `&v["a"]` is not clearer than the direct idiom; the flat-helper does not fit the path shape |
| literal-default extraction `.unwrap_or("lit").to_string()` | 11 (helpscout_real 7, support_graph 4) | keep | the non-empty default is per-site knowledge; the shape differs from the pure extraction class |
| `.get("k").and_then(\|v\| v.as_str())` family | ~50 tree-wide | keep | BORROWED &str access (no allocation) — a different type shape; converting to the owned helper would change perf/type semantics |
| `.as_str().map(str::to_string)` without key index | 9 | keep | array-element Value→String conversion, no key access — different shape |
| embeddings.rs `embed_texts` send tail | 1 | keep | D-17 fold-rule keep: send-error message carries a remedy hint, text read is lazy (error branch only), parse via `res.json()` (from_slice — observably different from text+from_str on invalid UTF-8), text read failure is `unwrap_or_default` not propagated |
| version_check.rs `latest_release` | 1 | keep | single instance of its shape (404-hint branch); no duplication to fold |
| token_request tail (http.rs) | 1 | keep | different error contract (returns Result<Value, HsApiError>, OAuth-specific messages, `.json()` parse, no empty-body Null rule) |
| request_inner full tail (http.rs) | 1 | keep | the retry/ratelimit/301/401/429 machinery — its parse tail is embedded in bespoke control flow; consolidating would parameterize 6+ dimensions |
| connectors.rs http arm | 1 | keep | bounded streaming download with mid-stream size caps and content-type detection — a data-connector concern, not the API-client shape |
| connection_doctor.rs reach probe | 1 | keep | diagnostic flow (401-is-expected branch); the tail is DoctorStep reporting, not value parsing |
| xtask fetch_pinned_with_retry | 1 (blocking) | keep | separate crate, blocking client, anyhow domain — D-14 xtask dev-machinery precedent |
| reqwest::Client::builder sites | 6 | keep | each encodes a distinct policy (timeout+retry-tracing / redirect-none ×2 / default-headers bearer / plain / blocking+timeouts); a shared constructor would parameterize policy = anti-DRY |
| `.chars().take(N).collect()` tree-wide | ~100 | noted, deferred | domain-cap truncation idiom (40/240/300/4000/8000…), NOT response parsing — out of R3.3's class; recorded for a possible future misc-DRY batch |
| `.as_i64().unwrap_or(0)` / `.as_bool()` etc. | many | keep | canonical serde_json Value API — one call, no boilerplate to fold |

**R3.3 verdict summary:** the plan's "reqwest response parsing + JSON mapping duplication" letter resolves to THREE real consolidation classes (4+2 send tails, 122 flat string-extraction sites) + a wall of differing-contract keeps. HTTP surface census: 13 production send-tails, 6 client builders, 7 reqwest-using files + connection_doctor via the shared client.

### R3.4 — page scaffolding (session 11; census: hosting layer already shared; ONE real repeat family)

| candidate | sites | verdict | outcome |
|---|---|---|---|
| "Sync now" toolbar fragment (button_with_icon Refresh Outline + fixed hover + engine match + 3 note strings) | 4 byte-identical (dashboard 518, customers 328, reports 365, organizations 203) | consolidate | → `sync_health::sync_now_button(ui, db, engine, &mut sync_note) -> bool`; the bool is the None-arm re-read signal for the caller-side page-specific `invalidate()`; all five strings byte-verified identical old-vs-new programmatically (continuation-resolution + \u{2026} decode); the None-arm statement order flips (invalidate→note became note→invalidate via the flag return) — proven state-neutral: the four `invalidate()` impls (dashboard data=None, customers stale=true, organizations loaded/orgs/detail, reports overview/sla/lists) never touch `sync_note`, disjoint fields, and nothing reads page data between the two writes; pixel-neutral by construction (same widget calls, same order, same scope) |
| inbox Sync button | 1 (inbox.rs ~1461) | keep | different policy: live label (`Syncing…`/`Sync now` by engine.is_running), different hover text (30 s-while-busy/backoff cadence), `&& let Some(engine)` guard (no None-arm invalidate), set_status_message channel with header-ring wording |
| ensure_loaded latches | ~20 | keep | each latch condition IS the page's staleness policy (loaded flag / is_none / stale+reset / time-throttle+timestamp / composite search condition) — abstracting parameterizes policy = anti-DRY (D-17 discipline) |
| RANGES chip loop | 2 (dashboard card + reports header) | keep | the RANGES table is already single-source (dashboard::RANGES pub(crate), reports imports it); the 5-line loop repeats with disjoint field paths (scope.days vs days) and closure-extraction is borrow-conflicted; a 2-site repeat below parameterization cost |
| LIST_CAP + "first 200" hint | 1 (organizations) | keep | single site, no repeat |
| host compositor / page_header / chips / cards / empty-state CTAs | already shared via design:: | keep | plan letter already satisfied by construction (session-10 prep: host_standard_page + host_inbox_page, one dispatch site app/mod.rs:848/852) |

**R3.4 verdict summary:** the plan's "repeated page composition through the shared host compositor" letter resolves to ONE real consolidation — the 4× Sync-now fragment (25 lines each → 3-line calls + a 32-line helper; net −56: +41/−97 across 5 files). The pointer's 5-site estimate censused to 4 (inbox is a different policy). Dashboard's orphaned ButtonVariant import trimmed with the fold.

### R3.5 — PageKey tables (session 12; census: scanner r35_census.py in tool-results + manual alias-verified sweep — the "titles" enumeration extended beyond matches to the page_header literal sites; ZERO table folds land)

| candidate | sites | verdict | outcome |
|---|---|---|---|
| `PageKey::label()` (pages/mod.rs:349) — 23-arm exhaustive match, `&'static str` payload | 1 | keep | THE single source of nav/display labels — already consolidated; consumed by chrome.rs nav_row ×3 (label, interact-id, WidgetInfo) and smoke.rs; no second str-payload match exists tree-wide → fails the ≥2-same-shape fold precondition |
| `Icon::for_page(key)` (icons.rs:57) — 23-arm exhaustive match, `Icon` variant payload | 1 | keep | THE single source of per-page nav icons; payload shape disjoint from label()'s str — a combined (label, icon) table would be a different-shape merge, explicitly excluded by the fold rule; compiler exhaustiveness (adding a page forces this arm + label()'s arm + all()'s array-length sync) already delivers the table's add-a-page safety |
| `apply_invalidations` (app/mod.rs:190) — 23-arm exhaustive, `self.<field>.invalidate()` field dispatch | 1 | keep | not table-convertible: arms target distinct struct fields through `&mut self` — a data table cannot pre-bind per-field mutable borrows; the `Shell => {}` arm is the honest no-state case |
| render dispatch (app/mod.rs:852) — 23-arm exhaustive, heterogeneous payloads | 1 | keep | per-page `ui()` calls with differing arg tuples (sync/bus) + side effects (notifications badge latch); the `Inbox => unreachable!("inbox handled above")` arm is the documented D-14/D-15 exhaustiveness artifact (memory.md:182); fails same-shape precondition |
| `page_header` title literals across pages (implicit, non-match duplication of label() values) | ~20 byte-match + Shell; 2 diverge | keep | title and nav-label are SEPARATE UI roles — proven by 2 intentional divergences (Onboarding "Welcome to SupportOS+", Operations "Operations Center") and Inbox rendering no title literal at all; pages don't hold their PageKey, so folding = threading a key/label through 20+ ui() signatures for byte-identical output (pixel-neutral by construction but churn-heavy, zero behavior gain); design/mod.rs:2446 "Inbox" call is the D-10 test-region demo — exempt |
| route strings: `page_key_from_drill(&str)` (pages/operations.rs:112) — 5-arm partial over wire slugs | 1 + cross-crate producer | keep (R3.6 pointer) | intentional subset (only pages operations tiles drill into); slugs are the server wire vocabulary produced by core/src/operations.rs `page_drill(...)` — the core↔app-shell slug pair is cross-crate vocabulary, queued to R3.6's census with the R3.5 seed recorded in the CURRENT POINTER |
| `invalidated_pages` (pages/queries.rs) — match over `ServerEvent` producing `Vec<PageKey>` groupings | n/a | keep (out of scope) | event-keyed, not PageKey-keyed — the arm payloads are semantic invalidation groupings per event, not per-variant attributes |
| `NAV_GROUPS` (chrome.rs:50) + `PageKey::all()` (pages/mod.rs:377) | n/a | keep (already tables) | the sidebar order/membership and the ordered enumeration are already data tables; all()'s `[Self; 23]` length type forces compiler-checked sync with the variant count |

**R3.5 verdict summary:** ZERO folds land — the plan's "match-arm duplication (label/route/etc.)" letter is already satisfied by construction: each PageKey attribute has exactly ONE 23-arm match with its own payload shape (str / Icon / field-dispatch / heterogeneous render), no two sites share a shape, and the nav/title/route consumers already route through the single sources (nav_row consumes label() + for_page; NAV_GROUPS drives membership; the drill slugs are a wire contract with an R3.6 pointer). Fourth consecutive census correction (R2.5 panics 30→4, R3.1 duplicate enums 0, R3.4 sync-fragment 5→4, R3.5 fold sites N→0) — the plan's R3 one-liners systematically overcount; the census discipline is load-bearing.

### R3.6 — cross-crate vocabulary (session 12; census: r36_census.py const scanner + `as_str() ==` idiom sweep + alias discipline D-20d; scanner found 4 const-level coincidences — ALL keeps; the real duplication was the parse idiom and the drill-slug wire pair)

| candidate | sites | verdict | outcome |
|---|---|---|---|
| `GraphNodeKind` inline-parse twins of the EXISTING `GraphNodeKind::parse` | 7 (core/support_graph ×4: other_kind_str loop, node_specs `.expect("12 kinds")`, target_kind, source_kind; core/ai_tools/execute.rs:918; core/reports validate_graph_node_kind; app-shell kind_from_str) | consolidate | all seven now call catalog's parse — the find-over-ALL-by-as_str idiom is byte-identical to `parse` by construction (catalog graph.rs:55); validate's `unknown graph node kind: {kind}` message preserved byte-identical via ok_or_else; kind_from_str kept as a 1-line local alias (1 prod caller + 3 test refs — deletion would churn 5 sites for zero gain) |
| `AiAttributeKey` membership twin of the EXISTING `parse` | 1 (core/reports/mod.rs:984, `ALL.iter().any(as_str == key)` inside the closed-catalog check) | consolidate | → `AiAttributeKey::parse(key).is_some()` — identical semantics (parse = find by as_str over ALL); the `Attribute key '{key}' is not in the closed catalog.` message path untouched |
| drill-slug wire vocabulary (5 tokens: issues/automation/diagnostics/sync-health/outreach) | core producer ×7 literals (page_drill calls) ↔ app-shell consumer 5-arm match | consolidate → catalog | new 5-line `catalog::routes` module (ISSUES/AUTOMATION/DIAGNOSTICS/SYNC_HEALTH/OUTREACH); core's 7 literals → consts; app-shell's `page_key_from_drill` match arms now bind the consts (const-in-pattern for &str verified by a rustc 1.99.1 probe before the edit); rationale: both sides must agree by construction (protocol, not role separation — the D-20 title ruling does NOT apply); pre-fold drift was silent (dead drill button); core's test region `drill_payloads_name_real_destinations` pins the wire VALUES (D-10 exempt literals stay) |
| `kind_label` (core/support_graph:238) — display labels over GraphNodeKind | 1 | keep | wire-vs-display role separation (as_str slugs vs "Known issue" labels — the D-20 PageKey title ruling applies); no app-shell twin (labels flow core→app-shell via JSON) |
| `tile_label` (app-shell/operations.rs:49) — display labels over OperationsTileKey | 1 | keep | same role ruling: catalog as_str is the wire vocab ("needs_first_response"), tile_label is display ("Needs first response") |
| ResponseState derivation from actor_type (core/activity.rs:193) | 1 | keep | maps the DB actor vocabulary (customer/user/agent) onto ResponseState — domain logic, not a parse of ResponseState's own vocab |
| ReportMetricKey/ReportDimensionKey inline parses (core/reports/config.rs:96,111) | 1 each | keep | single-crate single-site; adding catalog `parse` for a single consumer fails the ≥2 rule (Q-03 family: the owner can order the catalog parse API if wanted) |
| `parse_type` for NotificationType (core/notifications.rs:146) | 1 | keep | same single-site rule |
| DB CHECK constraint vocab (support_graph/mod.rs:26-27) duplicating GraphHumanRelation as_str slugs as SQL text | 1 | keep | schema DDL text; drift fails CLOSED and LOUD (inserts rejected at the constraint) — parameterizing DDL via format! over ALL is anti-DRY (D-17); leave the schema self-contained |
| const-level coincidences: `"SupportOS-Plus"` (core HOOK_LABEL vs xtask PRODUCT_ID), `"supportos-plus"` (core DATA_DIR_NAME vs xtask DEB_PACKAGE/BINARY_NAME), `200` triple (segment MAX_NODE_COUNT / GRAPH_MAX_NEIGHBOR_EDGES / orgs LIST_CAP), `100_000` | 4 groups | keep | different policies that happen to share values (webhook label vs product id; XDG dir vs deb/binary name; three unrelated caps) — parameterizing unrelated policies is anti-DRY (D-17); xtask is build machinery, not shipped vocabulary |
| `GraphHumanRelation::parse` — zero production callers | 1 | keep (note) | pub API surface with no consumer; recorded for R4.3's visibility pass, not a vocabulary fold |
| edge_specs machine-edge vocab ("belongs_to"…) vs GraphHumanRelation | 1 table | keep | different domains: machine edges vs human relations — not twins |

**R3.6 verdict summary:** the plan's letter ("constants duplicated between core and catalog") censused to ZERO const-level core↔catalog duplication (4 cross-crate groups, all coincidental/xtask); the REAL cross-crate vocabulary debt was (a) 8 inline-parse twins of catalog parse fns that already existed, and (b) the R3.5-seeded drill-slug wire pair. Executed: +24/−44 across 7 files + new 5-line catalog::routes (net −20); battery green at exact baselines (520/528/293+2/56, clippy x2, gate, icons, strip-check 207/207 — +1 file for routes.rs). STAGE R3 closes with this batch (R3.1–R3.6 all terminal).

### R4.1 — naming (session 13; census-first per standing discipline — scanner r41_census.py in tool-results: bool decls 341 (272 prod) + fn-returns-bool 118 + short-id inventory 478 kinds + suspicious-word pass 232; every weak candidate then read in context)

The plan's letter ("booleans read is_/has_; no non-domain-standard abbreviations") reads as a mass-rename mandate; the precise census shows the tree's naming is overwhelmingly honest — SEVENTH consecutive census correction. The rename set is surgical:

| module | construct | verdict | outcome |
|---|---|---|---|
| core/workers/execute.rs ×2 chat closures (translate ~1216, coaching ~1320) | `temp: f64` | rename | `temperature` — abbreviation collides with "temporary"; the callee `chat_qa` already names it temperature (LLM-sampling domain word); 2 params + 2 forwarding uses |
| core/migrations/db_breadth.rs:1108 | `let already: bool` (sqlite_master index check) | rename | `already_exists` — bare adverb → complete predicate |
| core/sync_engine/mod.rs:1378 + 1542 | `let already: bool` (queued rebuild_search_index job check) | rename | `already_queued` — mirrors the EXISTING `already_queued` local in the same engine family (loops.rs:1070, identical query) |
| core/ai_hub.rs:452, core/embeddings.rs:297+320+1460 | `let has: bool` (pragma_table_info column checks) | rename | `has_column` — bare "has" is an incomplete predicate; mirrors the existing has_column local (outreach/mod.rs:1934); 1460 rides inside a #[cfg(test)] audit fn — local var, not a golden key (D-06 intact) |
| core/intelligence_features.rs:170+177 | `let known: bool` / `let conversation: bool` | rename | `known_issue_exists` / `conversation_exists` — noun-as-bool → complete predicates; mirrors the *_exists vocabulary (conv_exists, notification_exists) |
| app-shell/palette.rs:96 | `palette_take: bool` field | rename | `query_committed` — "palette_take" does not say what it is (deferred Enter-in-query commit flag across the layout closure); sibling vocabulary: query, query_changed, wants_focus; 4 sites (decl/init/store/load) |
| app-shell/pages/{customers,incidents,organizations}.rs dt_cell ×3 | `right: bool` alignment param | rename | `align_right` — "right" ambiguous (side vs correctness); the body's right_to_left branch makes the alignment meaning explicit at the signature |
| app-shell/design/mod.rs metric_tile (~1255) | `drill: bool` | rename | `drillable` — noun-as-flag → predicate adjective; sits beside `drill_line`; 1 param + 3 internal uses; positional callers untouched |
| app-shell/design/mod.rs cop_chat (~2201) | `assistant: bool` | rename | `from_assistant` — noun-as-bool → directional predicate; the body already computes `align_right = !assistant` |
| core/embeddings.rs:225, core/outreach/mod.rs:1126 | `let mut ins` (prepared INSERT statements) | rename | `stmt` — the tree's established statement vocabulary (169 sites); intra-codebase consistency |
| core/segment/mod.rs:531 | `fn aa_get(obj, key)` | delete | cryptic name wrapping a literal `Map::get`; D-18's standing rule (module-local rewrite of an existing API shape = construction debt); 3 callers inline `obj.get(...)` — net −3 lines |

**R4.1 keep rulings (the census's honest majority — every class with its reason):**

- **Wire/persisted bool fields (invariant 5):** AuditEntry.ai_involvement (audit_log DB column mirror), ChatCompletionRequest.stream (OpenAI-compatible wire), HsField.required (HelpScout wire), SpanStyle.{bold,italic,underline,quote,code,strikethrough} (rich-text format), settings structs' *_enabled family (persisted JSON keys), import_result's `changed` (JSON-key mirror), event payloads' ok/success — names ARE the wire vocabulary.
- **Truthy state adjectives/participles:** enabled/configured/connected/loaded/selected/stale/active/finished/deleted/detected/escalated/verified/revoked/replied/required/focused/folded/running/editable/gated (~200 sites incl. the wire-locked) — read as complete predicates at use sites; forcing is_ prefixes would churn for zero readability (Q-03 family).
- **Predicate-phrase fns (118 bool-returning):** exists/matches/contains/available/configured/enabled/stale/healthy/answers/needs/requires families already predicate (std precedent: Path::exists, str::contains — the is_ prefix is optional when the verb already predicates); egui widget fns (nav_row, theme_row, result_row, empty_state_cta, armed_danger_button, switch) return activation by widget-fn convention; action fns returning success (upsert, drop_collection, ensure_collection, recover_interrupted_state, set_tab_by_label, spawn_sync_action, insert_gap_candidate, verify_backup, cached_verify) honestly name the action — the bool return contract is a type-level concern outside R4.1's names-only scope (recorded, not acted).
- **Flag-parameter idioms:** force, include_*, show_*, unread_only, default/value (setter/getter-with-default idiom), on (switch state), check (xtask --check CLI flag), read/draft (message domain), big/wide/slim (size adjectives), b_* builder-form family (reports.rs — 14-field coherent page-local prefix convention, D-13 family), initial (first-pass sync term of art), intersect (set-operation domain), escalation (signals-family vocabulary — DeterministicSignals/LocalAnalysis fields sit among noun siblings urgency/frustration/technical), ai_suffix (param says exactly what it toggles).
- **Short-identifier inventory (478 kinds):** full words at short length (conn/db/id/rows/stmt/page/kind/name/text/note/key/tag) + domain abbreviations (conv/org/tx/rt/sig/cfg/ctx/msg/req/res/idx/cc/bcc/href/sql/json/api/url/ui/ai/hs) + single-letter closures/tight-scope locals (t=tokens, |h| hits, i/j/k) + readable contextual (buf/addr/arr/obj/dup) — the tree's established vocabulary; no misleading short name survived the context reads.
- **Type/module names:** zero vague types (no Data/Info/Item/Thing/Helper/Manager/Util anywhere); page-local `enum Tab` ×8 + xtask Cli/Cmd are self-describing in context (D-13's module_name_repetitions family); D-13's self-named-module ruling stands.
- **Test regions (69 bool decls + junk/out2/res2 fixtures):** D-10-adjacent naming exemption — honest in test context, zero golden-key contact.

**Census finding recorded, not acted (wrong change class for R4.1 — same pattern as D-18's deferred truncation note):** dt_cell exists as 3 byte-similar private copies (customers.rs:892 / incidents.rs:1041 / organizations.rs:608) — a cross-page helper fold candidate (R3.4-family); the rename above is applied to all three copies identically, the fold itself awaits its own batch if ordered.

### R4.2 — decomposition, batch a (session 14; census-first per standing discipline — scanner r42_census.py in tool-results: cfg-aware char-level fn-span lexer over all 207 .rs files; 3,277 production fns / 991 test fns; the first scanner run mis-flagged impl-level fns after top-level `#[cfg(test)] mod` blocks — boundary-line gate added and re-run)

Top-20 by line span (census order) and the batch-a disposition:

| fn | span | verdict | outcome |
|---|---|---|---|
| ai_tools/execute.rs `execute` | 1051 | split | 26-arm tool dispatch → 22 `tool_<name>` fns (params = exactly what each arm uses; conn/args/limit/index variants); dispatcher now 79 lines; every arm body moved verbatim (early `return json!` arms keep semantics — the arm value IS the fn result) |
| support_graph/derived.rs `derived_edges_touching` | 686 | split | match-kind dispatch → 11 `<kind>_edges` fns + 2 epilogue fns (incident_related / custom_object_links) + closures promoted to module fns (one_i64, many_i64, product_id_by_name, mirror) + BRANCH_CAP to module const; dispatcher 26 lines; per-arm `Result` preserves the `?` abort order |
| pages/incidents.rs `ui` | 527 | split (b) | dispatcher ~180 lines keeps prologue/table/pick/detail-panel scaffolding; 5 methods (filter_controls = the filter_bar closure body; empty_list_ui; lifecycle_card_ui → Option<NavRequest>; linked_column_ui → Option<NavRequest>; info_column_ui incl. the add-note card — needed a db param the draft missed) + module fns incident_row (→Option<i64> picked) / incident_detail_header; the description-fields for-loop and columns(2) scaffolding stay in the dispatcher (they ARE the panel structure) |
| workers/execute.rs `execute_job` | 495 | split (partial) | 3 fat arms → methods along the file's own run_*_job seam: sync_conversation_ratings_job (63→thin arm+55-line fn), embed_chunks_job (3 structurally-identical embed arms folded to ONE fn-pointer-parameterized method — the fn's own DRY win), automation_approval_job (57); early-`return Ok(())` arms stay inline (that return SKIPS job completion — extraction would change the fail-path contract; D-23); post-split ~376 = 40-arm dispatch table, thin arms are dispatch data |
| migrations/db_breadth.rs `add_missing_reference_columns` | 489 | keep | flat schema manifest — every line is one declarative `add()` call; per-table sub-fns would be artificial seams over data (same class as icons::draw); the `add` helper is already the shared seam |
| pages/ai_center.rs `hub_ui` | 481 | split (b) | 35-line dispatcher calls 4 card methods in order; hub_endpoints_card_ui carries the card + per-endpoint row/key-edit/price/add-form methods; roles/features/ledger cards are single methods (features needs no tokens — verified); `let t` re-threaded per-card (tokens() is a static getter — behavior-neutral); the draft's double-card wrapper bug caught by the multiset check and flattened to direct calls (D-24) |
| people_store/health.rs `build_support_health_report` | 465 | split | 9 metric-domain fns (volume/waiting/first_response/bad_rating/escalation/customer_effort/repeated_issue/unresolved_known/incident) returning metrics / (metrics,flags) / (metrics,flags,exposure) tuples — no out-parameters (§5.3); the `ev` closure deleted, call sites → `evidence_ids(conn, conv_scope, subject_id, …)`; dispatcher = 34-line manifest + the report json |
| quality/mod.rs `analyze_friction` | 410 | split | 6 finding fns (repeated_customer_explanations / repeated_agent_questions / troubleshooting_loop / repeated_handoffs / repeated_unresolved / duplicated_request) each returning `Vec<finding-tuple>`; `push` closure → `finding()` helper fn; persistence + report mapping stay in the dispatcher |
| migrations/mirror_parity.rs `apply_m046` | 405 | split | 10 `if !fk_declared { rebuild_table }` scaffolds → ONE `ensure_fk` helper + 10 `M046_<TABLE>: Rebuild<'static>` data consts; apply_m046 is now a 15-line manifest (rebuild_customers + 10 ensure_fk + version stamp); m044/m045 keep their unconditional rebuild form (no guard to fold) |
| icons.rs `draw` | 396 | keep | 34-arm icon dispatch table, arms 3-28 lines each — the arms ARE the data; per-icon fns with geometry params would add signature noise for zero abstraction gain |
| sync_schema.rs `apply_inner` | 396 | keep | guarded one-shot migration sequence — flat manifest class |
| pages/organizations.rs `ui` | 393 | split (b) | dispatcher ~170 lines; org_row module fn returns (pick, edit, delete) triple via the row-closure re-thread; org_detail_header_ui returns (edit_req, delete_req); members_card_ui → Option<NavRequest> (member_request already returns Option — the draft's Some() wrap would have double-Optioned; fixed); org_form_card_ui method carries the form card verbatim; the draft's orphan `);` slicing bug fixed (D-24) |
| pages/inbox.rs `copilot_pane_ui` | 389 | split (b) | dispatcher keeps the header/hint/detail-gate/ScrollArea/error+report gates; copilot_cards_ui / copilot_lmstudio_ui / assistant_ui methods carry the three sections verbatim; allow deleted (dispatcher ~90 lines) |
| workers/mod.rs `execute_outbound` | 386 | split (b) | 8-arm match-expression producing Result<String> → 6 send_* methods (create_reply/create_note/update_status/assign/update_tags/snooze; unsnooze arm and the unknown-kind Err stay inline — dispatch data); the num/str_or closures promoted to module fns payload_num/payload_str_or (the batch-a closure-promotion recipe); no arm carried an early return so the D-23 early-return rule never triggered here |
| ai_pipeline/interaction.rs `analyze_interaction` | 376 | keep | two-stage LLM state machine (stage1 heuristic merge → stage2 rec) with dense local threading (obs/latencies/usage/merged/stored/stage_current) — extraction needs a context struct = re-architecture, not decomposition; one level of abstraction already holds (the pipeline) |
| migrations/db_breadth.rs `create_missing_reference_tables` | 342 | keep | flat schema manifest class |
| sync_engine/loops.rs `sync_resource` | 333 | split (b) | 19-arm match → 16 sync_*(&self, resource: String) methods; conversations/threads delegation arms + the customers since-seam + the unknown-resource fallback stay inline (dispatch data); all arms were value-producing so the D-23 early-return rule never triggered; uniform method form chosen over folds because the debug-message strings differ per arm (organizations/workflows/saved_replies) — verbatim-move discipline beats a parameterized fold that would mint new strings |
| pages/settings.rs `helpscout_ui` | 330 | split (b) | 4 card methods (hs_connection_card_ui/hs_credentials_card_ui with &HelpScoutStatus threaded from the dispatcher's prologue let; webhook_relay_card_ui; workspace_purge_card_ui); connection_doctor_ui call + add_space rhythm stays in the dispatcher; the draft's double-card wrapper bug caught by the multiset check and flattened to direct calls (D-24) |
| copilot.rs `chat` | 324 | keep | single conversation loop — one abstraction level (rounds of chat); extraction threads 9 accumulators for no gain |
| pages/diagnostics.rs `ui` | 322 | split (b) | event_log_card_ui method (events param + returns the clear_requested flag); audit_trail_card_ui/sync_reports_card_ui/outbound_deliveries_card_ui module fns (pure data renders — no self); prologue keeps events/runs/audit clones + message/error flush |

Batch-a mechanics: all six splits executed by line-boundary Python
transforms (tool-results r42_split1..6.py) with hard asserts on every
boundary; bodies moved verbatim except the documented rewrites (closure
→ module fn with conn param; `push(` → `out.push(finding(` with the
paren-balance fix; fn-pointer fold for the embed trio). Compile-check
after each split (cold core check ~7 min once, then ~6-8 s incremental).
Two clippy needlessly-borrow fixes post-extraction (`arg_i64(&args,` →
`arg_i64(args,` ×13 in the tool fns; `repeated_spans(&customer,` →
`repeated_spans(customer,`). fmt --all applied once (write mode), one
trailing-whitespace strip (regex-introduced) that had tripped a rustfmt
internal error.

### R4.3 — visibility (session 16; census-first: scanner r43_census.py in tool-results — G-08 cross-line string machinery with the G-14 line-continuation fix + newline-position invariant asserted per file; consumer counting over a global identifier token index with use-span leaf skipping, rename-alias resolution, and cfg(test)-region exemption)

**Census (1,094 pub(prod) + 19 pub(test-region) items):** the tree was already 2/3 visibility-honest (587 `pub(crate) fn` + 48 `pub(super) fn` + 136 `pub(crate) struct` existed pre-batch). Token-consumer classification: 866 keep-pub (real out-of-crate-root consumers — the 3-crate app's genuine API), 222 downgrade candidates (crate/file-local tokens only), 6 dead.

| class | items | verdict | outcome |
|---|---|---|---|
| keep-pub (out-of-root consumers) | 866 | keep | the genuine cross-crate surface; core→app-shell/xtask, catalog→core-re-export, app-shell-lib→bin/tests seams all hold |
| census downgrade candidates | 222 | apply, then rustc-reachability loop | applied 223 edits; the compile loop (E0603 + private_interfaces/private_bounds/private_fields under -D warnings discipline) restored ~140 as EXPOSURE-PROVEN keeps: pub-fn signatures (types returned across crate roots — the token census cannot see inference flows: `use super::*`, module aliases `use x as engine`, value-through-return usage), pub TRAIT methods (Hs* DTO family behind HelpScoutProvider), pub STRUCT fields (PageKey in page_header, ConfirmRequest.action, Tokens in design), pub-field chains (EvidenceContext.threads→ThreadEntry), bin-crate return types (StripStats via `pub fn run` into the xtask bin; BootServices/BootMode/AcquireError/InstanceLock via main.rs bootstrap), and ancestor-module paths (G-15: private covers descendants only) |
| surviving downgrades | 82 | done | 57 pub(crate) (incl. 9 MODULE caps: core maintenance/migrations/mirror/response_state_sql/sandbox/webhook + migrations::intelligence + app-shell app + design::scale::space — the cheapest lever, one line caps a whole subtree) + 8 pub(super) (inbox_geo geometry consts + settings save) + 17 private; pub surface 1,113→1,023 |
| dead items (zero consumers, compiler-verified) | 9 + 2 fields | delete | SP6; customer_events rebuild; customers customer_timeline (+ cascade: CustomerTimelineEntry struct never constructed, fts_row never used); mentions team_members; operations count_of; response_state_sql filter_by_response_state; knowledge_store search_knowledge (PHANTOM consumer — the token census credited the search/lanes.rs same-named twin's callers; dead_code after downgrade exposed it) + fts_row cascade; AiJobRow slimmed of error/latency_ms (constructed-never-read fields). Unused-import cleanup rode each deletion |
| pub(test-region) items | 19 | keep (D-10) | test regions exempt; zero API meaning |
| pub struct fields (1,861) | census only | keep | fields ride their type's decision: capped types cap their fields, kept-pub types keep them (the tree's data surface); a field-level pass would churn 1.8K lines for zero reach change |
| CopilotTool (catalog) | 1 | keep-pub (Q-05) | dead vocabulary type (zero tree-wide consumers beyond its own root re-export) — kept per the R3.6 GraphHumanRelation::parse precedent (catalog = deliberate closed-vocabulary surface); deletion offered to the owner as Q-05 |
| `pub use` re-exports | 13 | keep (12) + 1 downgraded→reverted | all root re-exports carry out-of-root consumers (catalog vocabulary via core's glob seam; Error/Result; parse_report_config; execute; EmptyHelpScoutProvider is main's `spp_core::helpscout::` path). The helpscout/mod.rs EmptyHelpScoutProvider re-export downgrade was reverted in-loop (app-shell consumes exactly that path) |

**R4.3 verdict summary:** the NINTH consecutive census correction — the plan's "minimize pub" letter resolved to 82 honest downgrades + ~140 compile-proven keeps + 9 dead deletions; the compile loop IS the visibility arbiter (rustc's effective-reachability computation sees signature/transitive/inference exposure no token census can). Net pub surface −90 (8%), 40 files, +101/−321 (net −220: the dead code the pub veil had hidden). Battery green at exact baselines; pixel-neutral by construction (no widget-call motion — D-24's multiset check unnecessary for prefix-only edits, CI kittest drift gate rides the push).

### R4.4 — magic values (session 17; census-first: scanner r44_census.py in tool-results — the G-08/G-14 char-level walk lifted from r43, extended to COLLECT string-literal values + numeric literals from the sanitized text + `.take(N)`/SQL-`LIMIT N` idiom overlays + a const-initializer inventory so already-named values are distinguishable from raw call-site literals)

**Census (7,823 production numeric occurrences / 456 distinct values; 17,350 production string occurrences / 8,739 distinct):** the tree was overwhelmingly honest — the same-policy set is SIX families / 166 sites. The D-17 coincidental-value ruling (same number ≠ same policy) dissolved every large group: 200/500 mixes HTTP statuses with truncation caps and SQL LIMITs; 365/3600/86400 are civil-date ALGORITHM constants; 250 is mostly rgb() palette components; 32.0 spans five unrelated height policies.

| family | sites | verdict | outcome |
|---|---|---|---|
| `"(no subject)"` display fallback | 14 / 10 files / 2 crates | merge → catalog | `catalog::NO_SUBJECT` (root const, cross-crate per the R3.6 placement rule); core sites `crate::catalog::NO_SUBJECT` (10), shell sites `spp_core::catalog::NO_SUBJECT` (4); byte-identical output on every unwrap_or/unwrap_or_else form |
| `"—"` missing-value placeholder | 38 / 11 files / 1 crate | merge → design | `design::MISSING_VALUE` (pub(crate) root const); includes the sync_health `== "—"` SENTINEL COMPARISON (producer + comparator now single-sourced — the strongest DRY case in the batch) and the diagnostics `(BadgeTone::Neutral, "—")` tuple arm; single-crate → design home per the D-19 architecture |
| `"error: {err}"` page-error note | 17 / 17 files | merge → design | `design::error_note(ui, err)` — all 17 sites were byte-identical `error_text(ui, &format!("error: {err}"))` calls (R3.4-shaped fragment fold); `error_text` stays for custom messages |
| `28.0` single-line control height | 72 / 26 files | merge → scale | `design::scale::CONTROL_H` (pub(crate), scale root beside CONTENT_MAX): 66 `[w, 28.0]` add_sized arrays + 3 data_table row-height args + 2 inbox row/frame allocations + 1 design chip height. KEEPS: `AvatarSize::Md => 28.0` (avatar-scale DATUM, not the control-height policy — D-17) and design:2111's `+ 28.0` horizontal pad |
| `32.0` page-local `ROW_HEIGHT` decls | 3 identical consts (incidents/customers/organizations) + 11 refs | merge → scale | `design::scale::ROW_H` (pub(crate)); three same-named same-valued same-role consts were one policy; the OTHER 32.0s (egui interact_size, button heights, inbox taller inputs, composer) are separate policies sharing the number — keep (D-17) |
| `240` coaching excerpt cap | 15 (checks ×13 + mod ×2) | merge → module-local | `coaching::EXCERPT_CAP` (private; checks.rs sees it via its `use super::*`); KEEPS: search/mod.rs:422 `take(240)` (search-chunk snippet cap) and ai_pipeline/interaction.rs:425 `take(240)` (analysis excerpt) — coincidental values, different policies |
| civil-date algorithm constants (146_097/719_468/400/365/3600/86400) | ~30 / 6 files | keep | the Howard Hinnant civil_from_days family — algorithm DATA, not domain policy; naming them adds indirection without meaning |
| HTTP status literals (200/404/409/503…) | ~30 sites | keep | wire-protocol vocabulary, self-documenting at the match ladders; the R3.6 DB-DDL ruling extends: protocol constants are not domain magic |
| `.take(N)` truncation caps, other values (200/300/400/500/8000/1200/2000…) | ~80 sites | keep | per-site prompt/display budget knowledge; values coincide across consumers (D-17's three-unrelated-200s ruling); audit's 2-site 4000 pair is below the cross-module bar; D-18's deferred truncation-idiom note stays deferred (Q-03 family) |
| `LIMIT N` SQL caps | ~120 sites | keep | per-query policy; SQL strings stay literal (R3.6 DDL ruling: format!-parameterizing SQL is anti-DRY and drift fails closed+loud) |
| `rgb()` palette components | ~500 numeric sites | keep | palette DATA already owned by the named LIGHT/DARK token tables — the components are the tokens' definitions |
| wire-vocab strings (`'status'`/`'id'`/`'created_at'`/`'conversationId'`…) | thousands | keep | invariant 5: serde fields/DB columns/JSON keys are vocabulary, not magic |
| domain word comparisons (`== "high"` etc., 6 words) | 25 sites | keep | different domain DIMENSIONS sharing English words (confidence-high vs priority-high vs friction-high); the owning enums' as_str() are single-sourced in catalog; folding 25 scattered comparisons of six free-form wire fields is semantic-risk churn, not DRY |
| serde attribute strings (`'Option::is_none'` ×63, `'snake_case'` ×18) | 81 | keep | macro-machinery spelling required by serde's attribute API (skip_serializing_if / rename_all) |
| UI action labels (`'Cancel'` ×17 / `'Edit'` ×13 / `'Delete'` ×12) | 42 | keep (Q-06) | action-label PROSE: design::button is the single-sourced renderer, the label is the datum (D-23's indirection-without-abstraction ruling); the crisp line vs the folds above: fallback/SENTINEL strings are data (compared/reused as values — they folded); rendered-once words are prose. Q-06 records the owner option to fold them |
| icons.rs geometry coordinates | ~289 numeric sites | keep | per-icon drawing data (paths/quads/dots at literal coordinates) — not sizing policy |
| remaining pixel values (12.0/16.0/220.0/320.0/…) | ~1,500 sites | keep | per-site layout knowledge; the token system (space/text/radius + now CONTROL_H/ROW_H) owns the REUSABLE dims; a call-site 16.0 meaning icon-pixels or a width is not SP4's policy (D-17) |

**R4.4 verdict summary:** the TENTH consecutive census correction — the plan's "magic values become named constants" letter resolved to 6 folds / 166 sites out of 7,823+17,350 production literal occurrences. All folds value-identical (const value byte-equals the replaced literal) → pixel-neutral BY CONSTRUCTION; the CI kittest drift gate is the verifier. 30 files +254/−184 after fmt (reflow accounted); battery green at EXACT baselines (520/528/293/56, clippy ×2, gate, icons, strip 207/207). New policy recorded as memory D-26; the action-label option recorded as Q-06.

### R4.5 — file splits, batch a (session 18; census-first: line counts re-measured for every .rs file — 12 files now sit >2K, the session-1 top-10 plus quality/outreach; the prod/`#[cfg(test)]` split via the extractor's string-aware walk decides the tier; counts below are pre-batch)

| file (pre-batch total) | prod / test split | verdict | outcome |
|---|---|---|---|
| pages/inbox.rs 4543 | 3578 / 965 (6 mods) | split a+b | dir conversion + 6 mod files → host 3590; prod-region split queued to b (list/copilot/composer clusters) |
| pages/settings.rs 3235 | 2533 / 702 (3 mods) | split a+b | dir conversion + 3 files → 2539; queued to b (helpscout cards / oauth / doctor families) |
| core/workers/mod.rs 2807 | 1373 / 1434 (7 mods) | split a ONLY | 7 files extracted → 1387 — UNDER threshold, done |
| core/ai_pipeline/mod.rs 2768 | 1935 / 831 (5 mods) | split a ONLY | 5 files extracted → 1947 — under ~2K, done (the `#[cfg(test)] fn digest_row` helper and the interstitial prod fns stay) |
| core/helpscout_real/mod.rs 2658 | 2336 / 322 (4 mods) | split a+b | 4 files extracted → 2344; queued to b |
| pages/knowledge.rs 2516 | 1914 / 602 (1 mod) | split a ONLY | dir conversion + 1 file → 1916 — under, done |
| design/mod.rs 2510 | 2281 / 229 (1 mod) | split a+b | 1 file extracted → 2283; queued to b |
| core/workers/execute.rs 2455 | 2452 / — | b only | no test mass; the 22 R4.2-a tool fns are the seams |
| pages/reports.rs 2304 | 2160 / 148 (2 mods) | split a+b | dir conversion + 2 files → 2160; queued to b (csv_contains_id/toggle_csv_id helpers stay in mod.rs — they sit BETWEEN the two mods) |
| pages/ai_center.rs 2269 | 2001 / 268 (1 mod) | split a ONLY → at-threshold keep | extraction → 2003; the prod region sits AT the ~2,000 threshold after R4.2-b's 8-method decomposition — KEEP (D-27 ruling) |
| core/quality/mod.rs 2159 | 1618 / 541 (4 mods) | split a ONLY | 4 files extracted → 1626 — done |
| core/outreach/mod.rs 2144 | 1343 / 801 (4 mods) | split a ONLY | 4 files extracted → 1351 — done |

**R4.5-a verdict summary:** the ELEVENTH consecutive census correction — the plan's six named candidates met twelve current >2K files, but the honest first split point was the cfg(test) mod boundary: 37 inline test mods / 6,843 lines moved to 37 module files with ZERO semantic change (module paths, visibility, and `use super::*` resolution identical by construction — the move is pure file layout; `git mv` preserves rename history for the five flat→dir conversions: inbox/settings/knowledge/ai_center/reports). Six files exit the >2K set entirely; six remain for batch b. Keeps recorded: ai_center at-threshold; custom_objects 1961 and sync_engine 1811 carry big test mods but sit under 2K — no trigger, no churn (D-23 discipline). Battery green at EXACT baselines (520/528/293+2/56, clippy ×2, gate, icons, strip 245/245 — the 37 new files ride the whole-tree check); kittest-ui feature `--all-targets` compile pre-verified locally (the CI kittest lane compiles the suites with the feature on — their imports are unchanged). Pixel-neutrality: no widget call touched (module layout only); the CI kittest drift gate rides this push as verifier. New memory: D-27 (two-tier split ruling) + G-16 (string-brace span trap); G-10's background-death list re-confirmed.

### R4.5 — batch b (session 19; prod-region splits along the R4.2 seams; counts below are post-batch-a)

| host (pre-b) | child file(s) | lines moved | verdict notes |
|---|---|---|---|
| pages/inbox/mod.rs 3584 | composer.rs 314 (ToolAction + composer_tool_row items + thread/composer_pane/enqueue_composer_tool/apply_composer_act/mention_popup_ui methods) / copilot.rs 582 (copilot_pane/cards/lmstudio/assistant + insert_ai_draft/enqueue_composer_ai/poll_pending_ai/poll_assistant) / list.rs 782 (list_ui/search_filter/list_toolbar/conversation_rows/apply_filters/refresh_view_counts/reader/reader_head/action_bar/triage/briefing) | 1,664 | the three R4.2-b families; host 1924 — dispatcher ui + state/reload family + the coaching/state_history/qa impl stay (not R4.2-b seams, D-23) |
| pages/settings/mod.rs 2536 | helpscout_cards.rs 411 (helpscout_ui + 4 card methods + save_webhook_relay_url/queue_webhook_health_check) / doctor.rs 158 (run_connection_doctor + connection_doctor_ui + poll_doctor_result) / oauth.rs 161 (begin_oauth/at/on + connect_client_credentials + poll_connect_result) | 731 | the helpscout-card/oauth/doctor families; host 1824 — general/lmstudio/vector_index/hours/backups tabs stay with the dispatcher |
| core/workers/execute.rs 2455 → execute/ dir | ai.rs 761 (run_ai/attributes/memory/clustering/rewrite/translate/coaching/qa_ai/interaction/copilot_chat) / ops.rs 1077 (sync_conversation_ratings/embed_chunks/automation_approval + run_golden..run_attachment_downloads + mailbox_local_id) | 1,695 | the 22 tool fns = the seams; host execute/mod.rs 630 (execute_job dispatcher + lifecycle on_after_initial_sync/on_conversation_changed + run_engine + AiJob) |
| core/helpscout_real/mod.rs 2340 | credentials.rs 288 (TokenRow/token_row/TOKEN_EXPIRY + HsCredentials + CredentialSource(s) + merge_field/base64_of/stored_api_concurrency/urlencode + the SECRET_* consts) / rate_limiter.rs 179 (RateLimitState + HsRateLimiter + QueueRateLimiter impl) | 465 | the two standing domain families (http.rs precedent); host 1891 — provider struct + impls + the trait impl stay |
| design/mod.rs 2282 | composer.rs 345 (ComposerMode/ComposerAct/composer/composer_header_row/composer_footer_row/ComposerState/cop_suggest/cop_chat) / scaffold.rs 99 (page_header/centered_column/content_column/host_standard_page/host_inbox_page) | 444 | the composer/scaffold clusters; host 1853; paths preserved via `pub use` re-exports (incl. the mod-composer/fn-composer namespace pair — different namespaces, verified legal) |
| pages/reports/mod.rs 2158 | sla.rs 719 (sla_ui + pick_sla_cfg/save_sla_cfg + build_config/builder_ui/format_metric_value/save_report_config/load_saved_report/delete_saved_report) / insights.rs 373 (insights_ui + add_graph_node_action/inspect_graph_node/refresh_graph) | 1,092 | the two heavy tab families; host 1078 — ui dispatcher + overview + the ≤160-line tab views + csv helpers stay |

**R4.5-b verdict summary:** six hosts / 6,198 lines / 14 new child files; every split rides the R4.2 method-cluster seams named in the batch-a verdict table (reports had no R4.2 seam — its Tab families are the standing structure, the two >250-line families moved, the small tab views stay, D-23). G-15 ladder end state: impl-member moves need ONLY visibility (method-call syntax is path-free) — 78 private methods widened to exactly pub(super) (parent-dispatcher / cross-sibling / test-mod callers each proven by the consumer census; run_attachment_downloads was already pub(super) and kept its level); mailbox_local_id stayed private (its only caller moved with it); TokenRow + 4 fields widened to pub(super) (G-18 — the http.rs sibling reads them); design/helpscout_real top-level items keep their pub/pub(crate) levels with `pub use` re-exports keeping every existing path byte-stable (the pub consts SECRET_DOCS_API_KEY/SECRET_OAUTH_* included), while the four sibling-used private helpers (token_row/base64_of/urlencode/stored_api_concurrency/TOKEN_EXPIRY) got pub(super) + host `use` bindings so http.rs's `use super::*` glob still resolves them unchanged. Conservation machinery (r45b_split.py): string-aware boundary-depth asserts, per-child marker-count asserts, per-child and per-host whitespace-insensitive char-multiset conservation (old + additions == new — G-16's same-artifact lesson), git-mv rename history for execute.rs → execute/. D-24 multiset pre-gate: 4/4 UI hosts byte-identical widget-call multisets (268/255/240/272 calls). Battery green at EXACT baselines (520/528/293+2/56, clippy ×2 -D warnings, gate, fmt, icons, strip 259/259 = 245+14 new files); kittest-ui --all-targets compile green locally (42.8s); the CI kittest drift gate rides this push as the final pixel-neutrality verifier. Post-split tree: largest file = ai_center 2003 (the D-27 at-threshold KEEP) — the >2K set is EMPTY. New memory: D-28 + G-17/G-18/G-19.

## Closing verification statements (R6.2 — every line checked)

Every module cluster, the passes that swept it, and the end state the
battery + CI verified on the final head. "R1/R2/R3/R4" refer to the
verdict tables above; every cluster is additionally covered by the
whole-tree checks (strip-comments 259/259, clippy -D warnings both
lanes, the no-simulation gate, kittest drift gate).

| Cluster (files) | Passes applied | Verified end state |
|---|---|---|
| core/domain — people/customers/organizations/incidents/inbox/chats/coaching/custom_objects/analytics/automation/activity/digest/issue_impact/interaction_*/intelligence_features/richtext/business_hours/crypto/error | R1 (R1.2/R1.5/R1.6) · R2 (R2.1/R2.2/R2.4/R2.5) · R3.1 errors · R3.2 SQL · R3.3 json_fields · R4.1/R4.2-a/R4.3/R4.4 | comment-free; zero dead code; expects D-14-classed; errors thiserror-derived; row-collect via db::query_rows; naming honest; visibility reachability-proven |
| core/storage — db/migrations/maintenance/mirror/vectorstore_sqlitevec/oauth_state/backup_service/audit/config/data_reset | R1 (R1.5) · R2 (incl. R2.4 poison recovery) · R3.2 · R4.1/R4.3/R4.4 | same + sqlite-vec FFI the one confined unsafe island (D-13); oauth_state range unreachable! carries its D-15 proof; audit.rs gate-allowlist invariant 7 held all 20 batches |
| core/sync+workers — sync_engine/workers(+-execute dir)/connectors/loopback/api_queue/jobs/events/event_bus/provider_handle | R1 (R1.3) · R2 · R3.2/R3.3 · R4.1/R4.2-a/R4.2-b/R4.3/R4.5 | execute 1,051-line mega-fn → thin dispatcher + 22 arm fns; execute.rs 2,455 → execute/{mod,ai,ops}; the execute_job early-return rule held; poison recovery; D-14/D-15 keeps written |
| core/helpscout — helpscout_real({mod,credentials,rate_limiter})/helpscout + it_helpscout | R1 (R1.4) · R2 · R3.3 (docs_request_json) · R4.2-b/R4.5-b | split children; token envelope single-sourced; token refresh arms message-normalized |
| core/ai_* — ai_pipeline/ai_tools/ai_attributes/ai_center/ai_eval/ai_evidence/ai_hub/ai_lm_studio/ai_local/ai_memory_guard/ai_prompts/ai_provider/copilot/quality/outreach/search/knowledge_store/live_chats/embeddings | R1 (R1.2) · R2 · R3.3 (send_json/authorized/json_fields) · R4.1/R4.2-a/R4.3/R4.4 | comment-free; LM-Studio send tails consolidated; qdrant filter rebind; kittest-ui deterministic clock pin (UXB-T1) — goldens' date-ticks frozen across the whole program |
| app-shell/design — design/{mod,composer,scaffold,controls}/icons/chart/palette/theme | R1 (R1.7) · R2 · R3.4/R4.4 (hosts/CONTROL_H/ROW_H/MISSING_VALUE/error_note) · R4.2-b/R4.5-b | hosts shared 21/23 pages; scale tokens single-sourced; splits along composer/scaffold seams; icons geometry kept as drawing data (D-26) |
| app-shell/pages — 23 pages + the R4.5 split children + queries/json_fields/sync_health | R1 (R1.8/R1.9) · R2 · R3.4 sync_now_button · R4.1/R4.2-b/R4.3/R4.4 · R4.5-a/b | every page >2K split (inbox/settings/reports incl.); >2K set EMPTY; goldens byte-identical through runs #311–#358; page dispatch single-sourced (R3.5) |
| app-shell/infra — app/{mod,chrome}/main/single_instance/task_bridge/workers_host/fatal/toasts/overlays/mention_popup/smoke | R1 (R1.7) · R2 · R4.3 (app module cap) | comment-free; single_instance the shell's one FFI island (D-13); main.rs carries ZERO unwrap/expect (DoD bootstrap line unused); class-D infra keeps written (D-14) |
| app-shell/tests — 26 kittest suites + style_lint + testsupport + snapshots | R1 (R1.9) · R2.2 (test-region allows, D-10/D-13) | comment-free incl. fixtures; suite/fn names stable (D-06); 203 goldens + 2 style-lint tests = the drift gate that never fired |
| catalog | R1 (R1.1 — was already clean) · R3.6 (routes) · R4.4 (NO_SUBJECT) | 9 files / 1,028 LOC; closed vocabulary + routes wire consts; no I/O deps (F-02) |
| xtask — package/deb/icons/strip/lib/bin | R1 (R1.1) · R2.2 (machete false-positive keeps) | comment-free; strip-comments tool (R1.0) with 38 edge tests is itself the tree's comment verifier; 43 dev-machinery occurrences D-10-exempt |
| build files (root + crates manifests, deny.toml, rust-toolchain, .cargo, .gitignore, workflows, gate script) | R6.1 | comment-free (279 lines removed; D-29 taxonomy; shebang/docstrings functional keeps); functional zero-diff four-way proven; deny lane green against the live advisory DB on run#358 |

## Session log

### Session 25 — 2026-11-16
- Session-start protocol: environment intact (toolchain 1.99.1 at
  /tmp/my-project/{rustup,cargo-home}; HEAD edffa10 = origin/main,
  clean; CI run#385 green on the terminal head). Residue audit
  (owner's question): DoD closed, Q-01..Q-06 terminal; residue =
  the census-noted future-batch items (D-18 truncation idiom,
  R4.1 dt_cell), D-19(a)'s rejected title proposal, the mooted
  gate-docstring option, and D-36's R8-material pedantic line.
- OWNER DIRECTIVE received: "finish everything, take the most
  logical decision to complete everything" → PROGRAM 3 OPENED
  (plan §12; handover state line; memory directive note + D-37).
- R8.1 executed (census-first, two passes — line-level r25
  machinery + a text-level pass after the line scanner proved
  blind to multiline chains, 23 sites): true total 125 raw
  `.chars().take(N).collect()` occurrences = 121 production + 2
  test-region (D-10) + 2 xtask machinery (D-14). Fold: textutil
  (the existing text-mechanism home, pub(crate)→pub) gains
  `cap_chars` (pub, #[must_use]) + `truncate_chars` (pub(crate),
  the notifications/sweep twin fold — sweep's bare `trim` name
  fixed with it) + `excerpt` (pub(crate), the
  interaction_signals/extraction twin fold); analytics' hard-cap
  `truncate_chars` local deleted (its caller → cap_chars); domain
  variants kept (quality::excerpt_of, inbox::truncate_chars) with
  inners dogfooding cap_chars (D-17; full ruling in memory D-37).
  Terminal raw census: 4 occurrences (definition + 1 test + 2
  machinery). 41 files, +274/−314 after fmt; 118 external
  cap_chars call sites; truncate_chars 14 external callers;
  excerpt 4.
- Tooling fallout handled: fmt caught two imports placed inside
  multi-line braced use statements (ai_evidence, helpscout_real)
  — moved after the complete statements; clippy is the
  needless_borrow classifier for the mechanical `&recv` prefix.
- Sandbox constraint: cold cache + 4G RAM / 2 cores / no swap —
  the default-parallelism clippy build died silently mid-crate
  (OOM signature); detached background builds are reaped at
  tool-call boundaries (setsid does not escape it) — the battery
  runs in foreground chunks (timeout 560-570, incremental resume).
- Battery GREEN at the EXACT terminal baselines on the recovered
  cache: gate clean; unwrap-policy clean (production unwrap 0 /
  expect 1 allowed / panic-family 0; machinery 3+1; 2381
  test-region exempt); fmt --check; clippy x2 under -D warnings;
  core 520 at 490s; core-sandbox 528 at 458s; app-shell 293 at
  264s; catalog+xtask 79 (= the 56 baseline + R7.5's 23 scanner
  tests — the terminal tree's count); verify-icons; strip-comments
  261/261. Post-fix census: 121 cap_chars occurrences (118
  external call sites + definition + 2 internal), 40
  needless_borrow sites stripped by `cargo clippy --fix` (the
  classifier pass worked as designed); 45 files, +435/−324.
- R8.1 committed as 475e055 and pushed; CI run#386 GREEN on
  475e055, all four lanes — the kittest drift gate passed, so the
  fold is pixel-verified (203 goldens byte-identical).
- R8.2 executed (R4.1's deferred dt_cell note): the three
  byte-identical private copies folded to one pub(crate) fn
  dt_cell in design beside data_table (self-module scale::ROW_H;
  11 call sites qualified design::dt_cell; zero import churn —
  the pages' existing `use crate::design::{self, …}` carries it).
  Ruling in memory D-38. Battery green at the EXACT baselines
  (gate, unwrap-policy 0/1/0, fmt, clippy x2, core 520 at 471s,
  core-sandbox 528 at 451s, app-shell 293 at 289s, catalog+xtask
  79, verify-icons, strip 261/261). One environment event: the
  disk filled mid-lane (target 7.5G on the 9.9G root; the
  multi-variant cold build) — freed 4.0G by deleting
  target/debug/incremental; the 8 xtask test failures in the
  interrupted lane were disk-full artifacts, re-run clean (79/79).
- R8.3 ruled (D-39): page-title label() consolidation REJECTED
  terminally under the most-logical test (indirection without
  abstraction; D-19(a) annotated).
- R8.4 ruled (D-39): the gate-docstring option annotated MOOTED
  by R7.5 (scripts/ deleted; the note stands as history).
- R8.2 committed as d3cdb36 and pushed; CI run#387 GREEN on
  d3cdb36, all four lanes — the kittest drift gate passed (the
  dt_cell fold is pixel-verified; 203 goldens byte-identical).
- R8.5 executed: workspace version 1.10.0 -> 1.10.1 (root
  Cargo.toml; members are version.workspace) + Cargo.lock synced
  (exactly the 4 workspace-member version fields — the lock's
  fifth 1.10.1 is httparse, a pre-existing coincidence) +
  CHANGELOG [1.10.1] section (the residue-closure patch: the two
  folds + the terminal rulings, zero-behavior-change framing).
  Drift safety re-verified per the R7.6 analysis: CARGO_PKG_VERSION
  display sites are app/mod.rs:507 (sidebar chrome) + settings
  x2 + fatal.rs (error-path); grep confirms NO version content in
  any golden. Fast local gates green on the bumped tree (gate,
  unwrap-policy 0/1/0, fmt --check, clippy -D, verify-icons, strip
  261/261); full battery rides CI per the R7.6 precedent (the
  code tree differs from run#387's head only in version fields +
  ledger docs).
- R8.5 committed as d4a19f5 and pushed; CI run#388 GREEN on
  d4a19f5 (all four lanes; kittest drift gate green — zero golden
  drift as predicted); tag v1.10.1 pushed at d4a19f5 -> tag CI
  run#389 GREEN + Release run#39 GREEN (all 6 jobs:
  build+test+package, verify .deb ubuntu:22.04/24.04 + debian:12,
  verify AppImage, publish). Published 2026-11-16T18:10:27Z;
  live-verified: BUILD-INFO shows version 1.10.1 / commit
  d4a19f5bd8c4… (exact match), both assets HTTP 200 through the
  public URLs (AppImage 13,072,888 B; .deb 10,399,802 B),
  SHA256SUMS present (030918e2… / 3a2ff473…).
- PROGRAM 3 CLOSED (Rule 8 STOP): closing report in complete.md;
  pointer updated; handover state line -> all three programs
  closed; memory directive note closed out. Final state: 261
  files / 139,820 LOC / 0 comments / 0 production unwraps / 1
  justified expect / 0 panic-family / 203 goldens byte-identical
  / pub +1 (cap_chars). Terminal-head CI verification follows on
  this closing docs-only head (the R3.5 precedent; code tree
  byte-identical to run#388's d4a19f5).

### Session 24 — 2026-11-16
- Maintenance pre-flight: sandbox reset again; re-cloned via
  github-reconnect.sh; toolchain intact at /tmp/my-project/
  {rustup,cargo-home} (rustc 1.99.1). The R7.4-c ledger catch-up
  addendum (d8b770b) landed first (run#378 green on 4fd37af
  recorded); its own CI run rides this batch's verification.
- R7.5 executed (Q-01 guardrails, the ONE-batch letter):
  (1) R5.1 — scripts/gate_no_simulation.py ported line-for-line
  into crates/xtask/src/gate.rs (Cmd::Gate; the line-based
  cfg(test) blanker with both documented quirks preserved —
  brace-less mod decl regions end at the marker line;
  #[cfg(test)] inside a // comment is not an attribute; the
  audit.rs allowlist content-keyed with the same justifications;
  10 unit tests). One semantic necessity discovered: the Rust
  module SELF-FLAGS (it carries the marker vocabulary) — the
  python predecessor was structurally outside the scanned set;
  reproduced explicitly via MACHINERY_EXEMPT (D-35; the
  self-allowlist alternative is rustfmt-brittle). Both gates
  verified CLEAN on the tree pre-deletion. ci.yml: gate step
  moved after toolchain+cache (cargo needed); release.yml:
  python3 dropped from the container (G-03 RESOLVED — the build
  is single-language; the .py deleted with scripts/).
  (2) R5.2 — crates/xtask/src/unwrap_policy.rs (Cmd::UnwrapPolicy
  + CI step): string-aware scanner (normal/raw string contents
  blanked; char literals via the k=tick+1 escape-pair consumer —
  the k=tick+2 form OVERSHOOTS on '\\'' and desyncs every later
  line, G-22, caught by phantom violations + a stale-entry
  report for a live site); region walker handles the two r25
  blind spots (cfg(test) on MULTI-LINE fn signatures — the
  digest sites — and on MATCH ARMS — the batch-27 __test_panic);
  brace-less mod decls resolve the 45 external whole-file
  cfg(test) modules (exact r25 agreement, 45=45 empty diff).
  ALLOWED = 5 justified entries (interaction_signals D-14 keep;
  strip.rs expect + package.rs x2 + bin/xtask.rs machinery
  unwraps). Policy: production unwrap 0 / panic-family 0 in BOTH
  buckets / every expect on the exact allowlist (stale entries
  also fail). Clean-run census: production unwrap 0 / expect 1
  allowed / panic-family 0; machinery 3 unwrap + 1 expect
  allowed; 2381 test-region occurrences + 45 whole-file modules
  exempt. 13 unit tests.
  (3) R5.3 — the plan's "via clippy.toml" corrected (clippy.toml
  cannot ENABLE lints, D-36): [workspace.lints.clippy] with 20
  ZERO-FIRE high-value pedantic lints + `[lints] workspace =
  true` x4 members + clippy.toml (msrv 1.99, the letter's named
  file). Selection: full -W clippy::pedantic inventory on BOTH
  clippy surfaces (74 lints fired ~1800x; top:
  redundant_closure_for_method_calls 361, too_many_lines 173,
  map_unwrap_or 144, casts ~430 — deliberately NOT enabled;
  remediation is R8-material, not a guardrail). Four default-
  lint fixes in the new code (int_plus_one, manual_repeat_n,
  collapsible_if x2 -> edition-2024 let-chains).
- Battery green at exact baselines: gate clean (new xtask lane),
  unwrap-policy clean (new lane), fmt --check (fmt --all run
  BEFORE the battery, G-21), clippy workspace + core sandbox
  -D warnings (re-run post-fmt), core 520 (497s), core-sandbox
  528 (453s; one G-05 incremental wipe mid-battery — disk hit
  96% on the sandbox feature rebuild, standard rm -rf
  target/debug/incremental remedy, 4.2G freed), app-shell 293+2
  (263s), catalog+xtask 79 (56 baseline + 23 new unit tests),
  verify-icons, strip-comments 261/261 (259 + the 2 new
  modules). Diff reviewed pre-commit (session-22 lesson).
- CI verification: run#379 (id 43839357771) GREEN on d8b770b
  (docs-only R7.4-c catch-up addendum). Run#380 (id 43852870658)
  GREEN on 84cc569, all four lanes — the check lane exercised the
  NEW no-simulation gate and unwrap-policy steps on a real runner
  (both first-try success) and the pedantic subset live under
  -D warnings; kittest drift gate green (no app code touched —
  zero golden churn); packages x86_64 + arm64 green. Q-01 CLOSED.
- R7.6 executed (Q-02 version 1.10.0 + tagged release): sandbox
  reset AGAIN mid-batch — the uncommitted CHANGELOG draft was lost
  and rewritten from the session context (the session-21 lesson
  now doubly reinforced: commit before anything long). Version
  bump 1.9.8 -> 1.10.0 (workspace.package; Cargo.lock synced via
  `cargo update --workspace --offline` — exactly the 4 member
  version fields, no dependency churn). CHANGELOG [1.10.0] section
  + link ref. Drift safety verified pre-push: the CARGO_PKG_VERSION
  display lives in sidebar_ui (shell chrome — no kittest suite
  renders the sidebar; the suites render pages through the host
  funnel only) and fatal.rs/smoke.rs carry no golden-visible
  version content -> ZERO golden drift expected and CONFIRMED
  (kittest lane green on both the bump head and the tag run).
  Fast gates re-run green on the bumped tree; the full battery
  rode CI per the R3.5 docs-only precedent (code tree byte-
  identical to run#380's).
- Release evidence: CI run#381 GREEN on bdb6949 (R7.5 addendum);
  run#382 GREEN on 2f8a192 (the bump head, all four lanes); tag
  v1.10.0 pushed at 2f8a192 -> CI run#383 GREEN + Release run#38
  (id 43869218887) ALL 6 JOBS GREEN (build+test+package on
  ubuntu:22.04, verify .deb on 22.04/24.04/debian:12, verify
  AppImage, publish). Published 2026-11-16T13:17:07Z; live
  post-publish verification: release not-draft/not-prerelease,
  target 2f8a192, all 4 assets uploaded; BUILD-INFO.txt fetched
  live (version 1.10.0, commit 2f8a192c6225...); both binary
  assets HTTP 200 via the public download URLs; SHA256SUMS
  fetched live (AppImage 26db0052eaa8…, .deb 9c31bff9e42a…).
  Q-02 CLOSED.
- PROGRAM 2 CLOSED (Rule 8 STOP): complete.md carries the
  Program 2 closing report (R7.1–R7.6 terminal records + final
  metrics); the DoD checklist amended (R5 reopened-and-closed;
  Q-03/Q-04 strict-letter executed-and-closed); handover state
  line -> BOTH PROGRAMS CLOSED. Session-24 total: 4 commits
  (R7.5 code + addendum + R7.6 release + this close-out) + the
  v1.10.0 tag; CI runs #379–#384 + Release run#38 (run#384 =
  43875255895, GREEN on the close-out head a0fe69d itself, all
  four lanes — the evidence chain closes with the terminal head
  verified, matching the program-1 session-20 pattern).
  Next: nothing — the queue is empty; new work opens only on an
  owner directive.

### Session 23 — 2026-11-16
- Maintenance pre-flight: sandbox reset again; re-cloned via
  github-reconnect.sh; toolchain intact at /tmp/my-project/
  {rustup,cargo-home} (rustc 1.99.1). Discovered the repo had
  advanced past the session-22 close-out: R7.4-c was already
  committed (4fd37af) and pushed, but its ledger addendum never
  landed (context-budget stop before the four-piece update).
- CI verified: run#378 (id 43829896373) GREEN on 4fd37af, all
  lanes — R7.4-c closed. Battery per the commit message: green at
  exact baselines (gate, fmt --check with fmt --all run before the
  battery (G-21), clippy workspace (70s) + core sandbox (47s)
  -D warnings, core 520 (460s), core-sandbox 528 (450s), app-shell
  293+2 (270s), catalog+xtask 56, verify-icons, strip-comments
  259/259). Post-batch census: prod_expect r25-series 6 -> 3
  (truncation 5 -> 2) — CLASS D COMPLETE; remaining = the
  interaction_signals D-14 keep + 2 cfg(test)-region digest
  sites + xtask machinery; prod unwraps 0. The R7.4 a+b+c
  closure: Q-03's strict-letter conversion is tree-wide
  complete.
- This addendum (ledger catch-up): pointer -> R7.5; session-23
  row recorded; R7.4-c's own execution narrative lives in the
  4fd37af commit message (the E0308-verified spawn_blocking
  closure ruling and the three-site detail are authoritative
  there). Next: R7.5 (Q-01 guardrails) per the pointer.

### Session 22 — 2026-11-16
- Maintenance pre-flight: sandbox reset again (repo gone);
  reconnected via github-reconnect.sh; toolchain intact at
  /tmp/my-project/{rustup,cargo-home} (rustc 1.99.1). Discovered
  the repo had advanced past the session-21 handover: HEAD 21e1fca
  with R7.4-a/2 (security, 6c9538a, run#364), a/3 (coaching,
  dd9c3da, run#366), a/4 (interaction_engine, c391955, run#368)
  all CI-verified — the R7.4-a/5 quality prep census was already
  recorded read-only in the session-21 log. Verified pending
  run#369 (21e1fca, docs-only addendum) GREEN.
- R7.4-a/5 executed (quality cluster, 16 sites = mod.rs x13 +
  qa_ai.rs x3, six owner fns + two accessors, the full prep-mapped
  cascade): (1) is_closing_acknowledgment -> Result<bool> (two
  inline OnceLocks -> the D-34(a) Result form); fanout x4 '?'
  (baseline.rs:304, qa_ai.rs:262, mod.rs:980, mod.rs:1300).
  (2) rebuild_gaps -> Result<(usize,usize)>; workers caller:
  spawn_blocking closure annotated Result-returning + the double-
  '?' tail (D-34(f) closure-conversion). (3) loop_problem_re /
  direct_complaint_re -> D-34(a) accessors; the .filter closure
  consumer hoisted (complaint_re local, D-34(d)). (4)
  duplicated_request_findings -> Result<Vec<Finding>> via module
  rx() helper x8; the six-fold Vec<(String,String,Vec<Value>,
  String)> duplication folded to `type Finding` — the clippy
  type_complexity remedy (Result<Vec<tuple>> crossed the
  threshold; alias is the documented fix; file had
  AgentQuestion/RelatedConversation precedents). (5)
  rebuild_friction -> Result<(usize,usize)>. (6) analyze_friction
  -> Result<Vec<Value>> (let-else Ok(Vec::new()); two finding-fn
  extends with '?'). (7) compute_qa_deterministic ->
  Result<Option<Value>>: the conv `?` on Option in a Result fn is
  a TYPE ERROR — let-else restructure (D-34(g)); get_qa (pub,
  Option-returning, signature FROZEN) folds Err into its existing
  None channel via .ok()? (D-34(f) Option-channel fold);
  rebuild_qa -> Result<usize> (sweep-loop '?;' preserves the
  None-skip semantics); workers/mod.rs:1133 maintenance (unit-
  returning) folds via .unwrap_or(0), the adjacent idiom. qa_ai
  compute_qa_ai_layer gates through its EXISTING QaAiOutcome
  error channel (match form). (8) qa_ai derive_outcome ->
  Result<DerivedOutcome>: rx x2 + the escalated .any closure
  hoist (D-34(d)); segment/eval.rs:1294 eval_support_health
  (Vec<i64> sampler) consumes via `let Ok(out) = .. else {
  continue }` — the aggregate-sampler skip ruling (D-34(h)).
  Test regions (D-10): batch13 x4 + batch18 x5 + batch24 x2
  (chained expects preserve the original assertion strength;
  is_some -> matches!(Ok(Some(_)))). Pattern strings
  byte-identical. Tooling incidents this session: one MultiEdit
  mis-targeted an adjacent fn tail + a bad edit list — caught by
  git diff review, full file revert + redo (lesson: review the
  diff after every MultiEdit group; never trust atomicity);
  the first analyze_friction tail draft used
  collect::<Result<Vec>> which does NOT compile (infallible map
  items — the FromIterator-for-Result fallacy, D-34(i)); the
  r25-style census scanner did not survive the sandbox resets —
  rebuilt as a truncation scanner (validated: cfg(test)-at-end
  convention holds tree-wide) with a one-off vs the r25 series
  (HEAD 28 vs ledger 29); the SITE-LEVEL delta is exact either
  way: 16 converted, worktree 12 = HEAD 28 - 16, per-file
  composition identical for all non-quality files.
- CI: run#370 (id 43773001510) on e721e39 FAILED on the check
  lane ONLY: fmt drift — the fmt battery lane had run BEFORE the
  Finding-alias sed; the alias shortened five signatures under
  the line-width threshold and rustfmt wanted them collapsed
  (packages x86_64/arm64 + kittest were green on the same tree —
  zero semantic issue). Fix: 8c6dab2 (post-alias rustfmt re-run,
  five single-line signatures, formatting-only). Run#371 (id
  43773898385) GREEN on 8c6dab2, all four lanes (check incl.
  deny, packages x86_64, packages arm64, kittest drift gate).
  Battery green at exact baselines: gate, fmt (pre-sed + post-
  fix), clippy workspace (47s) + core sandbox (40s) -D
  warnings, core 520 (459s), core-sandbox 528 (438s), app-shell
  293+2 (271s), catalog+xtask 56, verify-icons, strip-comments
  259/259. Post-batch census: prod_expect 29 -> 13 (r25-series
  site delta; truncation-scanner cross-check 28 -> 12). Memory:
  D-34 additions (f)-(j) + the fmt-after-post-battery-transform
  battery-order lesson (G-21).
- R7.4-a/6 executed (ai_evidence.rs:770, the LAST class-A
  site): the chrono midnight post-condition converted via the
  let-chain fold — `&& let Some(midnight) = d.and_hms_opt(0,
  0, 0)` joined to the existing let-chain condition, return
  bound to midnight; on the never-taken None the format loop
  falls through to parse_epoch_millis's existing lenient 0
  channel (byte-identical observables; unreachable panic arm
  becomes the documented fallback). RULING: census lettered it
  class A, but the FORM is a constant-input post-condition in
  a deliberately-fixed lenient channel (parse_epoch_millis is
  the 0-on-unparseable parser; both consumers are non-Result
  closures/expressions — :500 map closure + :741 arithmetic)
  — the R2.4-b class-C elimination precedent (inert 644
  let-else) + D-34(f) fold rule select the restructure over
  signature churn. Callers untouched (rg re-census exact:
  :500, :741, fn def; zero test-region callers). Battery green
  at exact baselines: gate, fmt --check (AFTER the edit, G-21),
  clippy workspace (54s) + core sandbox (33s), core 520 (477s),
  core-sandbox 528 (454s), app-shell 293+2 (263s), catalog+
  xtask 56, verify-icons, strip-comments 259/259. Census:
  prod_expect 13 -> 12 (r25 series; truncation 12 -> 11).
  CLASS A COMPLETE.
- CI verification: run#372 (id 43775745350) GREEN on cab642d
  (docs-only addendum head). Run#373 (id 43778917792) GREEN on
  c540871, all four lanes (check incl. deny, packages x86_64,
  packages arm64, kittest drift gate). Session-22 total: 4
  commits (2 batch + 1 fmt fix + 1 addendum) + this addendum;
  CI runs #369-#373 (one mid-batch fmt failure #370 — root
  cause + G-21 recorded). Next: R7.4-b.
- R7.4-b executed (class-C post-conditions; FRESH census
  found the ledger's 8 = 6 PRODUCTION + 2 cfg(test)-region
  sites — ai_pipeline 1743/1747 sit inside #[cfg(test)] fn
  digest_row, D-10, not production; the truncation scanner's
  per-file 1 was right): (1) webhook.rs:29 HMAC fold into
  the signature-invalid channel (.map_err(|_| Error::
  WebhookSignature)?); (2) support_graph node_specs type-level
  correspondence — [(GraphNodeKind, &str); 12] with variant
  values, parse().expect deleted; (3) workers/execute 524 the
  R3.4-noted TYPE-SHAPE FIX: customer_thread_trigger_decision
  -> Option<(&'static str, i64)> bundling the thread id into
  the decision; caller destructures; expect deleted; 5
  batch25 test arms assert tuples (D-10); (4) embeddings 813
  the quality-629 Map recipe (Map::new + 8 inserts +
  conditional 9th + Value::Object wrap); (5) ai_pipeline
  1111 .ok_or_else(|| LmStudioError::new("draft row missing
  after insert", false)) — never-taken None carries an honest
  non-retryable message; (6) live_chats 111 let-else +
  spinner fallback (the fn's own later let-else precedent),
  counter hoisted, the header closure captures &counter only
  — pixel-neutral on the reachable path, KITTEST-VERIFIED in
  CI (run#375 drift gate green). Battery green at exact
  baselines: gate, clippy workspace (48s) + sandbox (35s),
  core 520 (450s), core-sandbox 528 (457s), app-shell 293+2
  (276s), catalog+xtask 56, verify-icons, strip 259/259,
  fmt --check green (G-21 applied: fmt --all after the edits;
  the hand-formatted inserts were rewrapped). Census:
  prod_expect 12 -> 6 (r25 series) / 11 -> 5 (truncation).
- CI verification: run#374 (id 43780751768) GREEN on 311e52a
  (docs-only addendum). Run#375 (id 43785468832) GREEN on
  4d91080, all four lanes (check incl. deny, packages x86_64,
  packages arm64, kittest drift gate — the live_chats
  restructure is pixel-verified). Session-22 total: 7 commits
  (3 batch + 1 fmt fix + 3 addenda incl. this one); CI runs
  #369-#375. Next: R7.4-c (3 class-D bootstrap sites).
- R7.4-c prep (read-only census, recorded for the next
  session; D-33(d) discipline; context-budget stop: the
  session-21 lesson says never carry uncommitted state across
  a battery when the context window may die): (1)
  workers/mod.rs:26 block_ai_pipeline<F> -> F::Output with
  Builder::new_current_thread().enable_all().build()
  .expect("AI pipeline runtime") — 13 call sites, ALL inside
  Result-returning job handlers (ops.rs 157/261/508 + ai.rs
  35/226/282/316/377/450/560/627 + more; the honest form:
  block_ai_pipeline -> Result<F::Output> via .map_err into
  the crate Error channel + 13 '?' at the call sites —
  every caller is already Result-aware, so the churn is one
  '?' per site; alternative shapes (thread-local cached
  runtime) rejected: per-call current-thread runtime is the
  existing design). (2) task_bridge.rs:41 thread Builder
  spawn .expect("failed to spawn the task bridge thread") —
  TaskBridge::spawn (app/mod.rs:105, the ONLY prod caller) +
  spawn_with (test region :27 region calls it directly); the
  honest form: spawn_with -> Result<Self> (std::io::Error ->
  Error::Other), spawn -> Result<Self>, the app bootstrap
  caller handles at startup (check app/mod.rs:105's enclosing
  fn — if it is infallible TODAY the startup-error channel
  needs the app run fn's error path — census at edit time);
  the test caller gains .expect (D-10). (3) design/mod.rs:239
  fonts.families.get_mut(&Proportional).expect("proportional
  family") — a post-condition on FontDefinitions::default()
  (the family is always present in a fresh default); the
  honest form: if-let gate on the push (zero signature
  churn; on the never-taken None the Hack font registration
  is skipped — pixel-neutral on the reachable path, the
  kittest gate rides the push); install() is called ONCE
  (app/mod.rs:102). Sub-batching: all three in one batch
  (small; the battery rides once).

### Session 21 — 2026-11-15
- Maintenance pre-flight (program-1-closed protocol): re-cloned via
  github-reconnect.sh (sandbox reset again); toolchain survived at
  /tmp/my-project/{rustup,cargo-home}; last program-1 head 6dd8c8b
  confirmed CI-green (run#360, all four lanes); terminal metrics
  spot-checked on the fresh clone (unwrap 0 / expect 56 / panic
  family 4 / comments 0 — exact match).
- OWNER OVERRIDE received: "complete anything ignored and skipped"
  -> Program 2 opened (plan §11; handover state line; memory
  override addendum + D-31).
- R7.1 code applied (census-first): copilot.rs cut to its three live
  consts (enum+impl deleted; lib.rs re-export dropped, module kept —
  the first draft deleted the whole file and compile-check caught the
  live consts, D-31 lesson); graph.rs GraphHumanRelation block deleted
  (94->58 lines); support_graph LinkInput struct deleted + import
  narrowed; catalog re-export narrowed to GraphNodeKind. Compile check
  green (-p catalog -p core).
- R7.1 battery green at exact baselines (gate, fmt, clippy x2, core
  520 at 490s, sandbox 528 at 470s, app-shell 293+2 at 289s,
  catalog+xtask 56, verify-icons, strip 259/259; one killed-window
  continuation G-10 on the workspace lane, re-run per-lane). Commit
  f9c568c.
- R7.2 executed (Q-06 action-label single-sourcing): 42 prod sites
  folded to design CANCEL_LABEL/EDIT_LABEL/DELETE_LABEL beside the
  MISSING_VALUE precedent (D-26 home rule); the 2 test-region literals
  in overlays.rs tests stay (D-10); cargo fmt --all applied for the
  line-length fallout. Battery green at exact baselines (app-shell
  293+2 at 265s, core 520 at 472s, sandbox 528 at 490s, rest clean).
  Commit e62bf08.
- Post-push verification: CI run#361 (id 43713359530) GREEN on
  10c3cde, all four lanes — the kittest drift gate passed, so the
  R7.3 dispatch-table restructure is pixel-verified (203 goldens
  byte-identical). Sandbox reset AGAIN mid-battery (the R7.4-a first
  cluster had been applied locally, uncommitted — wiped); re-cloned
  via github-reconnect.sh, toolchain still at /tmp/my-project/
  {rustup,cargo-home}.
- R7.4-a census recorded (fresh r25 run on the closed tree; cluster
  mapping for the remaining work): class A = 45 sites in 8 clusters
  — sandbox.rs x4 (SeccompRule/Condition const-args), security.rs x8
  (redaction_patterns struct-literal regexes; consumer bubble spans
  ai_tools/execute, helpscout_real/credentials, ai_attributes x2,
  config, migrations — its own sub-batch), coaching/checks.rs x5 +
  coaching/mod.rs x5 (OnceLock statics / &'static Regex accessor
  fns), interaction_engine/baseline.rs x4 + interaction_engine/
  mod.rs x1 (macro + local statics), quality/mod.rs x13 +
  quality/qa_ai.rs x3 (OnceLock statics in bool-fns + one
  struct-literal set in duplicated_request_findings), ai_evidence
  x1 (chrono midnight). Class C = 8 (live_chats 111, embeddings 813,
  webhook 29, ai_pipeline 1109/1740/1744, support_graph 277,
  workers/execute/mod 524). Class D = 3 (workers/mod 30, task_bridge
  41, design/mod 239).
- CI verification: run#362 (id 43719049086) GREEN on 05f7006,
  all four lanes (check incl. deny, packages x86_64, packages
  arm64, kittest drift gate). Session-21 total: 5 batches (R7.1,
  R7.2, R7.3, R7.4-a/1 + this addendum head), CI runs #361-#362.

- CI verification: run#367 (id 43752563443) GREEN on f870a77
  (docs-only ledger addendum head — the R3.5 docs-only
  precedent).
- CI verification: run#368 (id 43755169348) GREEN on c391955,
  all four lanes (check incl. deny, packages x86_64, packages
  arm64, kittest drift gate). Session-21 tally: 9 commits
  (5 batch + 4 addendum), CI runs #361-#368; prod_expect 29.
- R7.4-a/4 executed (interaction_engine cluster, 5 sites, the
  prep-mapped two shapes): (1) baseline.rs x4 — private rx()
  helper, sites '?'; the step_re ctor hoisted OUT of the
  response_style .then closure (non-Result closures cannot host
  '?', D-34(d)); compute_outcome already Result — signature
  untouched, callers already Result-aware. (2) mod.rs:23 fp!
  macro bubble: macro body '?', forbidden_patterns -> crate
  Result<&'static [ForbiddenPattern]> via
  OnceLock<Result<Vec<..>, regex::Error>> with the
  .as_ref().map(|v| v.as_slice()).map_err(...) tail —
  Result::as_deref does NOT unsize &Vec (D-34(b), empirically
  E0308-verified); sanitize_interaction_text /
  assert_interaction_text_safe -> Result<TextScan>; the 5
  ai_pipeline/interaction.rs field-read fanout sites
  (205/231/240/260/269 — '.ok' was the TextScan STRUCT FIELD)
  go fail-closed per the recorded ruling: matches!(.., Ok(scan)
  if scan.ok) — on the never-taken Err the text is dropped
  (D-34(c), the sanitizer gates AI-bound text). Pattern strings
  byte-identical (indentation-only restructure, verified in the
  diff). Full re-census before edits: exactly the 5 recorded
  sites + 5 fanout, no other/test-region callers. Battery green
  at exact baselines: gate, fmt, clippy workspace --all-targets
  (27s) + core sandbox (15s) with -D warnings, core 520 (470s),
  sandbox 528 (462s), app-shell 293+2 (264s), catalog+xtask 56,
  verify-icons, strip 259/259. Post-batch census: prod_expect
  34 -> 29. Commit c391955. Memory: D-34.
- R7.4-a/5 prep (read-only census, recorded for the next session;
  D-33(d) discipline): quality = 16 sites (mod.rs x13 +
  qa_ai.rs x3) in SIX owner fns + two accessors, with a
  cascading signature plan — (1) is_closing_acknowledgment
  (mod.rs:66, sites 74/80 inline OnceLocks, -> bool) ->
  Result<bool>; fanout 4: interaction_engine/baseline.rs:304
  (compute_outcome, already Result — gains '?'),
  quality/qa_ai.rs:262 (derive_outcome), quality/mod.rs:980
  (troubleshooting_loop_findings), quality/mod.rs:1300
  (compute_qa_deterministic -> Option<Value> — itself converts
  to Result<Option<Value>>). WARNING: compute_qa_deterministic's
  TEST-REGION callers (batch18_tests 'first compute' etc.)
  currently .expect on the Option — after the conversion .expect
  unwraps the Result leaving Option<Value>: those test arms need
  one more expect/unwrap (D-10 permits; census the arms at edit
  time). (2) rebuild_gaps (mod.rs:400, sites 406/410, ->
  (usize,usize)) -> Result<(usize,usize)>; caller = the workers
  rebuild_gaps job arm (workers/execute/ops.rs:207 region). (3)
  loop_problem_re (750, site 756) + direct_complaint_re (760,
  site 766): the D-34(a) accessor form; callers mod.rs:979
  (troubleshooting_loop_findings) and mod.rs:1200 — inside
  rebuild_friction's .filter closure -> HOIST per D-34(d). (4)
  duplicated_request_findings (mod.rs:1153, sites 1156-1169 x8
  local ctors, -> Vec<..>) -> Result<Vec<..>> via an rx helper;
  caller mod.rs:806 findings.extend in analyze_friction (->
  Vec<Value>; its conversion -> Result<Vec<Value>> — census
  analyze_friction's callers at edit time). (5) rebuild_friction
  (mod.rs:1237, -> (usize,usize)) contains the 1200 filter;
  converts -> Result<(usize,usize)>; census its callers at edit
  time. (6) qa_ai.rs derive_outcome (240, -> DerivedOutcome
  plain struct; sites 270/312 fn-level locals + 279 inside the
  .any(|n|{..}) escalated closure -> HOIST) ->
  Result<DerivedOutcome>; census derive_outcome's callers at
  edit time. ai_evidence x1 (chrono midnight) is the trivial
  last class-A site after this cluster. Sub-batching is the
  next session's call by remaining context budget.

- CI verification: run#366 (id 43750059829) GREEN on dd9c3da,
  all four lanes (check incl. deny, packages x86_64, packages
  arm64, kittest drift gate). Session-21 tally: 7 commits (5
  batch + 2 addendum), CI runs #361-#366; prod_expect 34.
- R7.4-a/3 executed (coaching cluster, 10 sites, the two census
  shapes): checks.rs local ctors x5 (acknowledgment/detail/
  wrong_context x2/preference_mismatch) + the accessor-consuming
  check_timeframe -> five check fns Result<Value> via a private
  rx() helper (Error::Other(Box::new(e)) — the R7.4-a/2
  security.rs form), sites '?', all return arms Ok-wrapped; the
  four regex-free registry fns (unanswered/duplicated/wording/
  internal_leakage) stay Value-returning — registry order
  byte-identical, the five converting arms just gain '?',
  propagating into review_draft's outer crate Result (ai.rs:583
  already logs 'review_draft: {reason}' — the existing
  outer-error channel). mod.rs accessors x3 covering 5 sites
  (ack/frustration/timeframe): OnceLock<Result<., regex::Error>>
  (regex::Error is Clone — refined from the census's String
  form; error clone only on the never-taken path);
  timeframe_patterns builds via array.into_iter()
  .map(Regex::new).collect() — pattern strings byte-identical
  (verified in the diff). Full re-census before edits: exactly
  the 10 recorded sites, zero fanout beyond the registry (no
  external and no test-region callers of the check fns).
  Battery green at exact baselines: gate, fmt, clippy workspace
  --all-targets (2m58s) + core sandbox (29s) with -D warnings,
  core 520 (478s), sandbox 528 (501s), app-shell 293+2 (281s),
  catalog+xtask 56, verify-icons, strip 259/259. Post-batch
  census: prod_expect 44 -> 34. Commit dd9c3da.
- R7.4-a/4 prep (read-only census, recorded for the next session;
  D-33(d) discipline): interaction_engine = 5 sites in two
  shapes — (1) baseline.rs x4 (307/313/314/335) all inside
  compute_outcome which ALREADY returns Result<Option<..>> —
  sites become '?' (rx-style helper or inline map_err);
  callers already Result-aware (baseline.rs:722 discard arm,
  ai_pipeline/interaction.rs:571 engine pipeline, mod.rs:1144
  test region stays D-10). (2) mod.rs:23 fp! macro = the full
  bubble: forbidden_patterns() -> &'static [ForbiddenPattern]
  (OnceLock<Vec>), sole consumer sanitize_interaction_text
  (mod.rs:81) -> TextScan{ok,warnings}; assert_interaction_
  text_safe wraps it; fanout = 5 field-read sites in
  ai_pipeline/interaction.rs (205/231/240/260/269 — `.ok` is a
  STRUCT FIELD read inside filter/is_some_and closures, NOT
  Result::ok()). Conversion ruling for edit time:
  forbidden_patterns -> crate Result<&'static [..]> (the
  coaching OnceLock<Result<., regex::Error>> form);
  sanitize/assert -> Result<TextScan>; the 5 fanout sites go
  fail-closed — on Err treat the text as not-safe and drop it
  (the sanitizer gates AI-bound text: cannot-certify must not
  pass through). Formalize as a memory D-entry when the batch
  executes.

- CI verification: run#364 (id 43726134629) GREEN on 6c9538a,
  all four lanes (check incl. deny, packages x86_64, packages
  arm64, kittest drift gate).
- R7.4-a/3 prep (read-only census, recorded for the next session;
  D-33(d) discipline): the coaching cluster's 10 sites are TWO
  shapes — (1) coaching/checks.rs x5: LOCAL `let re =
  Regex::new(...).expect("static regex")` constructors inside
  check fns returning Value (check_acknowledgment 263,
  check_detail 402, check_wrong_context 598+641,
  check_preference_mismatch 702) — conversion: check fns ->
  Result<Value>, sites become `?`; the caller is the review
  driver's `let mut checks: Vec<Value> = vec![check_..(), ...]`
  registry at coaching/mod.rs ~713 (9 arms; the driver fn's own
  signature decides the boundary — inspect at edit time);
  (2) coaching/mod.rs x5: OnceLock accessors (ack_markers/
  frustration_markers -> &'static Regex, timeframe_patterns ->
  &'static Vec<(Regex, &'static str)>, sites 204/212/221/225/229)
  — conversion: OnceLock<Result<.,String>> + accessor ->
  Result<&'static .> with the error clone happening only on the
  never-taken path; callers at checks.rs 185/278/297 gain `?`.

- R7.4-a/2 executed (security cluster, the full bubble — D-33):
  security.rs 8 sites via the rx() helper + Result signatures;
  chains — ai_pipeline chat boundary (LmStudioError::new(msg,
  false)), redact_for_model -> Result (std::result:: prefix — the
  alias collision E0107 caught at compile), run_extraction ->
  Result<Option<...>> (compute caller + the 722 test ride along);
  execute.rs tool layer — redact/redact_chars -> Result, the CENSUS
  CORRECTION: fanout was 23 sites / 16 tool fns (the recorded ~10
  was head-truncated rg; D-33(d) now mandates full re-census),
  settle() at the dispatcher arms, two-pass restructures for the
  rusqlite closures, ClusterRow/AttrRow structs where clippy
  type_complexity rejected tuples. Tests: batch29 +2 expects,
  attributes +2 expects (D-10 regions). Battery green at exact
  baselines: gate, fmt, clippy x2 (-D warnings), core 520 (453s),
  sandbox 528 (465s), app-shell 293+2 (261s), catalog+xtask 56,
  verify-icons, strip 259/259 (G-05 relief before the sandbox
  lane). Post-batch census: prod_expect 44.

- R7.4-a first cluster executed (sandbox.rs x4): the already-Result
  apply_seccomp narrows its two rule-construction sites — fd_rule
  closure returns Result<SeccompRule,String> with map_err(|e|
  e.to_string()), the SYS_mmap masked-eq rule propagates ? directly;
  error-path-only change (the constructors accept const args — the
  arms were never reachable). Battery green at exact baselines: gate,
  fmt, clippy workspace+sandbox (-D warnings), core 520 (477s),
  core-sandbox 528 (453s), app-shell 293+2 (301s), catalog+xtask 56,
  verify-icons, strip 259/259 (G-05 relief before the app-shell lane).

- R7.3 executed (Q-04 strict restructures; D-32 shapes recorded):
  app/mod.rs page dispatch -> the honest 23-arm table (each arm its
  own host; per-arm bodies byte-identical, exhaustiveness compiler-
  checked without the lie; pixel-drift risk rides the CI kittest gate
  per the Q-04 requirement); oauth_state.rs hex_digit -> Nibble
  range-newtype (constructors high/low encode the mask proof; HEX
  [char;16] indexing is in-bounds by constructor arithmetic);
  workers/execute/ops.rs (the R4.5-b child holding the old
  execute.rs:1833 site) -> outer-arm re-nesting into three real
  Result<Option> arms with the shared warn/published tail extracted
  as record_partial_publish. Post-edit census: prod_unreachable = 0
  (the sole remaining prod_panic is the D-10 arm-level cfg(test)
  site at workers/execute/mod.rs:317). Battery green at exact
  baselines (core 520 at 472s incl. the crypto hex test over the
  newtype; app-shell 293+2 at 274s; sandbox 528 at 473s; rest
  clean).

### Session 20 — 2026-11-15
- Pre-flight: run#357 (id 43682661859) green on `bf37d97` (the
  session-19 addendum head — the standard docs-only preflight gate).
- Environment: full reset AGAIN — clone wiped; re-cloned via
  github-reconnect.sh (credentials survived in tool-results);
  toolchain + registry cache survived at /tmp/my-project/
  {rustup,cargo-home} (third reset in a row; exports re-applied per
  bash call per §3); cold dep tree cost one killed-window continuation
  (G-10) on the first clippy lane, as always.
- R6.1 executed (build-file comment strip, plan §7 R6 letter):
  census-first (r61_strip.py census mode — 279 lines across the 12
  target files; .gitattributes censused ZERO comments, left
  byte-identical; `_archive/` deliberately out of scope — it is the
  program's own memory, not build machinery). Taxonomy from the
  census: 238 whole-line YAML/TOML comments + 21 shell comments
  inside YAML run-block scalars + 20 trailing `# vN` action-pin
  annotations on `uses:` lines. Applied by the same tool in apply
  mode with per-file equivalence asserts.
- Functional zero-diff proof, four independent layers: (1) tomllib
  loads(before)==loads(after) for all 8 TOML files incl. deny.toml;
  (2) PyYAML recursive structural equality for both workflows with
  run-block shell-comment-line normalization applied to BOTH sides;
  (3) Python AST dump equality for the gate script (comments are
  trivia; shebang + docstrings kept — D-29); (4) r61_audit.py
  multiset conservation audit over git HEAD vs worktree: 279 removed
  lines all comments, 20 added lines all pin-rewrite counterparts,
  per-file count reconciliation green.
- Battery (one lane per tool call, G-05 watch): gate clean; fmt
  --check clean; clippy workspace + sandbox (-D warnings) clean
  (cold tree: one killed-window continuation on lane 1); core 520 ✓
  (476s); core-sandbox 528 ✓ (466s, G-05 cleanup before it);
  app-shell 293+2 ✓ (288s); catalog+xtask 56 ✓; verify-icons ✓;
  strip-comments --check crates 259/259 ✓ (the .rs tree untouched).
  G-09 protocol for the touched deny.toml: cargo-deny 0.20.2
  installed as the PREBUILT musl release binary (G-20) and run
  locally — advisories ok, bans ok, licenses ok, sources ok (the
  advisory DB fetched live; no new advisories against the tree).
- Commits: code + ledger (D-08). CI closure (same session): run#358
  (id 43694505499) GREEN on `168e71a`, ALL FOUR LANES — check
  (fmt/clippy/test/deny — the stripped deny.toml against the LIVE
  advisory DB), packages x86_64, packages arm64, kittest drift gate
  (203 goldens byte-identical). R6.1 TERMINAL.
- R6.2 executed (same session, on the green 168e71a base): final
  audit per the plan letter — final metrics census re-run on the
  final tree (r25_triage.py: 0 prod unwraps / 56 expects / 4
  panic-family; allow census: 30, all D-13; per-crate files+LOC:
  259 / 138,593); DoD checklist closed in complete.md (R5 dropped —
  Q-01; expects/panic DoD line satisfied as-amended per D-14/D-15
  owner-defaults, Q-03/Q-04 hold the strict-letter option);
  per-module-cluster verification statements recorded above;
  final-metrics table + closing report written into complete.md.
  Docs-only closing head on the identical code tree (R3.5 docs-only
  precedent) — the session-20 battery on this exact tree + CI
  run#358 are the evidence; CI-VERIFIED same session: run#359
  (id 43696107214) green on `9522cde`, all 4 lanes.
- New memory: D-29 (build-file comment taxonomy + the docstring/
  shebang ruling) + G-20 (cargo-deny prebuilt musl asset).
- PROGRAM CLOSED: 20 sessions / 20 batches / CI runs #301–#359;
  standing owner options Q-01..Q-06 in memory.md.

### Session 19 — 2026-11-16
- Pre-flight: run#355 (id 43672660744) green on `5f69ada` (the
  session-18 addendum head — the standard docs-only preflight gate).
- Environment: full reset again — repo re-cloned via
  github-reconnect.sh; toolchain survived at /tmp/my-project/
  {rustup,cargo-home}; exports re-applied per bash call per §3; cold
  dep tree cost one killed-window continuation (G-10) on the first
  check.
- R4.5-b executed (prod-region splits): the six >2K hosts split along
  the R4.2 method-cluster seams into 14 child files / 6,198 lines
  moved (verdict table above). Census-first: method-level impl census
  (r45b_impl.py, fixed multiline-signature close detection) + consumer
  census per moved name (call-site file/line classification against
  staying/moved/test ranges → the exact G-15 level per item; the
  audit caught four draft rewrites already pub(crate)/pub(super) —
  G-17). Extractor r45b_split.py: string-aware boundary asserts +
  per-host whitespace-insensitive char-multiset conservation (old +
  additions == new) + git-mv rename for execute.rs → execute/.
- Compile loop: exactly one G-18 fix — TokenRow struct + 4 fields →
  pub(super) (http.rs sibling reads them; a pub(super) fn returning a
  private struct is a hard error). One apply-machinery fix — G-19
  (git mv needs the destination dir pre-created; clean reset+clean+
  rerun after two hosts were written; asserts gated everything).
- Battery: gate clean; fmt --check clean; clippy workspace + sandbox
  (-D warnings) clean; core lib 520 ✓ (471s), sandbox 528 ✓ (447s),
  app-shell 293+2 ✓ (270s), catalog+xtask 56 ✓; verify-icons ✓;
  strip-comments 259/259 ✓; kittest-ui --all-targets compile ✓
  (42.8s). D-24 multiset pre-gate 4/4 byte-identical. G-05 before
  every link-heavy lane.
- Commits: code + ledger (D-08). CI closure (same session): run#356
  (id 43681170622) green on `9139e8d`, ALL FOUR LANES — check
  (fmt/clippy/test/deny), packages x86_64, packages arm64, kittest
  snapshot locks (reskin drift gate): 203 goldens byte-identical,
  pixel-neutrality CONFIRMED for the prod-region splits. R4.5
  TERMINAL (a+b); complete.md terminal entry + final-metrics
  "Files split" recorded. This addendum commit's own docs-only run
  is the next session's preflight gate.
- New memory: D-28 (impl-member move recipe: visibility-only, paths
  free) + G-17/G-18/G-19.

### Session 18 — 2026-11-15
- Pre-flight: run#353 (id 43658004337) green on `70ab546` (the
  session-17 addendum head — the standard docs-only preflight gate).
- Environment: FULL reset again — repo re-cloned via
  github-reconnect.sh; toolchain survived at /tmp/my-project/
  {rustup,cargo-home}; exports re-applied per bash call per §3. The
  incoming handover summary was AGAIN the stale R2.5-era ammonia text
  (third time) — the committed ledger is the source of truth; R4.4
  TERMINAL since run#352.
- R4.5-a executed (file splits, census-first): re-measured every .rs
  file — 12 files >2K (the session-1 top-10 plus quality 2159 and
  outreach 2143); the prod/test split decided the two-tier remediation
  (verdict table above). Extraction machinery r45a_extract.py
  (tool-results): the G-08/G-14-family string-aware lexer — naive brace
  counting MISSES mods whose bodies carry unbalanced string braces
  (reports.rs proved it; G-16) — plus per-mod moved-line conservation
  asserts, dedent invariants, collision checks, and `git mv` dir
  conversions for the five flat pages. One mid-run assert redesign
  (the first char-conservation design compared region-with-braces to
  decl-with-semicolon — both sides must be the same BODY) cost a
  clean `git reset --hard` + rerun; no damage escaped the assert.
- Moves: 37 cfg(test) mod blocks / 6,843 lines → 37 module files;
  hosts trimmed to `#[cfg(test)] mod X;` declarations. Six files exit
  the >2K set (workers 1387 / ai_pipeline 1947 / knowledge 1916 /
  quality 1626 / outreach 1351 / ai_center 2003-keep); six queued to
  R4.5-b with exact post-batch counts.
- Battery: gate clean; fmt --check clean (fmt --all run in write mode
  between extraction and battery per D-12); clippy workspace 72s +
  sandbox 72s (-D warnings) clean; core lib 520 ✓ (435s), sandbox
  528 ✓ (445s), app-shell 293+2 ✓ (279s), catalog+xtask 56 ✓;
  verify-icons ✓; strip-comments --check 245/245 ✓; kittest-ui
  feature --all-targets check green (39s). G-05 before every
  link-heavy lane; cold-start dep tree needed one killed-window
  continuation (G-10) — foreground per call per G-10's fallback pattern
  (the setsid launch form re-confirmed G-10's death list: 0-byte logs).
- Module-tree invariance proof: test counts byte-equal at every lane —
  the moved tests compile and run from their new files with identical
  results (520/528/293+2/56); clippy -D warnings green over
  --all-targets (cfg(test) modules compile under test cfg).
- Commits: code + ledger (D-08). CI closure (same session): run#354
  (id 43670479211) completed / success on `c503d02` — all four lanes
  (check fmt/clippy/test/deny; packages x86_64; packages arm64;
  kittest snapshot locks drift gate, 203 goldens byte-identical — the
  pure file-layout move pixel-neutral as designed). R4.5-a closed;
  this addendum rides the next docs-only run (the standard
  next-session preflight gate). R4.5-b (prod-region splits, six
  files with exact post-batch counts + seam map in the verdict table)
  is the next session's batch.

### Session 17 — 2026-11-14
- Pre-flight: run#351 (id 43635267151) green on `8f41c19` (the session-16
  addendum head — the standard docs-only preflight gate).
- Environment: FULL reset again — the repo at /home/z/SupportOS-Plus was
  GONE (re-cloned via github-reconnect.sh; the toolchain again survived
  at /tmp/my-project/{rustup,cargo-home}, exports re-applied per bash
  call). Context-recovery note: the session-17 handover summary was the
  SAME stale R2.5-era ammonia text session 16 received (describing the
  long-closed ammonia battery mid-run) — the ledger was the source of
  truth again; recovery via plan §8.1 read order; the ammonia episode
  closed at `ea089b0` (G-09), R2.5 through R4.3 all terminal since.
- R4.4 executed (magic values, census-first): scanner r44_census.py
  (G-08/G-14 char-level walk + literal collection + take/LIMIT idiom
  overlays + const-initializer inventory). TENTH consecutive census
  correction: 7,823 prod numeric / 17,350 prod string occurrences → SIX
  same-policy families / 166 sites; every large group dissolved under
  D-17 (200/500 = status+cap+LIMIT mixes; 365/3600/86400 = civil-date
  algorithm; 250 = rgb components; 32.0 = five unrelated height
  policies). Verdict table above records the 16 family rulings.
- Folds: catalog::NO_SUBJECT (root const; 14 sites cross-crate);
  design::MISSING_VALUE (pub(crate); 38 sites incl. the `== "—"`
  sentinel comparator — producer + comparator single-sourced);
  design::error_note (17 byte-identical error_text+format sites);
  design::scale::CONTROL_H (72 single-line control-height sites; avatar
  datum + horizontal pad kept); design::scale::ROW_H (the 3 identical
  page-local ROW_HEIGHT decls + 11 refs); coaching::EXCERPT_CAP
  (private, 15 sites; search/interaction 240s kept as coincidental).
- Apply via r44_apply.py: 54 pattern groups with hard per-pattern count
  asserts, atomic write-after-all-asserts-pass; all asserts passed on
  first run (G-12 discipline — input proven, then compile + battery
  prove output); 30 files, +254/−184 after fmt (reflow accounted).
- Battery: gate clean; fmt --check clean; clippy workspace 51s + sandbox
  58s (-D warnings) clean; core lib 520 ✓ (446s), sandbox 528 ✓ (476s),
  app-shell 293 ✓ (268s), catalog 0 + xtask 56 ✓, verify-icons ✓,
  strip-comments --check 207/207 ✓. G-05 applied before link-heavy
  lanes (disk peaked 72%); the cold workspace check needed the G-10
  foreground-window continuation once (artifacts persisted).
- Diff class review: every hunk = a const/fn introduction, a
  literal→const-path replacement, or a rustfmt reflow — value-identical
  by construction (pixel-neutral; the CI kittest drift gate rides this
  push as the verifier, F-06).
- Commits: code + ledger (D-08). CI closure (same session): run#352
  (id 43655562331) completed / success on `3885eb4` — all four lanes
  (check fmt/clippy/test/deny; packages x86_64; packages arm64; kittest
  snapshot locks drift gate, 203 goldens byte-identical). R4.4
  TERMINAL; this addendum rides the next docs-only run (the standard
  next-session preflight gate).

### Session 16 — 2026-11-14
- Pre-flight: run#349 (id 43621514210) green on `71f462d` (the session-15
  addendum head — the standard docs-only preflight gate).
- Environment: full reset AGAIN (repo re-cloned via github-reconnect.sh;
  toolchain survived at /tmp/my-project/{rustup,cargo-home}).
  Context-recovery note: the session-16 handover summary described the
  R2.5-era ammonia state (STALE by ~20 CI runs) — the ledger was the
  source of truth again; the RUSTSEC ammonia episode closed long ago
  (G-09 memory). Recovery via ledger read per plan §8.1.
- R4.3 executed (visibility, census-first): scanner r43_census.py
  (token-index consumer census + use-span leaf skipping + rename-alias
  resolution + cfg(test) exemption). FIRST RUN carried a new G-08-family
  bug caught by its own apply-assertions: string `\<newline>` line
  continuations had their newlines BLANKED (sanitize escape branch
  consumed 2 chars unconditionally) → every later line number desynced
  (PageKey off by 9). Fix: preserve the newline; assert newline-position
  identity per file (G-14 recorded). Second census run produced
  line-verified decisions; apply (223 vis edits + 6 dead deletions +
  1 pub use downgrade) with hard per-line assertions.
- Compile loop (the batch's central mechanic — D-25): cargo check with
  rustc as the reachability arbiter. Core: 5 rounds (89 E0603 + 233
  private_interfaces → restore script r43_restore.py parsing the
  warning bodies' "reachable at visibility" text; guards' lifetime-
  suffixed names handled manually; ThreadEntry name-collision fixed
  per-module; cascade deletions CustomerTimelineEntry/fts_row/
  search_knowledge; orphaned #[derive]/#[must_use] attrs from
  deletions removed; unused imports cleaned). Workspace: 3 rounds
  (app-shell SP4 grandparent-path → pub(crate); inbox_geo consts →
  pub(super); StripStats bin-return → pub; BootServices/BootMode/
  AcquireError/InstanceLock/PageKey/ConfirmRequest/FilterBar/Tokens/
  ViewFilter/DestructiveAction exposure restores; AiJobRow dead-field
  slim). Converged: workspace check EXIT=0, ZERO warnings.
- Battery (post-convergence): gate clean, fmt --check clean, clippy
  workspace 60s + sandbox 41s (-D warnings), core lib 520 ✓ (465s),
  sandbox 528 ✓ (466s), app-shell 293 ✓ (274s), catalog 0 + xtask 56 ✓,
  verify-icons ✓, strip-comments --check 207/207 ✓. G-05 incremental
  rm before each link-heavy lane (disk peaked 72%).
- Diff class review: 40 files, +101/−321 — every hunk is a visibility
  prefix change, a module-cap line, a compiler-verified deletion, a
  dead-field slim, an unused-import cleanup, or a rustfmt reflow; no
  logic edits, no test changes (baselines exact).
- Commits: code + ledger (D-08). CI closure (same session): run#350
  (id 43633543056) completed / success on `f005d49` — all four lanes
  (check fmt/clippy/test/deny; packages x86_64; packages arm64; kittest
  snapshot locks drift gate, 203 goldens byte-identical). R4.3 TERMINAL;
  this addendum rides the next docs-only run (the standard
  next-session preflight gate).

### Session 15 — 2026-11-14
- Pre-flight: run#347 green on the session-14 addendum head `a18a8ea`
  (the standard docs-only preflight gate) — R4.2-b's green base.
- Environment: full reset AGAIN — repo re-cloned via github-reconnect.sh;
  toolchain survived at /tmp/my-project/{rustup,cargo-home} (same
  location as session 14; exports re-applied per call).
- Discovery: the context-exhausted session that queued R4.2-b had
  PRE-WRITTEN the nine transform scripts (tool-results
  r42b_split_{outbound,loops,incidents,aicenter,orgs,settings,
  diagnostics,inbox,design}.py, timestamps 11:47–12:14 vs the 10:49
  addendum push) but never ran them — no compile, no battery, no
  commit. Treated as UNVERIFIED DRAFTS: line-by-line script review
  against source BEFORE running found three slicing bugs (outbound's
  update_tags body kept the arm label + missed the match's closing
  brace; orgs kept an orphan `);` after the form-card replacement;
  design kept composer's now-unused `use egui::RichText as R`) plus
  a type bug (orgs members_card Some() around member_request, which
  already returns Option — would have double-Optioned); scripts
  patched, then run against the fresh a18a8ea clone (all boundary
  asserts green).
- Post-run verification loop: cold core check 7m06s → outbound+loops
  compiled CLEAN first try; app-shell check surfaced the remaining
  draft defects: incidents' incident_row missing the `);` of the
  allocate_ui_with_layout call (slice cut at the closure body, not
  the statement); info_column_ui missing the db param its add-note
  card needs; hub_ledger_card_ui carrying an unused db param; 12
  needlessly-borrowed `&incident` sites (the batch-a class).
- PIXEL-NEUTRALITY GATE (new, D-24): widget-call multiset comparison
  — for each of the 9 touched files, the multiset of `design::*`
  calls + `ui.*` method calls old-vs-new must be byte-identical. It
  PASSED 7 files and CAUGHT the two draft DESIGN bugs the compile
  loop cannot see: ai_center +4 and settings +4 card_titled_wide —
  the scripts had wrapped each moved card call in ANOTHER
  card_titled_wide shell (double-carded UI). Both flattened to direct
  method calls; the counts returned to baseline. ai_center's +2
  design::tokens is the static-getter re-thread (13→13 card calls,
  9→11 tokens — getter, no pixels — verified against source).
- Battery (final, post-fixes): gate clean, fmt --check, clippy
  workspace + sandbox (-D warnings), core lib 520 ✓ (470s), core
  --tests (520 again — crates/core/tests/ does not exist in the repo;
  all tests are inline), sandbox 528 ✓ (464s), app-shell 293+2 ✓
  (270s, 26 kittest binaries at 0 — F-10), catalog 0 + xtask 56 ✓,
  verify-icons ✓, strip-comments --check 207/207 ✓. G-05 applied
  twice (disk peaked 79% before the app-shell link; incremental
  removed, back to 3.0G free).
- Diff class review: 9 files, +2335/−2065 — every hunk is a
  mechanical move (arm/fn/closure-body extraction with the
  documented re-thread rewrites: pick→picked locals, panel_nav→nav
  locals, columns[0]→column params, ToolAction enum hoisted to module
  scope, token/t/status/db/&Value threading), the two flattened
  wrappers, or the four allow deletions; the 4 too_many_lines allows
  deleted with clippy -D warnings green proving every post-split fn
  under the 100-line threshold; no test changes (520/528/293+2/56 at
  EXACT baselines).
- Commits: `6eaf8c8` (code + ledger, battery-green). CI: run#348
  (id 43619290868) completed / success on `6eaf8c8`, all four lanes —
  check (fmt/clippy/test/deny), packages x86_64, packages arm64,
  and the kittest snapshot drift gate (203 goldens byte-identical —
  the pixel-neutrality proof). R4.2 closed terminal; this addendum
  rides run#349 (docs-only).

### Session 14 — 2026-11-14
- Pre-flight: run#345 green on the session-13 addendum head `155c277`
  (the standard docs-only preflight gate) — R4.2-a's green base.
- Environment: full reset again (repo wiped + toolchain home MOVED to
  /tmp/my-project/{rustup,cargo-home} — handover §3's session-13 path
  is stale; exports re-verified). Re-cloned via github-reconnect.sh.
- R4.2-a executed (decomposition, census-first): scanner r42_census.py
  (cfg-aware char-level fn-span lexer; first run had the impl-after-
  cfg(test)-mod false-positive — fixed with a boundary-line gate).
  3,277 production fns censused; the top-20 read in full; dispositions
  in the R4.2 verdict table: 6 core mega-fns split (execute 1051→79+22
  tool fns; derived_edges_touching 686→26+13 edge fns; apply_m046
  405→15 manifest+ensure_fk+10 consts; execute_job 3 fat arms→3
  methods + the embed×3 fold; build_support_health_report 465→34
  manifest+9 domain fns; analyze_friction 410→manifest+6 finding fns),
  6 keeps with written rulings (flat manifests / dispatch tables /
  state machines / conversation loop), 11 queued to R4.2-b (the UI
  T-composition twins incl. all four D-13 too_many_lines sites +
  execute_outbound + sync_resource via the proven dispatch recipe).
  ZERO app-shell source contact → pixel-neutral by construction; the
  kittest drift gate rides CI as usual.
- Battery (per-component after the workspace lane hit the 600s tool
  cap): gate clean, fmt --check, clippy workspace + sandbox (-D
  warnings), core lib 520 ✓, core integration stubs ✓, sandbox 528 ✓,
  app-shell 293+2 ✓ (26 kittest binaries at 0 local tests — F-10
  normal), catalog 0 ✓, xtask 56 ✓, verify-icons ✓, strip-comments
  --check 207/207 ✓. G-05 disk cleanup applied twice (81% peak).
- Diff class review: 6 core files, +2649/−2297 — every hunk is a
  mechanical move (arm/fn extraction), a documented closure→fn
  promotion, the ensure_fk fold, or the embed fn-pointer fold; no
  logic edits, no signature changes to any pub surface, no test
  changes (the 520-test suite is the semantic proof).
- Commits: `fa03cad` (code + ledger, battery-green). CI: run#346
  (id 43585107897) completed / success on `fa03cad` — all four lanes:
  check (fmt/clippy/test/deny), packages x86_64, packages arm64,
  kittest snapshot locks (drift gate green — expected, zero
  app-shell source contact). Addendum commit records this + queues
  R4.2-b.

### Session 13 — 2026-11-14
- Pre-flight: run#343 green on the session-12 addendum head `66f3968`
  (the standard docs-only preflight gate) — R4.1's green base.
- Environment: FULL reset (repo wiped; re-cloned via
  github-reconnect.sh; git identity re-applied by the script).
  Toolchain SURVIVED at /home/z/my-project/{rustup,cargo-home}
  (handover §3 corrected to this path — the audit/env.sh exports
  already pointed there); rustc 1.99.1 verified on first command.
- R4.1 executed (naming, census-first): scanner r41_census.py
  (tool-results, not committed) — bool decls 341 (272 prod / 69
  test), fn-returns-bool 118 (production 117 + 1 kittest helper),
  short-id inventory 478 kinds, suspicious-word pass 232 hits;
  every weak candidate read in context before its verdict. Result:
  SEVENTH consecutive census correction — the tree's naming is
  overwhelmingly honest; 10 rename families + 1 deletion executed
  (temp→temperature ×4, already→already_exists ×2, already→
  already_queued ×4, has→has_column ×6, known/conversation→
  known_issue_exists/conversation_exists ×6, palette_take→
  query_committed ×4, right→align_right ×6, drill→drillable ×4,
  assistant→from_assistant ×4, ins→stmt ×4, aa_get deleted with 3
  callers inlined to Map::get per D-18's standing rule). Keep
  rulings (wire vocab invariant 5 / truthy state adjectives /
  predicate-phrase fns / egui widget conventions / flag idioms /
  b_* form-state family / established short vocabulary / test
  regions) in the R4.1 verdict table. Zero kittest golden-key
  contact (positional callers unchanged); dt_cell ×3 fold candidate
  recorded for a possible future batch.
- Battery: COLD cache (fresh target/) — full battery green via the
  detached-driver pattern (G-10 subshell form, sequential lanes,
  G-05 incremental cleanup between codegen lanes): gate clean, fmt
  --check, clippy workspace + sandbox (-D warnings), core 520 ✓,
  core-sandbox 528 ✓, app-shell 293+2 ✓ (27 kittest binaries at 0
  local fns — F-10 normal), catalog+xtask 56 ✓, verify-icons,
  strip-comments --check 207/207 ✓. Total ~30 min (06:37–07:07);
  disk tripped 93% at the app-shell link lane — post-run
  `rm -rf target/debug/incremental` restored 2.7G free.
- Diff class review: 13 code files, +77/−51 (progress.md +30
  ledger) — every hunk a word-level rename or the aa_get deletion;
  no behavior, no wire, no golden contact.
- Commit `refactor(R4.1): naming …` + push; CI run#344 (id 43569924684)
  GREEN on `1b653d4`, all four lanes incl. the kittest drift gate
  (renames pixel-neutral by construction); ledger addendum pushed
  same-session (R4.1 terminal; R4.2 queued with decomposition notes).
- memory.md: D-22 added (naming end state + standing rules);
  handover §3 toolchain path corrected to /home/z/my-project/.

### Session 12 — 2026-11-14
- Pre-flight: run#340 green on the addendum head `cf5faa7` (id
  43531239531, all four lanes — the standard next-session preflight
  gate). Environment: PARTIAL reset only — the canonical clone at
  /home/z/SupportOS-Plus survived (G-12 intact); toolchain via the §3
  partial-reset exports (RUSTUP_HOME/CARGO_HOME under
  /tmp/my-project); rustc 1.99.1 verified; credentials active.
- R3.5 census executed: r35_census.py (tool-results, G-08
  discipline; naive brace-tracker + arm regex) found the exhaustive
  matches, then a MANUAL alias sweep caught what the scanner missed
  — icons.rs binds `use crate::pages::PageKey as K` (scanner lesson:
  alias-bound arms need a human pass; recorded for R3.6's scanner
  work). Verified-by-read: search.rs/reporting.rs matches are over
  other enums (Scope/Metric — false positives); palette/smoke/page
  files are constructions/iterations, not matches; queries.rs is
  ServerEvent-keyed; core's `match key` sites (workload/operations/
  it_helpscout/sync loops) are non-PageKey types.
- Census verdict: ZERO table folds (full verdict table above). The
  attribute layer is already single-sourced — the fourth
  consecutive plan-overcount correction. One cross-crate seed found
  for R3.6: the drill-slug vocabulary pair (core `page_drill` ↔
  app-shell `page_key_from_drill`).
- Zero code motion → no local battery needed for the R3.5 ledger commit
  (tree code-identical to the CI-green base; the ledger commit is
  docs-only per the addendum convention, riding CI run as its own
  verification).
- R3.6 executed same-session (two-batch session, the R1.2-R1.5
  precedent): census via r36_census.py (const scanner: 4 cross-crate
  groups, ALL keeps) + `as_str() ==` idiom sweep (the real debt: 8
  inline-parse twins of catalog parse fns that ALREADY existed —
  GraphNodeKind ×7, AiAttributeKey ×1) + the seeded drill-slug pair
  (const-in-pattern for &str probed with a rustc 1.99.1 one-liner
  BEFORE editing — it compiles, so the app-shell match kept its shape).
  Folds: 8 parse twins → parse calls (validate_graph_node_kind's
  message preserved byte-identical via ok_or_else; kind_from_str kept
  as a 1-line local alias); drill slugs → new 5-line `catalog::routes`
  module consumed by both sides. One rustfmt canonicalization en route
  (the ok_or_else block). Full battery green at EXACT baselines:
  gate clean, fmt clean, clippy x2, core 520 / sandbox 528 /
  app-shell 293+2 / catalog+xtask 56, verify-icons verified,
  strip-check 207/207 (was 206; +1 = routes.rs).
- Commits: `3074b5e` (R3.5 ledger) + this session's R3.6 batch commit.
- STAGE R3 CLOSED at R3.6 (R3.1–R3.6 terminal); R4.1 queued with the
  census-first pointer.

### Session 11 — 2026-11-13
- Pre-flight: run#338 green on `ae64a25` (all four lanes — the
  docs-only addendum head, the standard preflight gate). Sandbox was
  reset again: the clone under /tmp/my-project was gone (an empty
  dir remnant was all that survived of it); tool-results, rustup, and
  cargo-home under /tmp/my-project DID survive. Re-cloned to the
  canonical /home/z/SupportOS-Plus (handover §2's stated location —
  also the only tree the file-edit tool reaches, G-12);
  github-reconnect.sh needed the empty remnant removed first (its
  exists-branch only re-sets the remote URL, so a leftover empty dir
  silently skips the clone). Git identity + credentials re-applied by
  the script.
- Warm build ran detached (G-10 subshell form,
  `cargo test --locked --workspace --no-run`) while the census
  executed — the dependency compile came off the battery clock.
- Census first (no scanner needed — the fragment shape is textual):
  the "Sync now" toolbar fragment is byte-identical across exactly 4
  pages (dashboard 518, customers 328, reports 365, organizations
  203; the pointer's 5-site estimate included inbox, which censuses
  OUT — different label policy, hover, guard, and status channel).
  All other composition candidates censused to keeps (verdict table):
  hosting already shared via design::, latches are policy, RANGES
  already single-sourced with a 2-site loop, LIST_CAP single-site.
- R3.4 executed: helper `sync_now_button` in sync_health.rs (32
  lines: the button + hover + engine match + the three note strings,
  returning the re-read flag); the 4 call sites became 3-line ifs;
  the None-arm's invalidate() stays caller-side (page-specific). Net
  +41/−97 across 5 files (−56). String byte-equality verified
  programmatically — all five strings, old fragments vs helper,
  including the \u{2026} ellipsis decode and the line-continuation
  whitespace strip. Dashboard's ButtonVariant import orphaned by the
  fold — trimmed.
- Battery: gate clean, fmt --check clean, clippy workspace (3m46s) +
  sandbox lane (1m10s) clean, catalog+xtask 56, app-shell 293+2
  (287s), core 520 (501s), core-sandbox 528 (485s), verify-icons
  verified, strip-check 206/206 — EXACT baselines held. G-05
  preemptive incremental-rm before each test lane; disk never below
  2.0G free.
- One tooling incident during the call-site surgery: the first
  python fragment-locator matched the fragment END before the block
  had opened (a depth==0 precondition bug) and mangled three files
  in one write; recovered via git checkout + the fixed script
  (opened-flag guard); both runs re-verified with the Read tool.
  No repo damage persisted.
- Commit: `refactor(R3.4)` = `9edd9b9`; CI run#339 (id 43528209006)
  verified green the same session — all four lanes success, kittest
  drift gate green (pixel-neutral held on every golden).

### Session 10 — 2026-11-13
- Pre-flight: run#336 green on `ac63c83` (all four jobs verified via
  the API — the docs-only addendum head, D-08 satisfied). Clone,
  toolchain, and credentials all survived the session reset (partial
  reset pattern continues); disk freed to 2G budget before the battery
  (G-05 hit preemptively twice: 92% after the clippy lanes, 91% at the
  final strip-check — incremental rm between every lane held).
- Census first (r33_triage.py in tool-results, built on r25's
  strip_file + region tracker): 13 production send-tails, 6 client
  builders, and the string-extraction idiom family 139 (93 flat
  `.map(|s| s.to_string())` + 8 nested depth-2 + 4 `String::from` + 23
  `unwrap_or_default().to_string()` + 11 literal-default). Two
  scanner defects found and fixed en route (recorded in G-08's
  refinement + G-11): (a) `out.append('""')` appends a 2-char list
  ELEMENT so `len(out)` undercounts by one per string — line_starts
  desynced from the joined text (567/631 lines misaligned in
  ai_lm_studio; fixed by `out.extend`); the same latent bug existed in
  r25's original via `out.append("'c'")` — r25's reported LINE NUMBERS
  were never cross-checked, so it went unnoticed (counts were always
  right). (b) the bash tool's DISPLAY eats the literal `[m` (ANSI
  reset fragment) from shown output — files on disk are intact;
  verify via `wc -c`/computed counts, never by eyeballing displayed
  text (G-11).
- Execution: three prongs. (1) ai_lm_studio.rs — `send_json<T>`
  (private, `serde::de::DeserializeOwned`) reproducing the 4-stage
  tail with byte-identical `{op} failed/read failed/request failed
  (HTTP …)/parse failed` messages, plus `authorized()` on
  OpenAiCompatibleClient for the bearer-if-present attach (5 sites);
  4 tails converted (list_models, chat, post_chat_body, embed);
  is_available rides authorized only. (2) helpscout_real/http.rs —
  `docs_request_json(req, method_label, path, unreachable_friendly,
  invalid_json_friendly)`; both docs_request_inner and docs_request_send
  call it; the two continuation-form friendly strings were verified
  byte-identical by resolving the old `\`+newline+whitespace forms
  and substring-comparing against the new single-line literals.
  (3) new `core/src/json_fields.rs` (9 LOC: json_str =
  `v[key].as_str().map(str::to_string)`, json_string =
  `json_str().unwrap_or_default()`); 122 flat sites converted by
  r33_convert.py (whole-file multiline-tolerant regex with per-file
  count assertion against the census — the per-line variant failed on
  wrapped forms, the R3.2 multiline lesson reconfirmed); 14
  owned-Value receivers caught by the compiler and fixed to `&`
  (http.rs token fns ×8, mod.rs `full` ×5, analytics `q` ×1).
- Post-batch census: 0 convertible flat sites remain; keeps intact
  (nested 8, literal-default 11, send-tails 13→9 through the 2
  helpers, builders 6). 2 unit tests for json_fields were drafted
  then DROPPED to hold the exact-baseline battery discipline (the
  mock-server roundtrips in it_lm_studio/it_helpscout exercise both
  helpers through every converted consumer — the R3.2 no-new-tests
  precedent).
- Battery: gate clean, fmt --check clean, clippy workspace + sandbox
  clean, core 520 (468s), core-sandbox 528 (481s), app-shell 293+2
  (288s, all 26 kittest binaries linked, no G-05 hit), catalog+xtask
  56, verify-icons verified, strip-check 206/206 (205 + json_fields.rs
  — the +1 is the new file). ALL GREEN at EXACT baselines.
- Diff class review: programmatic per-line pattern audit + manual
  read; every hunk is helper-addition, call-site conversion, import
  addition, or fmt reflow of those. Net: 6 files modified + 1 new
  module + lib.rs declaration; +274/−312 plus the 9-line module.
- R3.3 done — CI closure: run#337 (id 43512962814) GREEN on `518ba64`
  — all four jobs success, kittest drift gate green (the HTTP/serde
  plumbing is data-path only; byte-identical messages and values feed
  the same rendered pages). complete.md R3.3 row filled; task TERMINAL.
  Pointer advances to R3.4 (page scaffolding) WITH the read-only prep
  census recorded in the pointer (the "Sync now" ×5 fragment is the
  leading candidate).
- Session closed: this closure line + the CI-verified rows ride the
  addendum commit; its docs-only CI run is session 11's preflight
  gate.

### Session 9 — 2026-11-13
- Pre-flight: sandbox PARTIALLY reset (clone + ~/.cargo survived; /tmp
  wiped; github-reconnect.sh re-applied credentials). Head `bfad483`
  (the G-10 refinement ledger commit) in sync with origin; run#334
  in_progress at session start — polled to GREEN (all four lanes)
  before any source edit, per D-08. Repo state was AHEAD of the
  session-8 handover narrative: session 8's tail had pushed the G-10
  refinement (`bfad483`), cancelling run#333.
- R3.2 executed (the CURRENT POINTER batch; census + helper design
  from session 8's read-only prep, regenerated and re-verified at
  session start — 73 sites, exact match):
  - db.rs gained `pub(crate) fn query_rows<T>(conn, sql, params: impl
    rusqlite::Params, map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>)
    -> Result<Vec<T>>` (prepare `?` + query_map `?` + lenient collect;
    mirrors rusqlite's own query_map bound shape so every existing
    argument form — `[]`, `[x]`, `params![…]`, `params_from_iter`,
    `&[&dyn ToSql]` — passes unchanged) and `pub(crate) fn
    rows_ok<T, F>(MappedRows<'_, F>) -> Vec<T>` (the tail-only form,
    generalizing ai_tools' Value-only local `collect_rows`, which was
    deleted).
  - 80 sites converted (75 full-folds + 5 tail-only + 7 turbofish
    census blind-spot + the absorbed helper's 2 call sites; verdict
    table above). Fold discipline per D-17: one-call collapse ONLY
    where prepare/query fallbacks are observably identical; the 5
    differing-fallback sites keep their prepare structure and convert
    the tail via `rows_ok`.
  - Interim compile checks after each ~10-file tranche caught nothing;
    two MultiEdit application gotchas en route (a mismatched indent on
    workers/execute's block form; a candidate_kinds fn duplicated
    across two test mods needing replace_all) — both fixed before the
    fmt pass.
- Battery (incremental after a subshell-detached warm build that ran
  during the run#334 wait): gate clean, fmt --check clean, clippy
  workspace + sandbox clean, core 520 (468s), core-sandbox 528 (453s),
  app-shell 293+2 (256s), catalog+xtask 56, verify-icons verified,
  strip-check 205/205 — ALL GREEN, counts EXACTLY at baseline.
  G-05 discipline: incremental rm between lanes (peaked 89% before the
  app-shell link, 86% after; never hit a link failure).
- Net diff: 32 files (db.rs + 31 site files), +657/−823 (net −166).
  Post-batch census scan: ZERO lenient tails remaining outside db.rs's
  own helper bodies (the 2 definitions); 77 query_rows call sites + 5
  rows_ok call sites + 0 collect_rows.
- R3.2 done — CI closure: run#335 (id 43492961540) GREEN on `5cc08fc`
  — all four jobs success, kittest drift gate green (the SQL plumbing
  touches no rendered-page layout; the data paths feed the same
  values). complete.md R3.2 row filled; task TERMINAL. Pointer advances
  to R3.3 (HTTP/serde plumbing, census first).
- Session closed: this closure line + the CI-verified rows ride the
  addendum commit; its docs-only CI run is session 10's preflight
  gate.

### Session 8 — 2026-11-13
- Pre-flight: sandbox reset at the context boundary (clone + toolchain
  gone) — re-provisioned per handover §2. Repo state was AHEAD of the
  session-7 handover summary (that summary was generated mid-fix-battery,
  before the session's tail work landed): the G-09 ammonia incident had
  closed (`ea089b0`, run#329 green), session 7 then executed R2.5 to
  terminal (`0984936`, run#330 green) plus the CI-verified addendum
  `17a3ff6` — run#331 GREEN confirmed via API before work began. STAGE R2
  TERMINAL on arrival; pointer at R3.1. (Ledger date note: session 6/7
  log headers say 2026-11-15 but the git commit timestamps are
  2026-11-13 — the machine clock is authoritative; session 8 dates from
  it.)
- R3.1 census (the R2.5 lesson applied — precise scan before sizing):
  `impl Display` tree-wide = exactly 3 hand-rolled (LmStudioError,
  HsApiError, AcquireError); `thiserror` usage = exactly 1 (core error.rs);
  error-named types = 5 total (census incl. a hidden-type sweep over
  `Result<T, E>` params, which surfaced ai_eval's private `CaseErr` tuple
  alias); catalog: zero error types; xtask: anyhow. ZERO duplicate error
  enums — the root-enum + Other-boxed-subtypes architecture already
  satisfies the plan's consolidation prong.
- R3.1 executed (verdict table above): the 3 hand-rolled Display/Error
  impls → thiserror derives with byte-identical messages (AcquireError's
  `Io(#[source] …)` reproduces the hand-rolled source() exactly — already
  Running → None, Io → Some(inner)); TranslateError enum → newtype (its
  variant class was never read by any consumer — execute.rs destructured
  all three arms identically; 8 ctor sites + the triple-arm match
  collapsed to `e.0`); app-shell manifest +`thiserror.workspace = true`
  (Cargo.lock diff: exactly one edge line — the crate was already in the
  lock via core; no version movement).
- Battery (cold target/): gate clean, fmt --check clean, clippy workspace
  (2m24s) + sandbox lane (40s) clean, core 520 (484s), core-sandbox 528
  (460s), app-shell 293+2 (266s — the 2 are single_instance's own tests,
  green on the derived AcquireError; 27 kittest binaries linked, 0 run,
  F-06), catalog+xtask 56, verify-icons verified, strip-check 205/205 —
  all green. G-05 managed preemptively (incremental rm between lanes;
  disk peaked 94% right after the app-shell link, back to 76% after rm).
- G-10 discovered and refined in-session: a `nohup … &`-launched cargo
  lane DIED at the end of the tool call that launched it (same class as
  the session-6 rustup kill; setsid+nohup also died). BUT the
  subshell-detached form `(cmd > log 2>&1 &)` SURVIVES — proven with the
  Next.js dev server across many later tool calls (incl. a full
  agent-browser session; the sandbox preview console verified
  end-to-end: tabs, 15 sliders w/ keyboard, register 35 rows w/ row
  expansion, lightbox+Escape, mobile 375 no-hscroll, zero console
  errors). Foreground-window iteration remains the fallback for cargo
  lanes that outlive one 10-min window. Memory.md carries the refined
  G-10.
- Net diff: 6 source files + app-shell/Cargo.toml + Cargo.lock,
  +19/−62 in sources (+1 manifest line, +1 lock edge line). R3.1 done —
  the CI row rides this push; terminal confirmation + run id at the next
  poll (addendum pattern). Pointer advances to R3.2 (SQL plumbing,
  census first).
- CI closure: run#332 (id 43476466581) GREEN on `5ca31af` — all four
  jobs success; kittest drift gate green (single_instance is not a
  rendered page — pixel-neutral trivially; the TranslateError collapse
  is worker-side only). complete.md R3.1 row filled; task TERMINAL.
- R3.2 census (read-only prep, done while run#332 executed — the
  executing session reads this instead of re-scanning):
  - THE actionable class: the lenient row-collect tail
    `.map(|rows| rows.filter_map(|r| r.ok()).collect())` — **73 sites
    across 28 files** (a few spell the row var `x`; regenerate the site
    list with `rg -n "filter_map\(\|[rx]\| [rx]\.ok\(\)\)\.collect\(\)"
    crates/core/src --type rust`). Candidate helpers in db.rs (the
    storage infrastructure module): (a) full wrapper
    `pub(crate) fn query_rows<T>(conn, sql, params: impl Params, map:
    impl FnMut(&Row<'_>) -> rusqlite::Result<T>) -> Result<Vec<T>>`
    (prepare + query_map + lenient collect; HRTB closure signature like
    rusqlite's own query_map) for the clean 3-step sites (~50–60 by
    crude scan); (b) tail-only `rows_ok(rows) -> Vec<T>` for the ~13
    sites that bind the query_map result to a var / interleave logic.
    BEHAVIOR CONTRACT (invariant 3): prepare Err propagates, query_map
    Err propagates, individual ROW errors are SILENTLY SKIPPED — the
    lenient semantics must be preserved exactly; do NOT "improve" it to
    strict during the DRY pass (that would be an unsanctioned behavior
    change smuggled into a mechanical batch).
  - KEEP verdicts (census-recorded, no action): strict collect
    `collect::<rusqlite::Result<Vec<_>>>()` (60 sites — the canonical
    one-expression idiom, OPPOSITE semantics to the lenient tail);
    scalar `|r| r.get(0)` mappers (362 sites — canonical rusqlite
    idiom, nothing to consolidate without obscuring); JSON column
    defaults (~15–20 sites across FOUR semantic shapes — json!({}) /
    Value::Null / Default / "{}" string — heterogeneous per-site
    choices; a helper per shape = 4 micro-helpers used a handful of
    times each, net complexity UP); per-module named row mappers
    (digest_from_row et al. — the established GOOD pattern; each maps
    its own table's columns; zero cross-module duplication found);
    incidental SQL literal repeats (short test/seed queries, 3–5
    textual coincidences — coupling unrelated code would be anti-DRY);
    `.optional()` (9 sites — idiomatic rusqlite extension).
  - Sizing: 73 mechanical edits + helper(s) + battery = one session's
    batch. Execute per-file (read site → edit tail) — NOT blind regex
    (indent levels and interleaved logic vary; ~13 sites bind the
    query_map result to a var first). Census-first discipline held: the
    plan's "row-mapping/statement boilerplate" letter shrank to ONE
    real class + a wall of canonical-idiom keeps.
- Session closed: this closure line + R3.2 prep + the R3.1 CI-verified
  rows ride the addendum commit; its docs-only CI run is session 9's
  preflight gate.

### Session 7 — 2026-11-15
- Pre-flight: context boundary + sandbox reset (clone, /tmp logs, and
  the session-6-tail ammonia lock bump all lost) — re-provisioned per
  handover §2. Head `cdc38ee` RED (run#328): RUSTSEC-2026-0533
  (ammonia 4.2.4 mutation-XSS) landed in the live advisory DB between
  runs #327/#328 on the same dependency tree — diagnosed from the
  session-6 job-log pull; incident protocol executed per G-09: lock-only
  bump (`cargo update -p ammonia` → 4.2.5; manifest "4.2" already
  covered), FULL battery re-run green (gate, fmt, clippy x2, core 520,
  sandbox 528, app-shell 293+2, catalog+xtask 56, verify-icons,
  strip-check 205/205), PLUS local `cargo deny check advisories bans
  licenses sources` green (0.20.2 installed ad hoc). Committed
  `ea089b0`; run#329 (id 43442412648) GREEN on it — base restored.
- R2.5 executed on the green base (the CURRENT POINTER batch; census
  + prep done session 6): policy D-15 written (memory.md) — 3
  exhaustiveness-forced `unreachable!` keeps with written local proofs,
  1 arm-level-cfg(test) panic reclassified D-10-exempt; ONE code
  increment: execute.rs:1833 bare `unreachable!()` normalized to
  `unreachable!("Ok(Some) is handled by the outer match arm")`;
  census correction vs the plan's letter recorded (4 real production
  occurrences, not 30 — 27 test-region overcounts; Q-04 owner-override
  record for the 3 keeps).
- Battery: gate clean, fmt --check clean, clippy workspace + sandbox
  clean, core 520, core-sandbox 528, app-shell 293+2, catalog+xtask 56,
  verify-icons verified, strip-check 205/205 — all green (post-batch
  values recorded in the closure push).
- R2.5 TERMINAL → STAGE R2 TERMINAL (R2.1–R2.5). Terminal row in
  complete.md; the R2.5 CI row rides this push. Pointer advanced to
  R3.1 (DRY pass, first batch — read plan §7 R3 next session start).
- Session closed: this log + pointer + D-15/Q-04/G-09 ride the R2.5
  commit `0984936` per D-08; CI run#330 (id 43451237165) verified
  GREEN on it before close (all four jobs success — kittest drift gate
  green with the execute.rs message edit). This closure line rides
  `0984936`+1 — its own docs-only run is the next session's preflight
  gate. STAGE R2 TERMINAL.

### Session 6 — 2026-11-15
- Pre-flight: sandbox reset AGAIN (clone + toolchain gone) — re-provisioned
  per handover §2 (re-clone via tool-results/github-reconnect.sh +
  rustup 1.29.3 + toolchain 1.99.1). CI run#324 (id 43397843971)
  GREEN on the session-5 closure head `9585089` confirmed before work
  began — the preflight gate the session-5 closeout line predicted.
- Repo state matched the ledger exactly: 6 commits since the session-4
  summary (R2.2 terminal f06e1ba→run#317, R2.3 981f2a0→run#319, R2.4-a
  d8cbb9c→run#322, triage adbb390→run#321, ledger addenda 68a06e5/9585089
  →runs #323/#324).
- R2.4-b executed (the CURRENT POINTER batch): the 17 class-C sites
  resolved per the session-5 prep notes. 9 ELIMINATED across 7 files
  (live_chats 349, notification_sweep 164, coaching 461+485, inert
  641-644, quality 629, embeddings 690+744 via the single
  `qdrant.filter(...)` rebind + two if-let gates, settings 19) —
  including ALL 4 remaining production unwraps; 8 KEPT with written
  D-14 justifications (see the class-C closure in memory.md).
  Notable mechanics: the live_chats rewrite leans on edition-2024
  if-let temporary scoping (the scrutinee borrow ends before the else
  arm's cache write — verified by compile); inert's two not-found
  arms produce byte-identical `not_found_remote("POST")` errors so the
  let-else reorder is unobservable; quality's Map build keeps the same
  serde_json::Map type and insert order as the json! literal.
- One fmt write pass needed (inert.rs chain reflow) — fmt --check green
  after it; net diff +32/−33 over 8 files (7 source + this ledger).
- Battery: gate clean, fmt --check clean, clippy workspace (52s) +
  sandbox (29s) clean, core 520 (469s), core-sandbox 528 (455s),
  app-shell 293+2 (285s; kittest binaries all linked, 0 run — no
  Vulkan, F-06), catalog+xtask 56, verify-icons verified, strip-check
  205/205 — all green. G-05 managed preemptively (rm incremental
  between lanes; peak 94% during app-shell link, then back to 77%).
- Post-batch scanner (G-08) census: production 65→56, unwraps 4→0,
  expects 61→56 — D-14's tree-wide end state reached (56 production
  occurrences, all expects, each with a written justification; zero
  production unwraps for the first time in the program's history).
- R2.4 TERMINAL (both batches). Terminal row written to complete.md;
  the R2.4 CI row rides this push — final confirmation next poll.
  Pointer advanced to R2.5 (panic!/todo!/unimplemented!, 30 + 3
  unreachable arms per the session-4 survey).
- CI closure: run#325 (id 43418287308) GREEN on `82570a4` — all four
  jobs success; notably the kittest snapshot drift gate passed with the
  live_chats.rs edit (the if-let rewrite was pixel-neutral as
  predicted — the then-arm's clones are identical values) and both
  packaging lanes green. complete.md's R2.4 row filled with the run id.
- R2.5 prep (read-only analysis done while CI ran; the scanner was
  extended in tool-results — r25_triage.py adds panic/todo/
  unimplemented/unreachable patterns to the G-08 machine, r24 kept
  intact as the unwrap/expect census):
  - PRECISE PRODUCTION CENSUS: FOUR occurrences, not the plan's 30
    (the session-4 crude count included test regions — the same
    overcount pattern R2.2's allow census hit). xtask: zero. todo!/
    unimplemented!: zero tree-wide in production.
  - app/mod.rs:891 `unreachable!("inbox handled above")` — KEEP
    shape: exhaustiveness-forced arm; proof is cross-branch (the
    match runs only in the `else` of `if self.page == PageKey::Inbox`,
    verified by read). Removal would need a wildcard-None arm (silent
    no-op — dishonest) or a UI-dispatch restructure (pixel drift,
    D-05).
  - oauth_state.rs:95 `unreachable!("hex_digit got {n} ...")` — KEEP
    shape: exhaustiveness-forced `_` on u8; proof is the callers'
    bit-masks (`b >> 4`, `b & 0x0f`); a range-newtype to satisfy the
    type system is overkill/R3 territory.
  - execute.rs:1833 `_ => unreachable!()` — BARE (message-less);
    exhaustiveness-forced for `Result<Option<T>, E>` narrowed by the
    outer match's `Ok(Some(article))` first arm (cross-match proof).
    CANDIDATE R2.5 CODE INCREMENT: message normalization
    `unreachable!("Ok(Some) handled by the outer arm")` (D-14 class-A
    precedent — under constraint 3 the message is the only in-code
    doc a survivor may carry).
  - execute.rs:468 `panic!("intentional test panic (batch 27 item 86)")`
    — inside `#[cfg(test)] "__test_panic" =>` match ARM: arm-level
    cfg(test) compiles the arm only in test builds — semantically test
    machinery (D-10 exempt), NOT a production panic. G-08 REFINEMENT
    (record, do not re-write the scanner): the region tracker models
    cfg(test) at mod level; arm-level attributes escape it; 1
    instance tree-wide, manual correction suffices.
  - Expected R2.5 shape: policy D-15 (panic family, D-14-shaped):
    three exhaustiveness-forced unreachable! keeps with written
    justifications + the one message normalization + the census
    correction vs the plan's letter (Q-04-style record or fold into
    Q-03's pattern). Docs-heavy, single small code edit; one commit.
- Session closed: ledger addendum pushed on `0941fb3`; its docs-only CI
  run#326 (id 43420872242) verified GREEN before close (all four jobs
  success). This final closure line rides commit `0941fb3`+1 — its own
  docs-only run is the next session's preflight gate.
- POST-CLOSE INCIDENT (recorded under G-09): after the closure push, a
  final pointer-refresh commit `cdc38ee` went out (pointer updated from
  the stale "needs census" wording to the done-census state — the
  census itself was done pre-close and recorded in the R2.5 prep
  above). Its docs-only run#328 (id 43426662490) came back RED —
  not from the ledger edit but from RUSTSEC-2026-0533 (ammonia 4.2.4
  mutation-XSS), a fresh advisory that landed in the live DB between
  runs #327 and #328 on the SAME dependency tree. Diagnosed via the
  job log (handover §7 step 3); fix = lock-only bump `cargo update -p
  ammonia` → 4.2.5 (manifest "4.2" already covers it; core's usage:
  richtext.rs sanitize_html — genuinely reachable, patched not
  ignored). Full battery re-run green (gate, fmt, clippy x2, core 520,
  sandbox 528, app-shell 293+2, catalog+xtask 56, verify-icons,
  strip-check 205/205) PLUS local `cargo deny check advisories bans
  licenses sources` green (cargo-deny 0.20.2 installed ad hoc) — the
  deny lane verified BEFORE the push this time. Fix commit rides the
  next push; its CI run is the new preflight gate for R2.5.

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
- Session closed: ledger addendum pushed on `68a06e5`; its docs-only CI
  run#323 (id 43394732056) verified GREEN before close (all four jobs
  success). This final closure line rides commit `68a06e5`+1 — its own
  docs-only run is the next session's preflight gate.

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
- PROGRAM 4 CLOSED (session 26, Rule 8 STOP): audit-fix closure
  complete — R9.1/R9.2/R9.3 terminal, v1.10.2 published; queue empty.

## session 26 — Program 4 (audit-fix closure)

- R9.1 executed: core::testkit::TempDir Drop-guard (+ app-shell
  testsupport twin) and the full test-region conversion sweep (28 files:
  7 workers files + sync_engine + connection_doctor + provider_handle
  + embeddings + vectorstore + knowledge_store + backup_service +
  config + bench + data_reset + connectors + search + ai_evidence +
  core lib.rs, and app-shell: testsupport + single_instance +
  workers_host + knowledge/settings tests + inbox rename); the inbox
  `truncate_chars` -> `ellipsize` rename (4 call sites); bench's local
  TempDirGuard folds into testkit. Net -175 lines. Battery at the exact
  terminal baselines: gate; unwrap-policy (0/1/0 production, machinery
  3+1, test-region 2356 exempt, 46 whole-file modules); fmt; clippy x2
  -D warnings; core 520; core-sandbox 528; app-shell 293; catalog+xtask
  79; verify-icons; strip-comments 262/262 (testkit counted). The leak
  check is the new evidence line: ZERO spp-* dirs in /tmp after the
  full battery (previously ~24+ leaked per run). CI verification on the
  pushed head to follow.- R9.2 executed: repo presentation surfaced at root — git mv
  _archive/{README.md,LICENSE,CHANGELOG.md} to the root (GitHub license
  detection + landing page render; CHANGELOG follows Keep-a-Changelog's
  root convention); release.yml's changes.md extractor follows the move
  (`CHANGELOG.md` path); the README's stale CI bullet (cited the
  R0-removed python gate scripts and traceability step) refreshed to the
  current CI truth (xtask gate + unwrap-policy, 20-lint clippy set,
  203-golden drift gate, arm64 packaging lane, dev-battery zero-comment
  note); the dangling `!.env.example` gitignore negation (no such file
  exists; nothing loads .env files) dropped. Code tree byte-identical to
  R9.1's head — the docs-only precedent equivalence; fast gates re-run
  green; CI verification on the pushed head to follow.- R9.3 executed: version 1.10.2 + CHANGELOG [1.10.2] (the audit-fix
  patch). Workspace version bump 1.10.1 -> 1.10.2 (root Cargo.toml;
  members are version.workspace = true) with Cargo.lock synced (exactly
  the 4 workspace-member version fields; no dependency changes — the
  1.10.1-era httparse coincidence does not recur). Drift safety per the
  R7.6 analysis: the CARGO_PKG_VERSION display lives in app/mod.rs
  sidebar chrome (not rendered by any kittest suite) + settings x2 +
  fatal.rs (error-path-only); grep confirms no version content in any
  golden. Fast local gates re-run green on the bumped tree (gate /
  unwrap-policy / fmt / verify-icons / strip 262/262); full battery
  rode R9.1's byte-identical code head 34b4186 (CI run#392) per the
  docs-only precedent equivalence. Tag v1.10.2 follows at the green
  head; release evidence chain to follow in complete.md.- R9.3 release evidence chain: CI #394 green on the release head
  3c0a64d (all four lanes; the kittest drift gate confirmed the
  203 goldens byte-identical through the R9.1 sweep); tag CI #395
  green; Release run#40 all-6-jobs green; live-verified post-publish
  (BUILD-INFO 1.10.2/3c0a64d match, assets HTTP 200, SHA256 digest
  MATCH x2, root-changelog release-note extraction working).
  Program 4 closing report in complete.md; handover state line ->
  ALL FOUR PROGRAMS CLOSED. Rule 8 STOP.
