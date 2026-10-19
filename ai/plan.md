# SupportOs-Plus — TODO

> Remaining work items, executed five at a time under `rule.md`.
> Status flow: `plan.md` → `progress.md` → implementation → verification → `completed.md`.
> Planning context and decision ledger: `decisions.md`.

## Items

**45. Real-service verification pass (Help Scout, LM Studio)**
- From the 2026-10-19 audit (P1-2). BLOCKED: requires user-provided Help Scout
  credentials (API token, or OAuth client id/secret) — none exist in the dev
  environment. When provided: verify a real sync round-trip, real webhook
  ingress + HMAC verification against the loopback listener, and the OAuth
  code exchange. LM Studio coverage requires a running local model server.

**46. Performance pass (optional, user's call)**
- From the 2026-10-19 audit (P2-7). Startup time and large-DB behavior have
  never been measured. Start only if the user confirms it matters.
