# Mandatory Audit Instruction Loading Gate

**Repository URL:** `[INSERT REPOSITORY URL]`  
**Local repository path, if known:** `[INSERT OR DISCOVER]`  
**Audit folder:** `repo-audit-YYYY-MM-DD/` at the repository root unless an existing in-progress audit folder must be resumed.

## Mission

Conduct a complete, evidence-based audit of the repository's actual application implementation. Understand what the program does, inventory all implemented features and options, build/install/launch it where feasible, manually test actual user workflows, identify incomplete work and defects, recommend technically justified fixes, preserve screenshots/videos/logs, and publish the audit and a resumable handover to the repository using authorized credentials.

This is not a superficial code review, a documentation review, an automated test run, a software rewrite, or an executive summary. Do not change application code or weaken tests unless separately and explicitly authorized.

## Mandatory instruction-loading gate — do this before audit work

Before inspecting application code, building the program, running tests, or conducting audit activities:

1. Read this file fully.
2. Read every instruction file in the following list, from beginning to end:
   - `01-MISSION-SOURCE-OF-TRUTH-AND-INVENTORY.md`
   - `02-BUILD-EXECUTION-AND-MANUAL-QA.md`
   - `03-EVIDENCE-AND-EXCEPTIONS.md`
   - `04-FINDINGS-REPORTING-AND-COMPLETION.md`
   - `05-HANDOVER-CHECKPOINTS-AND-GIT.md`
   - `06-REQUIREMENT-TRACEABILITY.csv`
3. Read actual file contents. Do not rely on filenames, headings alone, summaries, search snippets, previous messages, or assumptions about what the documents contain.
4. If a reading tool truncates output, continue in sequential or appropriately overlapping chunks until the entire file has been obtained. Verify that no section or trailing content was omitted. Do not claim a file was read completely if any of it was inaccessible.
5. Inspect the traceability CSV and reconcile it against the instructions. Preserve all requirements; do not silently discard or weaken any requirement that appears duplicated, ambiguous, conflicting, or infeasible.
6. Create the audit folder (unless resuming an existing one) and save an instruction-loading record and working checklist there. Record which instruction files and versions were read, the sections covered, and any unreadable content or unresolved interpretation issue.
7. Before proceeding, report which files were completely read, whether anything was truncated or inaccessible, whether the traceability checklist is reconciled, and which execution prerequisites are unavailable.

### Stop condition

**Do not begin the repository audit until the complete instruction set has been read and the requirement checklist reconciled.** If any instruction file cannot be accessed completely, report the exact file/section/tool limitation. Do not falsely claim the gate passed. Continue only with work that does not depend on the missing requirements, if any, and explicitly label the limitation.

## Ongoing compliance gate

Reading the instructions once does not permit the agent to ignore them later. Treat these files and the traceability CSV as the audit contract.

Before each major phase, retrieve the relevant instructions and check the planned work against the relevant requirement IDs. Update the checklist as requirements are completed, failed, blocked, or found infeasible. On a handover or fresh session, follow `05-HANDOVER-CHECKPOINTS-AND-GIT.md` and consult the current handover before continuing.

## Mandatory working principles

- **Source of truth:** actual source code and build/runtime-required files, corroborated by observable behavior. Existing tests are supplementary, never the product source of truth.
- **Inventory before conclusions:** understand the architecture before judging functionality.
- **Real program:** build, install, launch, and use the actual application through real interfaces where feasible.
- **End-to-end workflows:** clicking controls individually is not enough; complete connected user journeys and meaningful branches.
- **Evidence:** every executed manual UI step needs a validated, step-specific screenshot, unless a narrowly defined exception in the evidence protocol is justified and recorded.
- **Incremental reporting:** do not wait for completion to save and report supported findings.
- **Continuity:** maintain the handover and progress files throughout the task so another agent can resume without repeating completed work.
- **Honesty:** distinguish verified, inferred, suspected, blocked, not attempted, failed, and passed. Never fabricate progress or evidence.
- **Scope protection:** do not modify application code, discard user work, weaken tests, or perform unrequested product changes.
- **Safety:** use synthetic data, authorized test accounts, and isolated environments for destructive or externally visible operations.

## Required final behavior

As soon as meaningful findings exist, save them in the relevant audit files and report the actual progress. The final audit is a consolidated deliverable, not the first time findings are written down. Before any session ends or agent handover, persist the state and exact next action as specified in file 05.
