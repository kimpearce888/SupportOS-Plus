# Refactor Memory — Durable Decisions, Facts, Gotchas

The cross-session brain. Append-only; never rewrite history here. Every
session ends by adding what the next session must not have to rediscover.

## Decisions (D-xx)

- **D-01 (session 1).** Docs are not the source of truth (owner constraint
  1) → the traceability gate and its two doc inputs left the build in R0.
  The build now depends only on code and code-only machinery.
- **D-02 (session 1).** The no-simulation gate STAYS: it scans code only,
  is part of the build pipeline, and aligns with constraint 1. Its
  allowlist keys on exact path `crates/core/src/audit.rs` + two exact
  stripped line contents; stale entries are harmless (verified by reading
  the gate end-to-end), but reformatting/moving those lines without
  updating the allowlist fails CI.
- **D-03 (session 1).** Comment removal is done by a syntax-aware lexer
  (`xtask strip-comments`, task R1.0), never regex — string literals carry
  `//` (URLs), there are char literals, raw strings, and nested block
  comments. The tool is itself comment-free.
- **D-04 (session 1).** `#![warn(missing_docs)]` + `#![allow(missing_docs)]`
  pairs exist in catalog and core lib.rs; allow wins, so deleting doc
  comments keeps clippy `-D warnings` green. The redundant pairs are
  deleted in R1.10.
- **D-05 (session 1).** The 203 kittest golden PNGs are build artifacts
  (test baselines), not docs — they stay. Mechanical refactors must be
  pixel-neutral; deliberate rebaseline = UPDATE_SNAPSHOTS run + byte-
  identical re-run + CI (invariant 4).
- **D-06 (session 1).** Kittest suite file names and test fn names stay
  stable through R1–R4; they key the goldens (invariant 5).
- **D-07 (session 1).** Constraint 3 (no comments) applies to all new code
  written during the refactor, including the strip-comments tool itself.
  Where a rationale needs a home, it goes in this folder (these files),
  never in source comments.
- **D-08 (session 1).** Commit/push discipline: battery-green commit per
  batch, single push per session, next session blocked until CI green
  (invariants 1–2, handover §5/§6).
- **D-09 (session 1).** Sandbox resets wipe both the clone and
  `~/.cargo`; re-provision per handover §2 at every session start.
- **D-10 (session 1).** unwrap/expect/panic policy for R2.4/R2.5:
  forbidden in production paths; exempt only `main.rs` bootstrap and
  `#[cfg(test)]` regions. 121 `unwrap()` currently sit in src; 2,300
  `expect(` in src are mostly inline test modules — triage separates them.
- **D-11 (session 2).** The stripper's contract: whole-line comments
  delete their line+newline; trailing comments trim preceding whitespace;
  inline block comments keep the whitespace AFTER them (rustfmt
  canonicalizes — the batch procedure always runs `cargo fmt --all` after
  stripping).
- **D-12 (session 2).** Battery order matters for the strip batches:
  strip → `cargo fmt --all` → battery. fmt runs in WRITE mode between strip
  and the battery; the battery's fmt item is the --check verification.
- **D-13 (session 4, R2.2).** `#[allow]` survivor policy — the active
  justifications for every allow that stays (R2.2's mandate; constraint 3
  says no comment may carry it, so it lives here). Three vestigial allows
  were deleted in R2.1/R2.2 (testsupport dead_code, issue_radar
  unused_mut, search/lanes ref_option — each had outlived its reason).
  Survivors, by class:
  - **unsafe_code (3):** `core/vectorstore_sqlitevec.rs` (sqlite-vec C
    API FFI — the crate is otherwise `deny(unsafe_code)`; this is the one
    confined unsafe island), `core/sandbox.rs:153` (test-region FFI for
    the sandbox feature — D-10 exempts tests), `app-shell/src/lib.rs:14`
    (gates `pub mod single_instance` — the OS single-instance lock FFI;
    crate-level deny elsewhere). All three are minimal, single-purpose
    FFI boundaries — exactly what `allow(unsafe_code)` exists for.
  - **crate-level style pairs (3: app-shell/catalog/core lib.rs):**
    `missing_errors_doc` is UNSATISFIABLE under constraint 3 (its remedy
    is `///` doc comments, which are deleted everywhere); renaming to
    satisfy `module_name_repetitions` would churn the idiomatic
    self-named-module pattern (people_store::PeopleStore et al.) across
    23 routes for zero gain. Deliberate crate-wide style policy.
  - **too_many_arguments (15 in-src + 9 in-test):** parameter-object
    consolidation is R3.4's job — the DRY pass determines WHICH argument
    clusters repeat enough to become shared structs; fixing them ad-hoc
    now would mint one-off structs that R3 must then re-merge. The in-test
    nine (fixture/constructor helpers) additionally ride D-10's test-region
    exemption. Sites (in-src): core event_bus, ai_pipeline/mod, connectors
    x2, copilot, events, interaction_engine/mod x4, jobs x2, outreach/mod,
    quality/mod, app-shell testsupport (seed_conversation). In-test
    (9): kittest_inbox_copilot x2, kittest_list_pages x2,
    kittest_operations_page, kittest_inbox_page x2, kittest_reports_page,
    kittest_dashboard_page — all fixture/seed constructors.
  - **too_many_lines (4):** inbox.rs x3 (the page's ui() T-composition
    branches) + design/mod.rs — all are F-04/R4.5 split candidates; the
    split is a deliberate R4.5 batch (behavior risk without golden
    verification in-sandbox; kittest is CI-only per F-06).
