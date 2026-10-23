# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — Batch 27 (clean-audit items 77-86)

77. [med|issue] Sync invalidation silently reverts unsaved Settings edits — pages/settings.rs:550-552
78. [med|gap] Inbox hard-capped at 50, no pagination — pages/inbox.rs:109-110
79. [med|issue] Row click silently wipes the typed reply draft — pages/inbox.rs:742-747
80. [med|bug] Approvals attributed to hardcoded user 5 — pages/automation.rs:323-330
81. [med|bug] Known-issue links accept dangling ids and count them — pages/issue_radar.rs:147-149
82. [med|gap] Customers page fetches page 1 only (50 cap) — pages/customers.rs:217-222
83. [low|bug] LIKE input not escaped despite ESCAPE clause (customers.rs:162, inbox.rs:185, mirror_readouts.rs:207; trailing `\` errors)
84. [low|bug] jobs::fail called with literal job id 0 (2 sites) — workers.rs:2748,3067
85. [low|bug] Rating fetch errors swallowed; ratings permanently lost — workers.rs:1245-1246
86. [low|bug] Panic containment marks copilot jobs retryable (re-persists user message) — workers.rs:369-374
