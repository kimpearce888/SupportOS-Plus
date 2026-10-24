# 05 — Incremental Reporting, Checkpoints, Handover, Resume, and Git

## 1. Mandatory incremental reporting rule

**Never wait until the entire audit is finished before reporting meaningful progress or saving supported results.** The final report is the consolidated deliverable, not the first time discoveries are written down.

Report and persist work at these points:

1. **At the beginning:** after verification, report repository revision, environment, initial state, audit location, reading-gate result, and immediate next step.
2. **After each meaningful batch:** report completed work, significant findings, evidence captured, coverage state, blockers, and next action.
3. **After each major phase:** update `PROGRESS.md` and `HANDOVER.md` before proceeding.
4. **After a significant defect/blocker:** record it in `FINDINGS.md` promptly, even if root-cause analysis remains incomplete.
5. **After discovering functionality:** add it to `FEATURE-INVENTORY.md` as established, not from memory at the end.
6. **After testing a workflow:** update workflow status and evidence links immediately.
7. **When context is nearly exhausted, the task is interrupted, a session ends, or agent ownership changes:** perform the full handover protocol below.
8. **At each session end:** report actual work completed, what remains, and exact next action.

Updates must be factual and concise. No generic “still working” messages. Distinguish completed, started, planned, blocked, and unverified. Never claim an artifact, test, commit, push, or publication exists unless verified.

## 2. Required live records

Maintain these files in the audit folder and update them continuously:

| File | Responsibility |
|---|---|
| `HANDOVER.md` | Authoritative current snapshot and resume instructions |
| `PROGRESS.md` | Phase statuses, counts, blockers, overall completion state |
| `SESSION-LOG.md` | Chronological meaningful actions, results, decisions, failures, state changes |
| `NEXT-ACTIONS.md` | Ordered outstanding tasks and one explicit immediate next action |
| `FEATURE-INVENTORY.md` | All discovered capabilities, controls, and options |
| `WORKFLOW-COVERAGE.md` | Workflow branches, steps, outcomes, and test status |
| `FINDINGS.md` | Issues, evidence, cause, fix recommendations, status |
| `evidence/manifest.csv` | Index of evidence files and the exact claims they support |
| `evidence/exceptions.csv` | Step-specific evidence exceptions and limitations |
| `BUILD-AND-INSTALL.md` | Build/install commands, output, outcome, environment, resume point |
| `REPORT.md` | Consolidated narrative, updated incrementally and marked IN PROGRESS until final |

Do not duplicate the entire main report inside the handover. Reference detailed records using stable IDs and relative paths.

## 3. Mandatory handover trigger

Handover is required when:

- A session is ending or likely to end.
- Another agent is taking ownership.
- Context approaches its practical limit.
- Work is interrupted/paused.
- A major phase or batch completes and handoff is useful.
- An execution failure prevents the current agent continuing.
- The user requests a status/handover.

The task must remain resumable at every point; it cannot depend on one agent, conversation, context window, or uninterrupted run. Do not wait for the whole audit before creating/updating the handover.

## 4. Mandatory `HANDOVER.md` contents

Keep a current, directly actionable snapshot containing:

### A. Objective and constraints
- Repository and audit purpose.
- Governing instructions and final deliverables.
- Important constraints, especially source-of-truth rules, no product-code changes unless authorized, evidence requirements, privacy, and safe testing.

### B. Repository and Git state
- Repository URL and local path.
- Original application revision being audited.
- Current branch and HEAD.
- Audit branch, if used.
- Working-tree status.
- Which audit changes are local, uncommitted, committed, and/or pushed.
- Latest verified remote commit when applicable.

Keep the original audited application revision distinct from later audit commits.

### C. Current phase and exact progress
- Current phase/batch ID.
- Last fully completed phase/batch.
- Last completed feature/workflow/test-step ID.
- Partially completed operation, if any.
- Last action and actual result.
- Exact point where execution stopped.
- Any in-flight action and whether it is safe to repeat.

Avoid vague phrases such as “testing UI.” Identify concrete module/screen/workflow/step IDs and paths.

### D. Exact next action
Give one concrete action another agent can execute immediately: file/screen/workflow, current state/prerequisites, exact step ID, expected observation, evidence to capture, and which records to update. Example format (replace with real state):

> **Next action:** Resume `W-014` from the saved state described in `WORKFLOW-COVERAGE.md`. Open the settings panel, execute `W-014-S023`, capture the resulting state, and update its evidence and coverage entries. Do not repeat completed steps `W-014-S001`–`W-014-S022` unless verification shows evidence/results missing or invalid.

### E. Completed work
List significant source areas examined, architecture discoveries, features inventoried, workflows completed, build/install results, findings recorded, and evidence validated. Reference files and stable IDs; do not force the incoming agent to reconstruct results from a long session log.

### F. Coverage snapshot
Report discovered features/controls; tested/untested controls; identified/completed workflows; passed/failed/blocked/inconclusive/not-attempted tests; evidence files captured/validated; accepted exceptions and unresolved gaps; findings by severity/status. Include definitions or links. Mark preliminary counts not yet reconciled.

### G. Outstanding work
Prioritized ordered tasks based on dependencies and risk. Each task names its IDs/files, prerequisites, and expected result. `NEXT-ACTIONS.md` must make the single immediate next action unambiguous.

### H. Blockers and known problems
Confirmed blockers, failed commands, unresolved defects, unavailable dependencies, access limits, environmental restrictions, attempted workarounds, relevant sanitized errors, and whether causes are confirmed or suspected.

