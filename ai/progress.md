# progress.md — active items

> Exactly five active items at any time, per `rule.md`. Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — Batch 22 (clean-audit items 31-35)

31. [med|gap] `api_concurrency` setting parsed/saved but never applied — api_queue.rs:94-97
32. [med|bug] Webhook secret capped to 40 chars at registration but verified with the full secret — webhook_health.rs:255,168
33. [med|bug] Team id written into the user-assignee slot; `assigned_team_id` unconsumed — helpscout_real.rs:1392-1394
34. [med|bug] Attribute extraction parses raw output without fence-stripping/json_mode — ai_attributes.rs:817-818
35. [med|issue] Attribute extraction bypasses the prompt-redaction boundary — ai_attributes.rs:776-787
