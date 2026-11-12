# SupportOS-Plus Refactor — Master Plan

Owner: kimpearce888 · Repo: https://github.com/kimpearce888/SupportOS-Plus
Plan created: session 1 (2026-11-12) · Execution model: as many sessions as required
Folder: `_archive/refactor/` — the operational home of the whole refactor.
Read order for every new session: `memory.md` → `handover.md` → `progress.md` → this file.

---

## 1. Mission

Refactor the SupportOS-Plus codebase so that the program's own source — and
nothing else — is the single source of truth; the code is DRY, comment-free,
clean by construction, and carries no unnecessary line. The build must be
green at every pushed commit, and the program's behavior must stay identical
(except where a task explicitly improves error paths, and that change is
covered by the existing test suites).

## 2. Non-negotiable constraints (owner directives, session 1)

1. **Only the code/files that make the program are the source of truth.**
   Consult nothing else. No docs, no archived reports, no CHANGELOG prose,
   no old audit registers — they are not inputs to any decision. (This rule
   is why R0 exists: the traceability gate validated code against docs, so
   the doc side of the build had to go.)
2. **DRY — do not repeat yourself.** No duplicated logic, constants, type
   scaffolding, or boilerplate anywhere it can be consolidated.
3. **Remove all comments, keep nothing at all.** Every `//`, `///`, `//!`,
   `/* */` in the program sources goes. New code written during the refactor
   is also comment-free.
4. **Clean code method.** Small functions, honest names, single level of
   abstraction per function, minimal public surfaces, explicit error
   propagation.
5. **No unnecessary code.** Every line is checked and verified. Each module
   gets a line-by-line review verdict (keep / merge / delete / rename)
   before it is edited; verdicts are recorded in `progress.md`.
6. **Best-approach research is baked in** (section 5): syntax-aware tooling
   over regex, xtask as the tooling home, gate-aware refactoring, CI as the
   final arbiter.
7. **Multi-session execution framework** — this folder. `plan.md` (this
   file) is the task list; `progress.md` is live state; `complete.md` is the
   done ledger; `handover.md` is the operations manual; `memory.md` is the
   durable decision/gotcha log.

## 3. Source-of-truth definition

**Consulted (the entire universe of inputs):**

- `crates/**.rs` — program source and tests (204 files, 141,107 LOC)
- `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml`
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`
- `scripts/gate_no_simulation.py` (code-only build gate)
- `deny.toml`, `.gitignore`, `.gitattributes`

**Explicitly NOT consulted (moved out of the build in R0 or earlier):**
everything under `_archive/` except this folder — README, USER_MANUAL,
CHANGELOG, LICENSE, ai/, ux-audit/, manual/, mockups/, non-gate docs/, the
traceability gate and its two doc inputs.

## 4. Global invariants (hold at EVERY commit)

1. **Battery green locally before commit** (commands in `handover.md` §4):
   no-simulation gate, `cargo fmt --all --check`, clippy workspace lane,
   clippy sandbox lane, tests workspace lane, tests sandbox lane,
   `verify-icons`, and (from R1.0 on) `strip-comments --check`.
2. **CI green on the pushed head** before the next session starts work
   (`handover.md` §5 gives the monitor command). All four lanes: check,
   packages x86_64, packages arm64, kittest snapshot locks.
3. **Semantics preserved** unless a task says otherwise; the only sanctioned
   behavior changes are error-path improvements in R2.4/R2.5, each covered
   by tests.
4. **Kittest goldens (203 PNG baselines) are build artifacts, not docs.**
   Pure mechanical refactors must be pixel-neutral. Deliberate rebaseline
   (only if a task requires visual churn) follows the double-run discipline:
   `UPDATE_SNAPSHOTS=1` run, then a byte-identical re-run, then CI confirms.
5. **Kittest suite file names and test function names stay stable** through
   R1–R4 (they key the goldens); renames, if ever wanted, are a deliberate
   rebaseline decision.
6. **One batch = one commit.** Message protocol: `refactor(R<id>): <what>`.
   Commits accumulate locally during a session; one push per session; the
   pushed head must be green.
7. **The no-simulation gate allowlist** (`scripts/gate_no_simulation.py`)
   keys on exact path `crates/core/src/audit.rs` and two exact stripped
   line contents. Any batch touching that file must keep those lines intact
   or update the allowlist in the same commit.

## 5. Research: approaches adopted (and why)

### 5.1 Comment removal is done by a lexer, not regex

Regex stripping is unsafe in Rust: string literals contain `//` (URLs like
`https://…` are everywhere in `helpscout_real`), char literals (`'/'`),
raw strings (`r#"…//…"#`), byte strings, and nested block comments
(`/* /* */ */` is legal). A wrong strip changes program semantics silently.

