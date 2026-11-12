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

## Open questions / deferred owner decisions

- **Q-01.** Whether to do R5 (guardrails: gate port to xtask, unwrap lint,
  pedantic subset) — stage-gated, owner's call.
- **Q-02.** Whether the close-out (R6.2) should bump the version and cut a
  tagged release (v1.10.0) — out of scope unless the owner asks.
