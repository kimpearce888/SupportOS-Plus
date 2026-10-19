# progress.md — active items

> Exactly five active items at any time, per `rule.md`. Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — final batch (items 45-46, two items; the final-batch precedent from batch 8)

**46. Performance pass**
- Measure startup time, memory usage, database operations, and application responsiveness
  using realistically large datasets (user-directed re-scoping: start now, not optional).
- Reproducible procedures: a bench harness (`#[ignore]`d tests + documented commands) for
  DB operations on a large seeded dataset (migrations, mirror upserts, inbox/list queries,
  FTS + hybrid search, vector upsert/KNN) plus startup-time and max-RSS measurement of the
  release binary / AppImage smoke path.
- Identify and fix confirmed bottlenecks without regressions (full suite must stay green).
- Record reproducible test procedures, actual measurements, and verification results.

(Item 45 completed and verified — see `completed.md` batch 10. CI run `39198295485`.)