**Adopted:** an `xtask strip-comments` subcommand (task R1.0) — a
hand-written Rust lexer (no new dependencies; `syn` does not even surface
comments) that tracks: normal code, line comments, nested block comments,
doc comments (`///`, `//!`, `/** */` → all deleted), string/byte-string
literals with escapes, char literals vs lifetimes (lookahead), raw strings
with arbitrary `#` counts. Modes: `--write` and `--check` (exit 1 if any
comment remains — becomes a permanent battery item). Idempotence is proven
by construction and a unit-test suite covering every edge case above.
The tool itself is written comment-free (constraint 3 applies to it too).

Doc-comment removal is safe for the build: `catalog` and `core` carry
`#![warn(missing_docs)]` immediately overridden by `#![allow(missing_docs)]`
(verified session 1), and clippy's `-D warnings` therefore stays green. The
redundant warn/allow pair is deleted in R1.10. Doc-tests vanish with the
docs; the workspace test count drops accordingly — expected, not a failure.

### 5.2 DRY ladder (Rust-specific, applied in R3)

1. Same-file duplication → extract a private function.
2. Cross-module duplication → move to a shared module or trait.
3. Type-level repetition → generics or a shared enum.
4. `macro_rules!` only for ≥3 stable instantiations that stay readable
   (the codebase has just 3 macros today — do not add more casually).
5. Error types: consolidate per-error-domain `thiserror` enums.
6. SQL: consolidate rusqlite row-mapping and statement boilerplate.
7. UI: page-scaffold duplication through the shared host compositor —
   pixel-neutral by construction, goldens confirm.

Detection is per-module line-by-line review (constraint 5) plus pattern
greps; every consolidation candidate is listed in `progress.md` with its
verdict before the edit lands.

### 5.3 Clean code rules (R4)

- Names: say what it is; booleans read `is_/has_`; no abbreviations that
  are not domain-standard.
- Functions: soft cap 40 lines; one level of abstraction; early returns;
  no out-parameters; RAII for cleanup.
- Visibility: `pub(crate)`/`pub(super)` wherever possible; the public API
  is a deliberate surface, not an accident.
- Errors: `Result` propagation with `?`; no `unwrap`/`expect`/`panic!` in
  production paths. Policy: allowed only in `main.rs` bootstrap and inside
  `#[cfg(test)]`. (Current state: 121 `unwrap()` in src, 2,300 `expect(`
  in src — the bulk inside inline `#[cfg(test)]` regions; R2.4 separates
  them and eliminates the production remainder.)
- Magic values become named constants, ideally in `catalog` (the crate
  whose stated job is the closed vocabulary).
- Files over ~2,000 lines get split along module seams (mechanical move,
  no logic change): `inbox.rs` 4,715 / `settings.rs` 3,316 / `workers/mod.rs`
  2,826 / `ai_pipeline/mod.rs` 2,780 / `design/mod.rs` 2,698 /
  `helpscout_real/mod.rs` 2,675 are the known candidates.

### 5.4 Unnecessary-code detection (R2)