- **D-14 (session 5, R2.4).** `unwrap`/`expect` production policy — the
  per-class refinement of D-10's blanket "eliminate all production ones"
  (the plan's letter). Precise census at pass start (cfg(test)-aware
  scanner, G-08): 86 production occurrences in 4 classes.
  - **Class B — mutex/rwlock poison (21): ELIMINATED in R2.4-a.** Every
    lock in the tree guards plain data (sqlite connections, vec-index
    state, key slot, cloned inert-world snapshots) with no invariant a
    panicking holder could corrupt, so `.lock().expect("...")` became
    `.lock().unwrap_or_else(|p| p.into_inner())` (the std poison-recovery
    idiom — which `inert.rs::lock_world()` already used; the 5 inert
    sites collapsed INTO that helper instead of duplicating it). Happy
    path is behavior-identical; the panic class no longer exists.
  - **Class A — constant-input constructors (45): KEEP as
    `.expect(...)`.** 40 static `Regex::new(CONST)` compiles (+ 4 seccomp
    `SeccompRule/Condition::new(const-args)` in sandbox.rs, + 1 chrono
    `and_hms_opt(0,0,0)`). The input is a compile-time constant: failure
    is unrepresentable in a correct build, so fail-fast-at-first-use is
    the correct semantics; propagating an unrepresentable error would
    churn ~45 signatures for zero behavior gain and collide with R3's
    DRY pass (session-4 note). R2.4-a normalized the 20 message-less
    `.unwrap()`s of this class to `.expect("static regex")` /
    `.expect("midnight is always a valid time")` — error-path-only
    change; under constraint 3 the expect message is the only in-code
    documentation a survivor may carry.
  - **Class C — guarded post-conditions (17 after A/B): RESOLVED in
    R2.4-b — 9 eliminated, 8 kept.** Each was guarded by an explicit
    same-function check; the batch rule was "eliminate where a natural
    Option/Result-flow rewrite exists, keep where elimination would
    restructure control flow for a provably-unreachable branch."
    ELIMINATED (9): live_chats 349 (is_some_and+expect →
    `if let Some(cache)` over `.filter(...)` — edition-2024 if-let
    temporary scope makes the borrow end before the else arm's cache
    write); notification_sweep 164 (`Some(last().map(...).expect(...))`
    → plain `events.last().map(...)` — hit_cap implies non-empty, the
    Option-flow IS the shape); coaching 461+485
    (`unwrap_or("").is_empty() { } else { .unwrap() }` →
    `match as_str() { Some(s) if !s.is_empty() => ..., _ => "(see
    reasoning)" }`); inert 644 (is_none early-return + `wf.unwrap()` →
    `let Some(wf) = ... else { return Err }`; the two not-found errors
    are byte-identical `not_found_remote("POST")` values, so moving the
    wf check ahead of the conv lookup is unobservable); quality 629
    (`json!` literal + `as_object_mut().unwrap()` → build a
    `serde_json::Map` directly, `Value::Object` wrap once at the end —
    identical Map type and insert order); embeddings 690+744 (bool
    `qdrant_connected` + two far-below expects → single
    `qdrant.filter(|q| q.health().connected)` rebind + `if let Some(q)`
    gates — health() still called exactly once at the same point);
    settings 19 (`bytes.len() == KEY_LEN` guard +
    `try_into().expect("length checked")` → nested match on
    `<[u8; KEY_LEN]>::try_from(bytes.as_slice())` whose Err arm carries
    the same wrong-length Error::Config — one length check instead of
    two). KEPT (8): live_chats 111 (page-model contract; the expect sits
    inside an egui layout closure — unwinding it risks pixel drift
    against the kittest goldens, D-05); ai_pipeline 1116 (the draft row
    was just inserted by this same function — a Result propagation
    needs get_draft to return Result<Option<_>>, signature churn that
    collides with R3); ai_pipeline 1747+1751 (facts JSON and digest row
    are both written by this same pipeline — same-code post-conditions);
    support_graph 294 (static 12-kinds node_specs table ↔
    GraphNodeKind::ALL correspondence; a filter_map rewrite would
    silently DROP a node on an unrepresentable mismatch instead of
    failing loudly); workers/execute 675 (the trigger decision returns
    Some only with a thread id — the type-shape fix, returning the id
    inside the decision, is R3.4 parameter-object territory, noted
    there); webhook 29 (HMAC's `new_from_slice` Result exists for API
    symmetry only — the hmac crate contract accepts any key length);
    embeddings 813 (the json! object-literal post-condition — the
    alternative is duplicating the 8-field literal or bloating it into
    imperative Map inserts; the expect message states the invariant
    honestly, an `if let` would imply a reachable skip). Post-batch
    scanner census: 56 production occurrences, ALL expects, ZERO
    unwraps — D-14's tree-wide end state reached.
  - **Class D — runtime/bootstrap infrastructure (3): KEEP.**
    workers/mod.rs tokio runtime build, task_bridge.rs thread spawn,
    design/mod.rs egui default font family — one-time infra construction
    with no recovery path; bootstrap-equivalent (the plan exempts
    main.rs bootstrap; these are the same nature outside main.rs).
  - **xtask + benches are NOT production** (43 occurrences + bench.rs):
    xtask is dev-invoked build machinery (`cargo run -p xtask`), bench.rs
    is `#[cfg(test)]`-gated at its lib.rs declaration — both ride D-10's
    exemption. This answers the session-4 survey's classification
    question.
  - Deviation from the plan's letter (A/C/D keeps) is recorded as Q-03
    for owner override; the alternative was rejected on the R3-collision
    and unrepresentable-error grounds above.