### I. Evidence and artifacts
Paths for reports, inventories, workflow/test coverage, screenshots/videos, manifest, exception register, logs, findings, and state needed for continuation. State whether each key artifact exists and has been validated.

### J. Environment and constraints
OS, toolchain, running application state, automation/capture tools, required services, safe test accounts, data constraints, and continuation dependencies. Never store credentials/secrets.

### K. Decisions
Record decisions that affect the audit, why a test method was chosen, why a workflow is blocked, which exceptions were accepted/rejected, and what must not be repeated or changed.

### L. Publication status
Current local/remote status, audit branch, most recent relevant commit, whether incremental updates were pushed, and outstanding publication problems.

## 5. Checkpoint before handover/session end

1. Stop at a safe point; do not leave a destructive operation running undocumented.
2. Save the last completed operation's outcome.
3. Save current screenshots, logs, outputs, and evidence.
4. Update inventory, workflow coverage, finding register, manifest, and exception register as relevant.
5. Update `SESSION-LOG.md`, `PROGRESS.md`, and `NEXT-ACTIONS.md`.
6. Update `HANDOVER.md` last so it reflects the latest saved state.
7. Validate its paths, IDs, statuses, and exact next action.
8. State which changes are local, committed, and pushed.
9. If authorized and feasible, commit/push the checkpoint according to Section 6.
10. Provide a handover message with current status, exact next action, and publication status.

Never mark unfinished tests complete merely to make a handover look clean. If a push is impossible, save locally and report why. Do not claim another machine/session can access unpushed local-only files.

## 6. Fresh-session/incoming-agent resume procedure

**Before new audit activity, the incoming agent must read and verify the handover.**

1. Locate the existing audit folder and read `HANDOVER.md`.
2. Read `PROGRESS.md` and `NEXT-ACTIONS.md`.
3. Read relevant recent `SESSION-LOG.md` entries.
4. Read only the inventory/workflow/finding/evidence records referenced by the next action, plus relevant instruction files and requirement IDs.
5. Verify branch, HEAD, working-tree state, and latest available audit commit.
6. Confirm that required files and test state exist.
7. Check for changes since the handover that invalidate assumptions.
8. Correct the handover only if verification identifies a discrepancy.
9. Execute the documented next action and continue from the checkpoint.

The incoming agent must **not restart, rescan the whole repository, rerun every completed test, rebuild completed maps, or regenerate reports simply because it is a new session**. Consult the broader repository only where needed by the next action, where the previous state is inconsistent, or where new evidence reveals a material gap.

Use existing inventories/evidence as the baseline. Revisit completed work only for a concrete reason such as missing/corrupt evidence, incorrect result, changed application revision, or inconsistency. Record why rechecking was needed.

If the handover is absent, stale, or inconsistent, reconstruct the state from progress files, Git history, audit artifacts, and relevant files. Recover the narrowest reliable checkpoint; do not assume no work was done merely because handover is incomplete.

## 7. What does not qualify as a handover

A handover is not adequate if it contains only a generic task list, a summary without exact current state, chat messages not saved to the repository, no unfinished test IDs, local-only files inaccessible to the next environment, evidence without links to steps/workflows, or “continue testing” without concrete instructions.

Before claiming successful handover, verify saved records contain the exact next action, enough state to reproduce/restore the next step, and links to existing artifacts.

## 8. Incremental save, commit, and push policy

Do not wait for completion to save/publish already supported findings.

1. Create initial progress records early.
2. Save architecture discoveries, inventory updates, verified findings, workflow results, and evidence references as they are established.
3. Commit/push meaningful checkpoints after major phases or substantial logical batches when authorized and feasible.
4. Keep checkpoints on the designated audit branch so an incoming agent can retrieve them.
5. Use descriptive checkpoint commits.
6. Update `HANDOVER.md` before each handover/session boundary and include it in the checkpoint where feasible.
7. Verify remote branch/commit before claiming a push succeeded.
8. Continue updating the same audit folder rather than create a new one for every session.

Do not commit every trivial interaction separately. Use small reviewable batches while avoiding substantial work held only in transient memory or inaccessible local state. Mark interim docs `IN PROGRESS`/`PRELIMINARY` until reconciled.

Only commit intended audit artifacts. Do not force-push, rewrite unrelated history, discard user changes, modify product code, weaken tests, or include secrets. If remote publication is unavailable, keep locally saved checkpoints, state commit/uncommitted/push status precisely, and never claim remote availability. The final commit consolidates the audit; results must not remain scattered across chats or temporary files.

## 9. Git publication procedure

1. Record initial branch, HEAD, and working-tree state.
2. Preserve all existing user changes and untracked files.
3. Create one audit folder at repository root, or resume the existing one.
4. Store reports, coverage, evidence, and exception records inside it.
5. Validate references and evidence files.
6. Reconcile the manifest with actual files.
7. Review final diff; only intended audit deliverables may be staged.
8. Use a dedicated audit branch where appropriate.
9. Stage only the audit folder.
10. Commit with a descriptive message.
11. Push using authorized credentials.
12. Verify remote branch and commit.
13. Record commit hash, branch, and publication state.

Never expose credentials/tokens/private keys/passwords/customer data. Sanitize logs and evidence before publication. Never force-push, discard user work, rewrite unrelated history, modify application code, weaken tests, or add unrelated build artifacts. Do not open a PR, merge, or implement fixes unless specifically authorized.

If permissions, auth, connectivity, storage limits, hosting rules, or Git configuration block push, do not claim success. Preserve local results, document exact non-sensitive failure evidence, and state what is needed to publish.