- Compiler-driven: dead-code warnings, unused imports, unreachable arms
  (clippy `-D warnings` both lanes already enforce zero tolerance).
- `#[allow]` audit: 31 instances; each is either deleted (code fixed) or
  the allowance is removed and the code deleted.
- Dependency hygiene: `cargo machete` for unused deps (lockfile has ~500
  crates; every declared dependency must have a consumer).
- Feature audit: `sandbox`, `kittest-ui` features stay minimal and used.
- Manual triage of the 121 `unwrap()` / production-subset `expect()` /
  30 `panic!|todo!|unimplemented!` occurrences.

### 5.5 Process safety

- Small batches, one commit each → git bisect stays usable.
- Never push red; local battery before every commit, CI before next session.
- Every refactor batch's `git diff` must show only the intended class of
  change (comment deletions, moves, renames) — reviewed before commit.
- Sandbox resets are expected (session 1 lost the clone AND the toolchain):
  `handover.md` §2 has the full re-provisioning procedure.

## 6. Codebase metrics — session-1 baseline

| Crate | Files | LOC | Role (from the code itself) |
|---|---|---|---|
| core | 122 | 87,713 | business logic, storage, sync, workers |
| app-shell | 70 | 50,651 | egui UI, pages, design system, kittest suites |
| catalog | 8 | 1,024 | closed vocabulary: DB, validation, UI constants |
| xtask | 4 | 1,719 | build tooling (package, deb, icons, gates-to-be) |
| **total** | **204** | **141,107** | |

Comments (full-line counts; trailing inline comments add to this — exact
totals come from the stripper): 3,174 `//` + 1,005 `///` + 7 `//!` + 1 `/*`
≈ 4,187 lines.

Constructs: `unwrap()` 121 (src) / 2 (tests dir); `expect(` 2,300 (src,
bulk in inline test modules); `panic!|todo!|unimplemented!` 30; `#[allow]`
31; `macro_rules!` 3; `unsafe` 7 (all in `app-shell/src/single_instance.rs`,
which carries `#![allow(unsafe_code)]`; core and catalog `forbid` unsafe).

23 `PageKey` routes; 26 kittest suites + `style_lint.rs` + `snapshots/`.

Largest files: inbox.rs 4715 · settings.rs 3316 · workers/mod.rs 2826 ·
ai_pipeline/mod.rs 2780 · design/mod.rs 2698 · helpscout_real/mod.rs 2675 ·
knowledge.rs 2647 · workers/execute.rs 2537 · reports.rs 2374 ·
ai_center.rs 2299.

Target end-state metrics: 0 comments (verified by `strip-comments --check`),
0 `#[allow]` without an active justification entry in `memory.md`, 0
production `unwrap`, LOC delta reported in `complete.md`.

## 7. Task list

Every task: scope → steps → verification → done-criteria. Sub-tasks are
the batch unit (one commit each). Statuses live in `progress.md`.

---

### R0 — Pipeline alignment: docs leave the build
**Status: executed in session 1 (see `complete.md`).**

The traceability gate validated code against two docs; constraint 1 forbids
consulting docs, so the gate and its inputs leave the build:

- Remove the "Traceability gate" step from `ci.yml` and `release.yml`.
- `git mv scripts/gate_traceability.py _archive/scripts/`.
- `git mv` the two doc inputs back into `_archive/docs/audit/…`; root
  `docs/` disappears entirely.
- Keep `scripts/gate_no_simulation.py` (code-only; aligned with constraint 1).
- Verify: no-sim gate clean, `git grep` shows no remaining build references
  to `gate_traceability|docs/audit`, CI all-green on the pushed head.

---

### R1 — Comment removal (constraint 3), 10 batches

Order: tool first, then smallest crate as pilot, then core, then app-shell.

