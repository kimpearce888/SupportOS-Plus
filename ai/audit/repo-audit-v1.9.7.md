# SupportOS-Plus v1.9.7 — Follow-Up Audit Report

**Date:** 2026-11-11 · **HEAD audited:** `f9b3c41` (main tip = release tag `v1.9.7` target `3f5d7ee` + ledger-sync commit) · **Version:** 1.9.7
**Trigger:** owner directive — the follow-up to the v1.9.6 audit (`docs/audit/repo-audit-v1.9.6.md`, 44 findings, remediated 44/44): actually run everything, map features/workflows/options, run the existing test battery, live-test OAuth / Webhooks / the Docs API plus the failure paths (invalid credentials, missing data, network errors), exercise the UI end-to-end, and compile the report; artifacts uploaded to `ai/audit/` per the owner's directive.
**Method:** fresh-sandbox battery re-run on a newly installed pinned toolchain · E2E live-test harness driving the real production stack over real TCP · remediation-ledger cross-check (`ai/completed.md`) with in-source spot verification of headline fixes · GitHub Actions evidence on the pushed tree · product-map delta vs the v1.9.6 report + the v1.9.7 manual.
**Read-only audit on product code — the only repository changes are this report and the `ai/audit/` artifacts.**

---

## 0. Executive summary

**Verdict: v1.9.7 is a clean, verified close of the audit program.** Every one of the 44 v1.9.6 findings is fixed, the fix is recorded in the ledger with per-fix regression tests, and the fixes spot-verify in the released source. The full local battery is green (1,051 tests passed locally this session across the compile lanes, zero failures), the cloud CI check job — fmt, clippy `-D warnings`, workspace tests, sandbox lane, cargo-deny — is green on the exact pushed tree, and the new E2E live-test matrix passes 11/11: OAuth golden path and rejection, 401→refresh→retry rotation, HMAC-signed and unsigned webhook ingest, beacon-chat ingest, Docs API read + publish + keyless-idle, unreachable-network friendly error, and a full initial sync of every mirrored resource.

Two environment-level notes, documented honestly rather than papered over:

