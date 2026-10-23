# progress.md — active items

> Exactly ten active items at any time, per `rule.md` (batch size raised from five by the owner, 2026-10-23). Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — batch 26 (clean-audit items 67-76, MEDIUM tier)

Item 56 (report-builder wire-or-remove) stays parked in `plan.md` awaiting the
owner decision noted there; this batch is the next ten eligible items.

67. [med|gap] translation_cache never pruned; retention policy omits it — translation.rs:44 + maintenance.rs:113-118
68. [med|gap] Audit trail never surfaced; dead reader drops non-JSON payloads — audit.rs:153
69. [med|bug] Toasts expire on wall-clock while hidden (errors silently lost) — toasts.rs:98,115
70. [med|gap] Confirm-dialog system never armed; destructive actions unconfirmed — app.rs:570-585
71. [med|gap] SPP_SMOKE returns before shell/renderer boot — renderer regressions pass CI — main.rs:139-144
72. [med|bug] Palette Enter on zero hits defocuses the input; keystrokes swallowed — palette.rs:248-261
73. [med|issue] Transform completion overwrites typing made in flight — pages/inbox.rs:1749-1753
74. [med|bug] Role-model field persists + enqueues golden_eval per keystroke — pages/ai_center.rs:1210-1242
75. [med|bug] Copilot submits on a global Enter with no focus check — pages/ai_center.rs:657-658
76. [med|issue] Sync invalidation destroys the open Library edit form — pages/operations.rs:648-652
