# 03 — Screenshot, Video, Evidence Integrity, and Exception Protocol

## 1. Default rule

**Every executed manual UI test step must have a validated screenshot showing the actual application state after the interaction.** This includes successes, failures, validation errors, unexpected transitions, and attempts that expose blockers.

Capture a separate before-state when the prior state cannot be established from preceding evidence, or when it is necessary to prove a change. Evidence exceptions are specific to an individual step, not a whole screen, workflow, batch, or audit phase by default.

An exception explains why the standard evidence could not reasonably be supplied; it is not a pass and does not itself prove the behavior.

## 2. Allowed evidence exceptions

An exception is permitted only for a documented condition below. Assign a unique ID and link it to the exact test step.

### E1 — Non-visual operation

The operation genuinely produces no meaningful visual application state, e.g. a background job, CLI operation, database transaction, protocol response, or service-to-service action without a UI representation.

Required: explain why an application screenshot would not prove the result; identify the exact assertion; provide direct alternative evidence such as command output, a sanitized log, a protocol response, or before/after state comparison; use a terminal/diagnostic screenshot where feasible. Do not call a UI action non-visual merely because its result is difficult to observe.

### E2 — Test blocked by an established prerequisite

An actual prerequisite prevents execution, e.g. the application will not launch, a required authorized test account is unavailable, or an essential service is inaccessible.

Required: record the attempted prerequisite/action; capture the visible blocker when possible; otherwise provide relevant build/log/diagnostic evidence; explain why the step could not run; mark it `BLOCKED`, never `PASS`; state what must become available. A speculative concern about a dependency is not enough; establish the blocker through an attempt, configuration inspection, or reproducible observation.

### E3 — Crash or immediate process termination

The application closes/crashes/terminates before the resulting state can be captured.

Required: capture preceding state where feasible; provide crash report/log/process output/system diagnostic where available; record trigger sequence, timestamp, and app revision; capture any visible crash dialog; mark `FAIL` or `INCONCLUSIVE` based on evidence. Do not omit the failure because no after-screenshot exists.

### E4 — Transient or timing-sensitive visual state

A state is too brief for a reliable still capture or a still cannot establish sequence/order, e.g. a transient notification, race, intermittent transition, or rapidly disappearing loading state.

Required: record video containing the triggering action and state/transition; note exact timestamp/range; extract a representative frame where technically feasible. If neither screenshot nor usable video establishes the result, provide diagnostic evidence and leave visual verification explicitly incomplete. First attempt a reasonable capture method; transient behavior alone is not automatically an exception.

### E5 — Evidence-capture tooling failure

A screenshot or recorder genuinely fails after a reasonable attempt.

Required: record the tool/method and failure; preserve the original test result and diagnostics; retry with a reasonable alternative where available; provide substitute evidence; record the evidence limitation even if functionally successful. Do not retry forever, but do not skip capture merely because it is inconvenient.

### E6 — Sensitive information cannot safely be captured

A screen contains credentials, personal/customer data, or other sensitive values that cannot safely be retained without compromising confidentiality.

Required: use synthetic data or a safe test account wherever feasible; otherwise redact only sensitive values while preserving the relevant state; if safe redaction is impossible, do not commit the unredacted file, record the limitation, and provide a safe alternative. State what retained evidence proves and does not prove. Privacy is not a blanket exemption when a synthetic-data test could solve the problem.

## 3. Invalid exceptions

The following are never valid reasons to omit evidence:

- The test is trivial, repetitive, routine, or expected to pass.
- Capturing a screenshot is inconvenient.
- The screen resembles a previously captured screen.
- The result seems obvious from agent notes.
- Source/log evidence exists even though the claim is about visible UI behavior.
- The test passed.
- The audit is large or resources are low without an actual capture limitation.
- The step was forgotten or overlooked.
- The agent wants to avoid showing a failure or incomplete workflow.

A forgotten screenshot is `EVIDENCE MISSING` until captured or a genuine exception is established. A never-attempted step is `NOT ATTEMPTED`, not an executed step with an evidence exception; record its reason and available blocker evidence.

## 4. Mandatory exception register

Record one exception per affected test step in `evidence/exceptions.csv`. Required columns:

- `Exception ID` — e.g. `EX-001`.
- `Test Step ID` — exact ID, e.g. `W-014-S023`.
- `Exception Type` — `E1` through `E6`.
- `Intended Observation` — behavior being verified.
- `Reason` — factual explanation why screenshot is unavailable/unsuitable.
- `Capture Attempt` — method attempted and actual outcome.
- `Alternative Evidence` — paths, log ranges, video timestamps, or diagnostics.
- `Test Status` — Failed, Blocked, Inconclusive, or another justified status.
- `Evidence Limitation` — what available material does not establish.
- `Next Action` — retest, alternative capture, or prerequisite.
- `Exception Review Status` — accepted as justified, awaiting review, or rejected.

