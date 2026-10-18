# progress.md — active items

> Exactly five active items at any time, per `rule.md`. Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items

Final batch (items 36-39). Three plan items remained and the user directed a fourth
(the Phase 4 cutover left the WorkerManager unwired in the shell boot); the plan is
exhausted after this batch, so it runs with four items instead of five.

1. Wire WorkerManager into the shell boot: background timers (job drain, webhook
   drain, embedding pass, maintenance) must start with the app, plus restart on
   sync-interval change — user-directed addition (cutover gap, not previously planned)
2. tokio feature slim + 2-thread runtime (row 12)
3. Landlock + seccomp sandbox for pdf/zip parsing (Linux, feature-flagged)
4. CI: AppImage build + smoke-test job
