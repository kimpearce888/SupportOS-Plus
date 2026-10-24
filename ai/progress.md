# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — batch 34 (opened 2026-10-25)

Batch 33 (clean-audit items 137-146) is complete and verified; the entry is
recorded in `completed.md`. Item 56 (report-builder wire-or-remove) stays
parked in `plan.md` awaiting the owner decision noted there, as do 150/151
(dead-feature decisions). The owner's standing directive authorizes
continuing through the batches without a per-batch `continue` wait.

147. [low|issue] ensure_loaded-on-empty re-query pattern (7 pages, wasted work) — pages/outreach.rs:121 + 6 more
148. [low|gap] Remove dead Runner/JobRegistry/bulk_*/EventEmitter machinery — runner.rs, workers.rs, eventing.rs
149. [low|gap] Dead prompt surfaces (report_narrative, duplicate attribute prompt, ai_features runtime incl. mismatched translation-v1) — ai_pipeline.rs:1780, ai_prompts.rs:454, ai_features.rs:159
152. [low|gap] Dead copilot/embedding helper surface — copilot.rs:24-63, embeddings.rs:96-161, ai_lm_studio.rs:24-55
153. [low|gap] AppConfig helpscout/ai/sync/loopback fields constructed, never consumed — config.rs:18-61
154. [low|gap] Dead hybrid_search fusion stack (id-namespace mismatch latent) — hybrid_search.rs:162
155. [low|gap] Legacy FTS tables written every sync, never queried; chunk_version never read — search.rs:1356
156. [low|gap] Connector AI-search/auth-edit + data_tools runtime dead (legacy-schema drift) — connectors.rs:1024, data_tools.rs:63
157. [low|gap] AppDir staging never cleaned (stale files ship) — xtask/package.rs:157
158. [med|gap] appimagetool: unversioned URL, no checksum, executed in the release path — xtask/package.rs:61,551-574
