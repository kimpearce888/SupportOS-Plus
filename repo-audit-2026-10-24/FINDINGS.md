# FINDINGS.md — SupportOS-Plus audit (incremental; statuses per 04-FINDINGS §1)

Status: IN PROGRESS — findings added as evidence supports them. Confidence labels: Confirmed at runtime / Confirmed by implementation analysis / Suspected / Blocked.

---

## F-001 — Default webhook/OAuth loopback port (:3000) collides with common dev servers; webhook-only degradation is log-invisible
- **Classification:** design weakness / integration failure risk (not a crash bug)
- **Severity:** Medium · **Confidence/status:** Confirmed at runtime (listener behavior), Confirmed by implementation analysis (port-selection logic)
- **Affected:** Help Scout webhook ingress (`POST /webhook`), OAuth callback (`GET /oauth/callback`), Connectors/Settings OAuth flow, Dashboard "never synced"/sync behavior; candidates W-TBD (OAuth connect, webhook health)
- **Reproduction:** On a machine where any process already listens on :3000 (e.g. Next.js dev server), launch `supportos-plus` (default config). Log (evidence/logs/F-CAND-001-port-free.log line 25): `WARN supportos_plus: loopback listener could not be started, error: Address already in use (os error 98)` at crates/app-shell/src/main.rs:129. App continues normally.
- **Expected (per implementation intent):** loopback listener binds the port baked into the Help Scout redirect URI (default `http://localhost:3000/oauth/callback`, config.rs:209) to receive webhooks and OAuth redirects.
- **Actual:** bind fails; app degrades to no-inbound-HTTP mode. UI unaffected — no badge/banner observed (initial screenshot shows normal header with "never synced" chip, which exists regardless).
- **Evidence:** evidence/logs/F-CAND-001-port-free.log (failure), evidence/logs/F-CAND-001-loopback-port3999-ready.log (`loopback listener ready (POST /webhook, GET /oauth/callback), port: 3999` with `HELPSCOUT_REDIRECT_URI=http://localhost:3999/oauth/callback` — positive control proving the listener itself works), BUILD-002-launch-initial.png (UI shows no degradation signal)
- **Root cause:** port-preference chain at main.rs:103–113: `port_from_redirect_uri(redirect_uri)` wins; `SPP_LOOPBACK_PORT` is only consulted when the redirect URI parses without a port, so the env override is ineffective in the default config (verified: with SPP_LOOPBACK_PORT=0 set, app still attempted :3000 and failed — evidence/logs/F-CAND-001-loopback-port0.log line 25). :3000 is the default port of several dev servers (Next.js, React, Angular…), making collision likely on developer machines.
- **Mitigations already present (verified in source):** (1) settings-page listener health check before offering OAuth (loopback.rs ~"1.1.4" comment; connect button must explain the problem) — runtime UI verification queued; (2) pending webhook events are drained from DB at worker start (workers.rs:219 `drain_pending`) — code verified, runtime drain test queued.
- **Recommended fix (direction, to finalize after UI verification):** prefer `SPP_LOOPBACK_PORT` over the redirect-URI port (or make the override chain explicit/fall back automatically), and surface listener state in the UI (e.g. Settings/Connectors badge) rather than log-only. Rationale: fixes the cause (unreachable inbound HTTP) and the discoverability gap, instead of masking it.
- **Regression risks:** changing port preference may break Help Scout redirect-URI matching (redirect must match exactly per main.rs comment) — the fix must keep the OAuth redirect functional and tested against both sources.
- **Acceptance criteria:** with :3000 occupied and SPP_LOOPBACK_PORT set, listener binds the override; UI shows listener state; OAuth flow + webhook POST succeed on the bound port.
- **Verification plan:** runtime matrix: (a) :3000 free, default URI → ready; (b) :3000 occupied, default URI → degraded + visible UI signal; (c) :3000 occupied, SPP_LOOPBACK_PORT=3999 → ready on 3999; (d) webhook POST delivered while degraded → drained at next worker start.
- **Next action:** Settings/Connectors UI runtime check of the health-check message; webhook drain runtime test.

## F-002 — Second-instance guard works; message only in stderr log (observation, not a defect)
- **Classification:** observation/informational
- **Severity:** Low · **Confidence/status:** Confirmed at runtime
- **Reproduction:** launch second `supportos-plus` while one runs → exit code 1, log `ERROR supportos_plus: cannot acquire the instance lock, error: another SupportOS+ instance is already running` (main.rs:34, flock on instance.lock).
- **Evidence:** evidence/logs/single-instance-second-run.log
- **Note:** source comments state no IPC/focus-existing-window behavior in the native shell (single_instance.rs header). Whether a user-visible dialog appears is queued for UI verification; a bare non-zero exit with console-only message may be a UX gap for double-click launches. Will reclassify after UI test.

## FIND-003 — Prior audit committed app-local secret-envelope keys as repo artifacts (pre-existing; observation only)
- **Classification:** repository hygiene / security observation about pre-existing content, NOT a product-code defect and NOT introduced by this audit
- **Severity:** Low · **Confidence/status:** Confirmed at runtime (file inspection)
- **Details:** `qa-audit/20261019T050847Z/evidence/artifacts/qa-profile-*/secrets.key` are 32-byte binary files (app secret-envelope local keys per m049/m053 migrations) captured from QA app profiles. No reference found in that audit's README/summary. Anyone with repo access holds key material whose matching encrypted payloads (profile DBs) are plausibly also present in evidence artifacts.
- **This audit's action:** none (preserve user work; do not modify/delete pre-existing evidence). Recorded so the maintainer can decide (e.g. treat those QA profiles as fully synthetic, or rotate/purge if any real data was ever captured).
- **Evidence:** file inspection 2026-10-24 (masked; contents not reproduced here).