- **D-15 (session 7, R2.5).** `panic!`/`todo!`/`unimplemented!`/
  `unreachable!` production policy — the panic-family analog of D-14,
  refined from D-10's blanket form. Precise census at pass start
  (G-08 scanner extended with the panic-family pattern set —
  r25_triage.py; r24 kept intact as the unwrap/expect census): FOUR
  production occurrences, not the plan's 30 — the session-4 crude
  count included test regions (same overcount pattern as R2.2's allow
  census and R2.4's 126-vs-86); todo!/unimplemented! are ZERO
  tree-wide in production, xtask ZERO panic-family.
  - **Exhaustiveness-forced `unreachable!` (3): KEEP, each with a
    local proof.** app/mod.rs:891 `PageKey::Inbox` — the match runs
    only in the `else` of `if self.page == PageKey::Inbox` (verified
    by read); removal needs a wildcard-None arm (silent no-op —
    dishonest) or a UI-dispatch restructure (pixel drift, D-05).
    oauth_state.rs:95 `hex_digit` — the `_` on u8 is forced by the
    type; the proof is the callers' bit-masks (`b >> 4`, `b & 0x0f`);
    a range-newtype is R3-territory overkill. execute.rs:1833 —
    `Result<Option<T>, E>` narrowed by the outer match's
    `Ok(Some(article))` first arm; message normalized to
    `unreachable!("Ok(Some) is handled by the outer match arm")`
    (D-14 class-A precedent: under constraint 3 the message is the
    only in-code doc a survivor may carry) — the batch's one code
    increment.
  - **Arm-level `#[cfg(test)]` panic (1): NOT production.**
    execute.rs:468 `__test_panic` arm compiles only in test builds —
    D-10's exemption, reached through a scanner blind spot (G-08
    refinement: the tracker models cfg(test) at mod level; arm-level
    attributes escape it; one instance tree-wide).
  - **Census correction vs the plan's letter:** the plan's "30
    instances" does not exist in production — 27 of the 30 were
    test-region overcounts. The real deviation is keeping 3
    exhaustiveness-forced unreachable! arms, recorded as Q-04.

