# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — batch 32 (clean-audit items 127-136)

127. [low|bug] Digest chip 16.max slice (dead truncation + OOB risk) — pages/reports.rs:544
128. [low|bug] QA rows render `#Some(42)` — pages/reports.rs:885
129. [low|bug] Literal `\` mid-sentence in 3 hover texts — pages/issue_radar.rs:282,292,309
130. [low|bug] Notification toggle reads the wrong list for Mentions rows — pages/notifications.rs:111-116
131. [low|issue] Incidents: invalid ids silently dropped; filters without refetch — pages/incidents.rs:185-203,320-344
132. [low|issue] Diagnostics clear_log via reader lane, errors swallowed — pages/diagnostics.rs:101
133. [low|bug] Palette: same-frame Enter confirms stale hit; no outside-click close — palette.rs:258,215
134. [low|bug] Concave polygon to convex_polygon (area fill) — chart.rs:77-84
135. [low|bug] Stale inserted_draft provenance → phantom AI-draft acceptances — pages/inbox.rs:1435,1505
136. [low|gap] Mention popup highlight never moves — pages/inbox.rs:2277-2278

Item 56 (report-builder wire-or-remove) stays parked in `plan.md` awaiting
the owner decision noted there, as do 150/151 (dead-feature decisions).
The owner's standing directive authorizes continuing through the batches
without a per-batch `continue` wait.
