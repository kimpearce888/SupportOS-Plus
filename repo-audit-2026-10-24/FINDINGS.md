# FINDINGS.md — SupportOS-Plus audit (incremental; statuses per 04-FINDINGS §1)

Status: IN PROGRESS — findings added as evidence supports them. Confidence labels: Confirmed at runtime / Confirmed by implementation analysis / Suspected / Blocked.

---

## FIND-001 — Default webhook/OAuth loopback port (:3000) collides with common dev servers; webhook-only degradation is log-invisible
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

## FIND-002 — Second-instance guard works; message only in stderr log (observation, not a defect)
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

## FIND-004 — Pinned theme row overlaps the bottom sidebar navigation rows; Shell row only clickable through a ~10 px sliver at the default window size
- **Classification:** UI layout defect (usability; no data loss)
- **Severity:** Low · **Confidence/status:** Confirmed at runtime (pixel-measured + click-verified) and by implementation analysis
- **Affected:** Sidebar navigation at the default 1180x760 window (`lib.rs:241`); worst-hit row: "Shell" (last of 23); same overlap previously observed over "Automation"/"Support Graph"/"Onboarding" at scroll offset 0
- **Reproduction:** Launch the app at default size, scroll the sidebar nav to the bottom. The pinned theme row ("Switch to dark theme") is painted over the lower part of the last nav row: in the final walk the Shell row's text band is at y≈717 while the theme row's hover rect measures [714,743] (`feature-walk-events.jsonl`, `shell-probe`); the row is only half visible.
- **Root cause:** `app.rs sidebar_ui` (397–466): the nav `ScrollArea` (line 434) is laid out over the full remaining panel height, then the theme row is added in a `bottom_up` section (line 455) that paints on top instead of the scroll area reserving space for it.
- **Impact:** The last nav row looks truncated/cut and its clickable area is a thin sliver; for "Shell" no scroll position exposes it fully (max scroll keeps it under the overlay). A user can still click the sliver (the audit did, verified by selection pill + content diff), but the interaction is fragile and visually broken. Keyboard/palette alternatives for Shell were checked and do not exist (palette searches records, not pages; no CLI page arg).
- **Evidence:** evidence/screenshots/FEATURE/23-shell.png (Shell selected with theme text overlapping the row), 01-dashboard.png … 22-onboarding.png (overlap visible on the then-bottom row), evidence/artifacts/feature-walk-events.jsonl (`shell-probe` theme_rect [714,743]; `shell-attempt` y=703 verified), feature-walk.py (click zone capped at y≤700 to avoid theme toggles)
- **Recommended fix:** Reserve layout space for the pinned theme row (end the nav ScrollArea above it) so no nav row is ever occluded; alternatively shrink the scroll viewport. Keep the theme row inside the sidebar per design.
- **Regression risks:** Pure layout change; verify all 23 rows reachable at 1180x760 and at larger sizes, and that the theme toggle still sits bottom-left.
- **Acceptance criteria:** At the default window size every sidebar row, including Shell, is fully visible and directly clickable; hover of the theme row no longer covers any nav row.
- **Next action:** none queued beyond fix recommendation (layout defect confirmed; no deeper test needed)

## FIND-005 — Destructive actions without confirmation on several pages, while equivalent deletes elsewhere are confirm-gated (inconsistent destructive-action UX)
- **Classification:** UX safety inconsistency (no data loss observed at runtime; none of these were executed)
- **Severity:** Low · **Confidence/status:** Confirmed by implementation analysis (pass-2 source enumeration, four independent agents)
- **Instances (no confirmation):** Operations Library saved-reply "delete" (C-OPS-25, operations.rs — enqueues platform-side `saved_reply_write` delete via `jobs::enqueue_on(conn,"sync",…)`; failure surfaces only as an inline hint); Reports Builder saved-report "delete" (C-REPT-25..29, reports.rs:913-963, 1796-1820); Outreach "cancel remaining" sends (C-OUTR-11) and DNC "remove" (C-OUTR-29); AI Center global AI kill switch (C-AIC-42/43 — one click disables all AI behavior).
- **Contrast (confirm-gated equivalents):** knowledge document delete (shell confirm, knowledge.rs:823-841), custom-object type/object deletes (C-COBJ-03/06, shell confirm with "cannot be undone" text), connector delete (C-CONN-12), segment delete (C-OUTR-25), automation rule delete, diagnostics clear log (ConfirmRequest; exercised at runtime in pass 2).
- **Impact:** one accidental click silently deletes platform data or flips global behavior; the inconsistency makes the dangerous paths look as safe as the gated ones.
- **Evidence:** source line refs above (pass-2 ledger rows in FEATURE-INVENTORY.md); runtime reproduction deliberately not attempted (destructive).
- **Recommended fix:** route all destructive actions through the existing shell ConfirmDialog (the pattern is already implemented and proven at runtime).
- **Next action:** none queued (maintainer decision).

