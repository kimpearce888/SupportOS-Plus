# progress.md — active items

> Exactly five active items at any time, per `rule.md`. Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items

Batch 12 (selected from plan.md on owner "continue", 2026-10-21):

- **R6. Honest stubs** — Ratings and chats resources state their platform reality (no list endpoint for ratings; reports happiness needs Plus/Pro) instead of silently returning empty; dead no-op resources (attachments, report_data) removed from the sync sweep until implemented. Closes D11, D12.
- **R7. Token encryption, residue purge, version transparency, no-simulation gate** — OAuth access and refresh tokens move into the AES-256-GCM encrypted envelope. Residue purged (stale demo boot comment, fake-response string, simulated-attachment text, dead simulated-rating handler, dead legacy flows 18/19). Settings shows the running version vs the latest published release. A CI gate fails the build if production code reintroduces simulated/fake data markers. Closes D13, D14.
- **R8. Unified refresh, adaptive cadence, freshness ring** — Every refresh control performs a scoped real fetch and reports its outcome; a running sync shows busy state instead of a silent no-op (D6); adaptive polling (busy 30s, quiet back off toward 5 min, immediate poll after sends) replaces the fixed interval, consuming SYNC_INTERVAL_MINUTES (D18); a global "last synced Ns ago" freshness ring in the header links to Sync Health. Closes D6, D10, D18.
- **A1. Provider hub** — Endpoints list (local LM Studio default, optional Ollama, user-added OpenAI-compatible cloud endpoints BYOK per decision D-a); keys in the encrypted envelope; model-role routing (Embed/Classify/Summarize/Draft/Agent per blueprint Table 6); per-endpoint health probes; per-feature toggles and kill switches in AI Center; per-feature cost ledger (tokens, latency, estimated spend per day).
- **A2. Wire batch 1** — Attribute extraction (14-key catalog) populates ai_attributes so the AI Center Attributes tab and automation conditions go live; triage-on-arrival (intent/urgency/sentiment/topic chips with confidence, editable) in the inbox header; issue clustering generates real Issue Radar clusters; customer memory extraction lands in a "know before you reply" briefing panel and feeds the evidence pack.