## Facts (F-xx)

- **F-01.** Baseline (session 1): 204 .rs files, 141,107 LOC — core
  122/87,713 · app-shell 70/50,651 · catalog 8/1,024 · xtask 4/1,719.
  ~4,187 full-line comments (3,174 `//` + 1,005 `///` + 7 `//!` + 1 `/*`);
  trailing inline comments extra; exact total comes from the stripper.
- **F-02.** `catalog` is the closed-vocabulary crate (no I/O deps) — the
  designated home for shared constants (R3.6, R4.4).
- **F-03.** 23 `PageKey` routes; 26 kittest suites + `style_lint.rs` +
  `snapshots/` in `crates/app-shell/tests/`.
- **F-04.** Largest files: inbox.rs 4715 · settings.rs 3316 ·
  workers/mod.rs 2826 · ai_pipeline/mod.rs 2780 · design/mod.rs 2698 ·
  helpscout_real/mod.rs 2675 · knowledge.rs 2647 · workers/execute.rs
  2537 · reports.rs 2374 · ai_center.rs 2299 (split candidates, R4.5).
- **F-05.** Only 3 `macro_rules!` exist today; 31 `#[allow]` attributes;
  unsafe exists only in `app-shell/src/single_instance.rs`
  (`#![allow(unsafe_code)]` there; core/catalog `forbid` it).
- **F-06.** No local Vulkan → kittest suites run in CI only.
- **F-07.** rust-lang.org and github.com are reachable from the sandbox;
  crates.io was used by cold builds in earlier sessions.
- **F-08.** `catalog` is already fully comment-free (0 lines across its 8
  files) — the session-1 aggregate (~4.2K) lives almost entirely in core and
  app-shell. The stripper's report matched a manual count exactly (13 files
  scanned, 2 with comments).
- **F-09.** Local workspace test lanes: 520 tests (core, default features) +
  528 (core sandbox feature); kittest suites only run under
  `--features kittest-ui` (CI snapshots lane). Full local battery on a cold
  cache: ~25 min; incremental ~5-10 min.
- **F-10 (session 3).** The default `cargo test -p supportos-plus-app-shell`
  lane still BUILDS AND LINKS all 26 kittest test binaries (they report 0
  test fns locally — the fns are kittest-ui-gated). Consequences: (a) that
  lane is the local link-exercise for `crates/app-shell/tests/` edits
  (R1.9), (b) it is the lane that trips G-05 disk exhaustion first — keep
  ≥2G free before it. After R1.6 the whole `core` crate is comment-free;
  every remaining R1.x comment lives in `app-shell`.

## Gotchas (G-xx)

- **G-01.** `rg` exits 1 on no matches and silently kills `&&` chains —
  use `;` in exploratory commands (bit session 1).
- **G-02.** Fresh sandboxes set a wrong git identity; re-set it
  (handover §2 step 2) or commits carry the wrong author.
- **G-03.** The release workflow installs python3 in its container only
  for the no-simulation gate; if R5.1 ports the gate to xtask, remove that
  install too.
- **G-04.** `cargo` is never on PATH in a new shell — export it
  (handover §3).
- **G-05.** Sandbox disk is TIGHT (9.9G rootfs; target/ grows to 5.6G with
  both build lanes). When a link step dies with `ld terminated with signal 7
  [Bus error]`, free space with `rm -rf target/debug/incremental` (~2.3G,
  safe — it is only an incremental cache) and retry. Budget: keep ≥2G free
  before starting a battery.
- **G-06.** Huge test/link outputs exceed the tool frame — always redirect
  to a log file (`> /tmp/x.log 2>&1`) and rg the log; never tail a raw
  cargo test run.