## FIND-006 — Sync Health "Cancel running sync" on an idle engine is a silent no-op that still toasts "(started in background)" (misleading feedback)
- **Classification:** UX feedback defect
- **Severity:** Low · **Confidence/status:** Confirmed at runtime (click exercised; transient re-render measured) + Confirmed by implementation analysis (sync_health.rs:376 toast format; the already-running guard is deliberately skipped for Cancel)
- **Reproduction:** open Sync Health with no sync running (state NEW), click the red "Cancel running sync". A transient UI change renders (content diff 4.986 in the pass-2 run — consistent with the toast documented in source), while nothing is running and nothing is cancelled.
- **Evidence:** evidence/screenshots/CONTROL/SY1.png; control-verify-events.jsonl (SY1 rows); read-only DB check after the run: sync_runs=0, sync_cursors=0, sync_checkpoints=0, jobs=0 (no state consumed). The transient toast itself was not captured in the post frame (its lifetime was shorter than the parked-frame capture) — the toast text is source-quoted instead (sync_health.rs:376).
- **Recommended fix:** when no sync is running, disable "Cancel running sync" or toast "No sync is running" instead of "(started in background)".
- **Next action:** none queued.

## FIND-007 — Help-center search with zero hits silently persists "knowledge gap" records (hidden write side effect of a read-looking control)
- **Classification:** behavior surprise / data hygiene
- **Severity:** Low · **Confidence/status:** Confirmed by implementation analysis (knowledge.rs:1344-1366, fire-and-forget `record_knowledge_gap`)
- **Mechanism:** every zero-hit help-center search writes a knowledge-gap candidate row used by the Drafts gap picker and the "Knowledge gaps (tracked)" card. The control looks read-only; repeated empty searches accumulate rows without user awareness.
- **Runtime:** not reproduced (needs help-center data/credentials); the search control itself was exercised on the empty-query path (CONTROL/CU1.png is Customers; the knowledge search button is source-verified only).
- **Recommended fix:** keep the gap tracking but make it visible (e.g. a toast "tracked as a knowledge gap" or a badge on the search box), and/or dedupe+cap.
- **Next action:** none queued.