- **R1.0 — build `xtask strip-comments`.** Lexer per §5.1; `--write` /
  `--check` modes; unit tests for every lexical edge case; writes a report
  (files touched, comment lines removed, and FAIL if any non-comment line
  changed). Done when: tool is comment-free, tested, and `--check` is green
  on untouched tree (i.e., reports the existing ~4.2K+ comments as pending).
- **R1.1 — pilot:** `catalog/**` + `xtask/**` (12 files, ~2.7K LOC) +
  their tests. Gate allowlist untouched. Full battery.
- **R1.2 — core/ai_***: `ai_attributes/ ai_pipeline/ ai_tools/` +
  `ai_center.rs ai_eval.rs ai_evidence.rs ai_hub.rs ai_lm_studio.rs
  ai_local.rs ai_memory_guard.rs ai_prompts.rs ai_provider.rs` (~13 files).
- **R1.3 — core/sync+workers:** `sync_engine/ workers/ connectors.rs
  loopback.rs it_loopback.rs it_sync.rs api_queue.rs jobs.rs event_bus.rs
  events.rs provider_handle.rs`.
- **R1.4 — core/helpscout:** `helpscout_real/ helpscout/ it_helpscout.rs
  it_lm_studio.rs oauth_state.rs mirror.rs mockserver.rs`.
- **R1.5 — core/storage+config:** `db.rs migrations.rs migrations/
  people_store/ saved_views/ config.rs crypto.rs audit.rs logging.rs
  bootstrap.rs backup_service.rs data_reset.rs maintenance.rs error.rs`.
- **R1.6 — core/domain:** everything else in `core/src` (inbox, chats,
  customers, knowledge, reports/, quality/, outreach/, operations,
  automation, incidents, custom_objects, search/, segment/,
  support_graph/, coaching/, interaction_engine/, analytics, activity,
  digest, mentions, notifications, …) + `core/tests/`.
- **R1.7 — app-shell/infra:** `app/ design/ palette.rs chart.rs icons.rs
  overlays.rs mention_popup.rs theme.rs toasts.rs task_bridge.rs
  workers_host.rs fatal.rs single_instance.rs smoke.rs testsupport.rs
  main.rs lib.rs`.
- **R1.8 — app-shell/pages A** (the seven 2K+ LOC pages): inbox, settings,
  knowledge, reports, ai_center, custom_objects, outreach.
- **R1.9 — app-shell/pages B** (remaining 16 pages + `mod.rs queries.rs
  json_fields.rs`) + `app-shell/tests/**` (26 kittest suites, style_lint).
- **R1.10 — closeout:** delete the redundant `warn(missing_docs)` +
  `allow(missing_docs)` pairs in catalog/core; `strip-comments --check`
  green over all of `crates/`; full battery + CI; record exact removed-
  comment count in `progress.md`.

Per-batch verification: stripper report shows only comment-line deletions →
`cargo fmt` → battery (§4) → commit → next. Goldens cannot drift (comments
are not codegen inputs); CI kittest lane confirms.

---

### R2 — Unnecessary code removal (constraints 1, 5)

- **R2.1 dead-code sweep:** fix every dead-code/unused warning by deleting
  the code (preferred) or making it used; never by adding `#[allow]`.
- **R2.2 `#[allow]` audit:** all 31 instances; each is resolved by fixing
  the code (allow deleted) or deleting the code. Survivors (if any) are
  listed in `memory.md` with the active justification — no comment can
  carry it (constraint 3).
- **R2.3 dependency hygiene:** `cargo machete` (install ad hoc); remove
  every unused declared dependency; `cargo deny check bans` stays green;
  `Cargo.lock` diff reviewed.
- **R2.4 `unwrap`/`expect` policy pass:** separate production from
  `#[cfg(test)]` occurrences; eliminate all production ones (bootstrap in
  `main.rs` exempt). Behavior change is error-path-only and test-covered.
- **R2.5 `panic!|todo!|unimplemented!` (30):** same policy; unreachable
  arms deleted.

---

### R3 — DRY consolidation (constraint 2)

