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

- **D-16 (session 8, R3.1).** Error-type end state — the tree's error
  architecture was ALREADY consolidated (census: thiserror root
  `core::Error` + `Other`-boxed domain subtypes + string bridges; ZERO
  duplicate error enums; catalog has no error types; xtask rides anyhow
  as dev machinery per the D-14 xtask precedent; ai_eval's private
  `CaseErr` (String, Option<u64>) alias is golden-harness data, not a
  domain error). The batch's real work was the plan's second prong: the
  three hand-rolled `Display`/`std::error::Error` impls became thiserror
  derives with byte-identical messages — `LmStudioError`
  (`#[error("{message}")]`, Clone/PartialEq kept; `new()`/`lm_error()`/
  Other-box flow unchanged), `HsApiError` (`#[error("{friendly}")]`; the
  hand-written `From<HsApiError> for Error` bridge stays — it is the
  domain bridge, not duplication), `AcquireError` (per-variant messages;
  `Io(#[source] std::io::Error)` reproduces the hand-rolled source()
  exactly; app-shell gained `thiserror.workspace = true` — lock diff one
  edge line, no version movement). `TranslateError` collapsed
  enum→newtype: the three same-shaped variants (Client/Service/BadGateway)
  carried a class NO consumer ever read — the single caller (execute.rs
  translate_composer) destructured all three identically to the message;
  under constraints 2/5 the class dimension was unread structure.
  `TranslateError` deliberately stays Display-less: no consumer displays
  it (the message is extracted directly), so a thiserror Display would
  mint unused API surface (§5.3 minimal-surface rule). Standing policy:
  a NEW error type must either derive thiserror or be Display-less BY
  DESIGN with the no-consumer-displays proof recorded here; hand-rolled
  Display/Error impls are construction debt (constraint 2).

