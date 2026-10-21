# SupportOs-Plus — TODO

> Remaining work items, executed five at a time under `rule.md`.
> Status flow: `plan.md` → `progress.md` → implementation → verification → `completed.md`.
> Planning context and decision ledger: `decisions.md`.

## Items

Source: SupportOS+ v2 Product Blueprint (approved by owner "continue", 2026-10-21).
Phases: R = v1.4.0 Prove It · A = v1.5.0 Lights On · K = v1.6.0 Learn & Advise · P = v1.7.0 Power & Polish.
Defect IDs (D1–D21) refer to the blueprint's Appendix A register (file:line evidence in the
audit records of 2026-10-21). Owner decision defaults D-a..D-g apply as recorded in the blueprint.

**Phase R — Prove It (v1.4.0): trust and truth**

*(R6, R7, R8 moved to progress.md — active batch 12.)*

**Phase A — Lights On (v1.5.0): the AI becomes real**

*(A1, A2 completed in batch 12; A3–A6 completed in batch 13. Phase A — "Lights On" — is complete.)*

**Phase K — Learn & Advise (v1.6.0): the knowledge loop**

*(K1 completed in batch 13; K2, K3, K4, K5 completed in batch 14. Phase K — "Learn &
Advise" — is complete.)*

**Phase P — Power & Polish (v1.7.0): the long tail, honestly**

*(P1 completed in batch 14.)*

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