An exception is not accepted merely because it is recorded. Its reason and alternative evidence must be checked. Rejected/unresolved exceptions remain visible as evidence gaps.

## 5. Alternative evidence hierarchy

Choose evidence that directly proves the claim:

1. **Actual app screenshot:** preferred for visible UI state, controls, input, validation, navigation, and results.
2. **Actual screen video with timestamps:** sequence-dependent, transient, timing-sensitive, or intermittent behavior; extract a representative screenshot where feasible.
3. **Application/system diagnostics:** sanitized logs, crash reports, command output, network traces, or service diagnostics for otherwise unobservable results.
4. **Before/after state evidence:** file/database/application-state inspection for persistence/mutation. Record inspection method and values.
5. **Source-code analysis:** supports implementation explanation/root-cause hypotheses but does not by itself prove a UI workflow succeeded at runtime.

Use the strongest relevant evidence. Several weak sources do not necessarily establish a claim none proves individually.

## 6. Capture and validation rules

For every meaningful UI step:

1. Capture the real application after the action.
2. Include context sufficient to identify app/screen/control/input/result.
3. Capture a before-state when needed to establish transition.
4. Validate the saved image by opening it or using a reliable image check.
5. Link it to feature, workflow, exact step, app revision, environment where relevant, and any finding.
6. Confirm legibility and check for sensitive information.

Record screen video for workflows where transitions/timing materially affect interpretation: multi-page transitions, async behavior, intermittent failures, animations, complex bugs, or sequences where order matters. A continuous workflow recording is preferred where practical, in addition to step screenshots. Record exact workflow IDs and relevant timestamps. Video can replace a step-specific still only under a documented exception when the still cannot establish the behavior.

Never fabricate screenshots, construct mock images to represent observed UI, alter results to appear successful, invent logs, or misrepresent a recording. Redaction must protect secrets without hiding the behavior.

## 7. IDs, filenames, and storage

Use stable IDs for features/workflows/steps/evidence. Example filenames:

- `evidence/screenshots/F-001/W-014/W-014-S001.png`
- `evidence/screenshots/F-001/W-014/W-014-S002.png`
- `evidence/videos/W-014.mp4`
- `evidence/logs/W-014-crash.txt`

Do not use an unrelated or merely similar screenshot as a substitute for step-specific evidence. Reusing a file is acceptable only when it genuinely proves every referenced claim.

Estimate evidence volume for large audits; use reasonable formats/encoding while preserving readable details; avoid redundant copies; monitor free disk space. Never delete unrelated user files to make space. If repository/Git limits prevent storing evidence, identify the exact restriction and acceptable alternative. Never silently omit evidence or claim it was pushed when it was not.

## 8. Evidence manifest

Maintain `evidence/manifest.csv` with, at minimum:

- Evidence ID.
- Feature ID.
- Workflow ID (when applicable).
- Exact Test Step ID.
- Evidence type (screenshot, video, log, other).
- Relative path inside audit folder.
- Observation/claim it establishes.
- Test status.
- Relevant application revision.
- Environment/context where material.
- Related Finding ID, if any.
- Related Exception ID, if any.
- Validation status (not checked, validated, invalid/unreadable).

Do not create entries for nonexistent files. Do not label evidence validated until checked. Keep manifest synchronized with the test ledger.

## 9. Mandatory evidence accounting

Report separate counts for:

- Total manual test steps executed.
- Executed UI steps requiring screenshots.
- Executed UI steps with valid, step-specific screenshots.
- Accepted screenshot exceptions, grouped by E1–E6.
- Unresolved evidence gaps.
- Blocked test steps/workflows.
- Not-attempted steps.
- Missing, unreadable, unplayable, or invalid evidence files.
- Workflows with end-to-end video when video was warranted.
- Differences between test ledger, manifest, and files on disk.

Calculate screenshot coverage as:

**Screenshot coverage = executed UI steps with validated screenshots ÷ executed UI steps requiring screenshots × 100%.**

Report exception coverage separately; an exception is not a screenshot. Report genuinely non-visual tests separately from UI screenshot coverage. Do not combine values into a percentage that hides gaps.

A test may be functionally successful while its evidence is incomplete. Conversely, a failed test can have complete evidence. Report functional status and evidence status separately.

## 10. Recommended evidence deliverables

Within the audit folder, maintain:

- `evidence/manifest.csv`
- `evidence/exceptions.csv`
- `evidence/screenshots/`
- `evidence/videos/`
- `evidence/logs/`

Adapt paths only when needed and keep links stable and relative.