- **D-17 (session 9, R3.2).** SQL plumbing end state — the lenient
  row-collect tail (prepare → query_map → `filter_map ok`-collect) is
  consolidated into two db.rs helpers and the duplicated tail is GONE
  tree-wide (80 sites: the census's 73 + 7 turbofish spellings the
  pattern missed). THE CONTRACT (invariant 3; must survive future
  edits): `crate::db::query_rows(conn, sql, params, map)` = prepare `?`
  + query_map `?` + LENIENT row collect — prepare Err and query_map Err
  propagate as `crate::Error::Sqlite`; individual ROW mapper errors are
  SILENTLY SKIPPED. The lenient semantics predate the batch and are a
  load-bearing site contract, not an accident — do NOT "improve" the
  helper to strict collection; the 60 strict
  `collect::<rusqlite::Result<Vec<_>>>()` sites are a deliberate
  OPPOSITE semantic and stay as written (census keep).
  - **Fold rule (the batch's behavior-safety discipline):** a site
    collapses to ONE `query_rows` call only where its prepare-error and
    query-error fallbacks produce identical observable outcomes —
    `.unwrap_or_default()`/`.ok()`-chains, `?`-propagation,
    same-fallback matches, let-else whose else equals the tail's
    default, or an `is_empty` guard immediately after the block. Where
    the two error paths differ, the site keeps its prepare structure
    and converts only the query_map tail via `crate::db::rows_ok`
    (5 sites: embeddings ~613 vacuum abort-on-prepare, workers/execute
    537+569 early-return vs empty-continue, workload 495 team-skip
    `continue`, segment/mod 731 distinct prepare/query warn messages).
  - **Non-Vec collect targets** (HashSet/HashMap) re-add
    `.into_iter().collect()` after the fold (analytics bad_ids, outreach
    dnc, segment def_names) — same collection semantics, one re-step.
  - **Census correction (fifth consecutive):** the census pattern
    `…collect()` missed turbofish spellings — 7 same-class sites
    (segment/eval ×2, segment/mod ×2 incl. a HashMap target, ai_evidence
    ×1, copilot ×2) surfaced during execution and were converted under
    the same rules. Lesson: trailing-tail patterns must match `collect`
    WITH and WITHOUT generic arguments.
  - **ai_tools' local `collect_rows`** (Value-only, module-private) was
    a pre-existing one-off of the same shape — deleted; its 2 call sites
    ride `query_rows`. Standing rule: ONE lenient-collect definition
    tree-wide (db.rs); a new module-local one is construction debt.
  - **Keeps (census-recorded, no action):** strict collects (60), scalar
    `|r| r.get(0)` mappers (362), JSON column defaults (~15-20 across
    four semantic shapes), per-module named row mappers
    (digest_from_row et al.), incidental SQL literal repeats,
    `.optional()` (9).
  - Test-region sites converted too (8 census sites: quality ×4,
    knowledge_store ×1, automation ×2, it_helpscout ×1) — DRY is not a
    production-only policy; the automation/it_helpscout test helpers'
    paired `.expect("…")` messages merged into the prepare-side message
    (test-only observability, D-10 region).

- **D-18 (session 10, R3.3).** HTTP/serde plumbing end state — three
  helper definitions now own the previously-duplicated tails; a new
  module-local rewrite of the same shapes is construction debt
  (D-17's standing rule, extended to this domain).
  - **`ai_lm_studio::send_json<T>(req, op)`** (private): the
    OpenAI-compatible 4-stage tail (send → status+text → non-2xx
    300-char-snippet error → `from_str`), messages
    `"LM Studio {op} failed/read failed/request failed (HTTP
    {status})/parse failed"` with op ∈ {list_models, chat, embed} —
    byte-identical to the 4 folded sites. `OpenAiCompatibleClient::
    authorized(req)` owns the bearer-if-present attach. THE CONTRACT:
    text is read ALWAYS (status errors carry the body snippet) and
    parse is `text`+`from_str` — NOT `res.json()` (from_slice; the two
    differ observably on invalid UTF-8 — embeddings' kept site is the
    proof-by-contrast). Fold rule (D-17 discipline): embeddings'
    embed_texts KEEPS its own tail (send-error remedy hint, lazy text
    read, `.json()` parse, `unwrap_or_default` text).
  - **`helpscout_real::http::docs_request_json(req, method_label,
    path, unreachable_friendly, invalid_json_friendly)`** (private):
    the Docs-API read/write pair's shared skeleton; the two
    differing friendly strings are PARAMETERS (read-path vs
    write-path user messaging — the semantic distinction, made
    visible). token_request/request_inner keep their own tails
    (different error contracts; retry/ratelimit machinery).
  - **`json_fields::json_str(v, key) -> Option<String>` /
    `json_string(v, key) -> String`** (pub(crate), 9 LOC): the flat
    key-indexed extraction idioms. Standing fold rules recorded in
    the R3.3 verdict table: nested depth-2 paths stay direct
    (`v["a"]["b"]` — a two-arg call with `&v["a"]` is not clearer);
    literal-default sites stay direct (`.unwrap_or("lit")` carries
    per-site knowledge); the `.get(k).and_then(as_str)` family stays
    (borrowed &str, different type shape); `.as_i64()/.as_bool()` stay
    (canonical one-call serde_json API). Tree-wide standing rule:
    flat key-indexed owned-String extraction goes through these two
    fns.
  - Client-construction sites (6) are DELIBERATE keeps: each builder
    encodes a distinct policy (timeout+retry-tracing,
    redirect-none×2, default-headers bearer, plain,
    blocking+timeouts) — a shared constructor would parameterize
    policy = anti-DRY.
  - **Deferred census note:** `.chars().take(N).collect()` (~100
    sites, domain caps 40/240/300/4000/8000…) is a distinct
    truncation-idiom class, NOT response parsing — candidate for a
    future misc-DRY batch if the owner wants it; recorded here so it
    is not lost.

- **D-19 (session 11, R3.4).** Page-scaffold end state — the "Sync
  now" toolbar fragment is ONE definition
  (`sync_health::sync_now_button`, pub(crate)): the
  button_with_icon(Refresh, "Sync now", Outline) + the fixed hover
  text + the engine match + the three note strings ("syncing…",
  "a sync is already running", "re-read the local mirror (no
  engine)") live there; call sites pass `&mut sync_note` and act on
  the returned re-read flag with their page-specific `invalidate()`
  (the None-arm's caller-side part). Standing fold rules recorded in
  the R3.4 verdict table: the inbox live-label variant is a different
  policy (keep); ensure_loaded latch conditions are per-page
  staleness policy (keep — abstracting parameterizes policy,
  anti-DRY per D-17's discipline); RANGES is already single-source
  and its 2-site chip loop stays (borrow-conflicted extraction); a
  new page wiring the same toolbar MUST call the helper — a new
  inline copy is construction debt (D-17's standing rule, extended
  to page scaffolding). The None-arm statement-order flip
  (invalidate→note became note→invalidate via the flag return) is
  proven state-neutral (disjoint fields — no invalidate() impl
  touches sync_note) and pixel-neutral (no page-data reads between
  the two writes) — recorded so a future reader does not "restore"
  the order.

- **D-20 (session 12, R3.5).** PageKey-table end state — ZERO folds;
  the attribute layer was already single-sourced and stays that way:
  `label()` is the ONE str site (pages/mod.rs:349), `Icon::for_page`
  the ONE icon site (icons.rs:57), `apply_invalidations` the ONE
  field-dispatch site (app/mod.rs:190 — not table-convertible: arms
  target distinct struct fields through `&mut self`), render dispatch
  the ONE heterogeneous site (app/mod.rs:852). Four disjoint payload
  shapes → the ≥2-same-shape fold precondition never triggers.
  Durable sub-rulings: (a) page-title literals and nav labels are
  SEPARATE UI roles — Onboarding's "Welcome to SupportOS+" and
  Operations' "Operations Center" are the proven intentional
  divergences; a future "consolidate titles with label()" proposal
  must thread a PageKey through 20+ page ui() signatures for
  byte-identical output — rejected unless the owner asks (Q-03
  family). [Program-3 addendum, session 25 (R8.3): reviewed under
  the finish-everything directive's most-logical test and REJECTED
  terminally — threading PageKey through 20+ signatures for
  byte-identical output is indirection without abstraction (D-23);
  the proven intentional divergences stand; ruled in D-39.]
  (b) Adding a page: write the enum variant + label() arm +
  for_page arm + all() entry + NAV_GROUPS row + the two dispatch
  arms — the compiler FORCES all six (exhaustive matches + the
  `[Self; 23]` length type); this is the table's safety property,
  delivered by matches. (c) The drill-slug wire vocabulary
  (core/src/operations.rs `page_drill` ↔ app-shell
  pages/operations.rs `page_key_from_drill`) is the R3.6 seed —
  cross-crate pair, catalog-home question open there. (d) Scanner
  lesson: match-arm census regexes miss alias-bound arms
  (`use ... PageKey as K`) — every future census needs the manual
  alias sweep after the scanner pass.

- **D-21 (session 12, R3.6).** Cross-crate vocabulary end state — the
  plan's core↔catalog const duplication censused to ZERO (4 cross-crate
  value groups, all coincidental or xtask build machinery); the real
  debt was elsewhere and is now closed: (a) every inline
  `ALL.iter().find(|k| k.as_str() == x)` idiom that twinns an EXISTING
  catalog `parse` must call it instead — 8 sites folded (GraphNodeKind
  ×7 incl. validate_graph_node_kind's ok_or_else form and app-shell's
  kind_from_str 1-line alias; AiAttributeKey membership ×1); new
  twins are construction debt (D-17's rule, extended to vocabulary).
  (b) The drill-slug wire vocabulary lives in `catalog::routes`
  (ISSUES/AUTOMATION/DIAGNOSTICS/SYNC_HEALTH/OUTREACH — 5 consts);
  core's `page_drill` calls and app-shell's `page_key_from_drill` match
  arms bind them; &str consts ARE usable as match-arm patterns on this
  toolchain (probed before the edit) — that fact unlocks const-pattern
  folds generally. (c) Kept-vocabulary rulings: wire vs display roles
  (kind_label, tile_label vs as_str — the D-20 ruling extends to
  catalog-owned types); DB CHECK-constraint slugs stay as DDL text
  (drift fails closed+loud; format!-parameterizing DDL is anti-DRY);
  single-crate single-site parses do not earn catalog `parse` fns
  (ReportMetricKey/ReportDimensionKey/NotificationType — Q-03 family:
  the owner can order the parse API if wanted). (d) The parse-twin
  census idiom is `rg "as_str\(\) ==" across crates, then verify the
  scrutinee type is catalog-owned and catalog has the matching parse —
  cheaper and sharper than name-based const scanning.

- **D-22 (session 13, R4.1).** Naming end state — the §5.3 rules
  ("say what it is; booleans read is_/has_; no non-domain-standard
  abbreviations") are now enforced as WRITTEN POLICY, not blanket
  renames: the seventh consecutive census correction (341 bool decls,
  118 bool-returning fns, 478 short-id kinds censused; context-read
  every weak candidate) found the tree overwhelmingly honest. Standing
  rules for future code:
  - **A boolean name must READ as a complete predicate at its use
    site** — is_/has_ is ONE way, not the only way: state
    adjectives/participles (enabled, configured, loaded, escalated),
    predicate nouns with verb force (exists), filter imperatives
    (include_*, show_*), and domain flags (force, on, read) all
    qualify; nouns-as-bools (conversation, assistant, drill) and
    incomplete adverbs (already, known) do NOT — fix by completing the
    phrase (conversation_exists, from_assistant, drillable).
  - **Wire/persisted names (serde fields, DB columns, JSON keys,
    settings keys) are vocabulary, not style** — invariant 5 outranks
    §5.3; renaming them needs a migration note, and the default is a
    verdict-row keep (ai_involvement, stream, SpanStyle flags, the
    *_enabled settings family all keep).
  - **Action fns returning success-bools** (upsert, ensure_collection,
    verify_backup et al.) honestly name the ACTION; the bool return
    contract is a type-level concern — do not rename the fn to
    did_*; if the return contract ever needs tightening that is an
    API restructure, not a naming fix.
  - **Abbreviations are domain-relative:** measured against THIS
    tree's established vocabulary (conn/db/stmt/conv/org/tx/rt/sig/
    cfg/ctx/msg/req/res/idx/cc/bcc/href...), all keeps; a NEW
    abbreviation that collides with another word's meaning (temp =
    temperature vs temporary) or contradicts the established term for
    the same concept (ins vs the 169-site stmt) is debt.
  - **A module-local wrapper that only renames an existing std/API
    fn** (aa_get → Map::get) is construction debt under D-18's
    standing rule — delete it, call the API directly.
  - Deferred census note (not acted, R4.1 scope): dt_cell exists as
    3 byte-similar private copies (customers/incidents/organizations)
    — cross-page fold candidate awaiting its own batch if ordered.
  - Census tooling: r41_census.py (tool-results, not committed)
    reuses the G-08-fixed strip machinery; its lesson for future
    naming scans: enumerate then context-read, never trust a regex
    verdict — the FN_BOOL pattern needs per-site verification (the
    118 hits were 100% honest after reading, but only reading proved
    it), and the region tracker's test-flagging is directionally
    right only (assert!-bearing sites prove their own test regions).

- **D-26 (session 17, R4.4).** Magic-values end state — six folds
  landed (166 sites), and the STANDING RULES for judging future literal
  repetition (the tenth consecutive census correction: 7,823 prod
  numeric + 17,350 prod string occurrences held only six same-policy
  families):
  - **The fallback-vs-prose line for repeated UI strings:** strings
    reused as DATA (compared against, defaulted to, threaded as
    values — '(no subject)', '—') are POLICY and fold (catalog when
    cross-crate per R3.6 placement, design when shell-local per
    D-19); strings rendered ONCE as words (button labels 'Cancel'/
    'Edit'/'Delete') are PROSE and keep — the renderer is the single
    source, the word is the datum (Q-06 holds the owner option).
  - **Pixel dims fold only where the site's semantic ROLE matches
    the token's role:** a 28.0 that sizes a single-line control is
    CONTROL_H's policy; an AvatarSize::Md => 28.0 datum or a +28.0
    horizontal pad is NOT (D-17 discipline applied per-site, value
    equality is necessary but not sufficient).
  - **Repeated-const duplication is foldable even when the value is
    already named:** three identical page-local `ROW_HEIGHT = 32.0`
    decls were ONE policy with three definitions — folded to
    design::scale::ROW_H (same crate → design home).
  - **Cap values with same-value coincidences across module trees**
    (240: coaching excerpts vs search snippets vs analysis excerpts)
    fold only the same-module family (module-local const; visible
    to child modules via their `use super::*`); the coincidences
    keep as literals with the ruling recorded.
  - **Keeps that will recur in any future literal scan:** civil-date
    algorithm constants (146_097/719_468/365/3600/86400 — algorithm
    data); HTTP status literals (wire protocol, self-documenting —
    R3.6's DDL ruling extended); SQL LIMIT caps (per-query policy +
    SQL text); rgb() components (palette token DATA); serde attribute
    strings ('Option::is_none'/'snake_case' — macro-machinery
    spelling); wire-vocab strings (invariant 5); domain-word
    comparisons (== "high" across six free-form wire fields —
    different dimensions sharing English words).
  - The census machinery (r44_census.py) adds to r43's walk: string
    VALUE collection, sanitized-text numeric scan, `.take(N)` and
    SQL-`LIMIT N` overlays, and a const-initializer inventory (to
    distinguish already-named values from raw call-site literals).
- **D-27 (session 18, R4.5-a).** File-split end state, two-tier ruling:
  the "~2,000-line file" policy is remediated in tiers with different
  risk profiles. **Tier 1 = cfg(test) mod extraction** (inline `mod X
  {}` → `#[cfg(test)] mod X;` + sibling file): pure file-layout move —
  module tree byte-identical (paths, visibility, `use super::*`
  resolution all unchanged); this is the right tool when the overage
  is test mass (workers/mod was 2,807 total but 1,373 prod — the
  split took it under threshold with zero code motion). **Tier 2 =
  prod-region splits** along R4.2 method clusters with the G-15
  visibility ladder for moved items. Threshold measures the file as
  the editor sees it (total lines), but the RECIPE is chosen by the
  prod/test split. At-threshold ruling: ai_center 2,003 post-batch =
  keep (the "~" tolerance + R4.2-b already decomposed its fns).
  Sub-2K files with big test mods (custom_objects 1961, sync_engine
  1811) stay unsplit — no trigger, no churn (D-23 discipline).
- **D-28 (session 19, R4.5-b).** The impl-member move recipe (the
  Tier-2 companion to D-27): methods moved into a child module's
  `impl <Type> {}` block need ONLY a visibility change — method-call
  syntax is path-free (`self.list_ui(...)` resolves from any impl
  block in the crate at the method's visibility), so the entire
  caller surface survives untouched and the G-15 ladder reduces to
  one question: who calls it? parent/sibling/test-mod callers →
  `pub(super)`; callers that moved together → private stays. No
  re-export is ever needed for impl members. Top-level items are the
  opposite: paths must be preserved via host `pub use` re-exports
  (zero caller churn) and sibling-used private helpers need
  `pub(super)` + a host `use` binding so descendant globs (`use
  super::*` in http.rs) keep resolving them unqualified. Both moves
  ride the R4.5-a-proven child-file header `use super::*;` (the
  descendant glob sees the host's private items AND its private
  use-bindings). End state of the ~2K policy: largest file = ai_center
  2003 (the D-27 keep); the >2K set is empty.
- **D-29 (session 20, R6.1).** Build-file comment taxonomy (the
  plan-R6.1 ruling set, mirrors D-13's Rust-comment policy for the
  non-Rust build machinery):
  - **Whole-line comments** (TOML `#`, YAML `#`, .gitignore `#`):
    strip — the direct analog of R1's Rust strip; the rationale
    lives in this ledger now.
  - **Trailing comments on `uses:` action pins** (`…@<sha> # v4`):
    strip the fragment, keep the pin — the pin hash is the
    functional datum, the version tag was documentation. ONLY
    these carried trailing comments tree-wide (census-proven).
  - **Shell comments inside YAML `run: |` block scalars**: strip —
    they are script no-ops; removal is functional zero-diff for
    the script. The stripper MUST track block-scalar regions
    (the header regex must allow the space in `run: |`) so
    run-block lines are exempt from the YAML trailing-comment
    scan — a future ` #`-bearing DATA line inside a script
    (release.yml's changelog awk already carries `^## ` data)
    would otherwise be corrupted.
  - **Python (gate script): `#` comments only.** The shebang is
    interpreter machinery — kept; docstrings are runtime
    objects (`__doc__` state) — KEPT under the batch's own
    "functional zero-diff" bar (removing one would be a
    functional diff, however small; AST equality is the
    verifier). If the owner ever wants gate docstrings gone
    too, that is a one-minute follow-up batch — recorded as the
    standing owner option (same shape as Q-06's prose).
    [Program-3 addendum, session 25 (R8.4): MOOTED — R7.5 deleted
    scripts/ and ported the gate into the xtask `gate` subcommand;
    the option's object no longer exists; this note stands as
    history only.]
  - **Out of scope, deliberately:** `_archive/` (the program's
    own memory — not build machinery) and Cargo.lock (generated,
    zero comments). `.gitattributes` was censused (zero
    comments) and left byte-identical.
  - **Proof discipline for zero-diff claims over non-Rust
    formats:** per-format semantic parser equality (tomllib /
    PyYAML with run-block normalization / ast.dump) PLUS a
    multiset line-conservation audit over git HEAD (every
    removed line a comment; every added line a rewrite
    counterpart). difflib opcode alignment is NOT usable for
    this audit when the tree contains duplicate lines (four
    identical action-pin lines deranged its alignment — the
    multiset form is alignment-free and duplicate-safe).
- **D-30 (session 20, R6.2 close-out).** The DoD line "0 production
  unwrap/expect/panic!" is CLOSED as-amended: the strict letter was
  superseded by the D-14/D-15 owner-defaults (56 justified expects,
  4 classified panic-family members) — Q-03/Q-04 carry the
  strict-reading work itemized for the owner, and the bootstrap
  exemption line is unused (main.rs carries zero unwrap/expect).
  R5 dropped unexercised (Q-01 addendum). The close-out's battery
  evidence follows the R3.5 docs-only precedent: a closing
  docs-only head rides the identical code tree whose battery + CI
  run already ran green. The program's authority chain at close:
  plan §6 baseline → ledger censuses (ten consecutive corrections
  of the plan letters, each with its verdict table) → CI runs
  #301–#358 — the CI numbers are the ones that count.


- **D-31 (session 21, R7.1).** Owner override received (see the
  Open-questions addendum); Program 2 opened. R7.1's fresh census
  EXTENDED Q-05's letter in both directions: (a) copilot the MODULE
  stays — its MAX_TOOL_ROUNDS/MAX_TOOL_CALLS/TOOL_RESULT_MAX_CHARS
  consts are live (core/src/copilot.rs consumes all three); only the
  CopilotTool enum + its two impls + the root re-export are dead;
  (b) the GraphHumanRelation cascade is deeper than the recorded
  question — support_graph::LinkInput (the enum's ONLY consumer) has
  zero tree-wide constructors (graph_edges has no production INSERT;
  the write path exists only in tests), so the dead set is LinkInput +
  GraphHumanRelation + impl + re-export. Read path is String-typed
  (SQL reads thread raw TEXT), so the deletion is behavior-neutral.
  Lesson for future Q-letters: an option scoped to a TYPE needs a
  per-item census of its MODULE before whole-file action (the enum was
  the letter; the module was not the dead unit).


- **D-32 (session 21, R7.3).** The three Q-04 restructure shapes,
  chosen for honesty + D-23's dispatch-data discipline:
  (a) app/mod.rs page dispatch — the honest 23-arm match table, each
  arm wrapping its own design host (host_inbox_page for Inbox,
  host_standard_page for the rest); per-arm bodies byte-identical to
  the old inner arms, so the pixel risk rides the CI kittest gate
  (the golden-verified-batch requirement of Q-04); exhaustiveness is
  compiler-checked with no lie arm; per-arm host wrapping IS dispatch
  data (D-23's "1-line delegations stay" rule), not DRY debt. The
  exhaustiveness ghost was structural: any re-match of a 23-variant
  PageKey under a 22-variant guarantee needs either the lie arm, a
  silent wildcard (D-15-rejected), or a 21-variant newtype (rejected
  as R3-overkill); only the full table is honest.
  (b) oauth_state.rs hex_digit — Nibble range-newtype: private
  struct Nibble(u8) with high(byte)/low(byte) constructors (the masks
  ARE the proof, constructive arithmetic: b>>4 and b&0x0f are
  provably 0..=15), hex_digit indexes a [char;16] const — in-bounds
  by constructor truth, unreachable! deleted.
  (c) workers/execute/ops.rs (the R4.5-b child of old execute.rs:1833)
  — outer-arm re-nesting: the inner Result<Option> match's three
  shapes re-nested as the three real outer arms (Ok(Some) verify
  path unchanged; Ok(None) and Err(e) each build their msg and share
  the extracted record_partial_publish tail fn) — no ghost arm, DRY
  tail, same set_article_draft_publish_state sequence per path.
  Post-batch census: prod_unreachable = 0 tree-wide; panic family =
  the single D-10 arm-level cfg(test) site.


- **D-33 (session 21, R7.4-a/2).** Security-cluster conversion shapes
  (the Q-03 strict letter applied to redaction machinery):
  (a) security.rs — one named helper `rx(pattern) -> Result<Regex>`
  (map_err into Error::Other(Box<new(e))) replaces the 8 duplicated
  "static regex" expect-messages (DRY + honest name);
  redaction_patterns -> Result<Vec<RedactionPattern>>;
  redact_text -> Result<(String, Vec<Value>)> with the enabled
  early-return as Ok.
  (b) Boundaries per consumer chain, chosen by the chain's own
  error surface: ai_pipeline chat boundary maps into
  LmStudioError::new(msg, false) (retryable=false — a compile bug
  never retries); redact_for_model -> std::result::Result<String,
  LmStudioError> (the crate Result alias shadows two-generic form —
  E0107; the file's own precedent is the explicit std::result::
  prefix); run_extraction -> Result<Option<ExtractionOutput>>
  (None = provider unavailable stays Option; Err = the NEW error
  channel for redaction-build failure — semantics kept distinct).
  (c) The tool layer's error surface is the dispatcher's Value
  channel: json!({"error": ...}) is the protocol's existing error
  rendering (invalid args / unknown tool already use it), so the 16
  Result-returning tool fns settle at the arm via one `settle`
  helper; redact/redact_chars -> Result<String>; rusqlite
  query_map closures (rusqlite::Result-typed, cannot `?` a core
  Error) go two-pass — raw rows collected first, redaction applied
  in a Result-collecting second pass; plain Value-closures convert
  to Result-closures + collect::<Result<Vec<_>>>()?; map_cluster_row
  split into ClusterRow struct + raw_cluster_row collector + render
  pass (same for ai_attributes' AttrRow — clippy type_complexity
  rejected the 6-tuple, the struct is the honest shape).
  (d) CENSUS CORRECTION (the Q-03-mode lesson again): the recorded
  "consumer bubble: ~10 redact sites" was head-truncated rg output —
  the real fanout is 23 redact/redact_chars sites across 16 tool
  fns (search x4 + get_conversation/context/customer_history/
  similar/ai_attributes/timeline/memory + custom_objects/
  connector_data/knowledge_gaps/graph_neighbors + issue_clusters
  via map_cluster_row). Full-site rg re-census is now mandatory
  before any consumer-fanout edit.
  Post-batch census: prod_expect 44 (56 - 4 sandbox @R7.4-a/1 -
  8 security @this batch); test-region +4 (batch29/attributes
  test expects — D-10).

- **D-34 (session 21, R7.4-a/3 + R7.4-a/4).** OnceLock-accessor
  and gate-consumer conversion shapes (the Q-03 strict letter
  applied to the coaching + interaction_engine clusters):
  (a) Accessor form: OnceLock<Result<T, regex::Error>> (the
  constructor's natural error — regex::Error is Clone; avoids
  the double String conversion of the recorded prep sketch)
  + accessor -> crate Result<&'static T>, with the error clone
  happening ONLY on the never-taken path (.as_ref() +
  map_err(|e| Error::Other(Box::new(e.clone())))).
  (b) API SEMANTICS GOTCHA (empirically E0308-verified):
  Result::as_deref does NOT unsize &Vec<T> -> &[T] — the impl
  is <T: Deref, E> Result<T, E> and T = &Vec (references Deref
  to their pointee Vec) yields &Vec, never [T]. The correct
  tail for &'static-slice accessors is
  .as_ref().map(|v| v.as_slice()).map_err(...). Relevant to
  every remaining OnceLock cluster (quality x13).
  (c) Fail-closed ruling for SANITIZER gates: when a bool-gate
  consumer reads a STRUCT FIELD (`.ok` on TextScan was a field,
  not Result::ok()), the honest conversion is
  matches!(fn(..), Ok(scan) if scan.ok) — on the never-taken Err
  the text is treated as not-safe and dropped (cannot-certify
  must not pass through an AI-bound-text gate). Distinct from
  (d):
  (d) Hoist-and-propagate for ANALYSIS regexes: a ctor/accessor
  consumed inside a non-Result closure (.then(||), .filter(|m|),
  .any(|n|)) cannot host '?' — hoist the construction to the
  Result-returning fn body (baseline.rs step_re hoisted out of
  the response_style .then closure; quality mod.rs:1200's
  direct_complaint_re filter-closure caller will need the same
  hoist in R7.4-a/5). Same restructure class as D-33(c)'s
  two-pass rusqlite closures.
  (e) Registry-preserving conversion: `?` is expression-
  position — vec![a, b, c?] is legal, so the coaching checks
  registry kept byte-identical order with per-arm '?'.
  Post-batch census: prod_expect 29 (44 - 10 coaching @a/3 -
  5 interaction_engine @a/4).
- **D-34 additions (session 22, R7.4-a/5 quality cluster).**
  (f) Option-channel fold: an Option-returning pub fn whose
  signature is FROZEN (get_qa) absorbs a callee's new Result via
  `.ok()?` — the Err lands in the existing None channel (the
  caller-visible semantics "no row available" already covers
  it). Likewise a unit-returning maintenance sweep folds via
  `.unwrap_or(0)` (the adjacent-idiom rule). General principle:
  propagate with '?' where the channel exists; fold ONLY where
  the consumer's channel is deliberately fixed.
  (g) `?` on an Option inside a fn converted to Result is a
  TYPE ERROR — restructure each Option-early-return to
  let-else `return Ok(None)` / let-else `return Ok(Vec::new())`
  (compute_qa_deterministic's conv; analyze_friction's
  let-else already had the right shape). Corollary: `expr?;`
  where expr: Result<Option<T>> drops the yielded Option as a
  statement — empirically NO must_use warning (rustc
  unused_must_use does not fire for discarded Option/Result
  statement results; verified with a minimal repro).
  (h) Aggregate-sampler skip: a Vec<i64>-returning metric
  sampler (eval_support_health) consumes a Result-returning
  per-item fn via `let Ok(out) = .. else { continue };` — the
  same shape as the adjacent missing-customer_id let-else;
  on the never-taken Err the item contributes no sample.
  Distinct from (c) fail-closed (gates DROP) and from (f)
  folds (channel absorbs): samplers SKIP.
  (i) The collect::<Result<Vec<_>>> fallacy: an infallible
  map (json! produces Value) cannot collect into Result —
  FromIterator for Result requires Result ITEMS. The honest
  tail is Ok(iter.map(..).collect()). Empirically E0277.
  (j) Closure-conversion for spawn_blocking: when the fn
  already maps JoinError through .map_err(..)? — annotate the
  closure `move || -> Result<T>` and the tail becomes
  `.map_err(..)??` (double '?': JoinHandle layer then the
  inner Result). Also the empirically-verified operator
  precedence: `!f()?` parses as `!(f()?)` — postfix `?` binds
  tighter than unary `!` (else-if chains stay one-line).
  Census tooling note: the r25 scanner did not survive
  sandbox resets; rebuilt as a truncation scanner (first
  top-level #[cfg(test)] — the at-end convention verified
  tree-wide). Series continuity: r25 counted 29 pre-batch;
  truncation counts 28 (one-off tooling delta); the SITE
  delta is exact under both (16 converted; truncation 28 ->
  12). Future sessions should record both numbers.

- **D-35 (session 24, R7.5).** The no-simulation gate's Rust
  port self-flags: the gate module carries the marker
  vocabulary ("simulated"/"fake"/"demo") as const string
  literals, and the gate scans raw lines INCLUDING string
  contents (by design — user-visible strings are the point).
  The Python predecessor was structurally outside the scanned
  set (a scripts/*.py file); the port reproduces that boundary
  EXPLICITLY via MACHINERY_EXEMPT (one file:
  crates/xtask/src/gate.rs). The alternative (self-allowlist
  entries) is brittle: rustfmt re-wraps the allowlist consts
  and the fragment lines shift. The unwrap-policy scanner
  needs NO self-exemption: its string-aware stripping blanks
  its own allowlist literals (string contents are invisible
  to the occurrence patterns). r25-vs-unwrap_policy census
  corrections (both verified on the live tree): r25 misses
  cfg(test) attrs on MULTI-LINE fn signatures (digest_row's
  two expects counted as prod) and attrs on MATCH ARMS (the
  batch-27 __test_panic panic! counted as prod); the policy
  scanner handles both (pending-attr consumes to the first
  `{` regardless of item shape). External cfg(test) mod files
  agree exactly: 45 = 45, empty diff both directions.

- **D-36 (session 24, R7.5).** clippy.toml CANNOT enable
  lints (it only carries lint configuration, e.g. msrv) — the
  plan's "pedantic subset via clippy.toml" is implemented as
  [workspace.lints.clippy] in the root Cargo.toml + `[lints]
  workspace = true` in each member crate; clippy.toml exists
  with `msrv = "1.99"` as the plan-letter's named file (also
  redundant with Cargo.toml rust-version — belt and
  suspenders). Selection method (the "tuned" part): full
  `-W clippy::pedantic` fire inventory on BOTH clippy
  surfaces — workspace --all-targets (74 lints fired, ~1800
  fires; top: redundant_closure_for_method_calls 361,
  too_many_lines 173, map_unwrap_or 144, cast_precision_loss
  135, cast_possible_wrap 132, cast_possible_truncation 129)
  and core sandbox (subset, no new kinds) — then the enabled
  set = 20 ZERO-FIRE high-value lints (pure guardrails, no
  code churn): borrow_as_ptr, cast_ptr_alignment,
  comparison_chain, expl_impl_clone_on_copy, filter_map_next,
  invalid_upcast_comparisons, large_futures,
  large_types_passed_by_value, linkedlist, maybe_infinite_iter,
  missing_fields_in_debug, mismatching_type_param_order,
  option_option, ptr_as_ptr, ptr_offset_by_literal,
  return_self_not_must_use, string_add_assign,
  transmute_ptr_to_ptr, unnecessary_box_returns,
  unsafe_derive_deserialize. The big-fire lints are recorded
  as deliberately NOT enabled: remediating them is a separate
  churning program (R8-material), not a guardrail. Zero-fire
  status re-verified with the [lints] live under `-D
  warnings` on both lanes. NOTE: [lints] clippy:: entries are
  silently inert under plain rustc (tool-lint RFC behavior) —
  cargo test/build lanes are unaffected. [Program-3
  addendum, session 25: the R8-material line is reviewed
  under the finish-everything directive and ruled NOT OWED —
  ~1800 pedantic fires across 139.9K LOC is churn without
  guardrail value; the 20-lint subset stands as the
  terminal state; see D-39.]

- **D-37 (session 25, R8.1).** The truncation-idiom fold (D-18's
  deferred note, resolved under the Program-3 directive). Census
  required TWO passes — line-level (r25 region machinery) AND
  text-level (multiline chains are invisible to a line scanner:
  23 sites): true total 125 raw `.chars().take(N).collect()`
  occurrences = 121 production + 2 test-region (D-10 keeps) +
  2 xtask machinery (the byte-identical `preview()` twins, D-14
  precedent; xtask's core dep is not a reason to churn machinery).
  Terminal raw census: 4 (the definition itself + those keeps).
  The census also found the LOCAL-HELPER family (8 fns), which is
  the fold's real yield:
  (a) `notifications::truncate_chars` ≡ `notification_sweep::trim`
  (trim + count-check + saturating_sub(1) + '…') — folded to
  `textutil::truncate_chars` (pub(crate)); the fold also fixes
  sweep's misleading bare `trim` name (R4.1's bare-predicate
  vocabulary family).
  (b) `interaction_signals::excerpt` ≡
  `ai_attributes::extraction::excerpt` (whitespace-normalize +
  take(max_len − 3) + '…'; only the local binding name and the
  '\u{2026}' escape form differed — same character) — folded to
  `textutil::excerpt` (pub(crate)).
  (c) `analytics::truncate_chars` was a hard-cap one-liner (the
  cap_chars SEMANTICS under a truncate_chars NAME — a vocabulary
  collision with (a) resolved by the fold): deleted; its caller
  calls cap_chars.
  (d) KEPT domain variants (D-17 — different policies, not
  twins): `quality::excerpt_of` (ASCII "..." + take(max), no
  budget math), app-shell `inbox::truncate_chars` (no trim,
  take(max)), each now dogfooding cap_chars internally.
  Home: `textutil` — the tree's existing text-mechanism module
  (html_to_text's home; NOT a new module), made `pub` as the
  carrier for app-shell's sites; `#[must_use]` per the module's
  html_to_text precedent; no new tests (json_fields precedent —
  mechanism one-liners ride the suites; baselines stay EXACT).
  Mechanism-vs-policy split: cap_chars folds the char-safe
  TRUNCATION TECHNIQUE; every cap VALUE (literal/const/computed)
  stays at its call site (D-17 satisfied by construction). The
  pixel safety argument: mechanism-identical conversions render
  byte-identical strings; the CI kittest drift gate verifies.
  Conversion tooling note: the mechanical `&recv` prefix is
  classified by clippy (`needless_borrow` under -D warnings is
  the honest borrow-stripper); the import sorted-insert placed
  two imports inside multi-line braced use statements (caught by
  cargo fmt's parse pass — fmt doubles as the syntax checker;
  moved after the complete statements).

- **D-38 (session 25, R8.2).** The dt_cell fold (R4.1's deferred
  note, resolved under the Program-3 directive). The three private
  copies (customers.rs / incidents.rs / organizations.rs) were
  byte-identical (R4.1 had already renamed align_right across all
  three — the fold precondition was prepared by that batch). Home:
  `design` — beside `data_table` (the cell renderer IS the data
  table's companion mechanism; the pages' convention of qualified
  `design::` calls carries the fold: 11 call sites prefixed, no
  import churn). pub(crate) visibility (all three consumers are
  app-shell pages). Body identical modulo the self-module scale
  reference (`design::scale::ROW_H` -> `scale::ROW_H`). Pixel
  neutrality: the widget-call sequence is byte-identical by
  construction; the CI kittest drift gate verified (run#386-class
  evidence on the R8.2 head).

- **D-39 (session 25, R8.3/R8.4 + the R8-material ruling).** The
  residue rulings under the directive's most-logical test:
  (a) page-title label() consolidation REJECTED terminally (the
  D-19(a) annotation): churn without abstraction.
  (b) the gate-docstring option MOOTED by R7.5 (the R8.4
  annotation): the option's object no longer exists.
  (c) D-36's R8-material big-fire pedantic remediation ruled NOT
  OWED: ~1800 fires across 139.9K LOC is churn without guardrail
  value; the 20-lint zero-fire subset stands as the terminal
  state; a future owner can still order it, but nothing in the
  finish-everything directive demands it (the residue audit's
  question was ignored/skipped WORK, and this was an explicit
  keep-with-reason).
  The lesson generalizes: "complete everything" resolves NOTES
  (census findings awaiting a decision), not KEEPS (decisions
  already made and recorded).

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
  install too. RESOLVED (session 24, R7.5): the gate is now the xtask
  `gate` subcommand; the python3 package was dropped from the release
  container's apt-get install list and `scripts/gate_no_simulation.py`
  was deleted (single-language build achieved).
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
  Session-10 refinement (r33_triage.py): `out.append('""')` appends a
  2-char string as ONE list element, so `len(out)` (used for
  line_starts) undercounts by one char per string — the joined-text
  positions desync (in r25's original the same latent bug existed via
  `out.append("'c'")` for char literals; r25's line numbers were
  never cross-checked against the file, only its counts were trusted,
  and the counts were unaffected). Fix: every multi-char placeholder
  goes through `out.extend(...)`. If a future scanner reuses
  strip_file, assert `stripped[ls[k]:ls[k]+len(line)] == line` for all
  k before trusting any line number.
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
- **G-10 (session 8; refined same session).** Background-process survival
  depends on the LAUNCH FORM. PROVEN WORKING: the subshell-detached form
  `(cmd > log 2>&1 &)` — a Next.js dev server launched this way survived
  across many subsequent tool calls (verified repeatedly, incl. through
  an agent-browser session). PROVEN DYING at tool-call end: `nohup cmd &`
  and `setsid nohup cmd &` (the session-6 rustup install; session-8's
  first clippy attempt and first dev-server attempt — both died with
  zero errors in their logs). For long cargo lanes the foreground-window
  pattern (repeated 10-min windows; artifacts persist between killed
  runs) remains the fallback when a lane outlives one window. Do not
  generalize from the nohup deaths to "all backgrounding is impossible" —
  use the subshell form and verify survival on the first poll.

- **G-11 (session 10).** The bash tool's DISPLAY pipeline eats the
  literal 3-char sequence `[m` (a bare ANSI-reset fragment) from any
  text it shows — commands and files are intact (proven: `printf '%s'
  'X[mY' | wc -c` = 4), but every `#[must_use]` rendered as
  `#ust_use]` in output, and any display containing `[m` is
  unreliable. Consequences: NEVER verify file content by eyeballing
  bash output when the text could contain `[m` (or any other escape
  fragment); use computed counts, `wc -c`, substring tests in python
  that print booleans/numbers, or the Read tool. This misled the
  session-10 census debugging for several steps (a phantom
  "corrupted file" chase) before the printf probe exposed it.

- **G-12 (session 11).** The file-edit tool can only write paths under
  /home/z — clone SupportOS-Plus at /home/z/SupportOS-Plus (handover
  §2's canonical location), NOT under /tmp, or every edit routes
  through python surgery instead. Related reset-recovery trap:
  github-reconnect.sh's exists-branch only re-sets the remote URL —
  after a reset that leaves an EMPTY dir remnant, remove it first or
  the clone silently never happens (bit session 11's pre-flight).
- **G-16 (session 18).** Naive brace counting is unusable for span
  location: reports.rs's two cfg(test) mods were INVISIBLE to the
  raw-count scanner because their bodies carry string literals whose
  braces don't balance (format templates etc.) — depth drifts inside
  the region and never returns to 0 at the true close (the r42 census
  hit the same class). Any span-finding machinery needs the full
  G-08/G-14 string-aware walk (strings, raw strings, char literals
  incl. `'\u{...}'` + `'\''` forms, and line continuations that must
  NOT swallow the newline bookkeeping — the trap recurred in the
  r45a lexer itself and was only caught by the reports.rs dry-run
  diff). Sister lesson: a conservation assert must compare the SAME
  artifact on both sides — r45a's first design compared
  region-with-braces to decl-with-semicolon and failed at mod #1
  (compare the BODY pre/post dedent, whitespace-insensitively, plus
  an exact dedent-equality pass); the failure cost one clean
  `git reset --hard` + rerun, no damage escaped the assert.
- **G-17 (session 19).** Audit CURRENT declaration levels before
  writing a visibility-rewrite table: four names drafted for the
  pub(super) rewrite were already wider (delete_saved_report
  pub(crate); record_response/record_error_429 pub(crate);
  run_attachment_downloads pub(super)) — the rewrite regex would not
  have matched two of them (0 hits = script assert) but the
  pre-existing-marker assert tripped first and the audit explained
  WHY. A pre-existing wider level is the correct level for the move
  (it already covers every current caller); never downgrade in a
  file-split batch — visibility levels are R4.3's jurisdiction.
- **G-18 (session 19).** G-15's sibling corollary, field edition:
  moving a private struct whose FIELDS are read from a sibling module
  (TokenRow ← http.rs) requires `pub(super)` on BOTH the struct and
  the fields — E0616 on each field, then "type TokenRow is private"
  on the pub(super) fn returning it (a function interface may not
  expose a type narrower than itself). One compile round fixed all 13
  errors; the class is now fully enumerable for future splits.
- **G-19 (session 19).** `git mv x.rs x/mod.rs` FAILS unless the
  destination directory already exists ("renaming ... failed: No such
  file or directory") — os.makedirs(dirname) before the mv. The
  r45b apply died mid-run AFTER two hosts were written; recovery =
  `git reset --hard` + `git clean -fd crates/` + fix + rerun (safe
  because every host's asserts re-derive from the reset tree — the
  same idempotency G-16's reset relied on).
- **G-20 (session 20).** Local cargo-deny for the G-09 protocol:
  install the PREBUILT release binary, never `cargo install` (the
  compile costs ~8 min on this sandbox). Asset naming trap: the
  release ships `x86_64-unknown-linux-MUSL` only (no `-gnu` asset) —
  the gnu-suffixed URL 404s. Working recipe:
  `curl -sL -o cdy.tar.gz https://github.com/EmbarkStudios/cargo-deny/releases/download/0.20.2/cargo-deny-0.20.2-x86_64-unknown-linux-musl.tar.gz`
  then `tar xzf` and run it straight from the extracted dir (no
  install step; the advisory DB fetches live at check time — network
  required).
- **G-21 (session 22).** Battery ordering: the fmt lane must run
  AFTER the last text transform of the batch (a sed/clippy-remedy
  alias change shortens signatures and rustfmt wants them
  re-wrapped — run#370 failed on exactly this: fmt ran before the
  type-alias sed, CI's --check caught five signatures that now fit
  one line). Rule: any post-battery edit (alias/sed/manual) re-runs
  `cargo fmt --all` + `-- --check` before the commit. Same class as
  the R7.2 line-length fallout rule but generalized to edits that
  CHANGE SIGNATURE WIDTH, not just add lines.


## Open questions / deferred owner decisions

- **OWNER DIRECTIVE (session 25).** "Finish everything, take the
  most logical decision to complete everything" — the residue
  audit's findings become Program 3 (plan §12): D-18's deferred
  truncation-idiom note (R8.1), R4.1's deferred dt_cell fold
  (R8.2), D-19(a)'s title-consolidation proposal (R8.3 —
  terminally ruled under the most-logical test), the mooted
  gate-docstring option (R8.4), and D-36's R8-material pedantic
  line (ruled not-owed, see D-39). The six Q-letters stay closed;
  no new Q-letters are opened — the residue was never
  option-gated, it was census-noted. **PROGRAM-3 CLOSE-OUT
  (session 25): all five R-letters executed and closed — R8.1
  (475e055, run#386), R8.2 (d3cdb36, run#387), R8.3/R8.4 (the
  D-39 rulings), R8.5 (d4a19f5, run#388/#389 + Release run#39;
  v1.10.1 published and live-verified). Rule 8 STOP; the residue
  ledger is fully resolved.**

- **OWNER OVERRIDE (session 21).** "Complete anything ignored and
  skipped" — every Q-01..Q-06 default is overridden; execution is
  Program 2 under plan §11. The option letters below remain the work
  specs; their "Default (standing unless overridden)" lines are dead.
  PROGRAM-2 CLOSE-OUT (session 24): ALL SIX Q-letters executed and
  closed — Q-01 via R7.5 (84cc569, run#380), Q-02 via R7.6 (2f8a192,
  run#382/#383 + Release run#38, v1.10.0 published), Q-03 via R7.4
  a+b+c (runs #362–#378), Q-04 via R7.3 (10c3cde), Q-05 via R7.1
  (f9c568c), Q-06 via R7.2 (e62bf08). Terminal records in
  `complete.md`'s Program 2 closing report; Rule 8 STOP.

- **Q-01.** Whether to do R5 (guardrails: gate port to xtask, unwrap lint,
  pedantic subset) — stage-gated, owner's call. CLOSE-OUT ADDENDUM
  (session 20): never exercised across the program; R5 recorded
  DROPPED-with-reason per the DoD. The option stands for the owner:
  R5.1 (gate port → drop python3 from the release container; G-03
  notes the install to remove), R5.2 (unwrap-policy lint subcommand
  enforcing D-14 in CI), R5.3 (clippy pedantic subset in
  clippy.toml).
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

- **D-23 (session 14, R4.2-a).** Decomposition end state — the census
  (r42_census.py: 3,277 production fns) found the tree's oversized fns
  in five classes, each with its own verdict shape (the eighth
  consecutive plan-letter correction — "top-20 to the 40-line soft cap"
  is not one operation):
  - **Match dispatches** (`execute` 26 tool arms; `derived_edges_touching`
    11 Kind arms) → extract per-arm fns with exactly the params the arm
    uses; the dispatcher becomes a thin table. Arms that are already
    1-line delegations ARE dispatch data — they stay (execute_job's ~37
    post-split lines are 40 arms of data, not debt).
  - **Guard-scaffold repeats** (apply_m046's 10× `if !fk_declared {
    rebuild_table(&Rebuild{…})?; }`) → ONE helper (`ensure_fk`) + the
    specs become `Rebuild<'static>` data consts + a manifest fn.
  - **Sequential accumulators** (health metrics; friction findings) →
    per-domain fns RETURNING tuples / Vecs (never `&mut` out-parameters
    — §5.3); closures deleted, call sites call the module fn with conn.
  - **Flat manifests** (db_breadth's add()/create tables; sync_schema's
    guarded migration sequence; icons.rs `draw`'s 34-arm table) → KEEP:
    each line is one declarative datum; per-datum sub-fns are artificial
    seams (indirection without abstraction).
  - **State machines / conversation loops** (analyze_interaction
    two-stage pipeline; copilot chat round loop) → KEEP: extraction
    means a context struct = re-architecture, not decomposition.
  - **The execute_job early-return rule (binding for R4.2-b):** a match
    arm containing `return Ok(())` after `jobs::fail` CANNOT be
    extracted naively — the return skips job completion (complete/
    complete_with_result) and the "Job completed" trace; in a method it
    would fall through to completion. Only arms without early returns
    extract; the fail-path arms stay inline in the dispatcher.
  - **UI T-composition twins** (the 11 queued fns) are R4.2-b: extract
    section renderers along existing seams; pixel-neutral by
    construction (identical widget-call sequence); the CI kittest
    drift gate is the verifier (F-06); delete a D-13
    too_many_lines allow only when its fn drops under the threshold.

- **D-24 (session 15, R4.2-b).** Pixel-neutrality verification recipe for
  mechanical UI refactors (standing alongside D-05/D-06): after every
  app-shell extraction batch, compare the per-file MULTISET of widget
  calls (`design::*` functions + `ui.*` methods) between `git show
  HEAD:<file>` and the working tree — byte-identical multisets are a
  necessary condition for pixel-neutrality and the check is cheap. It
  caught the one bug class the compile loop cannot see: a transform
  that wraps a moved card call in ANOTHER `card_titled_wide` shell
  (double-carded UI — compiles, renders differently). Known
  exceptions that legitimately shift counts and must be verified
  against source instead: `design::tokens` re-threading (static
  getter, no pixels) and statement-level closures whose opener/closer
  stay in the dispatcher. The CI kittest drift gate remains the final
  verifier (F-06); the multiset check is the local pre-gate. R4.2-b
  end state: all 11 queued fns split, all four D-13 too_many_lines
  sites' allows deleted (clippy -D warnings green = under the
  100-line threshold), every post-split fn private (fn, not pub).
- **G-12 (session 15).** Transform scripts handed across sessions are
  DRAFTS, never verified code. The context-exhausted session that
  queued R4.2-b pre-wrote nine r42b_split_*.py scripts and died before
  running any of them; boundary asserts did NOT catch four of the
  seven real defects (two of them — the double-card wrappers — were
  DESIGN errors invisible to any line-based assert). Standing rule:
  read every handed-over transform against source before running it,
  then compile, then run the D-24 multiset check, then battery. A
  script's asserts prove the INPUT matched expectations, not that the
  OUTPUT is sound.
- **G-13 (session 15).** `crates/core/tests/` does not exist in the
  repo (the audit-era e2e harness files were sandbox-side copies that
  restore.sh re-injected into old clones; a fresh clone has no
  integration binaries). The core test lane IS `--lib` (520 / 528
  sandbox); a separate `--tests` run is a no-op that re-runs the lib
  lane and costs another ~8 min. Don't run it twice.

- **D-25 (session 16, R4.3).** Visibility end state + the standing
  recipe. Census: the tree was already 2/3 visibility-honest
  (587 pub(crate) fn + 48 pub(super) fn pre-existed); the plan's
  "minimize pub" letter resolved to 82 honest downgrades + ~140
  compile-proven keeps + 9 dead deletions (pub surface 1,113→1,023).
  The recipe for ANY future visibility work (also the retroactive
  explanation of this batch's shape):
  - **Token censuses see NAMED consumers only.** rustc's
    effective-reachability sees what tokens cannot: types flowing
    through return-value inference (`use super::*` chains, module
    aliases like `use crate::interaction_engine as engine`, values
    crossing crate roots unnamed), pub TRAIT method signatures, pub
    STRUCT-field types, and pub-field→type chains. Therefore: apply
    the census, then let `cargo check` drive restores — the loop is
    monotone (restores only widen) and converges in ≤3 rounds per
    crate; parse the `reachable at visibility` text from
    private_interfaces warnings, it IS the required visibility.
  - **Module caps are the cheapest lever:** `pub mod X` →
    `pub(crate) mod X` at the crate root caps a whole subtree in one
    line while inner items keep their textual `pub` (no churn); nine
    landed (core maintenance/migrations/mirror/response_state_sql/
    sandbox/webhook, migrations::intelligence, app-shell app,
    design::scale::space).
  - **A `pub` item's signature types must be at least as visible as
    the item** — the compiler enforces this under -D warnings; never
    fight it, restore the type.
  - **Duplicate-name census artifacts:** same-named twins (two fns
    `search_knowledge`, two structs `ThreadEntry`) share one token
    stream — a twin's callers credit both defs; dead_code after a
    downgrade is the truth serum (knowledge_store::search_knowledge
    was a phantom-consumer dead fn all along — deleted).
  - **pub-field/struct-field census decision:** fields ride their
    type's visibility (1,861 pub fields; a field-level pass = churn
    for zero reach change). Ruled: keep.
- **G-14 (session 16).** The G-08 sanitize machinery has a THIRD
  length-preservation trap besides multi-line strings and char
  literals: **string line-continuations** (`\` followed by newline
  inside a normal string literal) — an escape branch that blanks 2
  chars unconditionally EATS the newline and desyncs every later line
  number (r43's PageKey was off by 9 from 9 continuations). Fix:
  blank the backslash, leave the newline; assert
  `[i for i,c in san if c=='\n'] == [i for i,c in text if c=='\n']`
  per file before trusting ANY line number. The apply script's
  per-line content assertions are the second net (this is what
  caught the bug before any wrong edit landed).
- **G-15 (session 16).** Rust visibility ladder direction, the
  R4.3 trap: **private covers the defining module and its
  DESCENDANTS only — never its ancestors or siblings.** An item in a
  nested module (design::scale::space) used from an ANCESTOR module
  (design) needs `pub(super)` or wider; `pub(super)` covers exactly
  ONE level up (the parent tree), so grandparent users (SP4 used
  from design two levels above space) need `pub(crate)`. Same-file
  is NOT same-module: inbox_geo's consts sit beside their consumers
  in inbox.rs but in a CHILD module — private broke them.
- **G-22 (session 24, R7.5).** Rust string/char strippers: the
  char-literal escape consumer must scan for the closing quote
  starting AT the escape opener (k = tick+1) and skip escape PAIRS
  (`\X`, k += 2 at a backslash) — starting at tick+2 ("after the
  opener") makes `'\\'` land ON the second backslash, skip 2 past
  the closing quote, and swallow code (with newlines collapsed,
  shifting EVERY later line number) until the next stray tick.
  Symptoms in unwrap_policy's first run: phantom violations at
  shifted line numbers + a "stale allowlist entry" for a site that
  exists (strip.rs's allowlisted expect). Same family as G-14: any
  sanitizer that can eat a newline desyncs line numbers; the
  line-count invariant (analysis lines == raw lines per file) is
  the assertion to trust.
- **Q-05 (session 16, R4.3).** catalog::CopilotTool is a dead
  vocabulary type: the 22-variant copilot tool enum (as_str/description
  impls, ~120 lines) has ZERO tree-wide consumers beyond its own root
  re-export (core's copilot dispatch uses its own wire structs and
  string literals instead). Kept `pub` this batch per the R3.6
  GraphHumanRelation::parse precedent (catalog = deliberate
  closed-vocabulary surface; a visibility pass deletes nothing
  vocabulary-shaped). If the owner wants it gone, the deletion is:
  the enum + impl + `pub use copilot::CopilotTool;` root re-export;
  the same question applies to GraphHumanRelation::parse (R3.6 note).
- **Q-06 (session 17, R4.4).** UI action-label prose (`"Cancel"` ×17,
  `"Edit"` ×13, `"Delete"` ×12 — 42 sites, all at design::button/ui.button
  call sites) was ruled a KEEP under D-26's fallback-vs-prose line. If the
  owner wants the standard action labels single-sourced (e.g. design
  consts CANCEL/EDIT/DELETE_LABEL), the fold is mechanical and
  pixel-neutral; recorded here as the standing owner option.