- **R3.1 error types:** per-domain `thiserror` consolidation; no duplicate
  error enums or hand-rolled Display impls.
- **R3.2 SQL plumbing:** rusqlite row-mapping/statement boilerplate →
  shared helpers in the storage modules.
- **R3.3 HTTP/serde plumbing:** reqwest response parsing + JSON mapping
  duplication → shared client-side helpers.
- **R3.4 page scaffolding:** repeated page composition through the shared
  host compositor; pixel-neutral, goldens confirm.
- **R3.5 `PageKey` tables:** match-arm duplication (label/route/etc.)
  → table-driven single source (names stay stable, invariant 5).
- **R3.6 cross-crate vocabulary:** constants duplicated between core and
  catalog move to catalog (its stated role).

---

### R4 — Clean code pass (constraint 4)

- **R4.1 naming:** modules, types, functions, variables per §5.3.
- **R4.2 decomposition:** top-20 largest functions split to the soft cap.
- **R4.3 visibility:** minimize `pub` to `pub(crate)`/`pub(super)`.
- **R4.4 magic values → catalog constants.**
- **R4.5 file splits** for the >2K-line files listed in §5.3 (mechanical).

---

### R5 — Guardrails (optional, stage-gated; only if owner wants)

- **R5.1 port the no-simulation gate into `xtask`** (Rust subcommand),
  dropping `python3` from the release container and making the build
  single-language.
- **R5.2 unwrap-policy lint subcommand** enforcing §5.3's policy in CI.
- **R5.3 clippy pedantic subset** via `clippy.toml` with a tuned,
  high-value lint set.

### R6 — Polish and close-out

- **R6.1 build-file comment strip** (Cargo.toml, workflows, .gitignore,
  deny.toml, .cargo, rust-toolchain.toml, gate script): functional
  zero-diff only; battery + CI after.
- **R6.2 final audit:** full battery, CI green, final metrics vs §6
  baseline, every-line verification statement per module cluster, closing
  report in `complete.md`. Optional owner decision afterward: version bump
  and a tagged release (out of scope of this plan).

## 8. Session protocol

1. Read `memory.md` → `handover.md` → `progress.md`.
2. Re-provision sandbox if reset (`handover.md` §2).
3. Confirm last pushed head is CI-green (`handover.md` §5).
4. Execute the batch at `progress.md`'s CURRENT POINTER. Line-by-line
   discipline: record verdicts (keep/merge/delete/rename) in `progress.md`
   before editing when the batch is a review pass (R2–R4).
5. Local battery → commit `refactor(R<id>): …`.
6. Update `progress.md` (status + session log); `complete.md` when a task
   goes terminal; `memory.md` for any new decision/gotcha.
7. Push once; monitor CI if time remains — otherwise the next session
   opens with step 3.

## 9. Risk register

| Risk | Mitigation |
|---|---|
| Comment strip corrupts a string literal | lexer + edge-case unit tests; stripper fails on any non-comment diff; battery; CI |
| Gate allowlist desync when `audit.rs` changes | invariant 7; same-commit allowlist update |
| Golden drift on UI refactors | R1 is pixel-neutral by construction; R3.4/R4 route through the shared host; rebaseline only deliberately (invariant 4) |
| Behavior change smuggled in | diff-class review before every commit; only R2.4/R2.5 sanctioned |
| Sandbox resets mid-plan | handover §2 re-provisioning; everything durable lives in git |
| Session context loss | this folder is the memory; every session updates it before ending |

## 10. File map of this folder

- `plan.md` — this document: constraints, research, tasks. Change only via
  explicit re-planning recorded in `memory.md`.
- `progress.md` — live task states, CURRENT POINTER, session log, review
  verdict tables.
- `complete.md` — definition of done + terminal task ledger (commit + CI
  evidence per entry).
- `handover.md` — operations manual: environment, commands, battery, CI
  monitor, gotchas.
- `memory.md` — durable decisions, facts, gotchas, open questions.
