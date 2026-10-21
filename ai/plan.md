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

*(A1, A2 moved to progress.md — active batch 12.)*

**A3. Wire batch 2** — Draft coaching (9 checks + AI layer) as composer pre-send review; rewrite
(shorten/expand/warmer) and translation (11 languages, cached) in a composer AI menu; post-
resolution QA's deterministic checks to their pages; interaction intelligence's AI stage on top
of the wired heuristic stage in the Customers interaction profile.

**A4. Retrieval spine** — Chunk-on-ingest: the conversation chunker gains its caller on the sync
path; the evidence engine's semantic-hits hook receives real sqlite-vec hits so similar-issues
becomes FTS+semantic re-ranked with match reasons; the search page drops its hardcoded
semantic-false and fuses BM25 + vector hits via the existing RRF k=60 with real reported flags.
Resolved conversations carry a resolution-excerpt chunk type. Privacy default: embeddings local.

**A5. Copilot chat on the 22-tool agent** — The dead tool-calling agent (22 read-only allowlisted
tools, redacted SQL, bounded loop, sessions, citations, audit trail) becomes the AI Center chat
and the inbox assistant; answers cite real mirror rows.

**A6. Feedback loop, golden-set runner, AI usage dashboard** — Feedback calls (accept,
edit-distance, reject) wired to the composer and review queue so edit-rate is real; the golden-set
runner executes the 9 seeded cases on demand, on prompt changes, and on model-role reassignment,
writing scores to reports; the AI usage dashboard renders per-feature tokens, latency, cost,
edit rate, and eval scores live.

**Phase K — Learn & Advise (v1.6.0): the knowledge loop**

**K1. Gap detection pipeline** — Cluster repeated unresolved patterns from resolved and reopened
conversations (the orphaned quality.rs machinery wired to the sync/worker path): surfaces
"asked often, documented nowhere" topics with counts and examples.

**K2. Real AI article drafting + review queue** — The AI drafts an article from the best
resolutions with citations to them; the human review queue is the only path forward; UI in
Knowledge.

**K3. Docs API publish path** — One click upserts a Docs API draft (PUT /v1/articles/{id}/drafts),
publishes on approval, verifies by re-syncing the article locally; freshness tracking scores
every source.

**K4. Weekly AI digest + anomaly alerts** — AI-written summary of volume, topics, sentiment trend,
anomalies, workload, and what changed (report_narrative wired), generated on demand and on
schedule, archived and searchable; lists new gaps and new articles.

**K5. Help Scout Reports API behind capability detection** — The 33 GET endpoints render on
Plus/Pro; Standard plans get one honest sentence instead of empty charts (decisions D-c, D-g).

**Phase P — Power & Polish (v1.7.0): the long tail, honestly**

**P1. Ratings and CSAT rework** — Reports happiness path (Plus/Pro), satisfaction webhook path
(relay-ready ingestion), honest degradation on Standard; the misdesigned 30-second ratings timer
retired. Fully resolves D11.

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
