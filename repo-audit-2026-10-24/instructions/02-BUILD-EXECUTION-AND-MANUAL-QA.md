# 02 — Environment, Build, Installation, and Human-Style Manual QA

## 1. Establish the execution environment

Before substantial work or testing:

1. Record repository URL, local path, current branch, HEAD commit, and working-tree state.
2. Preserve existing uncommitted changes, untracked files, and user-owned work.
3. Record OS, architecture, available disk space, relevant toolchain/dependency versions, and material configuration.
4. Determine which build, install, GUI, browser, desktop automation, screenshot, and recording facilities are actually available.
5. Verify that available facilities can reach the actual program and save files to accessible locations.
6. Establish required credentials, external services, network access, databases, hardware, and environment configuration without recording secrets.
7. Identify destructive/external effects and use an isolated test environment and synthetic data wherever possible.
8. Create or resume the audit folder and initial progress, coverage, and handover records before detailed work.

Do not assume a desktop, display server, browser, recorder, service, account, or network capability exists simply because the program requires it. Establish limitations by inspection or a reproducible attempt. Continue independent work that is feasible.

## 2. Build, install, launch, and lifecycle

Test the actual application, not just source code, test fixtures, mocks, isolated components, or API responses.

1. Determine supported targets from actual build files/configuration.
2. Record the audited application revision and environment.
3. Execute the supported build process; preserve relevant commands, exit status, warnings, errors, missing dependencies, and workarounds.
4. Install/deploy using the supported method wherever feasible.
5. Launch the real application and verify a usable initial state.
6. Test first-run initialization, setup, configuration, resource/database initialization, and required services.
7. Establish whether the runtime path is real or mock-only.
8. Identify undocumented manual setup and undeclared dependencies.
9. Test relevant startup, shutdown, restart, recovery, and supported installation targets where feasible.
10. Where practical, use a clean/isolated environment so cached files, old state, or undeclared system dependencies do not mask defects.

Run the existing test suite as supplementary verification only after understanding the repository and build path. Report its results separately from manual QA. A successful build or passing test suite is not proof that the application works.

If build/install/launch fails, capture the result, investigate root cause, record exact limitations, and continue all independent audit activities. Save exact commands, exit statuses, output paths, and environment details in `BUILD-AND-INSTALL.md`. Identify the last verified step so another session can continue without repeating successful work unnecessarily.

## 3. Phased execution and controlled batches

Work through:

1. Repository/architecture discovery.
2. Feature/control inventory.
3. Build/install/launch.
4. Individual control and option tests.
5. End-to-end workflow tests.
6. Edge cases, failures, recovery, and cross-feature tests.
7. Root-cause analysis/fix proposals.
8. Coverage/evidence reconciliation.
9. Report finalization and Git publication.

Divide large applications into logical screen/module/settings/integration/workflow batches. For each batch, define scope; enumerate relevant features; execute tests; capture evidence; update inventory, coverage, findings, and evidence records; validate files; record failures/blockers; update handover/next actions; and reconcile before moving on. Keep batches small enough to resume without repeating all work.

Do not stop after the first serious defect. Continue testing unrelated functionality.

## 4. Coverage model and realistic scope

Inspect every discovered feature and exposed control. Exercise every meaningful option and every distinct workflow branch that can reasonably be identified. Do not confuse complete feature coverage with testing every possible Cartesian combination of every value, state, permission, and environment.

Track separately:

- **Feature coverage:** every discovered capability is inventoried.
- **Control coverage:** each exposed control and meaningful option has a result/status.
- **Workflow coverage:** each identified journey and meaningful branch is executed or explicitly marked blocked/unverified.
- **Input coverage:** relevant valid, invalid, missing, duplicate, malformed, and boundary inputs tested where applicable.
- **State coverage:** significant initial, intermediate, saved, restored, error, and final states checked.
- **Interaction coverage:** important dependencies and cross-feature effects exercised.
- **Failure coverage:** relevant error/retry/recovery/interruption paths tested where safely reproducible.

Prioritize additional tests according to impact, implementation complexity, state interactions, and observed failures. Explain untested combinations. This limitation does not justify skipping exposed controls or obvious workflow branches.

Use statuses consistently: `NOT STARTED`, `IN PROGRESS`, `PASS`, `FAIL`, `PARTIAL`, `BLOCKED`, `NOT ATTEMPTED`, `INCONCLUSIVE`, `EVIDENCE MISSING`, and `NOT APPLICABLE` where relevant. A blocked or not-applicable item requires a specific reason. Missing results never default to PASS.