- **G-07 (session 3).** Comment removal can EXPOSE latent clippy lints:
  `let_and_return` and `collapsible_if` are suppressed when a comment sits
  between the `let`/outer-`if` and the tail expression / inner `if`; strip
  the comment and `-D warnings` goes red. Hit twice in R1.7 (design/mod.rs,
  workers_host.rs). Fix by minimal behavior-identical restructuring (inline
  the tail expression / collapse the if-chain), never by re-adding comments
  or `#[allow]` (allow-debt belongs to R1.10's cleanup). Expect more of
  these in R1.8/R1.9 (app-shell pages carry the most comments); run the
  clippy lane right after `cargo fmt`, before the test lanes, to catch them
  early.
- **G-08 (session 5; refined session 6).** cfg(test)-aware scanning of
  this tree needs a CROSS-LINE string state machine, never a per-line
  stripper: the storage modules carry multi-line SQL string literals
  (raw and `\`-continued), and a line-based stripper treats their
  continuation lines as code — brace counts drift, test regions get
  truncated, and production occurrences are misattributed (session 4's
  crude tracker overcounted production at 126; the precise count was
  86). Also: string contents must be blanked while PRESERVING
  newlines, or line numbers shift after the first multi-line string.
  External test modules (`#[cfg(test)] mod x;` semicolon form) resolve
  to whole FILES (bench.rs, it_*.rs, mockserver.rs, testsupport.rs) —
  the file-level tag must come from the declaration site, not the file
  itself. The session-5 scanner lives at
  `/home/z/my-project/tool-results/r24_triage.py` (sandbox-local; not
  committed — zero-repo-pollution policy for one-off triage tooling).
  Session-6 refinement: the tracker models cfg(test) at MODULE level
  only — ARM-level attributes (`#[cfg(test)] "..." =>` inside a
  production match, execute.rs:468's `__test_panic` arm) escape it and
  get counted as production. One instance tree-wide: correct manually
  in the census notes; a scanner re-write is not warranted at this
  density. The panic-family census extension lives alongside at
  `r25_triage.py` (r24 kept intact as the unwrap/expect census).
- **G-09 (session 6, post-close incident).** CI's deny step checks the
  LIVE RUSTSEC advisory DB — a docs-only push can go RED between two
  runs minutes apart when a new advisory lands: run#327 green, run#328
  red ~20 min later on the SAME dependency tree (RUSTSEC-2026-0533,
  ammonia 4.2.4 mutation-XSS via MathML/SVG integration points). This
  is not a code regression — diagnose via the failing job's log before
  touching anything. Fix protocol (proven): minimal lock-only bump
  (`cargo update -p ammonia` → 4.2.5; manifest "4.2" already covered
  it), FULL battery re-run (the lock change re-links everything), PLUS
  a local `cargo deny check advisories bans licenses sources`
  (cargo-deny 0.20.2 installed ad hoc per §3) so the deny lane itself
  is verified before the push, not just after. Expect this class of
  break to recur on any long-lived branch — the advisory DB never
  freezes. Ammonia usage here: core/src/richtext.rs one Builder for
  sanitize_html (customer-visible rich text), so the advisory is
  genuinely reachable — patch, never ignore.

## Open questions / deferred owner decisions

- **Q-01.** Whether to do R5 (guardrails: gate port to xtask, unwrap lint,
  pedantic subset) — stage-gated, owner's call.
- **Q-02.** Whether the close-out (R6.2) should bump the version and cut a
  tagged release (v1.10.0) — out of scope unless the owner asks.
- **Q-03 (session 5, D-14; count updated at R2.4-b close).** R2.4 ends
  with 56 production expects kept on D-14 grounds (45 constant-input
  constructors, 8 guarded post-conditions with written class-C
  justifications, 3 bootstrap-equivalent infra sites) instead of the
  plan's letter ("eliminate all production ones"). If the owner wants
  the strict reading anyway, the work is: class A → Result propagation
  across ~45 call sites (do it AFTER R3 to avoid double churn), the 8
  class-C keeps → per-site control-flow restructures (see D-14 for why
  each was judged unnatural), class D → startup error handling in app
  bootstrap. Default (standing unless overridden): D-14's keeps.
- **Q-04 (session 7, D-15).** R2.5 keeps 3 exhaustiveness-forced
  `unreachable!` arms (app/mod.rs:891, oauth_state.rs:95,
  execute.rs:1833 — each with a written local proof in D-15) instead of
  the plan's letter (a "30-instance" family, of which 27 were
  test-region overcounts; production held only these 4 minus the 1
  cfg(test) test arm). If the owner wants the strict reading anyway,
  the work is: app/mod.rs UI-dispatch restructure (D-05 pixel-drift
  risk — must ride a golden-verified batch), oauth_state range-newtype
  (R3-territory), execute.rs outer-arm re-nesting. Default (standing
  unless overridden): D-15's keeps.
