# Refactor Progress — Live State

Last updated: session 8 (2026-11-13). Update rule: every session updates
this file before ending. Statuses: `pending` / `in_progress` / `done` /
`terminal` (done + closed in `complete.md`) / `dropped` (with reason in
`memory.md`).

## CURRENT POINTER

- Next task: **R3.2 — SQL plumbing** (rusqlite row-mapping/statement
  boilerplate → shared helpers in the storage modules; see plan §7 R3).
  CENSUS FIRST — the standing R2.5 lesson (precise scan before sizing a
  plan batch; three consecutive census corrections came from skipping it).
- Pre-flight [session 8]: R3.1 executed on the green base `17a3ff6`
  (run#331); R3.1 head `5ca31af` is CI-VERIFIED GREEN — run#332
  (id 43476466581), all four lanes success (addendum commit below).
  Green base for R3.2: the addendum head's own docs-only run (the
  standard next-session preflight gate).
- Session counter: 8 completed (R1 terminal; R2 stage TERMINAL — R2.1
  through R2.5; R3.1 TERMINAL — run#332 green on `5ca31af`).
- Environment: session 8 began with a sandbox reset at the context boundary
  (standard §2 re-provision: re-clone + rustup 1.29.3 + pinned 1.99.1).
  Cold target/; G-05 managed preemptively (incremental rm between lanes,
  peak 94% after the app-shell link). NEW: G-10 — backgrounded builds DIE
  at tool-call end (nohup does not survive); the working pattern is
  repeated foreground 10-min windows (cargo artifacts persist between
  killed runs). Triage tooling: see G-08 — scanners at
  tool-results/r24_triage.py (unwrap/expect) and r25_triage.py (panic
  family).

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
| R3.2–R3.6 DRY | pending | see plan §7 R3 |
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

## Session log

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
- G-10 discovered: a backgrounded cargo lane (`nohup … &`) DIED at the
  end of the tool call that launched it — nohup does not survive the
  call boundary in this sandbox (same class as the session-6 rustup
  kill). Working pattern: repeated foreground windows at the max 10-min
  timeout; cargo preserves compiled artifacts between killed runs, so a
  cold lane grinds to completion across 2–3 windows. Recorded in
  memory.md.
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
