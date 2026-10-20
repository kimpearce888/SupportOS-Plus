# progress.md — active items

> Exactly five active items at any time, per `rule.md`. Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items

Batch 11 — Phase R (v1.4.0 "Prove It") items 1–5 of the approved v2 blueprint:

**R1. Connection Doctor; auth flow honesty** — Five sequential checks with per-step
results and remedies in Settings (reach api.helpscout.net → client-credentials token →
GET /v2/users/me → list mailboxes → read rate-limit headers); runs after every credential
change. Client-credentials promoted to the primary, recommended flow; the authorization-code
browser flow is demoted to a clearly-labeled secondary with an exact-match redirect-URL
warning. Closes D3.

**R2. Provider-kind guard and watermark integrity** — The auto-sync timer refuses to run
against the empty provider; checkpoints and cursors never advance on empty-provider results;
an Empty→Real provider transition (boot-time or activation-time) forces sync_state back to
NEW so the full initial sync actually runs. Closes D1.

**R3. In-session credential activation** — Saving or clearing credentials rebuilds the
provider and sync engine in-session under supervision (no restart ritual); activation is
observable in the UI. Closes D2.

**R4. Send honesty** — Customer remote id resolution with API fallback + mirror repair
before POST /reply; truthful enqueue (an INSERT failure renders an error, preserves the
composed text, never reports success with job id zero); settings-backed statusAfter
(default `pending`) on every reply send; immediate post-delivery single-conversation
refresh. Closes D7, D8, D9.

**R5. Error surfacing and complete invalidation** — ErrorOccurred gains real emitters from
sync, outbound, connect and worker failures (toast + diagnostics); the page invalidation map
covers every page (11 currently missing); zero-record sync completions render an explicit
"up to date, nothing new" outcome; docs-sync missing-key degradation (D15) and inbox-detail
error swallowing (D16) surface honestly. Closes D4, D5, D15, D16, D19, D20.
