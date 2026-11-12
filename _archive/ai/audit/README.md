# ai/audit/ — AI audit-program artifacts

Artifacts produced by the AI audit sessions and uploaded to this folder on
the owner's directive. They are point-in-time evidence records, kept outside
the `plan.md → progress.md → completed.md` work-item workflow (`ai/rule.md`).
Nothing here queues remediation — per Rule 8, fixes start only on an owner
directive.

## Contents

| File | What it is |
|---|---|
| `e2e-live-audit-harness.rs` | Self-contained E2E live-test harness from the v1.9.7 audit session. Drives the real production stack over real TCP — the loopback OAuth server, HMAC-signed webhook ingest, the Docs API (collections / articles), and the sync engine (full initial sync) — against a self-contained mock HelpScout endpoint set, using only the public API of `spp_core`. Product code untouched. 11 tests: T1 OAuth golden path · T2 token-endpoint rejection (invalid credentials) · T3 401 → refresh → retry rotation · T4 signed webhooks · T5 missing webhook secret → hard reject · T6 beacon-chat webhook ingest · T7 Docs API sync · T8 Docs without key → idle + warn · T9 unreachable network → friendly error · T10 full initial sync (all mirrored resources) · T11 Docs publish write path. |
| `repo-audit-v1.9.7.md` | The v1.9.7 follow-up audit report (2026-11-11): verifies the 44 v1.9.6 remediations on the released tree (tag `v1.9.7`, target `3f5d7ee`), maps the product delta, and records the full evidence — fresh test battery (1,051 local tests green across lanes), the E2E live-run matrix (11/11), cloud CI runs, the HelpScout live-network limitation, and the 2026-11-11 runner apt-DNS incident diagnosis. |

## Related artifacts, at their canonical locations

* v1.9.6 full product audit report (44 findings, remediated 44/44 in batches
  AUD-P0 / AUD-P2): `docs/audit/repo-audit-v1.9.6.md`
* User manual, re-stamped at release v1.9.7: `USER_MANUAL.md` (repo root)
* Release v1.9.7: tag `v1.9.7`, all four assets published and checksummed

## Running the harness

The harness is the durable record of the live-test method. It depends only on
`spp_core`'s public API and a hand-rolled TCP mock server (no network access
beyond loopback required). To execute it, drop it in as an integration test
of the `core` crate — e.g. copy to `crates/core/tests/e2e-live-audit.rs` —
then run:

```sh
cargo test --locked -p supportos-plus-core --test e2e-live-audit -- --nocapture
```

Execution results are recorded in `repo-audit-v1.9.7.md`, not in this file.
