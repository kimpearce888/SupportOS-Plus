# progress.md — active items

> Exactly five active items at any time, per `rule.md`. Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — Batch 15 (final): Phase P remainder (P2, P3, P4, P5)

Selected from `plan.md` on the owner's "continue" (2026-10-21). Four items, not five — the
plan is otherwise exhausted (Rule 6: every item processed; Rule 1 caps at five).

**P2. Live chat** — Real chats list and message send under the assigned-agent rule
(PUT /v2/chats/{id}/messages/{uuid}).

**P3. Saved replies CRUD + workflows surfaced** — Saved replies management UI over
/v2/mailboxes/{id}/saved-replies; workflows surfaced in Operations.

**P4. Webhook relay guide** — User-run tunnel documentation; hook health checks and automatic
re-registration guarding silent deactivation. Closes D21.

**P5. Remote and release hygiene** — Git origin restored (sandbox reset removed it locally;
remote intact through v1.3.0), release cadence verified end to end, token rotation reminder to the
owner. Note: per decision D-f the origin restore defaults to END of Phase R so v1.4.0 ships
through CI; if executed there, this item reduces to release-cadence verification + token reminder.