## FIND-008 — Issue Radar "Detect gaps now" / "Cluster now": enqueue results silently discarded, no completion signal
- **Classification:** feedback/degradation gap (related in spirit to FIND-001's log-only degradation, but a distinct control path)
- **Severity:** Low · **Confidence/status:** Confirmed at runtime (W-005, 2026-10-24: both enqueue buttons exercised; inline "gap detection queued — reload in a few seconds" instruction appears; jobs completed with zero data and NO completion toast/badge; results only visible after the manual reload) + Confirmed by implementation analysis (issue_radar.rs — `let _ = jobs::enqueue_on("ai", …)`; agent pass-2 read)
- **Mechanism:** clicking these buttons enqueues an AI job and returns immediately; the enqueue result is discarded and the UI instructs the user to "reload in a few seconds". Job failure (e.g. LM Studio unreachable) lands only in the jobs table — no toast, no badge.
- **Runtime:** RUNTIME-CONFIRMED 2026-10-24 (W-005, E-125..E-130): 'Detect gaps now' and 'Cluster now' each enqueued an `ai` job (jobs 0→2); an inline instruction line 'gap detection queued — reload in a few seconds to see what landed' appears on enqueue; with zero conversations/documents both jobs completed trivially (`ai`/`completed` ×2) and NO toast/badge/signal marked the completion — the stale instruction remained until the manual 'Reload clusters' click re-rendered the (still empty) page. "Reload clusters" exercised (empty path; matches pass-2 RA1). The instruction line itself is the behavior the original finding quoted; the finding's core claim (no completion signal; user must know to reload) stands.
- **Recommended fix:** surface enqueue/failure state (toast on enqueue + failed-jobs badge on the page).
- **Next action:** none queued.

## FIND-009 — Silent input coercion / weak validation on several numeric and date fields
- **Classification:** validation gaps
- **Severity:** Low · **Confidence/status:** Confirmed by implementation analysis (pass-2 source enumeration)
- **Instances:** Connectors refresh-seconds field — invalid input silently becomes the default 3600 (`parse::<i64>().unwrap_or(3600).clamp(60, 86400)`, no inline error); Operations workload capacity "+"/"−" — clamped 1..=500 with no boundary feedback; Reports Builder date fields — validated by length-10 only, calendar-invalid dates pass through.
- **Runtime:** the Connectors form was opened with the 3600 default visible (CONTROL/CN1.png); invalid-input behavior was not reproduced (would create records).
- **Recommended fix:** inline validation messages at the field level (the codebase already has this pattern elsewhere, e.g. SLA thresholds "both thresholds must be positive numbers").
- **Next action:** none queued.

## FIND-010 — Customers "← back to search" clears the results without re-running the still-applied query (UX/QA trap)
- **Classification:** UX defect (state loss)
- **Severity:** Low · **Confidence/status:** Confirmed by implementation analysis (customers.rs, agent pass-2 read)
- **Mechanism:** the back link clears `rows` and resets the pager but does not re-issue the search, so the user lands on "No customers loaded yet" even though a query was applied moments earlier.
- **Runtime:** not reproduced (needs customer data/credentials); the page's search field + button were exercised on the empty-query path (CONTROL/CU1.png).
- **Recommended fix:** re-run the last query on "back to search" (or keep the result set and scroll to top).
- **Next action:** none queued.

## FIND-011 — Command palette: the claimed click-outside backdrop does not reliably close it at runtime (position-dependent); on mouse-only interaction the palette can stay stuck open
- **Classification:** UI interaction defect (modal dismissal unreliable)
- **Severity:** Low · **Confidence/status:** Confirmed at runtime (6-point click matrix, two runs) + Confirmed by implementation analysis (palette.rs:209-226 claims a full-screen catcher; clean-audit item 133)
- **Source claim:** palette.rs:209-226 — "the palette is a modal overlay — a full-screen catcher behind the window closes it on an outside click … the catcher takes everything else" (`Area::fixed_pos(0,0)` + `allocate_response(available_size, Sense::click())`, Order::Middle, created before the palette Window so the window wins inside itself).
- **Observed at runtime (empty DB, Dashboard, palette opened via the header "Search ⌘K" button, verified open by screenshot + OCR):** outside clicks at (950,120), (600,500), (450,620), (1100,90) and (1000,620) did NOT close the palette and at least two of them reacted with the UI UNDERNEATH (diff 0.081 metric-card area, diff 0.449 header area — i.e. the catcher did not intercept them, contradicting the "keeps that click from landing on whatever is underneath" comment). One click at (300,300) — left of the palette window rect — DID close it (diff 1.69, verified closed). The dismissal therefore works only in a narrow, unmeasured region; root cause (Area/layer hit-testing) not yet determined.
- **Additional runtime observations:** the header button and sidebar trigger call `palette.open()` (app.rs:428-429, 487-490) — neither toggles, so with the palette open the mouse cannot dismiss it through them; the reliable close paths in source are Escape (palette.rs:163) and clicking a hit row (palette.rs:318-321) — the former is keyboard-blocked in this environment, the latter requires typed query hits (also keyboard-blocked; empty DB has no hits regardless).
- **Impact:** with a mouse only, a user who opens the palette may be unable to dismiss it (except by stumbling into the working region or clicking a hit row); clicks fall through to the UI underneath while the overlay is visible.
- **CONCRETE CONSEQUENCE (attributed 2026-10-24, W-002 prep):** the fall-through is not just cosmetic — the W-008 backdrop click at **(1100,90)** (events JSONL row `W08c-backdrop … diff 0.449, palette_still_visible: true`) landed on the Dashboard **"Sync now"** button (Incremental action) and spawned a sync action; the no-credentials guard refused it and wrote a row into `application_errors` ("Run incremental sync failed: Help Scout is not connected…", timestamp **21:31:15** — exactly the W-008 window). The side effect went unnoticed during W-008 and surfaced as an unexplained pre-existing DB row during W-002 prep. This upgrades FIND-011's impact: fall-through clicks can trigger REAL actions (including the destructive-action set in FIND-005's list) while the user believes they are only dismissing an overlay.
- **Evidence:** evidence/screenshots/CONTROL/W08-palette-open.png, W08-palette-closed.png, W08-palette-reopen.png, W08b-palette-closed.png, W08c-open.png; control-verify-events.jsonl (W08/W08b/W08c rows with the full click matrix); palette.rs:163, 205-226, 318-338.
- **Recommended fix:** make the catcher a true full-screen top-order area (or close on `ctx` click-outside-window-rect), and/or make the header/sidebar triggers toggle. Regression test: open palette, click each screen quadrant, assert closed.
- **Next action:** optional root-cause deep-dive in ARCHITECTURE.md deep-dive phase (egui Area layering); not blocking.