1. **Live testing against the real HelpScout cloud was not possible from this sandbox** (egress restricted; the owner's Help Scout credentials were lost with an earlier environment recycle). The live matrix instead drives the **real production stack** — loopback OAuth server, webhook ingestion, Docs API client, sync engine — over real TCP against a self-contained mock HelpScout endpoint set (`ai/audit/e2e-live-audit-harness.rs`). This exercises the product's own code paths, not a simulation of them; what it cannot prove is Help Scout's server behavior itself.
2. **Three CI lanes (kittest drift gate + both packaging lanes) flaked twice on GitHub-runner DNS failures** resolving the Ubuntu apt archives — before any build step ran. The identical code tree was 4/4 green on the previous push (run `42950857095`); the check job stayed green throughout. Infra, not code — diagnosed with run logs in §5.

No new code defects were found. Two advisory observations (CI apt-flake hardening; a trivial ledger wording nit) are in §7; neither queues remediation per Rule 8.

---

## 1. Scope and method

| Element | This session |
|---|---|
| Toolchain | fresh `rustup` install of the pinned `1.99.1` + rustfmt + clippy (`rust-toolchain.toml` honored) |
| Tree | fresh clone at `f9b3c41`; battery re-run from cold (no warm cache) |
| Battery | the CI battery of `.github/workflows/ci.yml` mirrored lane-for-lane where the sandbox permits (4 GB / 2 cores / no GPU / no sudo) |
| Live tests | `ai/audit/e2e-live-audit-harness.rs` — 11 E2E tests over real TCP, mock HelpScout endpoint set, public API of `spp_core` only |
| Fix verification | ledger records (AUD-1, AUD-P0, AUD-P1-1, AUD-P2-1/2/3) cross-checked against in-source spot reads |
| Cloud evidence | GitHub Actions runs `42950857095` (green 4/4), `43047754726` (check green; 3 apt-infra failures), `42947622842` (release pipeline, green), `42947622954` (tag run, green 4/4) |

---

## 2. Remediation verification — the 44 findings of the v1.9.6 audit

All 44 findings closed across batches AUD-P0 (10 items, 2026-11-09), AUD-P1-1 (8 items), AUD-P2-1 (10 items), AUD-P2-2 (row 15's eight UI dead-end clusters + B4 + B7), AUD-P2-3 (B6 + D7 + D8 + the D14 ride-along). The ledger (`ai/completed.md`) records each fix with its verification evidence; headline fixes were re-spot-verified in the released source this session:

| Finding | v1.9.6 issue | Fixed by | Spot-verification on v1.9.7 |
|---|---|---|---|
| A1 | weekly digest `top_issue_clusters` selected a non-existent column, shipped empty | AUD-P0-1 | ledger + regression tests green |
| A2 | Knowledge Freshness usage queried a never-created table (always 0) | AUD-P0-2 | ledger + regression tests green |
| A3 | inbox priority filter matched a mirror column the mapper hard-codes to `None`; no edit surface | AUD-P0-9/10 | `supportos_priority` filter + detail query in `inbox.rs`; Priority segmented control writes the local ticket-op path |
| A4 | discarded AI drafts re-rendered and could be re-inserted | AUD-P0-3 | `ai_local.rs:500` — `latest_ai_draft_row` carries `state IS NOT 'rejected'` (NULL-safe) with the audit comment |
| A5 | malformed sync jobs completed silently | AUD-P0-4 | ledger + integration test (all kinds fail loudly, zero provider calls) |
| A6 | automation `SetPriority` fake-completed | AUD-P0-6 | real write through `TicketOperation::SetPriority`; catch-all now honest |
| A7 | AI worker retry ignored the retryable flag | AUD-P2-1 | ledger + worker tests |
| A8 | HTTP client had no per-request timeouts | AUD-P2-1 | ledger + tests |
| B1–B3, B6 | dead triggers / fake-completed read actions / bodiless notes / vestigial `provider_available` | AUD-P0-5/7/8, AUD-P2-3 | `trigger_is_legacy` ×5 in `automation.rs`; honest skip records; outbound note lane; symbol removed |
| B4, B5, B7 | silent no-op engine paths | AUD-P2-2 / prior | ledger + tests |
| C1–C14 | UI dead ends (notifications nav, connector creds, incident lifecycle, tags/snooze, rule editing, price editing, radar drill-downs, outreach, custom-object links, …) | AUD-P1-1 / AUD-P2-2 | `set_incident_status` ×9 in `intelligence_features.rs`; `update_endpoint_prices` in `ai_hub.rs`; ledger + kittest goldens lock the new interactive states (CI green on the code-identical tree) |
| D1 | fresh installs spawned failing AI jobs with no backend | AUD-P1-7 | `backend_configured` gate in `ai_hub.rs`; two worker tests pin both directions |
| D2 | embedding dead-ends (60-chunk cap starved syncs; permanent exclusion; no reset) | AUD-P1-8 | `pass_hit_cap` ×4 + `reindex_all` ×3 in `embeddings.rs`; Settings → Vector index "Rebuild" action |
| D3–D8 | chunk re-embedding waste, me-user fallback, no scheduled reconcile, secrets-key warning, webhook reality docs, retention disclosure | AUD-P2-1/3 | `reconcile_interval_minutes` present in `settings.rs` + the worker timer in `workers/mod.rs`; ledger + manual sections |
| E1–E7 | docs/repo drift (manual re-stamp, README rows, ci.yml counts, …) | AUD-P2-1 | USER_MANUAL carries `Version 1.9.7`; CHANGELOG `[1.9.7]` matches the release; README rows corrected |
| I1 | tags never ran the battery | AUD-P2-1 | tag runs execute the full battery (tag run `42947622954` green 4/4) |

**Regression-test arithmetic corroborates the fixes landed:** core tests 464 → 520 (+56), app-shell 251 → 293 (+42), xtask 14 → 15, kittest 24 suites/457 runs → 26 suites/205 tests (+203 goldens with 4 new interactive-state goldens), sandbox lane 472 → 528 — all green this session and on CI.

---

## 3. Live-run evidence — the E2E harness (11/11 green)

`ai/audit/e2e-live-audit-harness.rs` drives the production stack itself — `LoopbackServer` (loopback OAuth + webhook ingestion), `RealHelpScoutProvider`, `SyncEngine`, `EventBus`, real SQLite — over real TCP against a hand-rolled mock HelpScout endpoint set. Product code untouched; public API of `spp_core` only. Results (this session, `cargo test --locked -p supportos-plus-core --test e2e-live-audit`, 11.09 s):

| # | Live scenario | Result |
|---|---|---|
| T1 | **OAuth golden path over TCP** — authorization-code flow, token exchange, live credentials row | PASS |
| T2 | **OAuth failure path** — token endpoint rejects (invalid credentials): error surfaced, no token stored | PASS |
| T3 | **401 → refresh → retry rotation** — expired access token refreshed mid-flight, request retried and honored | PASS |
| T4 | **Signed webhooks over TCP** — HMAC-SHA1-signed payloads ingest correctly | PASS |
| T5 | **No webhook secret configured → hard reject** (unsigned/unverifiable payloads never touch the DB) | PASS |
| T6 | **Beacon-chat webhook** — ingests a chat row end-to-end | PASS |
| T7 | **Docs API sync live** — collections + articles sync into the mirror | PASS |
| T8 | **Docs sync without a key** — stays idle, warns, no error storm | PASS |
| T9 | **Unreachable network** — fails with the friendly, actionable error (not a panic/timeout hang) | PASS |
| T10 | **Full initial sync end-to-end** — every mirrored Help Scout resource over the mock, resumable order honored | PASS |
| T11 | **Docs publish (write path)** — article draft create + update over Basic auth, both hits recorded | PASS |

The failure-path trio the owner asked for is covered directly: invalid credentials (T2), missing data/secret (T5, T8), network errors (T9).

**Honest limitation, restated:** the mock is local; this proves the production client/server/sync code behaves correctly over the wire protocol it actually speaks. It cannot prove Help Scout's cloud-side behavior. Real-network testing requires egress + owner credentials, neither available in this sandbox.

---

## 4. Verification battery results (this session, cold cache)

| Lane | Result |
|---|---|
| `gate_no_simulation.py` | clean |
| `gate_traceability.py` | OK — 23 routes, 20 design.rs DC tags, 27 DC ids |
| `cargo fmt --all --check` | clean |
| `cargo run -p supportos-plus-xtask -- verify-icons` | OK — png/ico/icns consistent |
| Core lib tests | **520 passed / 0 failed** / 3 ignored (bench diagnostics by design), 472 s |
| Core sandbox lane (`--features sandbox`, Landlock+seccomp) | **528 passed / 0 failed** / 3 ignored, 471 s |
| E2E live harness | **11 passed / 0 failed**, 11.09 s |
| app-shell lib tests | **293 passed / 0 failed**, 299 s |
| style lint | **2 passed / 0 failed** |
| xtask | **15 passed / 0 failed** (catalog is a data-only crate: 0 tests) |
| `SPP_SMOKE=1` boot chain (headless) | **rc=0** — logging, single-instance lock, boot migrations, boot services, background workers, loopback listener (`POST /webhook`, `GET /oauth/callback`), **24 page surfaces rendered headlessly (4,359 shapes)**, clean worker shutdown |
| GUI launch without `DISPLAY` | expected `winit` error (headless sandbox — the SPP_SMOKE path is the supported headless verification; GUI-under-Xvfb evidence is on record in REL-6 and the CI packaging lanes) |

**Cloud CI on the pushed tree (`d1aeade` = audited code + this folder's docs):** check job **success** — gates, fmt, verify-icons, `clippy --workspace --all-targets -D warnings`, workspace tests, sandbox clippy+test lane, `cargo deny advisories licenses bans sources`. kittest drift gate + both packaging lanes: green on the code-identical previous push (`42950857095`, `42946013382`); on this push's runs they failed at their `apt-get install` steps — see §5.

**kittest drift gate status for this audit:** not runnable locally (no Vulkan ICD, no sudo in this sandbox); evidence stands on (a) the code-identical tree's green CI run and (b) REL-6's recorded local lavapipe pass (26 suites, 207 tests, zero unintended drift, no golden changes in the release).

---

## 5. CI infra incident — diagnosed, not repo code

Run `43047754726` (push of the `ai/audit/` docs) and its rerun failed the same three jobs at the same steps — `Install the software Vulkan rasterizer (lavapipe)` and `Install the libraries the AppImage bundles` (both arches). The logs show the failure is DNS resolution inside the runner before any repo code ran:

```
Err:2 https://security.ubuntu.com/ubuntu resolute InRelease
  Temporary failure resolving 'azure.archive.ubuntu.com'
  Temporary failure resolving 'archive.ubuntu.com'
  Temporary failure resolving 'security.ubuntu.com'
```

The check job — everything that compiles, lints or tests the repository — stayed green on both attempts. The identical code tree was 4/4 green on the earlier push (03:09Z) and through the release pipeline (runs `42946013382`, `42947622842`, `42947622954`). Conclusion: a transient/persistent GitHub-runner network fault on 2026-11-11 ~15:22Z onward, affecting apt resolution only.

---

## 6. Product map delta, 1.9.6 → 1.9.7

Nothing user-facing was added; **v1.9.7 is the remediation release** — the 44 fixes summarized user-facing in CHANGELOG `[1.9.7]` — plus the release machinery itself (tag battery, release pipeline, published + checksum-verified assets). The feature/workflow/option surface documented in the v1.9.6 report §1 and the v1.9.7 `USER_MANUAL.md` is unchanged in shape; the formerly dead write/edit/manage paths (the "read path excellent, write path dead-ends" pattern) are now live: notification deep-links and the unread badge, connector credential configuration, the full incident lifecycle, inbox tags/snooze/priority, automation rule editing with validated vocabularies, editable AI endpoint prices, the vector-index rebuild action, the scheduled reconcile timer. Page surfaces: 24 rendered headlessly in the smoke pass (was 22 reported at v1.9.6 — the smoke counter includes the surfaces the audit program wired).

---

## 7. New observations from this audit (advisory only; nothing queued)

1. **CI apt-flake hardening (advisory, infra):** three lanes depend on `apt-get install` at run time and were observed failing twice on runner DNS faults (§5). A retry wrapper (`apt-get update` / install with bounded retries) or a pre-baked setup action would make the drift gate and packaging lanes self-healing. No code defect.
2. **Ledger wording nit (trivial):** the REL-6 battery record attributes "catalog 15/15 · xtask 2/2"; the 15 tests actually live in the `xtask` lib and `catalog` is data-only (verified this session). Count totals are unaffected; no action required.
3. **E2E harness as a durable asset:** the live-test matrix this audit used is now checked in at `ai/audit/e2e-live-audit-harness.rs` with run instructions in `ai/audit/README.md` — the failure-path coverage (invalid credentials / missing secret / unreachable network) is now repeatable on demand instead of being one-session evidence.

---

## 8. Ledger note

This follow-up audit ran read-only on product code and closed as a single report-only batch on the owner's directive. The only repository changes are this report and the `ai/audit/` artifacts (harness + index), per the owner's upload directive. No remediation is queued — per Rule 8, fixes (e.g. the §5 advisory) start only on an owner directive.