## 5. Mandatory real UI testing

Manual testing through the actual user interface is mandatory where the environment supports it. Use visible, observable interactions such as mouse, keyboard, menus, dialogs, navigation, and normal user workflows. Automation may assist repeatable execution or collection, but cannot replace inspecting the real interface and its observed results.

If the environment cannot run or interact with the GUI, do not substitute static analysis or API-only checks and call it human-style manual testing. Document the blocker and complete feasible independent work.

### A. Discover all entry points

Start at the program's initial state. Inspect every accessible screen, navigation item, menu, expanded section, context menu, dialog, scrollable area, conditional control, and alternate entry point. Record initial state and the discovered interaction set.

### B. Test every control

For each control:

1. Record its identity and available choices.
2. Capture or reference the known current state.
3. Perform the interaction as a user would.
4. Capture the resulting screenshot.
5. Record actual behavior and compare it with expected behavior justified by implementation.
6. Verify relevant data/state changes where appropriate.
7. Check navigation, cancellation, return, reopening, and preservation behavior as relevant.
8. Check repetition and error handling where relevant.
9. Link result and evidence to feature/workflow/test IDs.

A visual reaction alone is not a pass; confirm the intended operation and observable effect.

### C. Execute complete workflows

A workflow starts at a real entry point and ends only when its intended final outcome is reached or a specific blocker prevents completion. If button A opens page B, a choice reveals another control, and submission leads to page C, test the complete A→B→C journey. Then return to a valid starting state and execute distinct alternative paths.

For each workflow, document:

- Entry point and intended outcome.
- Every screen, meaningful action, decision, and branch.
- Intermediate state and data changes.
- Save/cancel/back-navigation behavior where relevant.
- Validation and error outcomes.
- Retry/recovery paths.
- Persistence and restart behavior where relevant.
- Final state and verification evidence.

Clicking every button separately does not establish workflow coverage. Test the connected sequence.

### D. Options, inputs, and state transitions

Test every implemented meaningful option and setting. Use representative and boundary-value tests for valid/invalid, empty/missing, duplicate, minimum/maximum/out-of-range values, relevant whitespace/special characters/malformed data, required/optional fields, dependent options, and combinations that materially change behavior.

Check initialization, changes, saving, reset, persistence, and restoration. For asynchronous operations inspect loading feedback, completion, failure, repeated requests, and interruption where practical. For permission/authentication paths, use authorized test accounts and verify allowed/denied behavior where feasible. Record untested combinations and why they were not exercised. Do not claim every conceivable input combination was tested.

### E. Cross-feature behavior

Test combinations sharing data, permissions, configuration, or state. Examples: create data in one area and consume it elsewhere; change configuration before an operation; edit data before navigation; delete a dependent object; repeat actions; restart after state changes. Check that one feature does not corrupt or break another.

### F. Failure and recovery paths

Where feasible, safely simulate invalid credentials, unavailable services, rejected requests, missing data, interrupted operations, and unavailable local resources. Verify user communication, data preservation/restoration, recovery, and consistency. Use synthetic data, authorized accounts, isolated environments, and controlled mocks where necessary; clearly distinguish mock-based checks from live integrations.

Do not send unsolicited messages, initiate real payments, delete production data, or trigger other unrequested destructive/external actions.

### G. Accessibility and usability

Inspect keyboard navigation, focus order/visibility, labels, validation feedback, dialogs, disabled controls, scrolling, discoverability, and error recovery. Record barriers that prevent or materially obstruct a user from completing a task.

## 6. Workflow resumption

For every workflow, save the last completed step, next step, prerequisites, expected starting state, and safe means of restoring/recreating state. If failure occurs mid-workflow, preserve the evidence and status. On resumption, continue from the last verified point unless state is unreliable. If restart or replay is necessary, explain why and distinguish the repeat from the original attempt.

## 7. Continuation policy under limitations

When resources, environment, context, or tools prevent full execution:

- Finish the current safe meaningful step where possible.
- Save results, evidence, and progress.
- Mark each outstanding task individually.
- Record the precise blocker and attempted workaround.
- Continue unaffected work.
- Never turn a partial test into a pass.

Do not wait to finish the entire audit before reporting. Save and report supported findings after meaningful batches, and always perform the handover procedure before a session ends or an agent changes.
