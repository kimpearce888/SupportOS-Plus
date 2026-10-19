# progress.md — active items

> Exactly five active items at any time, per `rule.md`. Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items — final batch (items 45-46, two items; the final-batch precedent from batch 8)

**45. Integration verification pass — local mock-server suite (Help Scout, LM Studio)**
- User-directed re-scoping (2026-10-19): replace the credential-dependent development tests
  with comprehensive local integration tests using mock servers, realistic fixtures, and
  generated test credentials. No user-provided credentials; no production credentials; no
  mandatory cloud dependencies; no cloud fallbacks.
- Verify against a local mock Help Scout API server: API behavior (list/get/create/update
  round-trips through `RealHelpScoutProvider`), sync round-trip (`SyncEngine::initial_sync`
  and `sync_single_conversation`), pagination (v2 page-based and v3 cursor via `_links.next`
  and `page.nextCursor`), error mapping (4xx/5xx → friendly, retryable), rate limits (429 +
  `x-ratelimit-*` headers + retry), webhook ingress + HMAC validation over real TCP through
  the `LoopbackServer`, and the OAuth authorization-code exchange end-to-end (loopback
  callback → `exchange_code` → token persistence → `get_me`).
- LM Studio: test inference against a running local model server if available (probe;
  none found in this environment), otherwise verify connection handling, error reporting,
  and recovery against a mock OpenAI-compatible server. Optional live test env-gated on
  `SPP_LM_STUDIO_URL`, never claimed as run unless it actually ran.
- Never claim real Help Scout integration or actual model inference passed unless genuinely
  tested. Document which tests passed, failed, or could not be performed, including the
  limitation that real Help Scout account verification requires external credentials.

**46. Performance pass**
- Measure startup time, memory usage, database operations, and application responsiveness
  using realistically large datasets (user-directed re-scoping: start now, not optional).
- Reproducible procedures: a bench harness (`#[ignore]`d tests + documented commands) for
  DB operations on a large seeded dataset (migrations, mirror upserts, inbox/list queries,
  FTS + hybrid search, vector upsert/KNN) plus startup-time and max-RSS measurement of the
  release binary / AppImage smoke path.
- Identify and fix confirmed bottlenecks without regressions (full suite must stay green).
- Record reproducible test procedures, actual measurements, and verification results.
